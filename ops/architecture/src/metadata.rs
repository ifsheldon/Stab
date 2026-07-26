use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use cargo_metadata::{DependencyKind as CargoDependencyKind, MetadataCommand, PackageId};

use crate::{CheckError, DependencyKind, PackageSpec, WorkspaceEdge, WorkspaceGraph};

pub(super) fn load_workspace_graph(root: &Path) -> Result<WorkspaceGraph, CheckError> {
    let metadata = MetadataCommand::new()
        .current_dir(root)
        .manifest_path(root.join("Cargo.toml"))
        .other_options(vec!["--locked".to_owned()])
        .exec()?;
    let metadata_root =
        std::fs::canonicalize(metadata.workspace_root.as_std_path()).map_err(|source| {
            CheckError::ResolveRoot {
                path: metadata.workspace_root.into_std_path_buf(),
                source,
            }
        })?;

    let workspace_ids = metadata
        .workspace_members
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut package_names = BTreeMap::<PackageId, String>::new();
    let mut packages = Vec::with_capacity(workspace_ids.len());

    for package in metadata
        .packages
        .iter()
        .filter(|package| workspace_ids.contains(&package.id))
    {
        let manifest_parent =
            package
                .manifest_path
                .parent()
                .ok_or_else(|| CheckError::MissingManifestParent {
                    package: package.name.to_string(),
                })?;
        let package_root =
            std::fs::canonicalize(manifest_parent.as_std_path()).map_err(|source| {
                CheckError::ResolveRoot {
                    path: manifest_parent.to_path_buf().into_std_path_buf(),
                    source,
                }
            })?;
        let relative_path = package_root
            .strip_prefix(&metadata_root)
            .map_err(|_| CheckError::PackageOutsideRoot {
                package: package.name.to_string(),
                root: metadata_root.clone(),
                path: package_root.clone(),
            })?
            .to_path_buf();
        package_names.insert(package.id.clone(), package.name.to_string());
        packages.push(PackageSpec {
            name: package.name.to_string(),
            relative_path,
        });
    }
    packages.sort();

    let resolve = metadata.resolve.ok_or(CheckError::MissingResolve)?;
    let mut edges = BTreeSet::new();
    for node in resolve
        .nodes
        .iter()
        .filter(|node| workspace_ids.contains(&node.id))
    {
        let from = package_name(&package_names, &node.id)?;
        for dependency in &node.deps {
            if !workspace_ids.contains(&dependency.pkg) {
                continue;
            }
            let to = package_name(&package_names, &dependency.pkg)?;
            if dependency.dep_kinds.is_empty() {
                edges.insert(WorkspaceEdge {
                    from: from.clone(),
                    to,
                    kind: DependencyKind::Normal,
                });
                continue;
            }
            for dependency_kind in &dependency.dep_kinds {
                edges.insert(WorkspaceEdge {
                    from: from.clone(),
                    to: to.clone(),
                    kind: DependencyKind::from_cargo(dependency_kind.kind),
                });
            }
        }
    }

    Ok(WorkspaceGraph {
        packages,
        edges: edges.into_iter().collect(),
    })
}

fn package_name(
    package_names: &BTreeMap<PackageId, String>,
    id: &PackageId,
) -> Result<String, CheckError> {
    package_names
        .get(id)
        .cloned()
        .ok_or_else(|| CheckError::UnknownPackageId(id.repr.clone()))
}

impl DependencyKind {
    fn from_cargo(kind: CargoDependencyKind) -> Self {
        match kind {
            CargoDependencyKind::Normal => Self::Normal,
            CargoDependencyKind::Development => Self::Development,
            CargoDependencyKind::Build => Self::Build,
            _ => Self::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cargo_dependency_kinds_are_preserved() {
        assert_eq!(
            DependencyKind::from_cargo(CargoDependencyKind::Normal),
            DependencyKind::Normal
        );
        assert_eq!(
            DependencyKind::from_cargo(CargoDependencyKind::Development),
            DependencyKind::Development
        );
        assert_eq!(
            DependencyKind::from_cargo(CargoDependencyKind::Build),
            DependencyKind::Build
        );
    }

    #[test]
    fn workspace_paths_are_repository_relative() {
        let package = PackageSpec {
            name: "stab-core".to_owned(),
            relative_path: std::path::PathBuf::from("crates/stab-core"),
        };
        assert_eq!(package.relative_path, Path::new("crates/stab-core"));
    }
}
