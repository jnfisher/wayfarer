# wayfarer (binary)

`wayfarer --version`, `wayfarer gateway`, `wayfarer brain`, `wayfarer serve` (the gateway and
the brain in one process, `--world PACK`; see `docs/brain.md`). `src/lib.rs` holds the in-process
brain wiring that tests reach.

`wayfarer import-world --export DIR [--megamud ZIP|DIR] --out PACK.sqlite` builds the world pack
from the stock-world export and MegaMud's path set; see `crates/wayfarer-world/README.md`.

`wayfarer replay DIR [--speed N|--instant] [--brain] [--config PATH] [--bind ADDR]` plays a
recorded session (`logs/<id>/`) back into the UI, and with `--brain` into an in-process brain
on the recording's clock (`spawn_replay_brain`); see `docs/replay.md`.
