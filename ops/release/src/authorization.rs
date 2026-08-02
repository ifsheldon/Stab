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
    cancellation.check("A9 release authorization")?;
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
    let authorization = (|| {
        let cargo_target = work.create_directory(OsStr::new("cargo-target"))?;
        let cargo = cargo::CargoSandbox::create(root, &work, &cargo_target)?;
        cargo.run(
            root,
            qualification_status_arguments(),
            AUTHORIZATION_TIMEOUT,
            MAX_AUTHORIZATION_OUTPUT_BYTES,
        )
    })();
    let cleanup = work.remove_tree();
    authorization?;
    cleanup?;
    cancellation.check("A9 release authorization")
}

fn qualification_status_arguments() -> Vec<OsString> {
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
    .to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authorization_uses_the_checked_a9_status_contract() {
        assert_eq!(
            qualification_status_arguments(),
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
}
