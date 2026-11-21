//! End-to-end tests that install the real hook binary into throwaway repositories and drive it through `git commit`.
//!
//! These tests require `git` on `PATH` and fail with a clear message when it is missing.
//! Rust tools never run here: `STAB_PRE_COMMIT_DRY_RUN_TOOLS` makes the hook report the exact commands it would run.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    reason = "integration tests use direct assertions for concise failure diagnostics"
)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const HOOK_BINARY: &str = env!("CARGO_BIN_EXE_stab-pre-commit");
const INHERITED_GIT_VARIABLES: &[&str] = &[
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_CEILING_DIRECTORIES",
    "GIT_COMMON_DIR",
    "GIT_CONFIG",
    "GIT_CONFIG_COUNT",
    "GIT_CONFIG_PARAMETERS",
    "GIT_DIR",
    "GIT_INDEX_FILE",
    "GIT_NAMESPACE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_PREFIX",
    "GIT_WORK_TREE",
];

struct Sandbox {
    _temp: tempfile::TempDir,
    home: PathBuf,
    repo: PathBuf,
}

impl Sandbox {
    /// A repository with a committed, policy-clean baseline and no hook installed yet.
    fn new() -> Self {
        let temp = tempfile::tempdir().expect("create sandbox");
        let home = temp.path().join("home");
        let repo = temp.path().join("main");
        std::fs::create_dir_all(&home).expect("create home");
        std::fs::create_dir_all(&repo).expect("create repo");
        std::fs::write(home.join(".gitconfig"), "").expect("write empty global config");
        let sandbox = Self {
            _temp: temp,
            home,
            repo,
        };
        sandbox.git_ok(&sandbox.repo, &["init", "-q", "-b", "main"]);
        sandbox.write("README.md", "root\n");
        sandbox.write("AGENTS.md", "rules\n");
        sandbox.symlink("AGENTS.md", "CLAUDE.md");
        sandbox.write(".gitignore", "ignored/\n");
        sandbox.write("src/lib.rs", "pub fn f() {}\n");
        sandbox.write("tasks.just", "default:\n    true\n");
        sandbox.git_ok(&sandbox.repo, &["add", "-A"]);
        sandbox.git_ok(
            &sandbox.repo,
            &["commit", "-q", "--no-verify", "-m", "base"],
        );
        sandbox
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.repo.join(relative)
    }

    fn write(&self, relative: &str, content: &str) {
        let path = self.path(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create parent");
        }
        std::fs::write(path, content).expect("write file");
    }

    fn symlink(&self, target: &str, relative: &str) {
        let path = self.path(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create parent");
        }
        std::os::unix::fs::symlink(target, path).expect("create symlink");
    }

    fn command(&self, program: &str, cwd: &Path) -> Command {
        let mut command = Command::new(program);
        for variable in INHERITED_GIT_VARIABLES {
            command.env_remove(variable);
        }
        command
            .current_dir(cwd)
            .env("HOME", &self.home)
            .env("XDG_CONFIG_HOME", self.home.join(".config"))
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", self.home.join(".gitconfig"))
            .env("GIT_AUTHOR_NAME", "Hook Test")
            .env("GIT_AUTHOR_EMAIL", "hook@test.invalid")
            .env("GIT_COMMITTER_NAME", "Hook Test")
            .env("GIT_COMMITTER_EMAIL", "hook@test.invalid")
            .env("STAB_PRE_COMMIT_DRY_RUN_TOOLS", "1");
        command
    }

    fn git(&self, cwd: &Path, args: &[&str]) -> Output {
        self.command("git", cwd)
            .args(args)
            .output()
            .unwrap_or_else(|error| {
                panic!("these tests require `git` on PATH; running git failed: {error}")
            })
    }

    fn git_ok(&self, cwd: &Path, args: &[&str]) {
        let output = self.git(cwd, args);
        assert!(
            output.status.success(),
            "git {args:?} failed:\n{}",
            combined(&output)
        );
    }

    fn hook(&self, cwd: &Path, args: &[&str]) -> Output {
        self.command(HOOK_BINARY, cwd)
            .args(args)
            .output()
            .expect("run stab-pre-commit")
    }

    fn install(&self, cwd: &Path, args: &[&str]) -> Output {
        let mut full = vec!["install"];
        full.extend_from_slice(args);
        self.hook(cwd, &full)
    }

    fn installed(self) -> Self {
        let output = self.install(&self.repo, &[]);
        assert!(
            output.status.success(),
            "install failed:\n{}",
            combined(&output)
        );
        self
    }

    fn commit(&self, args: &[&str]) -> Output {
        let mut full = vec!["commit", "-q", "-m", "change"];
        full.extend_from_slice(args);
        self.git(&self.repo, &full)
    }
}

fn combined(output: &Output) -> String {
    format!(
        "status: {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn assert_rejected(output: &Output, expected: &str) {
    let text = combined(output);
    assert!(
        !output.status.success(),
        "commit should be rejected:\n{text}"
    );
    assert!(text.contains(expected), "missing {expected:?} in:\n{text}");
}

fn assert_accepted(output: &Output) {
    assert!(
        output.status.success(),
        "commit should pass:\n{}",
        combined(output)
    );
}

fn oversized_just_file() -> String {
    "recipe:\n".to_string() + &"    true\n".repeat(1_300)
}

#[test]
fn commit_all_checks_the_temporary_index() {
    let sandbox = Sandbox::new().installed();
    sandbox.write("tasks.just", &oversized_just_file());

    let output = sandbox.commit(&["-a"]);

    assert_rejected(&output, "tasks.just");
    assert!(combined(&output).contains("FAIL large files"));
}

#[test]
fn pathspec_commit_checks_the_temporary_index() {
    let sandbox = Sandbox::new().installed();
    sandbox.write("tasks.just", &oversized_just_file());

    let output = sandbox.commit(&["--", "tasks.just"]);

    assert_rejected(&output, "tasks.just");
}

#[test]
fn readme_staged_without_its_agents_file_is_rejected() {
    let sandbox = Sandbox::new().installed();
    sandbox.write("d/README.md", "d\n");
    sandbox.write("d/AGENTS.md", "d rules\n");
    sandbox.symlink("AGENTS.md", "d/CLAUDE.md");
    sandbox.git_ok(&sandbox.repo, &["add", "d/README.md"]);

    assert_rejected(
        &sandbox.commit(&[]),
        "d/README.md has no colocated AGENTS.md",
    );

    sandbox.git_ok(&sandbox.repo, &["add", "d"]);
    assert_accepted(&sandbox.commit(&[]));
}

#[test]
fn agents_file_removed_from_the_index_is_rejected_while_still_on_disk() {
    let sandbox = Sandbox::new().installed();
    sandbox.write("d/README.md", "d\n");
    sandbox.write("d/AGENTS.md", "d rules\n");
    sandbox.symlink("AGENTS.md", "d/CLAUDE.md");
    sandbox.git_ok(&sandbox.repo, &["add", "d"]);
    assert_accepted(&sandbox.commit(&[]));

    sandbox.git_ok(&sandbox.repo, &["rm", "-q", "--cached", "d/AGENTS.md"]);
    let output = sandbox.commit(&[]);

    assert_rejected(&output, "d/README.md has no colocated AGENTS.md");
    assert!(combined(&output).contains("d/CLAUDE.md is a broken symlink"));
}

#[test]
fn force_added_readme_in_an_ignored_directory_is_scanned() {
    let sandbox = Sandbox::new().installed();
    sandbox.write("ignored/README.md", "ignored\n");
    sandbox.git_ok(&sandbox.repo, &["add", "-f", "ignored/README.md"]);

    assert_rejected(
        &sandbox.commit(&[]),
        "ignored/README.md has no colocated AGENTS.md",
    );
}

#[test]
fn rust_checks_refuse_partially_staged_and_untracked_rust_paths() {
    let sandbox = Sandbox::new().installed();
    sandbox.write("src/lib.rs", "pub fn f() {}\npub fn g() {}\n");
    sandbox.git_ok(&sandbox.repo, &["add", "src/lib.rs"]);
    sandbox.write(
        "src/lib.rs",
        "pub fn f() {}\npub fn g() {}\npub fn h() {}\n",
    );

    let partial = sandbox.commit(&[]);
    assert_rejected(&partial, "src/lib.rs has unstaged changes");
    assert!(!combined(&partial).contains("would run"));

    sandbox.git_ok(&sandbox.repo, &["add", "src/lib.rs"]);
    sandbox.write("src/extra.rs", "pub fn extra() {}\n");
    assert_rejected(
        &sandbox.commit(&[]),
        "src/extra.rs is untracked and not ignored",
    );

    std::fs::remove_file(sandbox.path("src/extra.rs")).expect("remove untracked file");
    let clean = sandbox.commit(&[]);
    assert_accepted(&clean);
    assert!(
        combined(&clean).contains(
            "would run `cargo clippy --workspace --all-targets --locked --config build.warnings='deny'`"
        ),
        "{}",
        combined(&clean)
    );
}

#[test]
fn toolchain_lint_format_and_cargo_config_paths_trigger_rust_checks() {
    let sandbox = Sandbox::new();
    for path in [
        "rust-toolchain.toml",
        "rust-toolchain",
        "clippy.toml",
        ".clippy.toml",
        "rustfmt.toml",
        ".rustfmt.toml",
        ".cargo/config.toml",
    ] {
        sandbox.write(path, "# config\n");
        sandbox.git_ok(&sandbox.repo, &["add", path]);

        let output = sandbox.hook(&sandbox.repo, &[]);
        let text = combined(&output);

        assert!(output.status.success(), "{path}:\n{text}");
        assert!(text.contains("staged: rust=1"), "{path}:\n{text}");
        assert!(
            text.contains("would run `cargo fmt --all --check`"),
            "{path}:\n{text}"
        );

        sandbox.git_ok(&sandbox.repo, &["rm", "-q", "--cached", path]);
        std::fs::remove_file(sandbox.path(path)).expect("remove config file");
    }
}

#[test]
fn manual_run_reports_an_empty_index() {
    let sandbox = Sandbox::new();

    let output = sandbox.hook(&sandbox.repo, &[]);

    assert!(output.status.success(), "{}", combined(&output));
    assert!(combined(&output).contains("no staged changes"));
}

#[test]
fn install_refuses_a_foreign_hook_unless_forced_and_recognises_its_own_copy() {
    let sandbox = Sandbox::new();
    let hook = sandbox.path(".git/hooks/pre-commit");
    std::fs::create_dir_all(hook.parent().expect("hooks dir")).expect("create hooks dir");
    std::fs::write(&hook, "#!/bin/sh\nexit 0\n").expect("write foreign hook");

    let refused = sandbox.install(&sandbox.repo, &[]);
    assert_eq!(refused.status.code(), Some(1), "{}", combined(&refused));
    assert!(combined(&refused).contains("--force"));
    assert_eq!(
        std::fs::read_to_string(&hook).expect("read hook"),
        "#!/bin/sh\nexit 0\n"
    );

    let forced = sandbox.install(&sandbox.repo, &["--force"]);
    assert!(forced.status.success(), "{}", combined(&forced));
    assert_eq!(
        std::fs::read(&hook).expect("read installed hook"),
        std::fs::read(HOOK_BINARY).expect("read hook binary")
    );

    let again = sandbox.install(&sandbox.repo, &[]);
    assert!(again.status.success(), "{}", combined(&again));
    assert!(combined(&again).contains("replaced a previous stab-pre-commit copy"));
}

#[test]
fn install_honours_core_hooks_path() {
    let sandbox = Sandbox::new();
    sandbox.git_ok(&sandbox.repo, &["config", "core.hooksPath", "custom-hooks"]);

    let output = sandbox.install(&sandbox.path("src"), &[]);

    assert!(output.status.success(), "{}", combined(&output));
    assert!(sandbox.path("custom-hooks/pre-commit").is_file());
    assert!(!sandbox.path(".git/hooks/pre-commit").exists());
    sandbox.write("d/README.md", "d\n");
    sandbox.git_ok(&sandbox.repo, &["add", "d/README.md"]);
    assert_rejected(
        &sandbox.commit(&[]),
        "d/README.md has no colocated AGENTS.md",
    );
}

#[test]
fn install_from_a_linked_worktree_targets_the_common_hooks_directory() {
    let sandbox = Sandbox::new();
    let worktree = sandbox.repo.with_file_name("linked");
    let worktree_arg = worktree.to_str().expect("UTF-8 temp path");
    sandbox.git_ok(
        &sandbox.repo,
        &["worktree", "add", "-q", "-b", "linked", worktree_arg],
    );

    let output = sandbox.install(&worktree, &[]);

    assert!(output.status.success(), "{}", combined(&output));
    assert!(sandbox.path(".git/hooks/pre-commit").is_file());
    std::fs::write(worktree.join("README2.md"), "x\n").expect("write file");
    std::fs::create_dir_all(worktree.join("w")).expect("create dir");
    std::fs::write(worktree.join("w/README.md"), "w\n").expect("write readme");
    sandbox.git_ok(&worktree, &["add", "w/README.md"]);
    let rejected = sandbox.git(&worktree, &["commit", "-q", "-m", "w"]);
    assert_rejected(&rejected, "w/README.md has no colocated AGENTS.md");

    sandbox.git_ok(&worktree, &["rm", "-q", "--cached", "w/README.md"]);
    sandbox.git_ok(&worktree, &["add", "README2.md"]);
    let accepted = sandbox.git(&worktree, &["commit", "-q", "-m", "ok"]);
    assert_accepted(&accepted);
}
