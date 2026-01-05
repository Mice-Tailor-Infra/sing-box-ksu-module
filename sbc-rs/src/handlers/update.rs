use anyhow::{Context, Result, bail};
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

pub fn handle_update(
    template_url: String,
    template_path: PathBuf,
    env_url: Option<String>,
    env_path: Option<PathBuf>,
) -> Result<()> {
    println!("📡 Connecting to remote server...");

    // Generate cache buster
    let timestamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)?
        .as_secs();
    let cache_buster = format!("?t={}", timestamp);

    // 1. Update Template
    let full_template_url = format!("{}{}", template_url, cache_buster);
    println!("Downloading template from: {}", full_template_url);
    
    let template_body = ureq::get(&full_template_url)
        .call()
        .with_context(|| format!("Failed to download template from {}", full_template_url))?
        .into_string()?;

    // Validation: Check for "inbounds" to ensure it's a valid config (manifest check)
    if !template_body.contains("inbounds") {
        bail!("❌ Validation failed: Downloaded content does not look like a valid sing-box config (missing 'inbounds').");
    }

    // Atomic Write
    let tmp_path = template_path.with_extension("tmp");
    fs::write(&tmp_path, &template_body)?;
    fs::rename(&tmp_path, &template_path)?;
    println!("✅ Template updated successfully.");

    // 2. Update Env Example (if requested)
    if let (Some(e_url), Some(e_path)) = (env_url, env_path) {
        let full_env_url = format!("{}{}", e_url, cache_buster);
        println!("Downloading env example from: {}", full_env_url);
        
        match ureq::get(&full_env_url).call() {
            Ok(resp) => {
                let env_body = resp.into_string()?;
                let tmp_env = e_path.with_extension("tmp");
                fs::write(&tmp_env, env_body)?;
                fs::rename(&tmp_env, &e_path)?;
                println!("📝 Env example updated.");
            },
            Err(e) => eprintln!("⚠️ Failed to update env example: {}", e),
        }
    }

    Ok(())
}
