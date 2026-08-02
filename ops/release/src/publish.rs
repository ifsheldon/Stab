use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use crate::{RELEASE_VERSION, ReleaseError, archive, package, registry, repository, safe_fs};

const MAX_CARGO_OUTPUT_BYTES: usize = 8 << 20;
const CARGO_PUBLISH_TIMEOUT: Duration = Duration::from_secs(30 * 60);
static PUBLICATION_COUNTER: AtomicU64 = AtomicU64::new(0);

pub(crate) fn publish_reviewed(
    root: &Path,
    preflight: &Path,
    confirmation: &str,
) -> Result<(), ReleaseError> {
    if confirmation != RELEASE_VERSION {
        return Err(ReleaseError::PublicationConfirmation {
            expected: RELEASE_VERSION.to_string(),
            actual: confirmation.to_string(),
        });
    }
    let (preflight_directory, report) = package::load_reviewed_report(root, preflight)?;
    let reviewed_packages = preflight_directory.open_directory(OsStr::new("packages"))?;
    let registry = registry::CratesIo::new();
    let cargo = repository::cargo_program();

    for package in &report.packages {
        if registry::require_absent_or_matching(
            &registry,
            &package.name,
            &package.version,
            &package.sha256,
        )? {
            println!(
                "[stab-release] crates.io already has reviewed {} {} ({})",
                package.name, package.version, package.sha256
            );
            continue;
        }

        let work_name = format!(
            ".publish-{}-{}",
            std::process::id(),
            PUBLICATION_COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        let work = safe_fs::RetainedDirectory::create_new_under(
            root,
            Path::new("target/releases").join(&work_name).as_path(),
            Some(Path::new("target/releases")),
        )?;
        let cargo_target = work.create_directory(OsStr::new("cargo-target"))?;
        let environment = vec![(
            OsString::from("CARGO_TARGET_DIR"),
            cargo_target.path().as_os_str().to_os_string(),
        )];
        let package_args = individual_package_arguments(&package.name);
        repository::run_with_environment(
            root,
            &cargo,
            &package_args,
            &environment,
            CARGO_PUBLISH_TIMEOUT,
            MAX_CARGO_OUTPUT_BYTES,
        )?;
        cargo_target.revalidate()?;
        let rebuilt_directory = cargo_target.open_directory(OsStr::new("package"))?;
        require_rebuilt_match(&rebuilt_directory, package, &report.commit)?;
        preflight_directory.revalidate()?;
        reviewed_packages.revalidate()?;
        repository::require_unchanged(root, &report.commit)?;
        repository::require_toolchain(root, &report.toolchain)?;

        let publish_args = individual_publish_arguments(&package.name);
        repository::run_with_environment(
            root,
            &cargo,
            &publish_args,
            &environment,
            CARGO_PUBLISH_TIMEOUT,
            MAX_CARGO_OUTPUT_BYTES,
        )?;
        rebuilt_directory.revalidate()?;
        require_rebuilt_match(&rebuilt_directory, package, &report.commit)?;
        repository::require_unchanged(root, &report.commit)?;
        repository::require_toolchain(root, &report.toolchain)?;
        registry::wait_for_matching_checksum(
            &registry,
            &package.name,
            &package.version,
            &package.sha256,
        )?;
        work.revalidate()?;
        fs::remove_dir_all(work.path()).map_err(|source| ReleaseError::io(work.path(), source))?;
        println!(
            "[stab-release] published and checksum-verified {} {} ({})",
            package.name, package.version, package.sha256
        );
    }
    preflight_directory.revalidate()?;
    repository::require_unchanged(root, &report.commit)?;
    println!(
        "[stab-release] all {} reviewed crates are visible with matching checksums",
        report.packages.len()
    );
    Ok(())
}

fn require_rebuilt_match(
    rebuilt_directory: &safe_fs::RetainedDirectory,
    package: &package::PackageArchive,
    commit: &str,
) -> Result<(), ReleaseError> {
    let archive_name = package::archive_name(&package.name, &package.version);
    let rebuilt_path = rebuilt_directory.path().join(&archive_name);
    let rebuilt_file = rebuilt_directory.open_regular(OsStr::new(&archive_name))?;
    let rebuilt = archive::read_file_and_validate(
        rebuilt_file,
        &rebuilt_path,
        &package.name,
        &package.version,
        commit,
    )?;
    let bytes = u64::try_from(rebuilt.bytes.len()).map_err(|_| {
        ReleaseError::PackageContract(format!(
            "rebuilt {} archive size does not fit in u64",
            package.name
        ))
    })?;
    if rebuilt.sha256 != package.sha256 || bytes != package.bytes {
        return Err(ReleaseError::ArchiveContract {
            path: rebuilt_path,
            detail: format!(
                "Cargo rebuild is {} bytes with SHA-256 {}, reviewed archive is {} bytes with SHA-256 {}",
                bytes, rebuilt.sha256, package.bytes, package.sha256
            ),
        });
    }
    Ok(())
}

fn individual_package_arguments(package: &str) -> Vec<OsString> {
    ["package", "--locked", "--no-verify", "--package", package]
        .map(OsString::from)
        .to_vec()
}

fn individual_publish_arguments(package: &str) -> Vec<OsString> {
    [
        "publish",
        "--locked",
        "--registry",
        "crates-io",
        "--package",
        package,
    ]
    .map(OsString::from)
    .to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publication_is_explicit_and_registry_pinned() {
        assert_eq!(
            individual_package_arguments("stab-core"),
            [
                "package",
                "--locked",
                "--no-verify",
                "--package",
                "stab-core"
            ]
            .map(OsString::from)
        );
        assert_eq!(
            individual_publish_arguments("stab-core"),
            [
                "publish",
                "--locked",
                "--registry",
                "crates-io",
                "--package",
                "stab-core"
            ]
            .map(OsString::from)
        );
    }
}
