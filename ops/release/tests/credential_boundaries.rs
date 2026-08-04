#![allow(
    clippy::expect_used,
    reason = "integration tests use concise process fixture assertions"
)]

use std::process::{Command, Output};

const RELEASE_CREDENTIALS: &[&str] = &[
    "CARGO_REGISTRY_TOKEN",
    "CARGO_REGISTRIES_CRATES_IO_TOKEN",
    "GITHUB_TOKEN",
    "GH_TOKEN",
];
const SECRET_VALUE: &str = "must-not-appear-in-diagnostics";

fn run_operator(args: &[&str], variables: &[(&str, &str)]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_stab-release"));
    command.args(args);
    for name in RELEASE_CREDENTIALS {
        command.env_remove(name);
    }
    command.envs(variables.iter().copied());
    command.output().expect("run release operator")
}

fn assert_startup_rejection(args: &[&str], allowed: (&str, &str), forbidden: &str) {
    let output = run_operator(args, &[allowed, (forbidden, SECRET_VALUE)]);
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("UTF-8 diagnostics");
    assert!(stderr.contains("release credential environment"));
    assert!(stderr.contains(forbidden));
    assert!(!stderr.contains(SECRET_VALUE));
}

#[test]
fn publish_reviewed_rejects_unrelated_credentials_at_startup() {
    let args = [
        "publish-reviewed",
        "--preflight",
        "target/releases/missing-preflight",
        "--confirm-version",
        "0.2.0",
    ];
    for forbidden in [
        "CARGO_REGISTRIES_CRATES_IO_TOKEN",
        "GITHUB_TOKEN",
        "GH_TOKEN",
    ] {
        assert_startup_rejection(
            &args,
            ("CARGO_REGISTRY_TOKEN", "reviewed-registry-secret"),
            forbidden,
        );
    }
}

#[test]
fn create_draft_rejects_unrelated_credentials_at_startup() {
    let args = [
        "create-draft",
        "--assets",
        "target/releases/missing-assets",
        "--tag",
        "v0.2.0",
        "--confirm-version",
        "0.2.0",
    ];
    for forbidden in [
        "CARGO_REGISTRY_TOKEN",
        "CARGO_REGISTRIES_CRATES_IO_TOKEN",
        "GH_TOKEN",
    ] {
        assert_startup_rejection(&args, ("GITHUB_TOKEN", "reviewed-github-secret"), forbidden);
    }
}
