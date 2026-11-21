use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use crate::{
    RELEASE_VERSION, ReleaseError, archive, authorization, cargo, package, registry, repository,
    safe_fs,
};

const MAX_CARGO_OUTPUT_BYTES: usize = 8 << 20;
const CARGO_PACKAGE_TIMEOUT: Duration = Duration::from_secs(30 * 60);
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
    let reviewed_metadata = preflight_directory.open_directory(OsStr::new("registry-metadata"))?;
    let cancellation = crate::cancellation::ReleaseCancellation::for_signals()?;
    authorization::require_release_evidence(root, &cancellation)?;
    preflight_directory.revalidate()?;
    reviewed_packages.revalidate()?;
    reviewed_metadata.revalidate()?;
    repository::require_unchanged(root, &report.commit)?;
    repository::require_toolchain(root, &report.toolchain)?;
    let registry = registry::CratesIo::new(cancellation.clone());

    for package in &report.packages {
        cancellation.check("reviewed package publication")?;
        let crate_name = registry::CrateName::try_new(&package.name)?;
        let crate_version = registry::CrateVersion::try_new(&package.version)?;
        if registry::require_absent_or_matching(
            &registry,
            &package.name,
            &package.version,
            &package.sha256,
        )? {
            // A recovered run may have stopped before the index caught up; the
            // next crate's packaging resolves this one through the sparse index.
            registry::wait_for_matching_checksum(
                &registry,
                &cancellation,
                &crate_name,
                &crate_version,
                &package.sha256,
            )?;
            println!(
                "[stab-release] crates.io already has reviewed {} {} ({})",
                package.name, package.version, package.sha256
            );
            continue;
        }

        let work = create_publication_work(root)?;
        // A failed upload leaves no populated cargo-home or rebuilt archive
        // behind; the first error is reported over a cleanup failure.
        let published: Result<(), ReleaseError> = (|| {
            let cargo_target = work.create_directory(OsStr::new("cargo-target"))?;
            let cargo = cargo::CargoSandbox::create(root, &work, &cargo_target)?;
            let package_args = individual_package_arguments(&package.name);
            cargo.run(
                root,
                &package_args,
                CARGO_PACKAGE_TIMEOUT,
                MAX_CARGO_OUTPUT_BYTES,
            )?;
            cargo_target.revalidate()?;
            let rebuilt_directory = cargo_target.open_directory(OsStr::new("package"))?;
            require_rebuilt_match(&rebuilt_directory, package, &report.commit)?;
            preflight_directory.revalidate()?;
            reviewed_packages.revalidate()?;
            reviewed_metadata.revalidate()?;
            repository::require_unchanged(root, &report.commit)?;
            repository::require_toolchain(root, &report.toolchain)?;

            let metadata_name = package::registry_metadata_name(&package.name, &package.version);
            let metadata_bytes = reviewed_metadata.read_bounded(
                OsStr::new(&metadata_name),
                registry::MAX_REGISTRY_METADATA_BYTES,
            )?;
            if registry::metadata_sha256(&metadata_bytes) != package.registry_metadata_sha256
                || u64::try_from(metadata_bytes.len()).ok() != Some(package.registry_metadata_bytes)
            {
                return Err(ReleaseError::PublicationState(format!(
                    "reviewed registry metadata for {} changed before upload",
                    package.name
                )));
            }
            registry::validate_reviewed_metadata(&metadata_bytes, &package.name, &package.version)?;
            let archive_name = package::archive_name(&package.name, &package.version);
            let reviewed_archive = reviewed_packages.open_regular(OsStr::new(&archive_name))?;
            let upload = sealed_reviewed_upload(
                &reviewed_archive,
                package,
                &report.commit,
                reviewed_packages.path(),
            )?;
            cancellation.check("reviewed package publication")?;
            let token = registry::CratesIoToken::from_environment()?;
            registry.publish_reviewed(&metadata_bytes, &upload, &token)?;
            drop(token);
            cancellation.check("reviewed package publication")?;
            rebuilt_directory.revalidate()?;
            require_rebuilt_match(&rebuilt_directory, package, &report.commit)?;
            reviewed_packages.revalidate()?;
            reviewed_metadata.revalidate()?;
            repository::require_unchanged(root, &report.commit)?;
            repository::require_toolchain(root, &report.toolchain)?;
            registry::wait_for_matching_checksum(
                &registry,
                &cancellation,
                &crate_name,
                &crate_version,
                &package.sha256,
            )?;
            work.revalidate()
        })();
        let cleanup = work.remove_tree();
        published?;
        cleanup?;
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

fn create_publication_work(root: &Path) -> Result<safe_fs::RetainedDirectory, ReleaseError> {
    let work_name = format!(
        ".publish-{}-{}",
        std::process::id(),
        PUBLICATION_COUNTER.fetch_add(1, Ordering::Relaxed)
    );
    safe_fs::RetainedDirectory::create_new_under(
        root,
        Path::new("target/releases").join(&work_name).as_path(),
        Some(Path::new("target/releases")),
    )
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

/// Validates the reviewed archive and returns a sealed in-memory copy of the
/// validated bytes, so the upload transmits exactly what was checked even if
/// the reviewed file changes afterwards.
fn sealed_reviewed_upload(
    archive_file: &std::fs::File,
    package: &package::PackageArchive,
    commit: &str,
    directory: &Path,
) -> Result<std::fs::File, ReleaseError> {
    let archive_name = package::archive_name(&package.name, &package.version);
    let archive_path = directory.join(&archive_name);
    let validated = archive::read_file_and_validate(
        archive_file
            .try_clone()
            .map_err(|source| ReleaseError::io(&archive_path, source))?,
        &archive_path,
        &package.name,
        &package.version,
        commit,
    )?;
    if validated.sha256 != package.sha256
        || u64::try_from(validated.bytes.len()).ok() != Some(package.bytes)
    {
        return Err(ReleaseError::PublicationState(format!(
            "reviewed archive for {} changed before upload",
            package.name
        )));
    }
    safe_fs::sealed_copy(&archive_name, &validated.bytes)
}

fn individual_package_arguments(package: &str) -> Vec<OsString> {
    ["package", "--locked", "--no-verify", "--package", package]
        .map(OsString::from)
        .to_vec()
}

#[cfg(test)]
mod tests {
    use std::io::{Read as _, Seek as _, SeekFrom, Write as _};

    use super::*;

    /// Regression: validation hashed the archive through one descriptor and
    /// the upload reread the same file, so a rewrite in between was uploaded.
    #[test]
    fn upload_reader_yields_the_validated_bytes_after_the_file_is_rewritten() {
        let commit = "1".repeat(40);
        let reviewed = archive::fixture_crate(
            "stab-core",
            RELEASE_VERSION,
            &commit,
            "[package]\nname = \"stab-core\"\n",
        );
        let directory = tempfile::tempdir().expect("reviewed packages");
        let path = directory
            .path()
            .join(package::archive_name("stab-core", RELEASE_VERSION));
        std::fs::write(&path, &reviewed).expect("reviewed archive");
        let record = package::PackageArchive {
            name: "stab-core".to_string(),
            version: RELEASE_VERSION.to_string(),
            archive: String::new(),
            bytes: u64::try_from(reviewed.len()).expect("archive length"),
            sha256: archive::sha256_bytes(&reviewed),
            vcs_commit: commit.clone(),
            shared_readme: String::new(),
            registry_metadata: String::new(),
            registry_metadata_bytes: 0,
            registry_metadata_sha256: String::new(),
        };
        let file = std::fs::File::open(&path).expect("open reviewed archive");
        let mut upload =
            sealed_reviewed_upload(&file, &record, &commit, directory.path()).expect("upload");

        let mut rewritten = std::fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .expect("rewrite reviewed archive in place");
        rewritten
            .write_all(&vec![0x5a; reviewed.len()])
            .expect("same-length rewrite");
        drop(rewritten);

        let mut uploaded = Vec::new();
        upload.read_to_end(&mut uploaded).expect("read upload");
        assert_eq!(uploaded, reviewed);
        upload.seek(SeekFrom::Start(0)).expect("rewind upload");
        assert!(upload.write_all(b"x").is_err(), "the upload copy is sealed");

        // A rewrite before validation is still caught instead of uploaded.
        std::assert_matches!(
            sealed_reviewed_upload(&file, &record, &commit, directory.path()),
            Err(ReleaseError::ArchiveContract { .. } | ReleaseError::PublicationState(_))
        );
    }

    #[test]
    fn publication_rebuild_is_explicit() {
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
    }
}
