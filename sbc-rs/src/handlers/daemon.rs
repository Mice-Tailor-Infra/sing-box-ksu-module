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

pub fn handle_run(config_path: Option<PathBuf>, template_path: Option<PathBuf>, working_dir: Option<PathBuf>) -> Result<()> {
    // 0. 路径解析
    // 如果没传 config_path，则假定在默认位置
    let resolved_config = config_path.unwrap_or_else(|| PathBuf::from("/data/adb/sing-box-workspace/etc/config.json"));
    let workspace = get_workspace_path(&resolved_config);
    let pid_file = get_pid_file_path(&workspace);
    let stop_flag = workspace.join("STOP");

    // 1. 加载环境配置
    let env_path = workspace.join(".env");
    if let Err(e) = load_env_file(&env_path) {
        warn!("⚠️ 无法在 {:?} 加载 .env 文件: {}", env_path, e);
    }

    // 设置信号处理
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
        info!("⏳ 接收到终止信号，正在准备退出...");
    }).context("设置信号处理程序失败")?;

    let mut retry_count = 0;
    let max_retries = 4;

    // 工作目录准备
    let final_wd = working_dir.unwrap_or_else(|| workspace.clone());
    if !final_wd.exists() {
        fs::create_dir_all(&final_wd).context("无法创建工作目录")?;
    }

    while running.load(Ordering::SeqCst) {
        // 1. 检查手动停止标志
        if stop_flag.exists() {
            info!("🛑 检测到停止标志 (STOP Flag)，终止监听。");
            break;
        }

        // 2. 日志轮转
        if let Some(log_file) = env::var_os("LOG_FILE").map(PathBuf::from) {
             if log_file.exists() {
                if let Ok(metadata) = fs::metadata(&log_file) {
                    if metadata.len() > 1024 * 1024 { // 1MB
                        let old_log = log_file.with_extension("log.old");
                        let _ = fs::rename(&log_file, old_log);
                        info!("🔄 日志已轮转 (超过 1MB)");
                    }
                }
             }
        }

        info!("🚀 正在启动 sing-box 监控进程...");
        info!("🏷️  版本 (构建时间): {}", crate::build::BUILD_TIME);
        info!("📂 工作目录: {:?}", final_wd);

        // 3. 自动渲染
        if let Some(ref template) = template_path {
            info!("🎨 正在从模板自动渲染配置: {:?}", template);
            render::handle_render(template.clone(), resolved_config.clone())?;
            info!("✅ 配置渲染成功。");
        }

        // 4. 定位并启动进程
        use std::os::unix::process::CommandExt;
        let mut singbox_bin = "sing-box".to_string();
        if let Ok(exe_path) = env::current_exe() {
            if let Some(parent) = exe_path.parent() {
                let sibling = parent.join("sing-box");
                if sibling.exists() {
                    singbox_bin = sibling.to_string_lossy().to_string();
                }
            }
        }

        info!("💨 执行指令: {} run -c {:?} -D {:?}", singbox_bin, resolved_config, final_wd);
        
        // 创建 Command 并配置
        let mut child_cmd = Command::new(&singbox_bin);
        child_cmd.arg("run")
            .arg("-c")
            .arg(&resolved_config)
            .arg("-D")
            .arg(&final_wd)
            .current_dir(&final_wd);
            
        unsafe {
            child_cmd.pre_exec(|| {
                libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
                Ok(())
            });
        }

        let mut child = child_cmd.spawn()
            .context("启动 sing-box 进程失败")?;

        let pid = child.id();
        info!("✅ sing-box 已启动，PID: {}", pid);
        let _ = fs::write(&pid_file, pid.to_string());

        // 5. 辅助杀死线程
        let killer_running = running.clone();
        let pid_to_kill = pid;
        thread::spawn(move || {
            while killer_running.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(500));
            }
            unsafe { libc::kill(pid_to_kill as i32, libc::SIGTERM); }
        });

        // 6. 等待循环
        let mut exit_status = None;
        while running.load(Ordering::SeqCst) {
            match child.try_wait() {
                Ok(Some(status)) => {
                    exit_status = Some(status);
                    break;
                }
                Ok(None) => {
                    thread::sleep(Duration::from_millis(500));
                }
                Err(e) => {
                    error!("❌ 等待子进程时出错: {}", e);
                    break;
                }
            }
        }

        // 7. 处理退出结果
        if let Some(status) = exit_status {
            if status.success() {
                info!("✨ sing-box 正常退出。");
                break; 
            } else {
                error!("⚠️ sing-box 异常退出: {}", status);
                retry_count += 1;
            }
        } else if !running.load(Ordering::SeqCst) {
            info!("🛑 收到退出信号，终止运行。");
            let _ = child.kill();
            break;
        }

        if retry_count >= max_retries {
            error!("❌ 已达到最大重试次数，监护停止。");
            break;
        }

        info!("⏳ 将在 10 秒后进行第 {}/{} 次重启尝试...", retry_count, max_retries);
        thread::sleep(Duration::from_secs(10));
    }

    let _ = fs::remove_file(&pid_file);
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
