use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::time::Duration;

use cargo_metadata::{Metadata, Package};

use crate::{PRODUCT_PACKAGE_ORDER, RELEASE_VERSION, ReleaseError, cargo, repository};

const README_FILE: &str = "README.crates.md";
const RELEASE_KEYWORDS: &[&str] = &[
    "quantum-computing",
    "error-correction",
    "stabilizer",
    "simulation",
];
const ARCHITECTURE_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const METADATA_TIMEOUT: Duration = Duration::from_secs(2 * 60);
const MAX_COMMAND_OUTPUT_BYTES: usize = 16 << 20;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReleasePackage {
    pub(crate) name: String,
    pub(crate) version: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReleaseWorkspace {
    pub(crate) packages: Vec<ReleasePackage>,
}

pub(crate) fn validate_architecture(
    root: &Path,
    cargo: &cargo::CargoSandbox,
) -> Result<(), ReleaseError> {
    cargo.run(
        root,
        [
            OsString::from("run"),
            OsString::from("--quiet"),
            OsString::from("--locked"),
            OsString::from("--package"),
            OsString::from("stab-architecture"),
            OsString::from("--"),
            OsString::from("check"),
        ],
        ARCHITECTURE_TIMEOUT,
        MAX_COMMAND_OUTPUT_BYTES,
    )?;
    Ok(())
}

pub(crate) fn inspect(root: &Path) -> Result<ReleaseWorkspace, ReleaseError> {
    let metadata_json = repository::run_capture(
        root,
        &repository::cargo_program(),
        [
            OsStr::new("metadata"),
            OsStr::new("--format-version"),
            OsStr::new("1"),
            OsStr::new("--locked"),
        ],
        METADATA_TIMEOUT,
        MAX_COMMAND_OUTPUT_BYTES,
    )?;
    inspect_metadata(root, &metadata_json)
}

pub(crate) fn inspect_isolated(
    root: &Path,
    cargo: &cargo::CargoSandbox,
) -> Result<ReleaseWorkspace, ReleaseError> {
    let metadata_json = cargo.run_capture(
        root,
        [
            OsStr::new("metadata"),
            OsStr::new("--format-version"),
            OsStr::new("1"),
            OsStr::new("--locked"),
        ],
        METADATA_TIMEOUT,
        MAX_COMMAND_OUTPUT_BYTES,
    )?;
    inspect_metadata(root, &metadata_json)
}

fn inspect_metadata(root: &Path, metadata_json: &str) -> Result<ReleaseWorkspace, ReleaseError> {
    let metadata: Metadata = serde_json::from_str(metadata_json)?;
    let workspace_ids = metadata.workspace_members.iter().collect::<BTreeSet<_>>();
    let workspace_packages = metadata
        .packages
        .iter()
        .filter(|package| workspace_ids.contains(&package.id))
        .collect::<Vec<_>>();
    let product_packages = workspace_packages
        .iter()
        .copied()
        .filter(|package| package_is_under(root, package, "crates"))
        .collect::<Vec<_>>();
    let actual_names = product_packages
        .iter()
        .map(|package| package.name.as_str())
        .collect::<BTreeSet<_>>();
    let expected_names = PRODUCT_PACKAGE_ORDER
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if actual_names != expected_names {
        return Err(ReleaseError::PackageContract(format!(
            "product package set differs: expected {expected_names:?}, found {actual_names:?}"
        )));
    }

    let by_name = product_packages
        .iter()
        .map(|package| (package.name.as_str(), *package))
        .collect::<BTreeMap<_, _>>();
    validate_order(&by_name)?;
    let packages = PRODUCT_PACKAGE_ORDER
        .iter()
        .map(|name| {
            let package = by_name.get(name).ok_or_else(|| {
                ReleaseError::PackageContract(format!("missing product package {name}"))
            })?;
            validate_metadata(root, package)?;
            Ok(ReleasePackage {
                name: name.to_string(),
                version: package.version.to_string(),
            })
        })
        .collect::<Result<Vec<_>, ReleaseError>>()?;
    Ok(ReleaseWorkspace { packages })
}

fn package_is_under(root: &Path, package: &Package, prefix: &str) -> bool {
    package
        .manifest_path
        .as_std_path()
        .strip_prefix(root)
        .ok()
        .is_some_and(|relative| relative.starts_with(prefix))
}

fn validate_order(packages: &BTreeMap<&str, &Package>) -> Result<(), ReleaseError> {
    let positions = PRODUCT_PACKAGE_ORDER
        .iter()
        .enumerate()
        .map(|(index, name)| (*name, index))
        .collect::<BTreeMap<_, _>>();
    for (name, package) in packages {
        let position = positions.get(name).copied().ok_or_else(|| {
            ReleaseError::PackageContract(format!("missing release position for {name}"))
        })?;
        for dependency in &package.dependencies {
            let Some(dependency_position) = positions.get(dependency.name.as_str()).copied() else {
                continue;
            };
            if dependency.path.is_none() || dependency_position >= position {
                return Err(ReleaseError::PackageContract(format!(
                    "{name} must follow internal dependency {} in publication order",
                    dependency.name
                )));
            }
            if dependency.req.to_string() != format!("={RELEASE_VERSION}") {
                return Err(ReleaseError::PackageContract(format!(
                    "{name} requires internal package {} with {}, expected ={RELEASE_VERSION}",
                    dependency.name, dependency.req
                )));
            }
        }
    }
    Ok(())
}

fn validate_metadata(root: &Path, package: &Package) -> Result<(), ReleaseError> {
    if package.version.to_string() != RELEASE_VERSION {
        return Err(ReleaseError::PackageContract(format!(
            "{} has version {}, expected {RELEASE_VERSION}",
            package.name, package.version
        )));
    }
    if package.publish.is_some() {
        return Err(ReleaseError::PackageContract(format!(
            "{} must publish to the default crates.io registry",
            package.name
        )));
    }
    if package.description.as_deref().is_none_or(str::is_empty)
        || package.license.as_deref() != Some("MIT")
        || package.repository.as_deref() != Some("https://github.com/ifsheldon/Stab")
        || package.homepage.as_deref() != Some("https://github.com/ifsheldon/Stab")
    {
        return Err(ReleaseError::PackageContract(format!(
            "{} has incomplete crates.io description, license, repository, or homepage metadata",
            package.name
        )));
    }
    let readme = package.readme.as_ref().ok_or_else(|| {
        ReleaseError::PackageContract(format!("{} has no package README", package.name))
    })?;
    let manifest_parent = package.manifest_path.parent().ok_or_else(|| {
        ReleaseError::PackageContract(format!("{} has no manifest parent", package.name))
    })?;
    let readme_path = manifest_parent.join(readme);
    let resolved_readme = std::fs::canonicalize(readme_path.as_std_path())
        .map_err(|source| ReleaseError::io(readme_path.as_std_path(), source))?;
    let expected_readme = std::fs::canonicalize(root.join(README_FILE))
        .map_err(|source| ReleaseError::io(root.join(README_FILE), source))?;
    if resolved_readme != expected_readme {
        return Err(ReleaseError::PackageContract(format!(
            "{} uses README {}, expected {}",
            package.name,
            resolved_readme.display(),
            expected_readme.display()
        )));
    }
    if package.keywords != RELEASE_KEYWORDS {
        return Err(ReleaseError::PackageContract(format!(
            "{} has unexpected crates.io keywords {:?}",
            package.name, package.keywords
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_workspace_matches_release_order_and_metadata() {
        let root = std::fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
            .expect("repository root");
        let workspace = inspect(&root).expect("release workspace");
        assert_eq!(
            workspace
                .packages
                .iter()
                .map(|package| package.name.as_str())
                .collect::<Vec<_>>(),
            PRODUCT_PACKAGE_ORDER
        );
        assert!(
            workspace
                .packages
                .iter()
                .all(|package| package.version == RELEASE_VERSION)
        );
    }
}
