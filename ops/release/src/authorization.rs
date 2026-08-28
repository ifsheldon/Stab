use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use crate::{ReleaseError, cancellation::ReleaseCancellation, cargo, safe_fs};

const MAX_AUTHORIZATION_OUTPUT_BYTES: usize = 8 << 20;
const AUTHORIZATION_TIMEOUT: Duration = Duration::from_secs(30 * 60);
static AUTHORIZATION_COUNTER: AtomicU64 = AtomicU64::new(0);

pub(crate) fn require_release_evidence(
    root: &Path,
    cancellation: &ReleaseCancellation,
) -> Result<(), ReleaseError> {
    require(
        root,
        cancellation,
        "E2E release authorization",
        &[e2e_release_check_arguments(root)],
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

fn e2e_release_check_arguments(root: &Path) -> Vec<OsString> {
    let mut arguments = [
        "run",
        "--quiet",
        "--locked",
        "--package",
        "stab-bench",
        "--",
        "--root",
    ]
    .map(OsString::from)
    .to_vec();
    arguments.push(root.as_os_str().to_os_string());
    arguments.push(OsString::from("e2e-release-check"));
    arguments
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authorization_uses_the_checked_e2e_evidence_contract() {
        let root = Path::new("/source");
        assert_eq!(
            e2e_release_check_arguments(root),
            [
                "run",
                "--quiet",
                "--locked",
                "--package",
                "stab-bench",
                "--",
                "--root",
                "/source",
                "e2e-release-check",
            ]
            .map(OsString::from)
        );
    }
}
