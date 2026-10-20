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
fn gateway_and_serve_are_listed() {
    let help = String::from_utf8_lossy(&wayfarer(&["--help"]).stdout).into_owned();
    assert!(help.contains("gateway") && help.contains("serve"));
}

#[test]
fn gateway_refuses_a_non_loopback_bind_without_a_token_and_makes_one_on_request() {
    let dir = std::env::temp_dir().join(format!("wayfarer-cli-token-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let config = dir.join("wayfarer.toml");
    let config = config.to_str().unwrap();
    for cmd in ["gateway", "serve"] {
        let run = wayfarer(&[cmd, "--config", config, "--bind", "0.0.0.0:0"]);
        assert_eq!(run.status.code(), Some(1), "{cmd}");
        assert!(String::from_utf8_lossy(&run.stderr).contains("--new-token"));
    }

    let run = wayfarer(&["gateway", "--config", config, "--new-token"]);
    assert!(run.status.success());
    let printed = String::from_utf8_lossy(&run.stdout).trim().to_owned();
    assert_eq!(printed.len(), 48);
    assert_eq!(
        std::fs::read_to_string(dir.join("gateway-token")).unwrap(),
        printed
    );
    std::fs::remove_dir_all(&dir).unwrap();
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

#[test]
fn brain_refuses_a_settings_file_that_is_not_valid() {
    // Over a second: it builds a world pack from the fixture export first (a real `import-world`).
    let dir = std::env::temp_dir().join(format!("wayfarer-cli-settings-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let export =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/world");
    let pack = dir.join("world.sqlite");
    let built = wayfarer(&[
        "import-world",
        "--export",
        export.to_str().unwrap(),
        "--out",
        pack.to_str().unwrap(),
    ]);
    assert!(built.status.success());
    let config = dir.join("wayfarer.toml");
    std::fs::write(dir.join("settings.toml"), "[health]\nflee_below_hp = 95\n").unwrap();
    let run = wayfarer(&[
        "brain",
        "--config",
        config.to_str().unwrap(),
        "--world",
        pack.to_str().unwrap(),
        "--gateway",
        "ws://127.0.0.1:1/ws",
    ]);
    assert_eq!(run.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&run.stderr).into_owned();
    assert!(
        stderr.contains("settings.toml") && stderr.contains("health.flee_below_hp is 95"),
        "{stderr}"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}
