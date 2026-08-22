use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use crate::{ReleaseError, cancellation::ReleaseCancellation, cargo, safe_fs};

const MAX_AUTHORIZATION_OUTPUT_BYTES: usize = 8 << 20;
const AUTHORIZATION_TIMEOUT: Duration = Duration::from_secs(30 * 60);
static AUTHORIZATION_COUNTER: AtomicU64 = AtomicU64::new(0);

pub(crate) fn require_a9_release(
    root: &Path,
    cancellation: &ReleaseCancellation,
) -> Result<(), ReleaseError> {
    require(
        root,
        cancellation,
        "A9 release authorization",
        &[qualification_status_arguments(true)],
    )
}

pub(crate) fn require_rehearsal(
    root: &Path,
    cancellation: &ReleaseCancellation,
) -> Result<(), ReleaseError> {
    require(
        root,
        cancellation,
        "release rehearsal authorization",
        &[
            architecture_check_arguments(root),
            qualification_status_arguments(false),
        ],
    )
}

fn require(
    root: &Path,
    cancellation: &ReleaseCancellation,
    operation: &'static str,
    commands: &[Vec<OsString>],
) -> Result<(), ReleaseError> {
    cancellation.check(operation)?;
    let work_name = format!(
        ".authorize-{}-{}",
        std::process::id(),
        AUTHORIZATION_COUNTER.fetch_add(1, Ordering::Relaxed)
    );
    let work = safe_fs::RetainedDirectory::create_new_under(
        root,
        Path::new("target/releases").join(work_name).as_path(),
        Some(Path::new("target/releases")),
    )?;
    let authorization: Result<(), ReleaseError> = (|| {
        let cargo_target = work.create_directory(OsStr::new("cargo-target"))?;
        let cargo = cargo::CargoSandbox::create(root, &work, &cargo_target)?;
        for arguments in commands {
            cargo.run(
                root,
                arguments.iter().map(OsString::as_os_str),
                AUTHORIZATION_TIMEOUT,
                MAX_AUTHORIZATION_OUTPUT_BYTES,
            )?;
        }
        Ok(())
    })();
    let cleanup = work.remove_tree();
    authorization?;
    cleanup?;
    cancellation.check(operation)
}

fn qualification_status_arguments(require_release_completion: bool) -> Vec<OsString> {
    let mut arguments = [
        "run",
        "--quiet",
        "--locked",
        "--package",
        "stab-bench",
        "--",
        "qualification-status",
        "--check",
    ]
    .map(OsString::from)
    .to_vec();
    if require_release_completion {
        arguments.push(OsString::from("--require-release-completion"));
    }
    arguments
}

fn architecture_check_arguments(root: &Path) -> Vec<OsString> {
    let mut arguments = [
        "run",
        "--quiet",
        "--locked",
        "--package",
        "stab-architecture",
        "--",
        "--root",
    ]
    .map(OsString::from)
    .to_vec();
    arguments.push(root.as_os_str().to_os_string());
    arguments.push(OsString::from("check"));
    arguments
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authorization_uses_the_checked_a9_status_contract() {
        assert_eq!(
            qualification_status_arguments(true),
            [
                "run",
                "--quiet",
                "--locked",
                "--package",
                "stab-bench",
                "--",
                "qualification-status",
                "--check",
                "--require-release-completion",
            ]
            .map(OsString::from)
        );
    }

    #[test]
    fn rehearsal_uses_architecture_and_non_release_status_contracts() {
        let root = Path::new("/source");
        assert_eq!(
            architecture_check_arguments(root),
            [
                "run",
                "--quiet",
                "--locked",
                "--package",
                "stab-architecture",
                "--",
                "--root",
                "/source",
                "check",
            ]
            .map(OsString::from)
        );
        assert_eq!(
            qualification_status_arguments(false),
            [
                "run",
                "--quiet",
                "--locked",
                "--package",
                "stab-bench",
                "--",
                "qualification-status",
                "--check",
            ]
            .map(OsString::from)
        );
    }
}
