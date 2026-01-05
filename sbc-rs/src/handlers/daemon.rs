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

// Simple .env loader
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
    
    // 0. Load Env
    let env_path = workspace.join(".env");
    if let Err(e) = load_env_file(&env_path) {
        warn!("⚠️ Failed to load .env file at {:?}: {}", env_path, e);
    }

    info!("🚀 Starting sing-box supervisor...");
    info!("📂 Workspace: {:?}", workspace);
    
    // 0. Auto-Render (if requested)
    if let Some(template) = template_path {
        info!("🎨 Auto-rendering config from template: {:?}", template);
        if let Err(e) = render::handle_render(template, config_path.clone()) {
            error!("❌ Render failed: {}", e);
            return Err(e);
        }
        info!("✅ Config rendered successfully.");
    }

    let pid_file = get_pid_file_path(&workspace);
    
    // Ensure run dir exists
    if let Some(parent) = pid_file.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
             warn!("⚠️ Failed to create run dir {:?}: {}", parent, e);
        }
    }

    // 1. Start Child Process
    // Use working_dir if provided, otherwise default to workspace root
    let final_wd = working_dir.unwrap_or_else(|| workspace.clone());
    if !final_wd.exists() {
        fs::create_dir_all(&final_wd).context("Failed to create working directory")?;
    }

    let mut child = Command::new("sing-box")
        .arg("run")
        .arg("-c")
        .arg(&config_path)
        .current_dir(&final_wd) // All relative paths in config will resolve here
        .spawn()
        .context("Failed to spawn sing-box process")?;

    let pid = child.id();
    info!("✅ sing-box started with PID: {} | WD: {:?}", pid, final_wd);

    // 2. Write PID file
    fs::write(&pid_file, pid.to_string())?;

    // 3. Setup Signal Handling
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    let child_pid = pid;

    ctrlc::set_handler(move || {
        if !r.load(Ordering::SeqCst) { return; }
        r.store(false, Ordering::SeqCst);
        
        info!("🛑 Received termination signal, shutting down child...");
        let pid = Pid::from_raw(child_pid as i32);
        match signal::kill(pid, Signal::SIGTERM) {
             Ok(_) => info!("Sent SIGTERM to child process"),
             Err(e) => error!("Failed to forward signal to child: {}", e),
        }
    }).context("Error setting Ctrl-C handler")?;

    // 4. Supervisor Loop
    match child.wait() {
        Ok(status) => {
            if !status.success() {
                 anyhow::bail!("sing-box exited with error: {}", status);
            }
            info!("sing-box exited with: {}", status);
        },
        Err(e) => error!("Error waiting for sing-box: {}", e),
    }

    let _ = fs::remove_file(pid_file);
    Ok(())
}

pub fn handle_stop() -> Result<()> {
    // deduce workspace for stop too
    let workspace = PathBuf::from(env::var("WORKSPACE").unwrap_or_else(|_| "/data/adb/sing-box-workspace".into()));
    let pid_file = get_pid_file_path(&workspace);
    
    if !pid_file.exists() {
        warn!("⚠️ No running instance found (PID file missing at {:?}).", pid_file);
        return Ok(());
    }

    let pid_str = fs::read_to_string(&pid_file)?.trim().to_string();
    let pid_num: i32 = pid_str.parse()?;
    let pid = Pid::from_raw(pid_num);

    info!("🛑 Send SIGTERM to PID: {}", pid_num);
    
    match signal::kill(pid, Signal::SIGTERM) {
        Ok(_) => {
            info!("⏳ Waiting for process to exit...");
            for _ in 0..50 { 
                thread::sleep(Duration::from_millis(100));
                if signal::kill(pid, None).is_err() { 
                    info!("✅ Process exited gracefully.");
                    let _ = fs::remove_file(pid_file);
                    return Ok(());
                }
            }
            warn!("⚠️ Process {} did not exit after 5 seconds.", pid_num);
        },
        Err(e) => {
            error!("Failed to send signal: {} (Process might be already dead)", e);
            let _ = fs::remove_file(pid_file);
        }
    }

    Ok(())
}
