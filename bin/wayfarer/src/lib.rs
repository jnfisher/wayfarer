//! The pieces of the `wayfarer` binary that tests reach.

use std::path::Path;
use std::sync::Arc;

use tokio::sync::watch;
use tokio::task::JoinHandle;
use wayfarer_brain::{Brain, BrainError, ChannelLink, Clock, Store, SystemClock, World};
use wayfarer_gateway::Hub;
use wayfarer_store::Note;

/// The brain's store at `path`, made if missing and made afresh if unreadable (both logged);
/// `None`, with a warning, if it cannot be opened at all: the brain then runs without one.
pub fn open_store(path: &Path) -> Option<Store> {
    match Store::open(path) {
        Ok((store, note)) => {
            match note {
                Some(note @ Note::Recreated { .. }) => tracing::warn!("{note}"),
                Some(note) => tracing::info!("{note}"),
                None => tracing::info!(store = %path.display(), "store opened"),
            }
            Some(store)
        }
        Err(e) => {
            tracing::warn!("{e}: running without a store (a restart will not resume the bots)");
            None
        }
    }
}

/// A replay's brain (`wayfarer replay --brain`, `docs/replay.md`): in-process like
/// [`spawn_brain`], on `clock` (the recording's) and a throwaway store, attached while
/// `attached` is `true` and detached while it is `false`, as the recorded brain was; the same
/// brain each time, as a restarted gateway would find it. The task ends when the hub stops.
pub fn spawn_replay_brain(
    hub: &Hub,
    world: World,
    clock: Arc<dyn Clock>,
    mut attached: watch::Receiver<bool>,
) -> JoinHandle<Result<(), BrainError>> {
    let hub = hub.clone();
    tokio::spawn(async move {
        let mut brain = Brain::new(world, clock, None);
        match Store::in_memory() {
            Ok(store) => brain = brain.with_store(store),
            Err(e) => tracing::warn!("a throwaway store: {e}: the replay's brain runs without one"),
        }
        loop {
            if attached.wait_for(|on| *on).await.is_err() {
                return Ok(());
            }
            let (to_gateway, from_gateway) = hub.connect();
            let mut link = ChannelLink::new(to_gateway, from_gateway);
            tokio::select! {
                // The hub stopped (or refused it).
                ended = brain.run(&mut link) => return ended,
                _ = attached.wait_for(|on| !*on) => {}
            }
        }
    })
}

/// Run a brain inside the gateway's process: it attaches to `hub` over channels, with no
/// socket, and sends and hears the same messages as a brain on the WebSocket. The task ends
/// when the hub stops, or at once with the reason if the hub refuses the brain. The brain
/// presents the hub's own `token`, so a hub that checks one still welcomes it.
pub fn spawn_brain(
    hub: &Hub,
    world: World,
    token: Option<String>,
    store: Option<Store>,
) -> JoinHandle<Result<(), BrainError>> {
    let (to_gateway, from_gateway) = hub.connect();
    tokio::spawn(async move {
        let mut brain = Brain::new(world, Arc::new(SystemClock), token);
        if let Some(store) = store {
            brain = brain.with_store(store);
        }
        let mut link = ChannelLink::new(to_gateway, from_gateway);
        brain.run(&mut link).await
    })
}
