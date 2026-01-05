use clap::{Parser, Subcommand};
use std::path::PathBuf;
use crate::build;

#[derive(Parser)]
#[command(
    author, 
    version = build::BUILD_TIME,  // 使用构建时间戳作为版本号
    long_version = build::CLAP_LONG_VERSION,
    about = "Mice System Tools - sing-box 增强型监控工具",
    long_about = None
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// 从模板渲染配置
    Render {
        /// 配置模板文件的路径
        #[arg(short, long)]
        template: PathBuf,

        /// 输出配置文件的路径
        #[arg(short, long)]
        output: PathBuf,
    },
    /// 从远程 URL 更新模板
    Update {
        /// 配置模板的 URL/路径
        #[arg(short = 'u', long)]
        template_url: String,

        /// 保存配置模板的本地路径
        #[arg(short = 't', long)]
        template_path: PathBuf,

        /// 环境示例文件的 URL/路径 (可选)
        #[arg(long)]
        env_url: Option<String>,

        /// 保存环境示例文件的本地路径 (可选)
        #[arg(long)]
        env_path: Option<PathBuf>,
    },
    /// 以后台监控模式运行 sing-box
    Run {
        /// 要使用的配置文件路径
        #[arg(short, long)]
        config: PathBuf,

        /// 配置模板文件路径 (可选，用于自动渲染)
        #[arg(short, long)]
        template: Option<PathBuf>,

        /// sing-box 的工作目录 (可选，缓存/UI 文件将放置于此)
        #[arg(short = 'D', long)]
        working_dir: Option<PathBuf>,
    },
    /// 优雅地停止正在运行的监控进程
    Stop,
}
