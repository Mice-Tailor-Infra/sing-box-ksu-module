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

fn get_pid_file_path() -> PathBuf {
    env::var("SBC_PID_FILE").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("/data/adb/sing-box-workspace/run/sing-box.pid"))
}

// Simple .env loader to avoid extra dependencies
fn load_env_file(path: &PathBuf) -> Result<()> {
    if !path.exists() { return Ok(()); }
    let content = fs::read_to_string(path)?;
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        if let Some((k, v)) = line.split_once('=') {
            // Remove quotes if present
            let clean_v = v.trim().trim_matches('"').trim_matches('\'');
            unsafe { env::set_var(k.trim(), clean_v); }
        }
    }
    Ok(())
}

pub fn handle_run(config_path: PathBuf, template_path: Option<PathBuf>) -> Result<()> {
    // 0. Load Env (Restoring functionality lost from service.sh)
    // Assuming .env is in WORKSPACE (parent of config_path which is etc/config.json -> workspace/etc/config.json? No.)
    // WORKSPACE is usually /data/adb/sing-box-workspace
    // config_path passed from CLI is usually absolute.
    // Let's deduce workspace or just fail-safe to known paths?
    // service.sh passes full path.
    // Let's hardcode the search for .env in /data/adb/sing-box-workspace since this is a tailored module.
    let env_path = PathBuf::from("/data/adb/sing-box-workspace/.env");
    if let Err(e) = load_env_file(&env_path) {
        warn!("⚠️ Failed to load .env file: {}", e);
    }

    info!("🚀 Starting sing-box supervisor...");
    
    // 0. Auto-Render (if requested)
    if let Some(template) = template_path {
        info!("🎨 Auto-rendering config from template: {:?}", template);
        if let Err(e) = render::handle_render(template, config_path.clone()) {
            error!("❌ Render failed: {}", e);
            return Err(e);
        }
        info!("✅ Config rendered successfully.");
    }

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
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    
    // Extract PID for the closure to avoid moving 'child'
    let child_pid = pid;

    // Use ctrlc to trap SIGINT/SIGTERM and forward to child
    ctrlc::set_handler(move || {
        if !r.load(Ordering::SeqCst) {
             return; // Already handling
        }
        r.store(false, Ordering::SeqCst);
        
        info!("🛑 Received termination signal, shutting down child...");
        // Retrieve PID (copy from captured variable)
        let pid = Pid::from_raw(child_pid as i32);
        match signal::kill(pid, Signal::SIGTERM) {
             Ok(_) => info!("Sent SIGTERM to child process"),
             Err(e) => error!("Failed to forward signal to child: {}", e),
        }
    }).context("Error setting Ctrl-C handler")?;

    // 4. Supervisor Loop (Blocking Wait)
    // child is NOT MOVED because we used child_pid in closure
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
