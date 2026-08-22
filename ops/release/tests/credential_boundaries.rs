#![allow(
    clippy::expect_used,
    reason = "integration tests use concise process fixture assertions"
)]

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
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

#[test]
fn rehearsal_draft_rejects_unrelated_credentials_at_startup() {
    let args = [
        "create-draft",
        "--assets",
        "target/releases/missing-assets",
        "--tag",
        "v0.2.0-rehearsal-0123456789abcdef0123456789abcdef01234567",
        "--confirm-repository",
        "ifsheldon/Stab-release-rehearsal",
    ];
    for forbidden in [
        "CARGO_REGISTRY_TOKEN",
        "CARGO_REGISTRIES_CRATES_IO_TOKEN",
        "GH_TOKEN",
    ] {
        assert_startup_rejection(
            env!("CARGO_BIN_EXE_stab-release-rehearsal"),
            &args,
            ("GITHUB_TOKEN", "reviewed-github-secret"),
            forbidden,
        );
    }
}

#[test]
fn rehearsal_operator_exposes_no_irreversible_publication_command() {
    let output = run_operator(
        env!("CARGO_BIN_EXE_stab-release-rehearsal"),
        &["--help"],
        &[],
    );
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 help");
    assert!(stdout.contains("create-draft"));
    for forbidden in [
        "publish-reviewed",
        "verify-published-release",
        "confirm-version",
    ] {
        assert!(
            !stdout.contains(forbidden),
            "unexpected command {forbidden}"
        );
    }

    let rejected = run_operator(
        env!("CARGO_BIN_EXE_stab-release-rehearsal"),
        &["publish-reviewed"],
        &[],
    );
    assert!(!rejected.status.success());
    let stderr = String::from_utf8(rejected.stderr).expect("UTF-8 diagnostics");
    assert!(stderr.contains("unrecognized subcommand"));
}

#[cfg(unix)]
#[test]
fn rehearsal_operator_supports_the_token_free_cargo_reexec_boundary() {
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
        env!("CARGO_BIN_EXE_stab-release-rehearsal"),
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

#[cfg(unix)]
#[test]
fn rehearsal_authorization_command_shapes_run_from_a_private_root() {
    use std::os::unix::ffi::OsStringExt as _;
    use std::os::unix::fs::symlink;

    fn rustup_which(program: &str) -> PathBuf {
        let output = Command::new("rustup")
            .args(["which", program])
            .output()
            .expect("query pinned Rustup program");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let mut path = output.stdout;
        while matches!(path.last(), Some(b'\n' | b'\r')) {
            path.pop();
        }
        assert!(!path.is_empty(), "Rustup returned an empty program path");
        PathBuf::from(OsString::from_vec(path))
    }

    fn ambient_cargo_home() -> PathBuf {
        if let Some(path) = std::env::var_os("CARGO_HOME") {
            return PathBuf::from(path);
        }
        PathBuf::from(std::env::var_os("HOME").expect("HOME for Cargo cache")).join(".cargo")
    }

    fn share_public_cache(source_home: &Path, private_home: &Path, name: &str) {
        let source = source_home.join(name);
        if source.exists() {
            symlink(source, private_home.join(name)).expect("share public Cargo cache");
        }
    }

    fn cargo_arguments(root: &Path, package: &str, command: &[&str]) -> Vec<OsString> {
        let mut arguments = [
            "run",
            "--quiet",
            "--locked",
            "--package",
            package,
            "--manifest-path",
        ]
        .map(OsString::from)
        .to_vec();
        arguments.push(root.join("Cargo.toml").into_os_string());
        arguments.push(OsString::from("--"));
        arguments.extend(command.iter().map(OsString::from));
        arguments
    }

    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root");
    let fixture = tempfile::tempdir().expect("temporary authorization sandbox");
    let home = fixture.path().join("home");
    let cargo_home = fixture.path().join("cargo-home");
    let target = fixture.path().join("target");
    let temporary = fixture.path().join("tmp");
    let hostile_working_directory = fixture.path().join("hostile-working-directory");
    for path in [
        &home,
        &cargo_home,
        &target,
        &temporary,
        &hostile_working_directory,
    ] {
        fs::create_dir(path).expect("isolated Cargo directory");
    }
    let hostile_config = hostile_working_directory.join(".cargo");
    fs::create_dir(&hostile_config).expect("hostile Cargo config directory");
    fs::write(
        hostile_config.join("config.toml"),
        "[build]\nrustc-wrapper = \"/attacker/missing-wrapper\"\n",
    )
    .expect("hostile Cargo config");
    let ambient_cargo_home = ambient_cargo_home();
    share_public_cache(&ambient_cargo_home, &cargo_home, "registry");
    share_public_cache(&ambient_cargo_home, &cargo_home, "git");
    let config = cargo_home.join("config.toml");
    fs::write(&config, "[net]\ngit-fetch-with-cli = false\n").expect("isolated Cargo config");

    let cargo = rustup_which("cargo");
    let rustc = rustup_which("rustc");
    let rustdoc = rustup_which("rustdoc");
    let root_text = workspace.to_str().expect("UTF-8 workspace path");
    let commands = [
        cargo_arguments(
            &workspace,
            "stab-architecture",
            &["--root", root_text, "check"],
        ),
        cargo_arguments(
            &workspace,
            "stab-bench",
            &["--root", root_text, "qualification-status", "--check"],
        ),
    ];

    for cargo_arguments in commands {
        let mut arguments = [
            OsString::from("__isolated-cargo"),
            OsString::from("--cargo"),
            cargo.as_os_str().to_os_string(),
            OsString::from("--rustc"),
            rustc.as_os_str().to_os_string(),
            OsString::from("--rustdoc"),
            rustdoc.as_os_str().to_os_string(),
            OsString::from("--home"),
            home.as_os_str().to_os_string(),
            OsString::from("--cargo-home"),
            cargo_home.as_os_str().to_os_string(),
            OsString::from("--target"),
            target.as_os_str().to_os_string(),
            OsString::from("--temporary"),
            temporary.as_os_str().to_os_string(),
            OsString::from("--config"),
            config.as_os_str().to_os_string(),
            OsString::from("--"),
        ]
        .to_vec();
        arguments.extend(cargo_arguments);

        let mut command = Command::new(env!("CARGO_BIN_EXE_stab-release-rehearsal"));
        command
            .current_dir(&hostile_working_directory)
            .args(arguments);
        for name in RELEASE_CREDENTIALS {
            command.env_remove(name);
        }
        let output = command
            .output()
            .expect("run rehearsal authorization command");
        assert!(
            output.status.success(),
            "stdout={}\nstderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
