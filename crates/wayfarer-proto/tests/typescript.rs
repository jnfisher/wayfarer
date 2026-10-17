//! The TypeScript in `ui/src/proto/` must match what the Rust types generate.
//!
//! Regenerate it with `cargo test -p wayfarer-proto -- --ignored regenerate`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

fn committed_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/src/proto")
}

/// Every file in `dir` by name, with its contents.
fn read_all(dir: &Path) -> BTreeMap<String, String> {
    let Ok(entries) = fs::read_dir(dir) else {
        return BTreeMap::new();
    };
    entries
        .map(|entry| {
            let entry = entry.unwrap();
            let name = entry.file_name().into_string().unwrap();
            (name, fs::read_to_string(entry.path()).unwrap())
        })
        .collect()
}

#[test]
fn committed_typescript_is_current() {
    let fresh = Path::new(env!("CARGO_TARGET_TMPDIR")).join("proto-ts");
    let _ = fs::remove_dir_all(&fresh);
    fs::create_dir_all(&fresh).unwrap();
    wayfarer_proto::export_typescript(&fresh).unwrap();

    let want = read_all(&fresh);
    let have = read_all(&committed_dir());
    assert!(want.contains_key("ToClient.ts") && want.contains_key("index.ts"));
    if want != have {
        let differ: Vec<_> = want
            .keys()
            .chain(have.keys())
            .filter(|name| want.get(*name) != have.get(*name))
            .collect();
        panic!(
            "ui/src/proto/ is out of date ({differ:?} differ); \
             run `cargo test -p wayfarer-proto -- --ignored regenerate`"
        );
    }
}

#[test]
#[ignore = "writes ui/src/proto/; run on purpose to regenerate"]
fn regenerate() {
    let dir = committed_dir();
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    wayfarer_proto::export_typescript(&dir).unwrap();
}
