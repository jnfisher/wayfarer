//! The `wayfarer` binary: one entry point for the gateway, the brain and the tools.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};
use wayfarer_gateway::{
    Gateway, Hub, Options, Paths, ServerOptions, bind, create_token, serve, stop_signals,
    token_for_bind,
};

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

#[derive(Args)]
struct GatewayArgs {
    /// The config file (`wayfarer.toml`); credentials, logs and the token go beside it.
    /// Default: the user's wayfarer data directory.
    #[arg(long)]
    config: Option<PathBuf>,
    /// Where to listen. Anything but a loopback address needs a token (`--new-token`).
    #[arg(long, default_value = "127.0.0.1:7373")]
    bind: SocketAddr,
    /// The built web UI.
    #[arg(long, default_value = "./ui/dist")]
    ui_dir: PathBuf,
    /// Make a new token in `gateway-token`, print it once and exit.
    #[arg(long)]
    new_token: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Run the gateway: telnet sessions, logs and the WebSocket server.
    Gateway(GatewayArgs),
    /// Run everything in one process: the gateway now, the brain from M1.
    Serve(GatewayArgs),
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
    let (name, args) = match cli.command {
        Command::Gateway(args) => ("gateway", args),
        Command::Serve(args) => ("serve", args),
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
    match run_gateway(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("wayfarer {name}: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run_gateway(args: GatewayArgs) -> Result<(), Box<dyn std::error::Error>> {
    let paths = match args.config {
        Some(config) => Paths::for_config(config),
        None => Paths::in_dir(
            Paths::default_data_dir().ok_or("no data directory on this platform: use --config")?,
        ),
    };
    let token_file = paths.credentials.with_file_name("gateway-token");
    if args.new_token {
        println!("{}", create_token(&token_file)?);
        eprintln!("token written to {}", token_file.display());
        return Ok(());
    }
    let token = token_for_bind(args.bind, &token_file)?;

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(async {
            tracing_subscriber::fmt()
                .with_env_filter(
                    tracing_subscriber::EnvFilter::try_from_default_env()
                        .unwrap_or_else(|_| "info".into()),
                )
                .init();
            let gateway = Gateway::start(&paths, Options::default())?;
            let listener = bind(args.bind).await?;
            tracing::info!("listening on {}", listener.local_addr()?);
            let options = ServerOptions {
                ui_dir: args.ui_dir,
            };
            let hub = Hub::new(gateway, token);
            serve(listener, hub, options, stop_signals()?).await?;
            Ok(())
        })
}
