use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod commands;

#[derive(Parser)]
#[command(name = "omnihive", version, about = "Omnihive Execution Control Plane CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Show all active tasks in current directory
    Status,

    /// Replay trace events from a JSONL file
    Replay {
        /// Path to trace JSONL file
        trace_file: PathBuf,
        /// Filter by task ID
        #[arg(long)]
        task_id: Option<String>,
    },

    /// Watch a task's live status
    Watch {
        /// Task ID to watch
        task_id: String,
        /// Project directory (default: current)
        #[arg(long, default_value = ".")]
        dir: PathBuf,
    },

    /// Validate JSON schemas
    Validate {
        /// Path to schema file
        schema: PathBuf,
        /// Path to data file to validate
        data: PathBuf,
    },

    /// Compute eval metrics from trace JSONL files
    Eval {
        /// Path to trace JSONL file or directory containing .jsonl files
        trace_path: PathBuf,
        /// Output JSON report to file (optional)
        #[arg(long)]
        output: Option<PathBuf>,
    },
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Status => commands::status(),
        Commands::Replay { trace_file, task_id } => commands::replay(&trace_file, task_id.as_deref()),
        Commands::Watch { task_id, dir } => commands::watch(&task_id, &dir),
        Commands::Validate { schema, data } => commands::validate(&schema, &data),
        Commands::Eval { trace_path, output } => commands::eval_cmd(&trace_path, output.as_deref()),
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
