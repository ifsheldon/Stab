use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::{
    PRODUCT_PACKAGE_ORDER, RELEASE_VERSION, ReleaseError, artifact, repository, workspace,
};

const MAX_PACKAGE_LIST_BYTES: usize = 4 << 20;

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct ReleasePreflightReport {
    schema_version: u32,
    version: String,
    commit: String,
    publication_order: Vec<String>,
    packages: Vec<PackageArchive>,
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct PackageArchive {
    name: String,
    version: String,
    archive: String,
    bytes: u64,
    sha256: String,
    shared_readme: String,
}

pub(crate) fn check(root: &Path, output: &Path) -> Result<PathBuf, ReleaseError> {
    artifact::validate_report_output(root, output)?;
    let commit = repository::require_clean(root)?;
    let workspace = workspace::inspect(root)?;
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    for package in &workspace.packages {
        let package_list = repository::run_capture(
            root,
            &cargo,
            &["package", "--list", "--locked", "--package", &package.name],
            MAX_PACKAGE_LIST_BYTES,
        )?;
        if !package_list.lines().any(|line| line == "README.crates.md") {
            return Err(ReleaseError::PackageContract(format!(
                "{} package does not include README.crates.md",
                package.name
            )));
        }
    }
    let package_args = coordinated_package_arguments(&workspace);
    repository::run_inherit(root, &cargo, &package_args)?;

    let mut archives = Vec::with_capacity(workspace.packages.len());
    for package in &workspace.packages {
        let metadata = fs::symlink_metadata(&package.archive)
            .map_err(|source| ReleaseError::io(&package.archive, source))?;
        if !metadata.file_type().is_file() {
            return Err(ReleaseError::NotRegularFile(package.archive.clone()));
        }
        let relative_archive = package.archive.strip_prefix(root).map_err(|_| {
            ReleaseError::PackageContract(format!(
                "package archive is outside repository: {}",
                package.archive.display()
            ))
        })?;
        archives.push(PackageArchive {
            name: package.name.clone(),
            version: package.version.clone(),
            archive: path_text(relative_archive)?,
            bytes: metadata.len(),
            sha256: artifact::sha256_file(&package.archive)?,
            shared_readme: "README.crates.md".to_string(),
        });
    }
    repository::require_unchanged(root, &commit)?;

    let report = ReleasePreflightReport {
        schema_version: 1,
        version: RELEASE_VERSION.to_string(),
        commit,
        publication_order: PRODUCT_PACKAGE_ORDER
            .iter()
            .map(|name| name.to_string())
            .collect(),
        packages: archives,
    };
    let mut report_bytes = serde_json::to_vec_pretty(&report)?;
    report_bytes.push(b'\n');
    let output = artifact::create_report_directory(root, output)?;
    let report_path = output.join("report.json");
    artifact::write_new_file(&report_path, &report_bytes)?;
    artifact::sync_directory(&output)?;
    println!(
        "[stab-release] packaged {} crates for Stab {} at commit {}",
        report.packages.len(),
        report.version,
        report.commit
    );
    println!("[stab-release] wrote {}", report_path.display());
    Ok(output)
}

fn coordinated_package_arguments(workspace: &workspace::ReleaseWorkspace) -> Vec<&str> {
    let mut args = vec!["package", "--locked", "--no-verify"];
    for package in &workspace.packages {
        args.push("--package");
        args.push(package.name.as_str());
    }
    args
}

fn path_text(path: &Path) -> Result<String, ReleaseError> {
    path.to_str()
        .map(ToString::to_string)
        .ok_or_else(|| ReleaseError::InvalidPath(path.to_path_buf()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preflight_report_paths_stay_below_release_target() {
        let root = tempfile::tempdir().expect("root");
        assert!(
            artifact::validate_report_output(root.path(), Path::new("target/releases/a9")).is_ok()
        );
        for path in ["dist", "target/release", "target/releases"] {
            assert!(matches!(
                artifact::validate_report_output(root.path(), Path::new(path)),
                Err(ReleaseError::InvalidPath(_))
            ));
        }
    }

    #[test]
    fn package_assembly_uses_one_coordinated_cargo_invocation() {
        let workspace = workspace::ReleaseWorkspace {
            packages: PRODUCT_PACKAGE_ORDER
                .iter()
                .map(|name| workspace::ReleasePackage {
                    name: name.to_string(),
                    version: RELEASE_VERSION.to_string(),
                    archive: PathBuf::from(format!("target/package/{name}.crate")),
                })
                .collect(),
        };
        let arguments = coordinated_package_arguments(&workspace);
        assert_eq!(
            arguments.get(..3).expect("Cargo package prefix"),
            ["package", "--locked", "--no-verify"]
        );
        assert_eq!(
            arguments
                .get(3..)
                .expect("package selectors")
                .chunks_exact(2)
                .map(|chunk| chunk.get(1).copied().expect("package name"))
                .collect::<Vec<_>>(),
            PRODUCT_PACKAGE_ORDER
        );
    }
}
