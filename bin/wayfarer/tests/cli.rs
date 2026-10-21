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

#[test]
fn settings_shows_the_layers_and_where_each_value_comes_from() {
    let dir = std::env::temp_dir().join(format!("wayfarer-cli-show-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("characters")).unwrap();
    std::fs::write(dir.join("settings.toml"), "[bot]\nstyle = \"bold\"\n").unwrap();
    std::fs::write(
        dir.join("characters/thorgrim.toml"),
        "[movement]\nsearch_limit = 500\n",
    )
    .unwrap();
    let config = dir.join("wayfarer.toml");
    let run = wayfarer(&[
        "settings",
        "--config",
        config.to_str().unwrap(),
        "--character",
        "thorgrim",
        "--server",
        "127.0.0.1:2323",
    ]);
    assert!(run.status.success(), "{run:?}");
    let out = String::from_utf8_lossy(&run.stdout).into_owned();
    // The files, least specific first.
    assert!(
        out.contains("my defaults") && out.contains("settings.toml"),
        "{out}"
    );
    assert!(
        out.contains("this server") && out.contains("not there"),
        "{out}"
    );
    assert!(
        out.contains("this character") && out.contains("thorgrim.toml"),
        "{out}"
    );
    // Bold sets the flee line to 20; the character's file sets the search limit.
    let row = |key: &str| {
        out.lines()
            .find(|l| l.starts_with(key))
            .unwrap_or("")
            .to_owned()
    };
    assert!(
        row("health.flee_below_hp").contains("20 %")
            && row("health.flee_below_hp").contains("the class and style")
    );
    assert!(
        row("movement.search_limit").contains("500 tries")
            && row("movement.search_limit").contains("this character")
    );
    assert!(row("bot.style").contains("\"bold\"") && row("bot.style").contains("my defaults"));
    assert!(row("bot.passed_by_minutes").contains("the default"));

    // A character's file that is not valid is named.
    std::fs::write(
        dir.join("characters/thorgrim.toml"),
        "[movement]\nsearch_limit = 0\n",
    )
    .unwrap();
    let bad = wayfarer(&[
        "settings",
        "--config",
        config.to_str().unwrap(),
        "--character",
        "thorgrim",
    ]);
    assert_eq!(bad.status.code(), Some(1));
    let err = String::from_utf8_lossy(&bad.stderr).into_owned();
    assert!(
        err.contains("thorgrim.toml") && err.contains("movement.search_limit is 0"),
        "{err}"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn settings_for_a_class_shows_the_class_presets() {
    let dir = std::env::temp_dir().join(format!("wayfarer-cli-class-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let config = dir.join("wayfarer.toml");
    let show = |class: &[&str]| {
        let mut args = vec!["settings", "--config", config.to_str().unwrap()];
        args.extend_from_slice(class);
        String::from_utf8_lossy(&wayfarer(&args).stdout).into_owned()
    };
    let row = |out: &str, key: &str| {
        out.lines()
            .find(|l| l.starts_with(key))
            .unwrap_or("")
            .to_owned()
    };
    let none = show(&[]);
    assert!(
        row(&none, "health.flee_below_hp").contains("25 %"),
        "{none}"
    );
    // `--write` makes the character's starter file once; `--force` replaces it.
    let wrote = show(&["--write", "--character", "waydeva"]);
    let file = dir.join("characters/waydeva.toml");
    assert!(wrote.starts_with("wrote ") && file.is_file(), "{wrote}");
    assert!(show(&["--write", "--character", "waydeva"]).contains("is there already"));
    assert!(show(&["--write", "--force", "--character", "waydeva"]).starts_with("wrote "));
    let mage = show(&["--class", "Mage"]);
    let flee = row(&mage, "health.flee_below_hp");
    assert!(
        flee.contains("45 %") && flee.contains("the class and style"),
        "{mage}"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}
