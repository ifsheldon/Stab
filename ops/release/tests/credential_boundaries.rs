#![allow(
    clippy::expect_used,
    reason = "integration tests use concise process fixture assertions"
)]

use std::fs;
use std::process::{Command, Output};

const RELEASE_CREDENTIALS: &[&str] = &[
    "CARGO_REGISTRY_TOKEN",
    "CARGO_REGISTRIES_CRATES_IO_TOKEN",
    "GITHUB_TOKEN",
    "GH_TOKEN",
];
const SECRET_VALUE: &str = "must-not-appear-in-diagnostics";

fn run_operator(binary: &str, args: &[&str], variables: &[(&str, &str)]) -> Output {
    let mut command = Command::new(binary);
    command.args(args);
    for name in RELEASE_CREDENTIALS {
        command.env_remove(name);
    }
    command.envs(variables.iter().copied());
    command.output().expect("run release operator")
}

fn assert_startup_rejection(binary: &str, args: &[&str], allowed: (&str, &str), forbidden: &str) {
    let output = run_operator(binary, args, &[allowed, (forbidden, SECRET_VALUE)]);
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
            env!("CARGO_BIN_EXE_stab-release"),
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
        assert_startup_rejection(
            env!("CARGO_BIN_EXE_stab-release"),
            &args,
            ("GITHUB_TOKEN", "reviewed-github-secret"),
            forbidden,
        );
    }
}

#[cfg(unix)]
#[test]
fn production_operator_supports_the_token_free_cargo_reexec_boundary() {
    let root = tempfile::tempdir().expect("temporary isolated Cargo paths");
    let home = root.path().join("home");
    let cargo_home = root.path().join("cargo-home");
    let target = root.path().join("target");
    let temporary = root.path().join("tmp");
    for path in [&home, &cargo_home, &target, &temporary] {
        fs::create_dir(path).expect("isolated Cargo directory");
    }
    let config = cargo_home.join("config.toml");
    fs::write(&config, "[net]\noffline = true\n").expect("isolated Cargo config");
    let output = run_operator(
        env!("CARGO_BIN_EXE_stab-release"),
        &[
            "__isolated-cargo",
            "--cargo",
            "/bin/true",
            "--rustc",
            "/bin/true",
            "--rustdoc",
            "/bin/true",
            "--home",
            home.to_str().expect("UTF-8 temporary path"),
            "--cargo-home",
            cargo_home.to_str().expect("UTF-8 temporary path"),
            "--target",
            target.to_str().expect("UTF-8 temporary path"),
            "--temporary",
            temporary.to_str().expect("UTF-8 temporary path"),
            "--config",
            config.to_str().expect("UTF-8 temporary path"),
            "--",
            "--version",
        ],
        &[],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
