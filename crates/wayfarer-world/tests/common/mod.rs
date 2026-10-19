//! The pack built from `tests/fixtures/world`, shared by the tests that read one.

use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

pub fn repo(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel)
}

/// The pack, built once into the target directory's test scratch space and reused by every
/// later test process. The name carries a hash of the export's MANIFEST.json and the builder's
/// source, so a changed export or builder builds a new pack; each build goes to a file of its
/// own and is renamed into place, so test processes building at once never read a partial pack.
pub fn pack_path() -> &'static Path {
    static PATH: OnceLock<PathBuf> = OnceLock::new();
    PATH.get_or_init(|| {
        let export = repo("tests/fixtures/world");
        let manifest = std::fs::read(export.join("MANIFEST.json")).unwrap();
        let builder = [
            include_str!("../../src/build.rs"),
            include_str!("../../src/schema.rs"),
            include_str!("../../src/mm.rs"),
            include_str!("../../src/walk.rs"),
            include_str!("../../src/cp437.rs"),
        ];
        // FNV-1a
        let tag = builder
            .iter()
            .flat_map(|s| s.as_bytes())
            .chain(&manifest)
            .fold(0xcbf2_9ce4_8422_2325u64, |h, &b| {
                (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
            });
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR"));
        let path = dir.join(format!("world-{tag:016x}.sqlite"));
        if path.exists() {
            return path;
        }
        // One process builds while the others wait for it; a lock left by a process that died
        // is ignored after a minute.
        let lock = dir.join(format!("world-{tag:016x}.lock"));
        let stale = |l: &Path| {
            std::fs::metadata(l)
                .and_then(|m| m.modified())
                .is_ok_and(|t| t.elapsed().is_ok_and(|e| e > Duration::from_secs(60)))
        };
        if stale(&lock) {
            let _ = std::fs::remove_file(&lock);
        }
        let builder = OpenOptions::new().write(true).create_new(true).open(&lock);
        if builder.is_err() {
            let start = Instant::now();
            while !path.exists() && start.elapsed() < Duration::from_secs(60) {
                std::thread::sleep(Duration::from_millis(50));
            }
            if path.exists() {
                return path;
            }
        }
        let mine = dir.join(format!("world-{tag:016x}.{}.sqlite", std::process::id()));
        wayfarer_world::build(&export, None, &mine).unwrap();
        std::fs::rename(&mine, &path).unwrap();
        let _ = std::fs::remove_file(&lock);
        path
    })
}
