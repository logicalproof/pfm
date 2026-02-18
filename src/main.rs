mod commands;
mod config;
mod templates;

use clap::{Parser, Subcommand};
use std::env;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "pfm", version, about = "Production Flow Manager — orchestrates Claude Code role agents")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize .pfm/ structure in the current repo
    Init,
}

fn find_repo_root() -> Result<PathBuf, String> {
    let mut dir = env::current_dir()
        .map_err(|e| format!("failed to get current directory: {}", e))?;

    loop {
        if dir.join(".pfm").exists() || dir.join(".git").exists() {
            return Ok(dir);
        }
        if !dir.pop() {
            return env::current_dir()
                .map_err(|e| format!("failed to get current directory: {}", e));
        }
    }
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Init => {
            let base = find_repo_root().unwrap_or_else(|_| {
                env::current_dir().expect("cannot determine working directory")
            });
            commands::init::run(&base)
        }
    };

    if let Err(e) = result {
        eprintln!("error: {}", e);
        std::process::exit(1);
    }
}
