#![cfg(unix)]
#![allow(
    clippy::expect_used,
    reason = "integration tests use concise process fixture assertions"
)]

use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

const RELEASE_CREDENTIALS: &[&str] = &[
    "CARGO_REGISTRY_TOKEN",
    "CARGO_REGISTRIES_CRATES_IO_TOKEN",
    "GITHUB_TOKEN",
    "GH_TOKEN",
];

struct RecipeFixture {
    temporary: tempfile::TempDir,
    root: PathBuf,
    fake_bin: PathBuf,
    operator: PathBuf,
}

impl RecipeFixture {
    fn new() -> Self {
        let temporary = tempfile::tempdir().expect("temporary recipe fixture");
        let fake_bin = temporary.path().join("bin");
        fs::create_dir(&fake_bin).expect("fake binary directory");
        let operator = temporary.path().join("fake-stab-release");
        write_executable(
            &operator,
            r#"#!/bin/sh
set -eu
printf '%s\0' "$@" > "$STAB_RELEASE_RECIPE_ARGV"
{
    if [ "${CARGO_REGISTRY_TOKEN+x}" = x ]; then printf '%s\n' CARGO_REGISTRY_TOKEN; fi
    if [ "${CARGO_REGISTRIES_CRATES_IO_TOKEN+x}" = x ]; then printf '%s\n' CARGO_REGISTRIES_CRATES_IO_TOKEN; fi
    if [ "${GITHUB_TOKEN+x}" = x ]; then printf '%s\n' GITHUB_TOKEN; fi
    if [ "${GH_TOKEN+x}" = x ]; then printf '%s\n' GH_TOKEN; fi
} > "$STAB_RELEASE_RECIPE_ENV"
"#,
        );
        write_executable(
            &fake_bin.join("cargo"),
            r#"#!/bin/sh
set -eu
if [ "${CARGO_REGISTRY_TOKEN+x}" = x ] ||
   [ "${CARGO_REGISTRIES_CRATES_IO_TOKEN+x}" = x ] ||
   [ "${GITHUB_TOKEN+x}" = x ] ||
   [ "${GH_TOKEN+x}" = x ]; then
    printf '%s\n' 'fake Cargo received a release credential' >&2
    exit 71
fi
mkdir -p "$CARGO_TARGET_DIR/debug"
cp "$STAB_RELEASE_FAKE_OPERATOR" "$CARGO_TARGET_DIR/debug/stab-release"
chmod 700 "$CARGO_TARGET_DIR/debug/stab-release"
printf '%s\n' "$CARGO_TARGET_DIR" > "$STAB_RELEASE_RECIPE_TARGET"
"#,
        );
        Self {
            temporary,
            root: Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .canonicalize()
                .expect("canonical repository root"),
            fake_bin,
            operator,
        }
    }

    fn run(&self, recipe: &str, args: &[&str], allowed_credential: (&str, &str)) -> RecipeResult {
        let argv = self.temporary.path().join(format!("{recipe}-argv"));
        let environment = self.temporary.path().join(format!("{recipe}-environment"));
        let target = self.temporary.path().join(format!("{recipe}-target"));
        let mut path = OsString::from(self.fake_bin.as_os_str());
        path.push(":");
        path.push(std::env::var_os("PATH").expect("test PATH"));

        let mut command = Command::new("just");
        command
            .current_dir(&self.root)
            .arg(format!("release::{recipe}"))
            .args(args)
            .env("PATH", path)
            .env("STAB_RELEASE_FAKE_OPERATOR", &self.operator)
            .env("STAB_RELEASE_RECIPE_ARGV", &argv)
            .env("STAB_RELEASE_RECIPE_ENV", &environment)
            .env("STAB_RELEASE_RECIPE_TARGET", &target);
        for name in RELEASE_CREDENTIALS {
            command.env_remove(name);
        }
        command.env(allowed_credential.0, allowed_credential.1);
        for name in RELEASE_CREDENTIALS {
            if *name != allowed_credential.0 {
                command.env(name, "unrelated-secret");
            }
        }
        let output = command.output().expect("run release recipe");
        assert!(
            output.status.success(),
            "release recipe failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        let target = fs::read_to_string(target)
            .expect("recorded operator target")
            .trim()
            .to_string();
        RecipeResult {
            argv: fs::read(argv)
                .expect("recorded argv")
                .split(|byte| *byte == 0)
                .filter(|field| !field.is_empty())
                .map(|field| String::from_utf8(field.to_vec()).expect("UTF-8 argument"))
                .collect(),
            credentials: fs::read_to_string(environment)
                .expect("recorded credential environment")
                .lines()
                .map(str::to_string)
                .collect(),
            target,
        }
    }

    fn remove_target(&self, target: &str) {
        let target = self.root.join(target);
        assert!(target.starts_with(self.root.join("target/releases/operators")));
        fs::remove_dir_all(target).expect("remove fake operator target");
    }
}

struct RecipeResult {
    argv: Vec<String>,
    credentials: Vec<String>,
    target: String,
}

fn write_executable(path: &Path, contents: &str) {
    fs::write(path, contents).expect("write executable fixture");
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .expect("set executable fixture permissions");
}

#[test]
fn irreversible_recipes_quote_arguments_scrub_credentials_and_isolate_operators() {
    let fixture = RecipeFixture::new();
    let marker = fixture.temporary.path().join("argument-was-executed");
    let hostile = format!("$(touch {})", marker.display());

    let publish = fixture.run(
        "publish-reviewed",
        &["--preflight", &hostile, "--confirm-version", "0.2.0"],
        ("CARGO_REGISTRY_TOKEN", "registry-secret"),
    );
    assert_eq!(
        publish.argv,
        [
            "publish-reviewed",
            "--preflight",
            hostile.as_str(),
            "--confirm-version",
            "0.2.0",
        ]
    );
    assert_eq!(publish.credentials, ["CARGO_REGISTRY_TOKEN"]);
    assert!(!marker.exists());

    let draft = fixture.run(
        "create-draft",
        &[
            "--assets",
            &hostile,
            "--tag",
            "v0.2.0",
            "--confirm-version",
            "0.2.0",
        ],
        ("GITHUB_TOKEN", "github-secret"),
    );
    assert_eq!(
        draft.argv,
        [
            "create-draft",
            "--assets",
            hostile.as_str(),
            "--tag",
            "v0.2.0",
            "--confirm-version",
            "0.2.0",
        ]
    );
    assert_eq!(draft.credentials, ["GITHUB_TOKEN"]);
    assert!(!marker.exists());

    assert_ne!(publish.target, draft.target);
    let operator_root = fixture.root.join("target/releases/operators");
    assert!(
        Path::new(&publish.target).starts_with(&operator_root),
        "publish target {:?} is outside {:?}",
        publish.target,
        operator_root
    );
    assert!(
        Path::new(&draft.target).starts_with(&operator_root),
        "draft target {:?} is outside {:?}",
        draft.target,
        operator_root
    );
    for target in [&publish.target, &draft.target] {
        let mode = fs::metadata(target)
            .expect("operator target metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o700);
    }
    fixture.remove_target(&publish.target);
    fixture.remove_target(&draft.target);
}
