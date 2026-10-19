//! The pieces of the `wayfarer` binary that tests reach.

use std::sync::Arc;

use tokio::task::JoinHandle;
use wayfarer_brain::{Brain, BrainError, ChannelLink, SystemClock, World};
use wayfarer_gateway::Hub;

/// Run a brain inside the gateway's process: it attaches to `hub` over channels, with no
/// socket, and sends and hears the same messages as a brain on the WebSocket. The task ends
/// when the hub stops, or at once with the reason if the hub refuses the brain. The brain
/// presents the hub's own `token`, so a hub that checks one still welcomes it.
pub fn spawn_brain(
    hub: &Hub,
    world: World,
    token: Option<String>,
) -> JoinHandle<Result<(), BrainError>> {
    let (to_gateway, from_gateway) = hub.connect();
    tokio::spawn(async move {
        let mut brain = Brain::new(world, Arc::new(SystemClock), token);
        let mut link = ChannelLink::new(to_gateway, from_gateway);
        brain.run(&mut link).await
    })
}
