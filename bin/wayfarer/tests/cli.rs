use std::process::Command;

fn wayfarer(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_wayfarer"))
        .args(args)
        .output()
        .expect("run wayfarer")
}

#[test]
fn version_exits_zero() {
    let out = wayfarer(&["--version"]);
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).starts_with("wayfarer "));
}

#[test]
fn subcommands_are_listed_and_not_yet_built() {
    let help = String::from_utf8_lossy(&wayfarer(&["--help"]).stdout).into_owned();
    assert!(help.contains("gateway") && help.contains("serve"));
    for cmd in ["gateway", "serve"] {
        assert_eq!(wayfarer(&[cmd]).status.code(), Some(2), "{cmd}");
    }
}

#[test]
fn import_world_is_listed_and_fails_cleanly_without_an_export() {
    let help = String::from_utf8_lossy(&wayfarer(&["--help"]).stdout).into_owned();
    assert!(help.contains("import-world"));
    let dir = std::env::temp_dir().join(format!("wayfarer-cli-{}", std::process::id()));
    let out = dir.join("world.sqlite");
    let run = wayfarer(&[
        "import-world",
        "--export",
        dir.join("missing").to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);
    assert_eq!(run.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&run.stderr).contains("MANIFEST.json"));
    assert!(!out.exists());
}
