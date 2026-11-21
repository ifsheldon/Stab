//! The dependency list a crates.io upload carries.
//!
//! `cargo package` writes a normalized `Cargo.toml` into the `.crate` archive:
//! it rewrites every path dependency into a registry dependency with its
//! declared `version` and drops every dev-dependency that names no version,
//! because only a path or git source could satisfy it and crates.io never sees
//! either. The reviewed registry metadata must describe exactly the
//! dependencies of that normalized manifest, so preflight derives them with
//! the same rule and then cross-checks them against the packaged manifest.

use std::path::Path;

use cargo_metadata::semver::VersionReq;
use cargo_metadata::{Dependency, DependencyKind, Package};

use super::{ReviewedRegistryDependency, parse_reviewed_metadata};
use crate::ReleaseError;

const KINDS: [(&str, &str); 3] = [
    ("dependencies", "normal"),
    ("dev-dependencies", "dev"),
    ("build-dependencies", "build"),
];
const DEPENDENCY_KEYS: [&str; 6] = [
    "version",
    "package",
    "features",
    "optional",
    "default-features",
    "default_features",
];

/// The dependencies Cargo keeps in the packaged manifest, in reviewed form.
pub(super) fn published_dependencies(
    package: &Package,
) -> Result<Vec<ReviewedRegistryDependency>, ReleaseError> {
    package
        .dependencies
        .iter()
        .filter(|dependency| !stripped_by_cargo(dependency))
        .map(|dependency| super::reviewed_dependency(package.name.as_str(), dependency))
        .collect()
}

/// A dev-dependency without a version requirement exists only for the local
/// workspace; Cargo removes it from the manifest it packages and publishes.
fn stripped_by_cargo(dependency: &Dependency) -> bool {
    dependency.kind == DependencyKind::Development
        && dependency.req == VersionReq::STAR
        && !dependency
            .source
            .as_ref()
            .is_some_and(cargo_metadata::Source::is_crates_io)
}

/// One dependency in the shape both the metadata and the manifest share.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct PublishedDependency {
    kind: String,
    target: Option<String>,
    name: String,
    rename: Option<String>,
    requirement: String,
    features: Vec<String>,
    optional: bool,
    default_features: bool,
}

/// Requires the reviewed registry metadata to list exactly the dependencies
/// of the archive's normalized `Cargo.toml`.
pub(crate) fn require_manifest_dependencies(
    metadata_bytes: &[u8],
    normalized_manifest: &str,
    archive_path: &Path,
) -> Result<(), ReleaseError> {
    let metadata = parse_reviewed_metadata(metadata_bytes, None, None)?;
    let mut reviewed = metadata
        .deps
        .iter()
        .map(reviewed_shape)
        .collect::<Result<Vec<_>, _>>()?;
    let mut packaged = manifest_dependencies(normalized_manifest, archive_path)?;
    reviewed.sort();
    packaged.sort();
    if reviewed == packaged {
        return Ok(());
    }
    let only_reviewed = reviewed
        .iter()
        .filter(|dependency| !packaged.contains(dependency))
        .collect::<Vec<_>>();
    let only_packaged = packaged
        .iter()
        .filter(|dependency| !reviewed.contains(dependency))
        .collect::<Vec<_>>();
    Err(ReleaseError::ArchiveContract {
        path: archive_path.to_path_buf(),
        detail: format!(
            "reviewed registry dependencies differ from the normalized Cargo.toml: only in metadata {only_reviewed:?}, only in the archive {only_packaged:?}"
        ),
    })
}

fn reviewed_shape(
    dependency: &ReviewedRegistryDependency,
) -> Result<PublishedDependency, ReleaseError> {
    let mut features = dependency.features.clone();
    features.sort();
    Ok(PublishedDependency {
        kind: dependency.kind.clone(),
        target: dependency.target.clone(),
        name: dependency.name.clone(),
        rename: dependency.explicit_name_in_toml.clone(),
        requirement: canonical_requirement(&dependency.version_req, &dependency.name)?,
        features,
        optional: dependency.optional,
        default_features: dependency.default_features,
    })
}

fn canonical_requirement(requirement: &str, name: &str) -> Result<String, ReleaseError> {
    let parsed = VersionReq::parse(requirement).map_err(|source| {
        ReleaseError::PackageContract(format!(
            "dependency {name} has an invalid version requirement {requirement:?}: {source}"
        ))
    })?;
    if parsed == VersionReq::STAR {
        return Err(ReleaseError::PackageContract(format!(
            "dependency {name} would be published with a wildcard version requirement"
        )));
    }
    Ok(parsed.to_string())
}

fn manifest_dependencies(
    manifest: &str,
    path: &Path,
) -> Result<Vec<PublishedDependency>, ReleaseError> {
    let manifest: toml::Table = toml::from_str(manifest).map_err(|source| {
        manifest_error(path, format!("normalized Cargo.toml is invalid: {source}"))
    })?;
    let mut dependencies = Vec::new();
    collect_kinds(&manifest, None, path, &mut dependencies)?;
    if let Some(targets) = manifest.get("target") {
        let targets = targets
            .as_table()
            .ok_or_else(|| manifest_error(path, "normalized target section is not a table"))?;
        for (target, section) in targets {
            let section = section.as_table().ok_or_else(|| {
                manifest_error(path, format!("normalized target {target:?} is not a table"))
            })?;
            collect_kinds(section, Some(target), path, &mut dependencies)?;
        }
    }
    Ok(dependencies)
}

fn collect_kinds(
    section: &toml::Table,
    target: Option<&str>,
    path: &Path,
    dependencies: &mut Vec<PublishedDependency>,
) -> Result<(), ReleaseError> {
    for key in ["dev_dependencies", "build_dependencies"] {
        if section.contains_key(key) {
            return Err(manifest_error(
                path,
                format!("normalized Cargo.toml uses the legacy {key} section"),
            ));
        }
    }
    for (key, kind) in KINDS {
        let Some(table) = section.get(key) else {
            continue;
        };
        let table = table
            .as_table()
            .ok_or_else(|| manifest_error(path, format!("normalized {key} is not a table")))?;
        for (toml_name, value) in table {
            dependencies.push(manifest_dependency(kind, target, toml_name, value, path)?);
        }
    }
    Ok(())
}

fn manifest_dependency(
    kind: &str,
    target: Option<&str>,
    toml_name: &str,
    value: &toml::Value,
    path: &Path,
) -> Result<PublishedDependency, ReleaseError> {
    let table = match value {
        toml::Value::String(version) => {
            let mut table = toml::Table::new();
            table.insert("version".to_string(), toml::Value::String(version.clone()));
            table
        }
        toml::Value::Table(table) => table.clone(),
        _ => {
            return Err(manifest_error(
                path,
                format!("normalized dependency {toml_name} is neither a version nor a table"),
            ));
        }
    };
    if let Some(key) = table
        .keys()
        .find(|key| !DEPENDENCY_KEYS.contains(&key.as_str()))
    {
        return Err(manifest_error(
            path,
            format!("normalized dependency {toml_name} has unsupported key {key:?}"),
        ));
    }
    let version = table
        .get("version")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| {
            manifest_error(
                path,
                format!("normalized dependency {toml_name} has no registry version"),
            )
        })?;
    let package = table
        .get("package")
        .map(|package| {
            package.as_str().map(str::to_string).ok_or_else(|| {
                manifest_error(path, format!("dependency {toml_name} package is not text"))
            })
        })
        .transpose()?;
    let mut features = match table.get("features") {
        None => Vec::new(),
        Some(features) => features
            .as_array()
            .and_then(|features| {
                features
                    .iter()
                    .map(|feature| feature.as_str().map(str::to_string))
                    .collect::<Option<Vec<_>>>()
            })
            .ok_or_else(|| {
                manifest_error(
                    path,
                    format!("dependency {toml_name} features are not text"),
                )
            })?,
    };
    features.sort();
    let flag = |key: &str, default: bool| -> Result<bool, ReleaseError> {
        table.get(key).map_or(Ok(default), |value| {
            value.as_bool().ok_or_else(|| {
                manifest_error(
                    path,
                    format!("dependency {toml_name} {key} is not a boolean"),
                )
            })
        })
    };
    let default_features = flag("default-features", true)? && flag("default_features", true)?;
    let (name, rename) = match package {
        Some(package) => (package, Some(toml_name.to_string())),
        None => (toml_name.to_string(), None),
    };
    Ok(PublishedDependency {
        kind: kind.to_string(),
        target: target.map(str::to_string),
        requirement: canonical_requirement(version, &name)?,
        name,
        rename,
        features,
        optional: flag("optional", false)?,
        default_features,
    })
}

fn manifest_error(path: &Path, detail: impl Into<String>) -> ReleaseError {
    ReleaseError::ArchiveContract {
        path: path.to_path_buf(),
        detail: detail.into(),
    }
}
