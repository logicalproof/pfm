mod adapters;
mod commands;
mod config;
mod state;
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

    /// Work item management
    #[command(subcommand)]
    Work(WorkCommands),

    /// Agent management
    #[command(subcommand)]
    Agent(AgentCommands),

    /// Run verification and security checks
    Check {
        /// Work item ID
        work_id: String,
    },

    /// Run the full pipeline for a work item
    Run {
        /// Work item ID
        work_id: String,

        /// Stop at this gate (inclusive)
        #[arg(long)]
        to: Option<String>,

        /// Execution mode
        #[arg(long, default_value = "classic")]
        mode: String,
    },
}

#[derive(Subcommand)]
enum WorkCommands {
    /// Create a new work item
    New {
        /// Work item title
        title: String,

        /// Explicit work ID (e.g., FEAT-login)
        #[arg(long)]
        id: Option<String>,

        /// Technology stack
        #[arg(long)]
        stack: Option<String>,
    },

    /// List all work items
    List,
}

#[derive(Subcommand)]
enum AgentCommands {
    /// Start a role agent for a work item
    Start {
        /// Role name (prd, orchestrator, env, test, implementation, review_security, qa, git)
        role: String,

        /// Work item ID
        work_id: String,
    },

    /// Nudge/resume a role agent
    Nudge {
        /// Role name
        role: String,

        /// Work item ID
        work_id: String,
    },
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

        Commands::Work(WorkCommands::New { title, id, stack }) => {
            let base = find_repo_root().unwrap_or_else(|e| {
                eprintln!("error: {}", e);
                std::process::exit(1);
            });
            commands::work::new_work(&base, &title, id.as_deref(), stack.as_deref())
                .map(|_| ())
        }

        Commands::Work(WorkCommands::List) => {
            let base = find_repo_root().unwrap_or_else(|e| {
                eprintln!("error: {}", e);
                std::process::exit(1);
            });
            commands::work::list_work(&base)
        }

        Commands::Agent(AgentCommands::Start { role, work_id }) => {
            let base = find_repo_root().unwrap_or_else(|e| {
                eprintln!("error: {}", e);
                std::process::exit(1);
            });
            let role: state::Role = role.parse().unwrap_or_else(|e| {
                eprintln!("error: {}", e);
                std::process::exit(1);
            });
            commands::agent::start(&base, &role, &work_id)
        }

        Commands::Agent(AgentCommands::Nudge { role, work_id }) => {
            let base = find_repo_root().unwrap_or_else(|e| {
                eprintln!("error: {}", e);
                std::process::exit(1);
            });
            let role: state::Role = role.parse().unwrap_or_else(|e| {
                eprintln!("error: {}", e);
                std::process::exit(1);
            });
            commands::agent::nudge(&base, &role, &work_id)
        }

        Commands::Check { work_id } => {
            let base = find_repo_root().unwrap_or_else(|e| {
                eprintln!("error: {}", e);
                std::process::exit(1);
            });
            commands::check::run(&base, &work_id)
        }

        Commands::Run { work_id, to, mode } => {
            let base = find_repo_root().unwrap_or_else(|e| {
                eprintln!("error: {}", e);
                std::process::exit(1);
            });
            let mode: commands::run::RunMode = mode.parse().unwrap_or_else(|e| {
                eprintln!("error: {}", e);
                std::process::exit(1);
            });
            commands::run::run(&base, &work_id, to.as_deref(), mode)
        }
    };

    if let Err(e) = result {
        eprintln!("error: {}", e);
        std::process::exit(1);
    }
}
