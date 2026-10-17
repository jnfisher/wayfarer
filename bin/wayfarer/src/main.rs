//! The `wayfarer` binary: one entry point for the gateway, the brain and the tools.

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
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let name = match cli.command {
        Command::Gateway => "gateway",
        Command::Serve => "serve",
    };
    eprintln!("wayfarer {name}: not yet (see docs/plan/tasks/)");
    ExitCode::from(2)
}
