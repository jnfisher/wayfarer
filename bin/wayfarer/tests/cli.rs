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
