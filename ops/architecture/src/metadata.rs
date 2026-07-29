use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use cargo_metadata::{DependencyKind as CargoDependencyKind, MetadataCommand, PackageId};

use crate::{CheckError, DependencyKind, PackageSpec, WorkspaceEdge, WorkspaceGraph};

pub(super) fn load_workspace_graph(root: &Path) -> Result<WorkspaceGraph, CheckError> {
    let metadata = MetadataCommand::new()
        .current_dir(root)
        .manifest_path(root.join("Cargo.toml"))
        .other_options(metadata_options())
        .exec()?;
    let metadata_root =
        std::fs::canonicalize(metadata.workspace_root.as_std_path()).map_err(|source| {
            CheckError::ResolveRoot {
                path: metadata.workspace_root.as_std_path().to_path_buf(),
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
            default_features: package
                .features
                .get("default")
                .into_iter()
                .flatten()
                .map(ToString::to_string)
                .collect(),
        });
    }
    packages.sort();

    let resolve = metadata
        .resolve
        .as_ref()
        .ok_or(CheckError::MissingResolve)?;
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
                    optional: false,
                });
                continue;
            }
            for dependency_kind in &dependency.dep_kinds {
                let cargo_kind = dependency_kind.kind;
                edges.insert(WorkspaceEdge {
                    from: from.clone(),
                    to: to.clone(),
                    kind: DependencyKind::from_cargo(cargo_kind),
                    optional: dependency_is_optional(&metadata, &node.id, &to, cargo_kind)?,
                });
            }
        }
    }

    Ok(WorkspaceGraph {
        packages,
        edges: edges.into_iter().collect(),
    })
}

fn dependency_is_optional(
    metadata: &cargo_metadata::Metadata,
    source_id: &PackageId,
    target_name: &str,
    kind: CargoDependencyKind,
) -> Result<bool, CheckError> {
    let package = metadata
        .packages
        .iter()
        .find(|package| package.id == *source_id)
        .ok_or_else(|| CheckError::UnknownPackageId(source_id.repr.clone()))?;
    let declarations = package
        .dependencies
        .iter()
        .filter(|dependency| dependency.name == target_name && dependency.kind == kind)
        .collect::<Vec<_>>();
    Ok(!declarations.is_empty() && declarations.iter().all(|dependency| dependency.optional))
}

fn metadata_options() -> Vec<String> {
    vec!["--locked".to_owned(), "--all-features".to_owned()]
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
            default_features: Vec::new(),
        };
        assert_eq!(package.relative_path, Path::new("crates/stab-core"));
    }

    #[test]
    fn metadata_exposes_forbidden_optional_product_edges() {
        let root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/optional-edge-workspace");
        let graph = load_workspace_graph(&root).expect("load optional-edge fixture");

        assert!(graph.edges.iter().any(|edge| {
            edge.from == "stab-model"
                && edge.to == "stab-engine"
                && edge.kind == DependencyKind::Normal
        }));
        let report = crate::policy::validate_graph(&graph);
        assert!(report.violations.iter().any(|violation| {
            violation.code == "forbidden-product-edge"
                && violation.message.contains("stab-model -> stab-engine")
        }));
    }
}
