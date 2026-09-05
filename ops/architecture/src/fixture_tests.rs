use std::path::PathBuf;

use cargo_metadata::semver::Version;
use serde::Deserialize;

use crate::policy::{
    PRODUCT_PACKAGE_CONTRACTS, is_stable_component, stable_rust_version, validate_graph,
};
use crate::{DependencyKind, PackageSpec, WorkspaceEdge, WorkspaceGraph};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GraphFixture {
    packages: Vec<FixturePackage>,
    edges: Vec<FixtureEdge>,
    expected_violation_codes: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FixturePackage {
    name: String,
    path: PathBuf,
    #[serde(default)]
    default_features: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureEdge {
    from: String,
    to: String,
    kind: FixtureDependencyKind,
    #[serde(default)]
    optional: bool,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum FixtureDependencyKind {
    Normal,
    Development,
    Build,
}

impl FixtureDependencyKind {
    const ALL: [Self; 3] = [Self::Normal, Self::Development, Self::Build];
}

impl From<FixtureDependencyKind> for DependencyKind {
    fn from(value: FixtureDependencyKind) -> Self {
        match value {
            FixtureDependencyKind::Normal => Self::Normal,
            FixtureDependencyKind::Development => Self::Development,
            FixtureDependencyKind::Build => Self::Build,
        }
    }
}

fn assert_fixture(source: &str) {
    let fixture: GraphFixture =
        serde_json::from_str(source).expect("architecture graph fixture should parse");
    let packages = fixture
        .packages
        .into_iter()
        .map(|package| {
            let rust_version = is_stable_component(&package.name).then(stable_rust_version);
            PackageSpec {
                version: Version::new(0, 2, 0),
                publish: if package.path.starts_with("crates") {
                    None
                } else {
                    Some(Vec::new())
                },
                binary_targets: if package.name == "stab-cli" {
                    vec!["stab".to_owned()]
                } else {
                    Vec::new()
                },
                name: package.name,
                relative_path: package.path,
                default_features: package.default_features,
                rust_version,
            }
        })
        .collect::<Vec<_>>();
    let graph = WorkspaceGraph {
        packages,
        edges: fixture
            .edges
            .into_iter()
            .map(|edge| WorkspaceEdge {
                from: edge.from,
                to: edge.to,
                kind: edge.kind.into(),
                optional: edge.optional,
            })
            .collect(),
        declared_path_dependencies: Vec::new(),
        resolved_dependencies: Vec::new(),
    };
    let report = validate_graph(&graph);
    let actual = report
        .violations
        .iter()
        .map(|violation| violation.code.to_owned())
        .collect::<Vec<_>>();
    assert_eq!(actual, fixture.expected_violation_codes);
}

#[test]
fn target_graph_fixture_is_permitted() {
    assert_fixture(include_str!("../tests/fixtures/permitted-target.json"));
}

#[test]
fn forbidden_edge_fixture_fails_closed() {
    assert_fixture(include_str!("../tests/fixtures/forbidden-edges.json"));
}

#[test]
fn every_product_edge_matches_the_target_graph_for_every_dependency_kind() {
    let product_packages = PRODUCT_PACKAGE_CONTRACTS
        .iter()
        .map(|contract| contract.name)
        .collect::<Vec<_>>();
    let packages = product_packages
        .iter()
        .map(|name| PackageSpec {
            name: (*name).to_owned(),
            relative_path: PathBuf::from("crates").join(name),
            default_features: Vec::new(),
            rust_version: is_stable_component(name).then(stable_rust_version),
            version: Version::new(0, 2, 0),
            publish: None,
            binary_targets: if *name == "stab-cli" {
                vec!["stab".to_owned()]
            } else {
                Vec::new()
            },
        })
        .collect::<Vec<_>>();
    for from in &product_packages {
        for to in &product_packages {
            for kind in FixtureDependencyKind::ALL {
                let graph = WorkspaceGraph {
                    packages: packages.clone(),
                    edges: vec![WorkspaceEdge {
                        from: (*from).to_owned(),
                        to: (*to).to_owned(),
                        kind: kind.into(),
                        optional: *to == "stab-kernels-simd",
                    }],
                    declared_path_dependencies: Vec::new(),
                    resolved_dependencies: Vec::new(),
                };
                let report = validate_graph(&graph);
                let is_permitted = PRODUCT_PACKAGE_CONTRACTS
                    .iter()
                    .find(|contract| contract.name == *from)
                    .is_some_and(|contract| contract.allowed_dependencies.contains(to));
                let valid_kernel_edge =
                    *to != "stab-kernels-simd" || matches!(kind, FixtureDependencyKind::Normal);
                assert_eq!(
                    report.violations.is_empty(),
                    is_permitted && valid_kernel_edge,
                    "{kind:?} product edge {from} -> {to}"
                );
            }
        }
    }
}
