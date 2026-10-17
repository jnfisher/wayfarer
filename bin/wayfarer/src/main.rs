//! The `wayfarer` binary: one entry point for the gateway, the brain and the tools.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "wayfarer",
    version,
    about = "A MajorMUD playing client and bot"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run the gateway: telnet sessions, logs and the WebSocket server (card C-006).
    Gateway,
    /// Run everything in one process: the gateway now, the brain from M1 (card C-006).
    Serve,
    /// Build a world pack (SQLite) from the stock-world export, optionally with MegaMud's path set.
    ImportWorld {
        /// The export directory: one `<TABLE>.jsonl.gz` per table and `MANIFEST.json`.
        #[arg(long)]
        export: PathBuf,
        /// MegaMud's zip, or the directory holding it (not shipped with wayfarer: a player's own
        /// MegaMud copy, or mmud-exploration's `reference/megamud/` when developing).
        #[arg(long)]
        megamud: Option<PathBuf>,
        /// The pack to write (replaced if it exists).
        #[arg(long)]
        out: PathBuf,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let name = match cli.command {
        Command::Gateway => "gateway",
        Command::Serve => "serve",
        Command::ImportWorld {
            export,
            megamud,
            out,
        } => {
            return match wayfarer_world::build(&export, megamud.as_deref(), &out) {
                Ok(report) => {
                    print!("{report}");
                    println!("wrote {}", out.display());
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("wayfarer import-world: {e}");
                    ExitCode::FAILURE
                }
            };
        }
    };
    eprintln!("wayfarer {name}: not yet (see docs/plan/tasks/)");
    ExitCode::from(2)
}
