mod cli;
mod handlers;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands};
use handlers::{render, update, daemon};

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Render { template, output } => render::handle_render(template, output),
        Commands::Update { template_url, template_path, env_url, env_path } => {
            update::handle_update(template_url, template_path, env_url, env_path)
        },
        Commands::Run { config } => daemon::handle_run(config),
        Commands::Stop => daemon::handle_stop(),
    }
}
