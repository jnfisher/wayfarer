//! Wire messages between the gateway, the brain and the UIs (docs/plan/02-architecture.md). No I/O.
//!
//! Scaffold only (C-001); the card that owns this crate fills it in.

/// The crate's name, so the scaffold has something to test.
pub const NAME: &str = env!("CARGO_PKG_NAME");

#[cfg(test)]
mod tests {
    #[test]
    fn name_is_set() {
        assert_eq!(super::NAME, "wayfarer-proto");
    }
}
