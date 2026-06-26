#![allow(
    clippy::expect_used,
    reason = "process smoke tests use direct assertions for compact diagnostics"
)]

use std::process::Command;

#[test]
fn stab_binary_is_invokable_with_stable_help_name() {
    let output = Command::new(env!("CARGO_BIN_EXE_stab"))
        .arg("--help")
        .output()
        .expect("run stab --help");

    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stderr).expect("stderr"), "");
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("Usage: stab [COMMAND]"));
}
