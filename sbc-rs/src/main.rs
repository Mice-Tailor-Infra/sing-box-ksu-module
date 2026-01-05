mod cli;
mod handlers;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands};
use handlers::{render, update, daemon};

fn main() -> Result<()> {
    // Initialize Logger (Default to INFO)
    // Initialize Logger with Local Time
    if std::env::var("RUST_LOG").is_err() {
        unsafe { std::env::set_var("RUST_LOG", "info"); }
    }
    
    use std::io::Write;
    env_logger::Builder::from_default_env()
        .format(|buf, record| {
            writeln!(
                buf,
                "[{} {}] {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                record.level(),
                record.args()
            )
        })
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Render { template, output } => {
            render::handle_render(template, output)
        }
        Commands::Update { template_url, template_path, env_url, env_path } => {
            update::handle_update(template_url, template_path, env_url, env_path)
        }
        Commands::Run { config, template, working_dir } => {
            daemon::handle_run(config, template, working_dir)
        }
        Commands::Stop => daemon::handle_stop(),
    }
}
