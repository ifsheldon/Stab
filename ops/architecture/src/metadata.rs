use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use cargo_metadata::{
    DependencyKind as CargoDependencyKind, MetadataCommand, PackageId, TargetKind,
};

use crate::{
    CheckError, DeclaredPathDependency, DependencyKind, PackageSpec, WorkspaceEdge, WorkspaceGraph,
    policy::ResolvedDependencyIdentity,
};

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
    let package_names = metadata
        .packages
        .iter()
        .map(|package| (package.id.clone(), package.name.to_string()))
        .collect::<BTreeMap<_, _>>();
    let mut package_roots = BTreeMap::<std::path::PathBuf, String>::new();
    let mut package_rust_versions = BTreeMap::new();
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
        package_roots.insert(package_root, package.name.to_string());
        package_rust_versions.insert(package.name.to_string(), package.rust_version.clone());
        let mut binary_targets = package
            .targets
            .iter()
            .filter(|target| target.kind.contains(&TargetKind::Bin))
            .map(|target| target.name.clone())
            .collect::<Vec<_>>();
        binary_targets.sort();
        binary_targets.dedup();
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
            version: package.version.clone(),
            publish: package.publish.clone(),
            binary_targets,
        });
    }
    packages.sort_by(|left, right| {
        (&left.name, &left.relative_path).cmp(&(&right.name, &right.relative_path))
    });

    let mut declared_path_dependencies = Vec::new();
    for package in metadata
        .packages
        .iter()
        .filter(|package| workspace_ids.contains(&package.id))
    {
        let from = package_name(&package_names, &package.id)?;
        for dependency in &package.dependencies {
            let Some(path) = dependency.path.as_ref() else {
                continue;
            };
            let dependency_root = std::fs::canonicalize(path.as_std_path()).map_err(|source| {
                CheckError::ResolveRoot {
                    path: path.clone().into_std_path_buf(),
                    source,
                }
            })?;
            let Some(to) = package_roots.get(&dependency_root) else {
                continue;
            };
            declared_path_dependencies.push(DeclaredPathDependency {
                from: from.clone(),
                to: to.clone(),
                kind: DependencyKind::from_cargo(dependency.kind),
                version_req: dependency.req.clone(),
            });
        }
    }
    declared_path_dependencies.sort_by(|left, right| {
        (
            &left.from,
            &left.to,
            left.kind,
            left.version_req.to_string(),
        )
            .cmp(&(
                &right.from,
                &right.to,
                right.kind,
                right.version_req.to_string(),
            ))
    });

    let resolve = metadata
        .resolve
        .as_ref()
        .ok_or(CheckError::MissingResolve)?;
    let mut resolved_dependencies = BTreeSet::new();
    for node in &resolve.nodes {
        let from_package = package_name(&package_names, &node.id)?;
        for dependency in &node.deps {
            let to_package = package_name(&package_names, &dependency.pkg)?;
            let to_workspace_package = workspace_ids
                .contains(&dependency.pkg)
                .then(|| to_package.clone());
            if dependency.dep_kinds.is_empty() {
                resolved_dependencies.insert(ResolvedDependencyIdentity {
                    from_package: from_package.clone(),
                    from_package_id: node.id.repr.clone(),
                    dependency_name: dependency.name.clone(),
                    to_package,
                    to_package_id: dependency.pkg.repr.clone(),
                    to_workspace_package,
                    kind: DependencyKind::Normal,
                });
                continue;
            }
            for dependency_kind in &dependency.dep_kinds {
                resolved_dependencies.insert(ResolvedDependencyIdentity {
                    from_package: from_package.clone(),
                    from_package_id: node.id.repr.clone(),
                    dependency_name: dependency.name.clone(),
                    to_package: to_package.clone(),
                    to_package_id: dependency.pkg.repr.clone(),
                    to_workspace_package: to_workspace_package.clone(),
                    kind: DependencyKind::from_cargo(dependency_kind.kind),
                });
            }
        }
    }

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
        declared_path_dependencies,
        package_rust_versions,
        resolved_dependencies: resolved_dependencies.into_iter().collect(),
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
            version: cargo_metadata::semver::Version::new(0, 2, 0),
            publish: None,
            binary_targets: Vec::new(),
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
        assert!(graph.declared_path_dependencies.iter().any(|dependency| {
            dependency.from == "stab-model"
                && dependency.to == "stab-engine"
                && dependency.kind == DependencyKind::Normal
                && dependency.version_req == cargo_metadata::semver::VersionReq::STAR
        }));
        for package in ["stab-model", "stab-engine"] {
            assert_eq!(
                graph.package_rust_versions.get(package),
                Some(&Some(crate::policy::stable_rust_version())),
                "{package} should retain its declared Stable MSRV"
            );
        }
        let report = crate::policy::validate_graph(&graph);
        assert!(report.violations.iter().any(|violation| {
            violation.code == "forbidden-product-edge"
                && violation.message.contains("stab-model -> stab-engine")
        }));
    }

    #[test]
    fn metadata_rejects_an_external_path_copy_of_a_product_package() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/external-product-copy-workspace");
        let graph = load_workspace_graph(&root).expect("load external-product-copy fixture");

        let dependency = graph
            .resolved_dependencies
            .iter()
            .find(|dependency| {
                dependency.from_package == "fixture-consumer"
                    && dependency.to_package == "stab-core"
            })
            .expect("fixture should retain the external stab-core dependency");
        assert_eq!(dependency.dependency_name, "stab_core_copy");
        assert_eq!(dependency.to_workspace_package, None);
        assert!(dependency.to_package_id.contains("external/stab-core"));

        let report = crate::policy::validate_graph(&graph);
        let violation = report
            .violations
            .iter()
            .find(|violation| violation.code == "external-product-dependency")
            .expect("external path copy should violate the product identity policy");
        assert!(violation.message.contains("fixture-consumer"));
        assert!(violation.message.contains("stab-core"));
        assert!(violation.message.contains("external/stab-core"));
    }

    #[test]
    fn metadata_drives_publication_contract_violations() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/publication-contract-workspace");
        let graph = load_workspace_graph(&root).expect("load publication-contract fixture");

        let cli = graph
            .packages
            .iter()
            .find(|package| package.name == "stab-cli")
            .expect("fixture should contain stab-cli");
        assert_eq!(cli.version, cargo_metadata::semver::Version::new(0, 2, 1));
        assert_eq!(cli.publish, None);
        assert_eq!(cli.binary_targets, ["stab", "stab-helper"]);
        assert!(graph.declared_path_dependencies.iter().any(|dependency| {
            dependency.from == "stab-cli"
                && dependency.to == "stab-core"
                && dependency.version_req
                    == cargo_metadata::semver::VersionReq::parse("0.2.0")
                        .expect("fixture requirement should parse")
        }));

        let report = crate::policy::validate_graph(&graph);
        let actual_codes = report
            .violations
            .iter()
            .map(|violation| violation.code)
            .collect::<Vec<_>>();
        assert_eq!(
            actual_codes,
            [
                "cli-binary-targets",
                "operational-package-publishable",
                "publishable-product-path-version",
                "publishable-product-version",
                "test-support-package-publishable",
            ]
        );
    }
}
