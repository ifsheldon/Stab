use std::path::Path;

use super::{
    CARGO_INVOCATION_ROOT, ExecutableIdentity, PinnedExecutable, PrivateRuntime,
    REQUIRED_EXECUTABLE_ROLES, link_cargo_cache_from, run_cargo_build, validate_identities,
};

#[test]
fn private_cargo_home_links_caches_without_host_configuration() {
    let source = tempfile::tempdir().expect("source Cargo home");
    let private = tempfile::tempdir().expect("private Cargo home");
    for directory in ["registry", "git"] {
        std::fs::create_dir(source.path().join(directory)).expect("source cache");
    }
    std::fs::write(
        source.path().join("config.toml"),
        b"[build]\nrustflags=[]\n",
    )
    .expect("host Cargo config");
    std::fs::write(
        source.path().join("credentials.toml"),
        b"[registry]\ntoken='secret'\n",
    )
    .expect("host Cargo credentials");

    link_cargo_cache_from(source.path(), private.path()).expect("link Cargo caches");

    for directory in ["registry", "git"] {
        assert_eq!(
            std::fs::read_link(private.path().join(directory)).expect("cache link"),
            source.path().join(directory)
        );
    }
    assert!(!private.path().join("config.toml").exists());
    assert!(!private.path().join("credentials.toml").exists());
}

#[test]
fn production_cargo_invocation_ignores_manifest_and_scratch_ancestor_configuration() {
    let temporary = tempfile::tempdir().expect("temporary root");
    let repository = temporary.path().join("repository");
    let runtime = temporary.path().join("runtime");
    let scratch = runtime.join("scratch");
    std::fs::create_dir_all(repository.join(".cargo")).expect("Cargo config directory");
    std::fs::create_dir_all(repository.join("src")).expect("source directory");
    std::fs::create_dir_all(runtime.join(".cargo")).expect("scratch ancestor config directory");
    std::fs::create_dir(&scratch).expect("scratch directory");
    std::fs::write(
        repository.join("Cargo.toml"),
        b"[package]\nname='config-isolation-probe'\nversion='0.0.0'\nedition='2024'\n",
    )
    .expect("probe manifest");
    std::fs::write(repository.join("src/main.rs"), b"fn main() {}\n").expect("probe source");
    std::fs::write(
        repository.join(".cargo/config.toml"),
        b"[build]\nrustc-wrapper='/definitely/missing/stab-cq1-wrapper'\n",
    )
    .expect("hostile manifest ancestor config");
    std::fs::write(
        runtime.join(".cargo/config.toml"),
        b"[build]\nrustc-wrapper='/definitely/missing/stab-cq1-scratch-wrapper'\n",
    )
    .expect("hostile scratch ancestor config");
    let cargo_home = runtime.join("cargo-home");
    let target = runtime.join("target");
    let home = runtime.join("home");
    std::fs::create_dir(&cargo_home).expect("private Cargo home");
    std::fs::create_dir(&home).expect("private home");
    let cargo = Path::new(env!("CARGO"));
    let rustc = cargo.with_file_name("rustc");
    let environment = vec![
        (std::ffi::OsString::from("CARGO_HOME"), cargo_home.into()),
        (std::ffi::OsString::from("CARGO_NET_OFFLINE"), "true".into()),
        (std::ffi::OsString::from("CARGO_TARGET_DIR"), target.into()),
        (std::ffi::OsString::from("HOME"), home.into()),
        (
            std::ffi::OsString::from("PATH"),
            std::env::var_os("PATH").expect("test PATH"),
        ),
        (std::ffi::OsString::from("RUSTC"), rustc.into_os_string()),
        (std::ffi::OsString::from("TMPDIR"), scratch.into()),
    ];
    let args = [
        std::ffi::OsString::from("check"),
        std::ffi::OsString::from("--offline"),
        std::ffi::OsString::from("--quiet"),
        std::ffi::OsString::from("--manifest-path"),
        repository.join("Cargo.toml").into_os_string(),
    ];

    run_cargo_build("Cargo isolation probe", cargo, args, &environment)
        .expect("production Cargo helper must ignore hostile source and scratch ancestors");
}

#[test]
fn private_runtime_quarantines_cleanup_beyond_the_depth_bound() {
    let runtime = PrivateRuntime::create().expect("private runtime");
    let runtime_path = runtime.path().to_path_buf();
    let mut nested = runtime_path.clone();
    for depth in 0..130 {
        nested.push(format!("nested-{depth}"));
        std::fs::create_dir(&nested).expect("nested runtime directory");
    }
    std::fs::write(nested.join("artifact"), b"quarantine").expect("deep runtime artifact");

    drop(runtime);

    assert!(
        runtime_path.exists(),
        "over-budget cleanup must quarantine instead of falling back to unbounded removal"
    );
    std::fs::remove_dir_all(runtime_path).expect("remove bounded test quarantine");
}

#[test]
fn pinned_executable_runs_sealed_bytes_after_its_path_is_replaced() {
    let directory = tempfile::tempdir().expect("temporary executable directory");
    let executable = directory.path().join("tool");
    std::fs::copy("/bin/echo", &executable).expect("copy echo");
    let pinned = PinnedExecutable::open("test-tool", &executable).expect("pin executable");
    let moved = directory.path().join("held-tool");
    std::fs::rename(&executable, &moved).expect("move held executable");
    std::fs::copy("/bin/false", &executable).expect("replace executable path");

    let output = crate::process::run_process(&pinned.program(), ["descriptor-owned"], &[], None)
        .expect("run held executable");

    assert!(output.success());
    assert_eq!(output.stdout.bytes, b"descriptor-owned\n");
}

#[test]
fn pinned_executable_runs_sealed_bytes_after_in_place_source_mutation() {
    let directory = tempfile::tempdir().expect("temporary executable directory");
    let executable = directory.path().join("tool");
    std::fs::copy("/bin/echo", &executable).expect("copy echo");
    let original = std::fs::metadata(&executable).expect("original metadata");
    let pinned = PinnedExecutable::open("test-tool", &executable).expect("pin executable");
    std::fs::copy("/bin/false", &executable).expect("mutate executable in place");
    let mutated = std::fs::metadata(&executable).expect("mutated metadata");
    use std::os::unix::fs::MetadataExt as _;
    assert_eq!(
        original.ino(),
        mutated.ino(),
        "test must retain the source inode"
    );

    let output = crate::process::run_process(&pinned.program(), ["sealed-bytes"], &[], None)
        .expect("run sealed executable");

    assert!(output.success());
    assert_eq!(output.stdout.bytes, b"sealed-bytes\n");
}

#[test]
#[ignore = "builds fresh Release Stab and Stim binaries for the CQ1 provenance probe"]
fn qualification_executables_prepare_fresh_private_builds() {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = crate::RepoRoot::resolve(
        manifest
            .parent()
            .and_then(std::path::Path::parent)
            .expect("workspace root"),
    )
    .expect("resolve workspace root");

    let executables = super::QualificationExecutables::prepare(&root)
        .expect("prepare fresh private qualification executables");
    let runtime = executables.metadata.runtime_path().to_path_buf();
    assert_eq!(
        executables.cargo_working_dir(),
        Path::new(CARGO_INVOCATION_ROOT)
    );
    super::validate_identities(executables.identities()).expect("complete executable ledger");
    assert_eq!(executables.environment_sha256().len(), 64);
    executables
        .verify_support()
        .expect("fresh support snapshots");
    for program in [executables.stab(), executables.stim()] {
        let output = crate::process::run_qualification_process_with_timeout(
            &program,
            ["help"],
            &[],
            Some(&root.path),
            std::time::Duration::from_secs(30),
            executables.environment(),
        )
        .expect("run fresh private executable");
        assert!(output.success(), "{} help failed", program.display());
    }
    drop(executables);
    assert!(
        !runtime.exists(),
        "private qualification runtime leaked at {}",
        runtime.display()
    );
}

#[test]
fn executable_identity_ledger_requires_every_canonical_role() {
    let identities = REQUIRED_EXECUTABLE_ROLES
        .iter()
        .map(|role| ExecutableIdentity {
            role: (*role).to_string(),
            bytes: 1,
            sha256: "a".repeat(64),
        })
        .collect::<Vec<_>>();
    validate_identities(&identities).expect("complete identity ledger");

    let missing = identities.iter().skip(1).cloned().collect::<Vec<_>>();
    assert!(validate_identities(&missing).is_err());
    let mut duplicated = identities;
    let duplicate = duplicated.first().expect("one identity").clone();
    duplicated.push(duplicate);
    assert!(validate_identities(&duplicated).is_err());
}
