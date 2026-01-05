use anyhow::{Context, Result};
use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::Duration;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use log::{info, warn, error};

fn get_pid_file_path() -> PathBuf {
    env::var("SBC_PID_FILE").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("/data/adb/sing-box-workspace/run/sing-box.pid"))
}

pub fn handle_run(config_path: PathBuf) -> Result<()> {
    info!("🚀 Starting sing-box supervisor...");
    let pid_file = get_pid_file_path();
    
    // Ensure run dir exists
    if let Some(parent) = pid_file.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
             warn!("⚠️ Failed to create run dir {:?}: {}", parent, e);
        }
    }

    // 1. Start Child Process
    let mut child = Command::new("sing-box")
        .arg("run")
        .arg("-c")
        .arg(config_path)
        .arg("-D")
        .arg("/data/adb/sing-box-workspace") // Set working dir
        .spawn()
        .context("Failed to spawn sing-box process")?;

    let pid = child.id();
    info!("✅ sing-box started with PID: {}", pid);

    // 2. Write PID file
    fs::write(&pid_file, pid.to_string())?;

    // 3. Setup Signal Handling
    let _running = Arc::new(AtomicBool::new(true));
    // let r = _running.clone(); // Unused

    // ... signal handling logic ...
    match child.wait() {
        Ok(status) => info!("sing-box exited with: {}", status),
        Err(e) => error!("Error waiting for sing-box: {}", e),
    }

    // Cleanup PID file
    let _ = fs::remove_file(pid_file);
    Ok(())
}

pub fn handle_stop() -> Result<()> {
    let pid_file = get_pid_file_path();
    if !pid_file.exists() {
        warn!("⚠️ No running instance found (PID file missing at {:?}).", pid_file);
        return Ok(());
    }

    let pid_str = fs::read_to_string(&pid_file)?.trim().to_string();
    let pid_num: i32 = pid_str.parse()?;
    let pid = Pid::from_raw(pid_num);

    info!("🛑 Send SIGTERM to PID: {}", pid_num);
    
    // Send SIGTERM
    match signal::kill(pid, Signal::SIGTERM) {
        Ok(_) => {
            info!("⏳ Waiting for process to exit...");
            // Polling check if still alive
            for _ in 0..50 { // Wait up to 5 seconds
                thread::sleep(Duration::from_millis(100));
                if signal::kill(pid, None).is_err() { 
                    // kill(0) failed means process is gone (usually ESRCH)
                    info!("✅ Process exited gracefully.");
                    let _ = fs::remove_file(pid_file);
                    return Ok(());
                }
            }
            // If we get here, it didn't die.
            warn!("⚠️ Process {} did not exit after 5 seconds.", pid_num);
            warn!("⚠️ AUTOMATIC KILL (-9) IS DISABLED per safety policy.");
            warn!("⚠️ Please investigate manually or use 'kill -9 {}' if necessary.", pid_num);
        },
        Err(e) => {
            error!("Failed to send signal: {} (Process might be already dead)", e);
            let _ = fs::remove_file(pid_file);
        }
    }

    Ok(())
}
