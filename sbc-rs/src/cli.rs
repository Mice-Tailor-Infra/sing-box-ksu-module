use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Render configuration from template
    Render {
        /// Path to the configuration template file
        #[arg(short, long)]
        template: PathBuf,

        /// Path to the output configuration file
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Update templates from remote URL
    Update {
        /// URL/Path to the config template
        #[arg(short = 'u', long)]
        template_url: String,

        /// Local path to save the config template
        #[arg(short = 't', long)]
        template_path: PathBuf,

        /// URL/Path to the env example (Optional)
        #[arg(long)]
        env_url: Option<String>,

        /// Local path to save the env example (Optional)
        #[arg(long)]
        env_path: Option<PathBuf>,
    },
    /// Run sing-box as a supervised daemon
    Run {
        /// Path to the config file to use
        #[arg(short, long)]
        config: PathBuf,
    },
    /// Stop the running daemon gracefully
    Stop,
}
