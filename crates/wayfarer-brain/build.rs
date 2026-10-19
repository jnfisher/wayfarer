//! `WAYFARER_BUILD`: the version and, when built from a git checkout, the commit
//! (`0.1.0 (3f2a9c1)`), for `brain_info`.

use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
        .filter(|s| !s.is_empty())
}

fn main() {
    let version = env!("CARGO_PKG_VERSION");
    let build = match git(&["rev-parse", "--short", "HEAD"]) {
        Some(commit) => format!("{version} ({commit})"),
        None => version.to_owned(),
    };
    // HEAD's reflog changes on every commit and checkout, in a worktree too.
    if let Some(reflog) = git(&[
        "rev-parse",
        "--path-format=absolute",
        "--git-path",
        "logs/HEAD",
    ]) {
        println!("cargo:rerun-if-changed={reflog}");
    }
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rustc-env=WAYFARER_BUILD={build}");
}
