use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::{
    PRODUCT_PACKAGE_ORDER, RELEASE_VERSION, ReleaseError, archive, cargo, registry, repository,
    safe_fs, workspace,
};

const MAX_PACKAGE_LIST_BYTES: usize = 4 << 20;
const MAX_CARGO_OUTPUT_BYTES: usize = 8 << 20;
const CARGO_PACKAGE_TIMEOUT: Duration = Duration::from_secs(30 * 60);
pub(crate) const RELEASE_PREFLIGHT_SCHEMA_VERSION: u32 = 4;
const RELEASE_VERIFICATION: &str =
    "fresh-coordinated-package-plus-cargo-publish-dry-run-plus-reviewed-registry-metadata";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReleasePreflightReport {
    pub(crate) schema_version: u32,
    pub(crate) version: String,
    pub(crate) commit: String,
    pub(crate) registry: String,
    pub(crate) verification: String,
    pub(crate) toolchain: repository::ToolchainIdentity,
    pub(crate) publication_order: Vec<String>,
    pub(crate) packages: Vec<PackageArchive>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PackageArchive {
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) archive: String,
    pub(crate) bytes: u64,
    pub(crate) sha256: String,
    pub(crate) vcs_commit: String,
    pub(crate) shared_readme: String,
    pub(crate) registry_metadata: String,
    pub(crate) registry_metadata_bytes: u64,
    pub(crate) registry_metadata_sha256: String,
}

pub(crate) fn check(root: &Path, output: &Path) -> Result<PathBuf, ReleaseError> {
    let commit = repository::require_clean(root)?;
    let toolchain = repository::capture_toolchain(root)?;
    let output = safe_fs::RetainedDirectory::create_new_under(
        root,
        output,
        Some(Path::new("target/releases")),
    )?;
    let work = output.create_directory(OsStr::new("work"))?;
    let cargo_target = work.create_directory(OsStr::new("cargo-target"))?;
    let cargo = cargo::CargoSandbox::create(root, &work, &cargo_target)?;
    workspace::validate_architecture(root, &cargo)?;
    let workspace = workspace::inspect_isolated(root, &cargo)?;
    validate_package_lists(root, &workspace, &cargo)?;

    cargo.run(
        root,
        coordinated_package_arguments(&workspace),
        CARGO_PACKAGE_TIMEOUT,
        MAX_CARGO_OUTPUT_BYTES,
    )?;
    repository::require_unchanged(root, &commit)?;

    cargo.run(
        root,
        coordinated_publish_arguments(&workspace),
        CARGO_PACKAGE_TIMEOUT,
        MAX_CARGO_OUTPUT_BYTES,
    )?;
    repository::require_unchanged(root, &commit)?;
    repository::require_toolchain(root, &toolchain)?;
    cargo_target.revalidate()?;
    let cargo_packages = cargo_target.open_directory(OsStr::new("package"))?;

    let packages_directory = output.create_directory(OsStr::new("packages"))?;
    let metadata_directory = output.create_directory(OsStr::new("registry-metadata"))?;
    let mut packages = Vec::with_capacity(workspace.packages.len());
    for package in &workspace.packages {
        let archive_name = archive_name(&package.name, &package.version);
        let source = cargo_packages.path().join(&archive_name);
        let source_file = cargo_packages.open_regular(OsStr::new(&archive_name))?;
        let reviewed = archive::read_file_and_validate(
            source_file,
            &source,
            &package.name,
            &package.version,
            &commit,
        )?;
        archive::write_immutable_copy(&packages_directory, OsStr::new(&archive_name), &reviewed)?;
        let metadata_name = registry_metadata_name(&package.name, &package.version);
        write_immutable_bytes(
            &metadata_directory,
            OsStr::new(&metadata_name),
            &package.registry_metadata,
        )?;
        let copied_path = packages_directory.path().join(&archive_name);
        let copied_file = packages_directory.open_regular(OsStr::new(&archive_name))?;
        let copied = archive::read_file_and_validate(
            copied_file,
            &copied_path,
            &package.name,
            &package.version,
            &commit,
        )?;
        if copied.sha256 != reviewed.sha256 || copied.bytes != reviewed.bytes {
            return Err(ReleaseError::ArchiveContract {
                path: copied_path,
                detail: "immutable reviewed copy differs from fresh Cargo archive".to_string(),
            });
        }
        packages.push(PackageArchive {
            name: package.name.clone(),
            version: package.version.clone(),
            archive: format!("packages/{archive_name}"),
            bytes: u64::try_from(reviewed.bytes.len()).map_err(|_| {
                ReleaseError::PackageContract(format!(
                    "{} archive size does not fit in u64",
                    package.name
                ))
            })?,
            sha256: reviewed.sha256,
            vcs_commit: reviewed.vcs_commit,
            shared_readme: "README.crates.md".to_string(),
            registry_metadata: format!("registry-metadata/{metadata_name}"),
            registry_metadata_bytes: u64::try_from(package.registry_metadata.len()).map_err(
                |_| {
                    ReleaseError::PackageContract(format!(
                        "{} registry metadata size does not fit in u64",
                        package.name
                    ))
                },
            )?,
            registry_metadata_sha256: registry::metadata_sha256(&package.registry_metadata),
        });
    }
    packages_directory.make_read_only()?;
    metadata_directory.make_read_only()?;

    work.revalidate()?;
    output.revalidate()?;
    work.remove_tree()?;
    output.revalidate()?;
    repository::require_unchanged(root, &commit)?;

    let report = ReleasePreflightReport {
        schema_version: RELEASE_PREFLIGHT_SCHEMA_VERSION,
        version: RELEASE_VERSION.to_string(),
        commit,
        registry: "crates-io".to_string(),
        verification: RELEASE_VERIFICATION.to_string(),
        toolchain,
        publication_order: PRODUCT_PACKAGE_ORDER
            .iter()
            .map(|name| name.to_string())
            .collect(),
        packages,
    };
    let mut report_bytes = serde_json::to_vec_pretty(&report)?;
    report_bytes.push(b'\n');
    let report_file = output.write_new(OsStr::new("report.json"), &report_bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        report_file
            .set_permissions(std::fs::Permissions::from_mode(0o444))
            .map_err(|source| ReleaseError::io(output.path().join("report.json"), source))?;
        report_file
            .sync_all()
            .map_err(|source| ReleaseError::io(output.path().join("report.json"), source))?;
    }
    output.sync()?;
    println!(
        "[stab-release] reviewed {} fresh registry-form crates for Stab {} at commit {}",
        report.packages.len(),
        report.version,
        report.commit
    );
    println!(
        "[stab-release] wrote {}",
        output.path().join("report.json").display()
    );
    Ok(output.path().to_path_buf())
}

pub(crate) fn load_reviewed_report(
    root: &Path,
    preflight: &Path,
) -> Result<(safe_fs::RetainedDirectory, ReleasePreflightReport), ReleaseError> {
    let directory = safe_fs::RetainedDirectory::open_under(
        root,
        preflight,
        Some(Path::new("target/releases")),
    )?;
    let report_bytes = directory.read_bounded(OsStr::new("report.json"), 1 << 20)?;
    let report: ReleasePreflightReport = serde_json::from_slice(&report_bytes)?;
    validate_report(root, &directory, &report)?;
    Ok((directory, report))
}

fn validate_report(
    root: &Path,
    directory: &safe_fs::RetainedDirectory,
    report: &ReleasePreflightReport,
) -> Result<(), ReleaseError> {
    if report.schema_version != RELEASE_PREFLIGHT_SCHEMA_VERSION
        || report.version != RELEASE_VERSION
        || report.registry != "crates-io"
        || report.verification != RELEASE_VERIFICATION
        || report.publication_order != PRODUCT_PACKAGE_ORDER
        || report.packages.len() != PRODUCT_PACKAGE_ORDER.len()
    {
        return Err(ReleaseError::PackageContract(
            "reviewed preflight report identity or package order is invalid".to_string(),
        ));
    }
    let current = repository::require_clean(root)?;
    if current != report.commit {
        return Err(ReleaseError::RepositoryChanged {
            before: report.commit.clone(),
            after: current,
        });
    }
    repository::require_toolchain(root, &report.toolchain)?;
    let packages = directory.open_directory(OsStr::new("packages"))?;
    let metadata = directory.open_directory(OsStr::new("registry-metadata"))?;
    for (expected_name, package) in PRODUCT_PACKAGE_ORDER.iter().zip(&report.packages) {
        let expected_archive = archive_name(expected_name, RELEASE_VERSION);
        let expected_metadata = registry_metadata_name(expected_name, RELEASE_VERSION);
        if package.name != *expected_name
            || package.version != RELEASE_VERSION
            || package.archive != format!("packages/{expected_archive}")
            || package.vcs_commit != report.commit
            || package.shared_readme != "README.crates.md"
            || package.registry_metadata != format!("registry-metadata/{expected_metadata}")
        {
            return Err(ReleaseError::PackageContract(format!(
                "reviewed package record for {expected_name} is invalid"
            )));
        }
        let archive_path = packages.path().join(&expected_archive);
        let archive_file = packages.open_regular(OsStr::new(&expected_archive))?;
        let reviewed = archive::read_file_and_validate(
            archive_file,
            &archive_path,
            &package.name,
            &package.version,
            &report.commit,
        )?;
        if reviewed.sha256 != package.sha256
            || u64::try_from(reviewed.bytes.len()).ok() != Some(package.bytes)
        {
            return Err(ReleaseError::ArchiveContract {
                path: archive_path,
                detail: "reviewed archive checksum or length differs from report".to_string(),
            });
        }
        let metadata_path = metadata.path().join(&expected_metadata);
        let metadata_bytes = metadata.read_bounded(
            OsStr::new(&expected_metadata),
            registry::MAX_REGISTRY_METADATA_BYTES,
        )?;
        registry::validate_reviewed_metadata(&metadata_bytes, &package.name, &package.version)?;
        if registry::metadata_sha256(&metadata_bytes) != package.registry_metadata_sha256
            || u64::try_from(metadata_bytes.len()).ok() != Some(package.registry_metadata_bytes)
        {
            return Err(ReleaseError::ArchiveContract {
                path: metadata_path,
                detail: "reviewed registry metadata checksum or length differs from report"
                    .to_string(),
            });
        }
    }
    metadata.revalidate()?;
    directory.revalidate()?;
    Ok(())
}

fn validate_package_lists(
    root: &Path,
    workspace: &workspace::ReleaseWorkspace,
    cargo: &cargo::CargoSandbox,
) -> Result<(), ReleaseError> {
    for package in &workspace.packages {
        let args = vec![
            OsString::from("package"),
            OsString::from("--list"),
            OsString::from("--locked"),
            OsString::from("--package"),
            OsString::from(&package.name),
        ];
        let package_list =
            cargo.run_capture(root, &args, CARGO_PACKAGE_TIMEOUT, MAX_PACKAGE_LIST_BYTES)?;
        if !package_list.lines().any(|line| line == "README.crates.md") {
            return Err(ReleaseError::PackageContract(format!(
                "{} package does not include README.crates.md",
                package.name
            )));
        }
    }
    Ok(())
}

pub(crate) fn coordinated_package_arguments(
    workspace: &workspace::ReleaseWorkspace,
) -> Vec<OsString> {
    coordinated_arguments("package", &["--locked", "--no-verify"], workspace)
}

fn coordinated_publish_arguments(workspace: &workspace::ReleaseWorkspace) -> Vec<OsString> {
    coordinated_arguments(
        "publish",
        &["--dry-run", "--locked", "--registry", "crates-io"],
        workspace,
    )
}

fn coordinated_arguments(
    command: &str,
    options: &[&str],
    workspace: &workspace::ReleaseWorkspace,
) -> Vec<OsString> {
    let mut args = vec![OsString::from(command)];
    args.extend(options.iter().map(OsString::from));
    for package in &workspace.packages {
        args.push(OsString::from("--package"));
        args.push(OsString::from(&package.name));
    }
    args
}

pub(crate) fn archive_name(package: &str, version: &str) -> String {
    format!("{package}-{version}.crate")
}

pub(crate) fn registry_metadata_name(package: &str, version: &str) -> String {
    format!("{package}-{version}.json")
}

fn write_immutable_bytes(
    directory: &safe_fs::RetainedDirectory,
    name: &OsStr,
    bytes: &[u8],
) -> Result<(), ReleaseError> {
    let file = directory.write_new(name, bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;

        file.set_permissions(std::fs::Permissions::from_mode(0o444))
            .map_err(|source| ReleaseError::io(directory.path().join(name), source))?;
        file.sync_all()
            .map_err(|source| ReleaseError::io(directory.path().join(name), source))?;
        Ok(())
    }
    #[cfg(not(unix))]
    {
        let _ = file;
        Err(ReleaseError::UnsupportedPlatform)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace_fixture() -> workspace::ReleaseWorkspace {
        workspace::ReleaseWorkspace {
            packages: PRODUCT_PACKAGE_ORDER
                .iter()
                .map(|name| workspace::ReleasePackage {
                    name: name.to_string(),
                    version: RELEASE_VERSION.to_string(),
                    registry_metadata: Vec::new(),
                })
                .collect(),
        }
    }

    #[test]
    fn release_verification_uses_fresh_package_and_coordinated_dry_run() {
        let workspace = workspace_fixture();
        let package = coordinated_package_arguments(&workspace);
        assert_eq!(
            package.get(..3),
            Some(
                ["package", "--locked", "--no-verify"]
                    .map(OsString::from)
                    .as_slice()
            )
        );
        let publish = coordinated_publish_arguments(&workspace);
        assert_eq!(
            publish.get(..6),
            Some(
                [
                    "publish",
                    "--dry-run",
                    "--locked",
                    "--registry",
                    "crates-io",
                    "--package",
                ]
                .map(OsString::from)
                .as_slice()
            )
        );
        assert_eq!(
            publish
                .get(5..)
                .expect("package selectors")
                .chunks_exact(2)
                .filter_map(|chunk| chunk.get(1))
                .collect::<Vec<_>>(),
            PRODUCT_PACKAGE_ORDER
                .iter()
                .map(OsString::from)
                .collect::<Vec<_>>()
                .iter()
                .collect::<Vec<_>>()
        );
    }
}
