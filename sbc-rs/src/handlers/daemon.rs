use anyhow::{Context, Result};
use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;
use crate::handlers::render;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::Duration;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use log::{info, warn, error};

fn get_workspace_path(config_path: &PathBuf) -> PathBuf {
    // 优先从环境变量获取
    if let Ok(ws) = env::var("WORKSPACE") {
        return PathBuf::from(ws);
    }
    // 兜底：从配置文件路径推导 (etc/config.json -> etc -> workspace)
    config_path.parent()
        .and_then(|p| p.parent())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/data/adb/sing-box-workspace"))
}

fn get_pid_file_path(workspace: &PathBuf) -> PathBuf {
    env::var("SBC_PID_FILE").map(PathBuf::from).unwrap_or_else(|_| workspace.join("run/sing-box.pid"))
}

// 简单的 .env 加载器
fn load_env_file(path: &PathBuf) -> Result<()> {
    if !path.exists() { return Ok(()); }
    let content = fs::read_to_string(path)?;
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        if let Some((k, v)) = line.split_once('=') {
            let clean_v = v.trim().trim_matches('"').trim_matches('\'');
            unsafe { env::set_var(k.trim(), clean_v); }
        }
    }
    Ok(())
}

pub fn handle_run(config_path: PathBuf, template_path: Option<PathBuf>, working_dir: Option<PathBuf>) -> Result<()> {
    let workspace = get_workspace_path(&config_path);
    
    // 0. 加载环境配置
    let env_path = workspace.join(".env");
    if let Err(e) = load_env_file(&env_path) {
        warn!("⚠️ 无法在 {:?} 加载 .env 文件: {}", env_path, e);
    }

    info!("🚀 正在启动 sing-box 监控进程...");
    info!("📂 工作目录: {:?}", workspace);
    
    // 0. 自动渲染（如果已请求）
    if let Some(template) = template_path {
        info!("🎨 正在从模板自动渲染配置: {:?}", template);
        if let Err(e) = render::handle_render(template, config_path.clone()) {
            error!("❌ 渲染失败: {}", e);
            return Err(e);
        }
        info!("✅ 配置渲染成功。");
    }

    let pid_file = get_pid_file_path(&workspace);
    
    // 确保运行目录存在
    if let Some(parent) = pid_file.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
             warn!("⚠️ 创建运行目录 {:?} 失败: {}", parent, e);
        }
    }

    // 1. 启动子进程
    // 如果提供了 working_dir 则使用，否则默认为工作空间根目录
    let final_wd = working_dir.unwrap_or_else(|| workspace.clone());
    if !final_wd.exists() {
        fs::create_dir_all(&final_wd).context("法创建工作目录")?;
    }

    use std::os::unix::process::CommandExt;
    let mut child_cmd = Command::new("sing-box");
    child_cmd.arg("run")
        .arg("-c")
        .arg(&config_path)
        .current_dir(&final_wd); // 所有配置中的相对路径都将相对于此目录解析

    unsafe {
        child_cmd.pre_exec(|| {
            // 内核级安全机制：如果父进程死亡，子进程将收到 SIGTERM 信号
            libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
            Ok(())
        });
    }

    let mut child = child_cmd.spawn()
        .context("启动 sing-box 进程失败")?;

    let pid = child.id();
    info!("✅ sing-box 已启动，PID: {} | 工作目录: {:?}", pid, final_wd);

    // 2. Write PID file
    fs::write(&pid_file, pid.to_string())?;

    // 3. Setup Signal Handling
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    let child_pid = pid;

    ctrlc::set_handler(move || {
        if !r.load(Ordering::SeqCst) { return; }
        r.store(false, Ordering::SeqCst);
        
        info!("🛑 接收到终止信号，正在关闭子进程...");
        let pid = Pid::from_raw(child_pid as i32);
        match signal::kill(pid, Signal::SIGTERM) {
             Ok(_) => info!("已向子进程发送 SIGTERM 信号"),
             Err(e) => error!("向子进程转发信号失败: {}", e),
        }
    }).context("设置 Ctrl-C 处理器出错")?;

    // 4. 监控循环
    match child.wait() {
        Ok(status) => {
            if !status.success() {
                 anyhow::bail!("sing-box 异常退出: {}", status);
            }
            info!("sing-box 已退出: {}", status);
        },
        Err(e) => error!("等候 sing-box 退出时出错: {}", e),
    }

    let _ = fs::remove_file(pid_file);
    Ok(())
}

pub fn handle_stop() -> Result<()> {
    // deduce workspace for stop too
    let workspace = PathBuf::from(env::var("WORKSPACE").unwrap_or_else(|_| "/data/adb/sing-box-workspace".into()));
    let pid_file = get_pid_file_path(&workspace);
    
    if !pid_file.exists() {
        warn!("⚠️ 未发现运行中的实例 (PID 文件缺失: {:?})。", pid_file);
        return Ok(());
    }

    let pid_str = fs::read_to_string(&pid_file)?.trim().to_string();
    let pid_num: i32 = pid_str.parse()?;
    let pid = Pid::from_raw(pid_num);

    info!("🛑 正在向 PID {} 发送 SIGTERM...", pid_num);
    
    match signal::kill(pid, Signal::SIGTERM) {
        Ok(_) => {
            info!("⏳ 正在等待进程退出...");
            for _ in 0..50 { 
                thread::sleep(Duration::from_millis(100));
                if signal::kill(pid, None).is_err() { 
                    info!("✅ 进程已正常退出。");
                    let _ = fs::remove_file(pid_file);
                    return Ok(());
                }
            }
            warn!("⚠️ 进程 {} 在 5 秒后仍未退出。", pid_num);
        },
        Err(e) => {
            error!("发送信号失败: {} (进程可能已经结束)", e);
            let _ = fs::remove_file(pid_file);
        }
    }

    Ok(())
}
