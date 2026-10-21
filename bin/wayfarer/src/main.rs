//! The `wayfarer` binary: one entry point for the gateway, the brain and the tools.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use clap::{Args, Parser, Subcommand};
use futures_util::StreamExt;
use wayfarer::{open_store, spawn_brain, spawn_replay_brain};
use wayfarer_brain::{Brain, BrainError, Clock, ReplayClock, SystemClock, World, WsLink};
use wayfarer_gateway::replay::{self, Pace, Recording, Speed};
use wayfarer_gateway::{
    Config, Gateway, Hub, Options, Paths, ServerOptions, bind, create_token, read_token, serve,
    stop_signals, token_for_bind,
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

#[derive(Args)]
struct ServeArgs {
    #[command(flatten)]
    gateway: GatewayArgs,
    /// The world pack (`wayfarer import-world` builds it). Default: `world.sqlite` in the data
    /// directory. Without one, serve runs the gateway alone.
    #[arg(long)]
    world: Option<PathBuf>,
    /// The brain's store: the bots' state and the statistics. Default: `store.sqlite` in the
    /// data directory.
    #[arg(long)]
    store: Option<PathBuf>,
}

#[derive(Args)]
struct BrainArgs {
    /// The gateway's WebSocket.
    #[arg(long, default_value = "ws://127.0.0.1:7373/ws")]
    gateway: String,
    /// The world pack (`wayfarer import-world` builds it). Default: `world.sqlite` in the data
    /// directory.
    #[arg(long)]
    world: Option<PathBuf>,
    /// The gateway's token. Default: the `gateway-token` file beside the config, if there is
    /// one (a loopback gateway needs none).
    #[arg(long)]
    token: Option<String>,
    /// The gateway's config file; its directory is the data directory. Default: the user's
    /// wayfarer data directory.
    #[arg(long)]
    config: Option<PathBuf>,
    /// The brain's store: the bots' state and the statistics, so a restarted brain resumes
    /// each bot. Default: `store.sqlite` in the data directory.
    #[arg(long)]
    store: Option<PathBuf>,
    /// Test only (the live check's flee scenario): the bots flee below this HP per cent
    /// instead of 30, so the first blow sets a flee off.
    #[arg(long, hide = true, value_parser = clap::value_parser!(i32).range(1..=100))]
    flee_below: Option<i32>,
    /// The bots flee on sight from monsters whose name holds NAME (any case), as if the player
    /// had set *flee from* for it; repeatable. Use the full name: `ooze` also matches the
    /// glittering ooze.
    #[arg(long, value_name = "NAME")]
    flee_from: Vec<String>,
}

#[derive(Args)]
struct SettingsArgs {
    /// The gateway's config file; its directory is the data directory (and holds
    /// `settings.toml`, `servers/` and `characters/`). Default: the user's wayfarer data directory.
    #[arg(long)]
    config: Option<PathBuf>,
    /// The character's id, as in `wayfarer.toml`. Without one: the settings for all characters.
    #[arg(long)]
    character: Option<String>,
    /// The game server as the gateway names it, `host:port` (for `servers/<host>_<port>.toml`).
    #[arg(long)]
    server: Option<String>,
    /// The character's class as the status screen names it (`Mage`), for the class presets.
    #[arg(long)]
    class: Option<String>,
    /// Write a starter file (every setting with its help, commented out) instead of showing the
    /// settings: the character's with `--character`, the server's with `--server`, else
    /// `settings.toml`. An existing file is left alone unless `--force`.
    #[arg(long)]
    write: bool,
    /// With `--write`: replace a file that is there.
    #[arg(long, requires = "write")]
    force: bool,
}

#[derive(Args)]
struct ReplayArgs {
    /// A session's log directory (`logs/<id>/`, with its `output.log`).
    dir: PathBuf,
    /// Recorded seconds per second.
    #[arg(long, default_value_t = 1.0, conflicts_with = "instant")]
    speed: f64,
    /// As fast as the session takes the output.
    #[arg(long)]
    instant: bool,
    /// The config whose server for this character gives the logon script the session's state
    /// follows (default: the silvermere preset's); its directory holds `gateway-token` and
    /// `world.sqlite`. Default: the user's wayfarer data directory, for those two.
    #[arg(long)]
    config: Option<PathBuf>,
    /// Where to listen. Anything but a loopback address needs a token (`wayfarer gateway
    /// --new-token`).
    #[arg(long, default_value = "127.0.0.1:7373")]
    bind: SocketAddr,
    /// The built web UI.
    #[arg(long, default_value = "./ui/dist")]
    ui_dir: PathBuf,
    /// Run a brain in-process on the recording's clock, with a throwaway store, attached when
    /// the recorded brain was.
    #[arg(long)]
    brain: bool,
    /// The world pack for `--brain`. Default: `world.sqlite` beside the config.
    #[arg(long)]
    world: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Command {
    /// Run the gateway: telnet sessions, logs and the WebSocket server.
    Gateway(GatewayArgs),
    /// Run the brain against a running gateway: parse every session, keep each character's
    /// state for the UIs, re-sync it (docs/brain.md).
    Brain(BrainArgs),
    /// Run everything in one process: the gateway, and the brain attached to it in-process.
    Serve(ServeArgs),
    /// Play a recorded session back into the UI (and a brain) as if the game were sending it
    /// (docs/replay.md).
    Replay(ReplayArgs),
    /// Show the settings in force for a character and where each value comes from; checks
    /// the settings files (docs/brain.md "Settings").
    Settings(SettingsArgs),
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
    let (name, args, world) = match cli.command {
        Command::Gateway(args) => ("gateway", args, None),
        Command::Serve(args) => ("serve", args.gateway, Some((args.world, args.store))),
        Command::Settings(args) => {
            return match show_settings(args) {
                Ok(text) => {
                    print!("{text}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("wayfarer settings: {e}");
                    ExitCode::FAILURE
                }
            };
        }
        Command::Replay(args) => {
            return match run_replay(args) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("wayfarer replay: {e}");
                    ExitCode::FAILURE
                }
            };
        }
        Command::Brain(args) => {
            return match run_brain(args) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("wayfarer brain: {e}");
                    ExitCode::FAILURE
                }
            };
        }
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
    match run_gateway(args, world) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("wayfarer {name}: {e}");
            ExitCode::FAILURE
        }
    }
}

/// `wayfarer settings`: the files read, and every setting with its value, where it came from
/// and what it is.
fn show_settings(args: SettingsArgs) -> Result<String, Box<dyn std::error::Error>> {
    let paths = paths(args.config)?;
    let dir = wayfarer_settings::SettingsDir::new(
        paths
            .credentials
            .parent()
            .unwrap_or(std::path::Path::new(".")),
    );
    let (server, character) = (
        args.server.as_deref().unwrap_or(""),
        args.character.as_deref().unwrap_or(""),
    );
    if args.write {
        let (path, written) = dir.write_starter(server, character, args.force)?;
        return Ok(if written {
            format!("wrote {}\n", path.display())
        } else {
            format!("{} is there already; --force replaces it\n", path.display())
        });
    }
    let settings = dir
        .resolve_for(server, character, args.class.as_deref())
        .map_err(|e| e.to_string())?;
    let mut out = String::new();
    for (layer, path) in dir.files(server, character) {
        let state = if path.is_file() { "read" } else { "not there" };
        out.push_str(&format!(
            "{:<16} {state:<10} {}\n",
            layer.to_string(),
            path.display()
        ));
    }
    out.push('\n');
    for (setting, value, layer) in settings.rows() {
        let unit = if setting.unit.is_empty() {
            String::new()
        } else {
            format!(" {}", setting.unit)
        };
        out.push_str(&format!(
            "{:<26} {:<12} {:<14} {}\n",
            setting.key,
            format!("{value}{unit}"),
            layer.to_string(),
            setting.label
        ));
    }
    Ok(out)
}

fn paths(config: Option<PathBuf>) -> Result<Paths, Box<dyn std::error::Error>> {
    Ok(match config {
        Some(config) => Paths::for_config(config),
        None => Paths::in_dir(
            Paths::default_data_dir().ok_or("no data directory on this platform: use --config")?,
        ),
    })
}

/// The settings files beside `beside` (the gateway's token file or credentials, so in the data
/// directory): `settings.toml`, `servers/` and `characters/` (`docs/plan/04-settings.md`). The
/// files for all characters are checked now, so a mistake in them stops the brain at start
/// instead of at a session; each character's own file is checked when its session is first
/// seen.
fn settings_dir(beside: &std::path::Path) -> Result<wayfarer_settings::SettingsDir, String> {
    let dir =
        wayfarer_settings::SettingsDir::new(beside.parent().unwrap_or(std::path::Path::new(".")));
    dir.resolve("", "").map_err(|e| e.to_string())?;
    Ok(dir)
}

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();
}

/// `brain`: `None` runs the gateway alone (`wayfarer gateway`); `Some((world, store))` is
/// `wayfarer serve`, which also runs the brain in-process (the default pack and store, in the
/// data directory, where they are `None`).
fn run_gateway(
    args: GatewayArgs,
    brain: Option<(Option<PathBuf>, Option<PathBuf>)>,
) -> Result<(), Box<dyn std::error::Error>> {
    let paths = paths(args.config)?;
    let token_file = paths.credentials.with_file_name("gateway-token");
    if args.new_token {
        println!("{}", create_token(&token_file)?);
        eprintln!("token written to {}", token_file.display());
        return Ok(());
    }
    let token = token_for_bind(args.bind, &token_file)?;
    let (world_path, store_path) = match brain {
        Some((world, store)) => (
            Some(world.unwrap_or_else(|| token_file.with_file_name("world.sqlite"))),
            store.unwrap_or_else(|| token_file.with_file_name("store.sqlite")),
        ),
        None => (None, PathBuf::new()),
    };

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(async {
            init_tracing();
            let gateway = Gateway::start(&paths, Options::default())?;
            let listener = bind(args.bind).await?;
            tracing::info!("listening on {}", listener.local_addr()?);
            let options = ServerOptions {
                ui_dir: args.ui_dir,
            };
            let hub = Hub::new(gateway, token.clone());
            let brain = match world_path {
                Some(path) if path.is_file() => {
                    let world = World::open(&path)?;
                    let settings = settings_dir(&token_file)?;
                    tracing::info!(
                        build = wayfarer_brain::BUILD,
                        world = %path.display(),
                        "brain running in-process"
                    );
                    Some(spawn_brain(
                        &hub,
                        world,
                        token,
                        open_store(&store_path),
                        Some(settings),
                    ))
                }
                Some(path) => {
                    tracing::warn!(
                        "no world pack at {path}: running the gateway alone, without the brain; \
                         build it with\n  wayfarer import-world --export tests/fixtures/world \
                         --out '{path}'",
                        path = path.display()
                    );
                    None
                }
                None => None,
            };
            serve(listener, hub, options, stop_signals()?).await?;
            if let Some(brain) = brain {
                // The hub stopped, so the brain's connection has ended and it returns.
                if let Err(e) = brain.await? {
                    tracing::warn!("the in-process brain stopped: {e}");
                }
            }
            Ok(())
        })
}

/// The brain's reconnect backoff, the web UI's: half a second, doubling up to ten.
const RETRY_MIN: Duration = Duration::from_millis(500);
const RETRY_MAX: Duration = Duration::from_secs(10);

fn run_brain(args: BrainArgs) -> Result<(), Box<dyn std::error::Error>> {
    let paths = paths(args.config)?;
    let dir = paths.config.parent().map(PathBuf::from).unwrap_or_default();
    let world_path = args.world.unwrap_or_else(|| dir.join("world.sqlite"));
    let store_path = args.store.unwrap_or_else(|| dir.join("store.sqlite"));
    if !world_path.is_file() {
        return Err(format!(
            "no world pack at {path}: build it from the stock-world export (in a wayfarer \
             checkout, tests/fixtures/world) with\n  wayfarer import-world --export \
             tests/fixtures/world --out '{path}'",
            path = world_path.display()
        )
        .into());
    }
    let token = match args.token {
        Some(token) => Some(token),
        None => read_token(&paths.credentials.with_file_name("gateway-token"))?,
    };

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(async {
            init_tracing();
            let world = World::open(&world_path)?;
            let settings = settings_dir(&paths.credentials)?;
            let mut brain =
                Brain::new(world, Arc::new(SystemClock), token).with_settings_dir(settings);
            if let Some(store) = open_store(&store_path) {
                brain = brain.with_store(store);
            }
            if let Some(percent) = args.flee_below {
                tracing::warn!(percent, "test knob: fleeing below {percent}% HP");
                brain = brain.with_flee_below(percent);
            }
            if !args.flee_from.is_empty() {
                tracing::info!(names = ?args.flee_from, "fleeing on sight from these monsters");
                brain = brain.with_flee_from(args.flee_from.clone());
            }
            tracing::info!(
                build = wayfarer_brain::BUILD,
                world = %world_path.display(),
                "brain started"
            );
            let mut stops = std::pin::pin!(stop_signals()?);
            let mut retry = RETRY_MIN;
            loop {
                let connected = tokio::select! {
                    link = WsLink::connect(&args.gateway) => link,
                    _ = stops.next() => break,
                };
                match connected {
                    Ok(mut link) => {
                        let ended = tokio::select! {
                            ended = brain.run(&mut link) => Some(ended),
                            _ = stops.next() => None,
                        };
                        // Nothing goes to the game on the way out: just close the socket.
                        link.close().await;
                        match ended {
                            None => break,
                            Some(Ok(())) => {
                                tracing::warn!("the gateway went away");
                                retry = RETRY_MIN;
                            }
                            Some(Err(e @ BrainError::Refused(_))) => return Err(e.into()),
                            Some(Err(e)) => tracing::warn!("{e}"),
                        }
                    }
                    Err(e) => tracing::warn!("{e}"),
                }
                tracing::info!("connecting again in {:.1} s", retry.as_secs_f64());
                tokio::select! {
                    () = tokio::time::sleep(retry) => {}
                    _ = stops.next() => break,
                }
                retry = (retry * 2).min(RETRY_MAX);
            }
            tracing::info!("brain stopped");
            Ok::<_, Box<dyn std::error::Error>>(())
        })
}

fn run_replay(args: ReplayArgs) -> Result<(), Box<dyn std::error::Error>> {
    let recording = Arc::new(Recording::load(&args.dir)?);
    let speed = match args.instant {
        true => Speed::Instant,
        false if args.speed > 0.0 && args.speed.is_finite() => Speed::Times(args.speed),
        false => return Err("--speed must be above 0".into()),
    };
    let paths = paths(args.config.clone())?;
    let dir = paths.config.parent().map(PathBuf::from).unwrap_or_default();
    let server = match &args.config {
        Some(path) => {
            let config = Config::load(path)?;
            config
                .characters
                .iter()
                .find(|c| c.id == recording.id)
                .and_then(|c| config.server(&c.server))
                .cloned()
        }
        None => None,
    };
    let token = token_for_bind(args.bind, &dir.join("gateway-token"))?;
    let world_path = args.world.unwrap_or_else(|| dir.join("world.sqlite"));
    if args.brain && !world_path.is_file() {
        return Err(format!(
            "--brain: no world pack at {path}: build it with\n  wayfarer import-world --export \
             tests/fixtures/world --out '{path}'",
            path = world_path.display()
        )
        .into());
    }
    let logs = replay::temp_logs(&recording);

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(async {
            init_tracing();
            let world = args.brain.then(|| World::open(&world_path)).transpose()?;
            let listener = bind(args.bind).await?;
            let pace = Arc::new(Pace::new(recording.origin_ms(), speed));
            let (gateway, connector) =
                replay::gateway(recording.clone(), pace.clone(), &logs, server)?;
            tracing::info!(
                session = %recording.id,
                chunks = recording.chunks.len(),
                minutes = (recording.end_ms() - recording.origin_ms()) / 60_000,
                logs = %logs.display(),
                "replaying on {}",
                listener.local_addr()?
            );
            let hub = Hub::new(gateway, token);
            let brain = world.map(|world| {
                let origin = std::time::UNIX_EPOCH + Duration::from_millis(pace.origin_ms());
                let clock: Arc<dyn Clock> = Arc::new(match pace.speed() {
                    Speed::Times(s) => ReplayClock::paced(origin, pace.start(), s),
                    Speed::Instant => ReplayClock::following(origin, pace.elapsed()),
                });
                let (attach, attached) = tokio::sync::watch::channel(false);
                let task = spawn_replay_brain(&hub, world, clock, attached);
                let (hub, recording, pace) = (hub.clone(), recording.clone(), pace.clone());
                let driver = tokio::spawn(async move {
                    replay::drive_brain(&hub, &recording, &pace, |on| {
                        attach.send_replace(on);
                        let hub = hub.clone();
                        async move { hub.wait_for_brain(on).await }
                    })
                    .await;
                    // Keep the brain as the recording left it until the replay stops.
                    std::future::pending::<()>().await;
                    drop(attach);
                });
                (task, driver)
            });
            let mut finished = connector.finished();
            let session = recording.id.clone();
            tokio::spawn(async move {
                if finished.wait_for(|f| *f).await.is_ok() {
                    tracing::info!(%session, "the recording has been played to its end; Ctrl-C stops the replay");
                }
            });
            let options = ServerOptions {
                ui_dir: args.ui_dir,
            };
            serve(listener, hub, options, stop_signals()?).await?;
            if let Some((task, driver)) = brain {
                driver.abort();
                if let Err(e) = task.await? {
                    tracing::warn!("the replay's brain stopped: {e}");
                }
            }
            Ok::<_, Box<dyn std::error::Error>>(())
        })
}
