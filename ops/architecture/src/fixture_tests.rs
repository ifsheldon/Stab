use std::path::PathBuf;

use serde::Deserialize;

use crate::policy::validate_graph;
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
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureEdge {
    from: String,
    to: String,
    kind: FixtureDependencyKind,
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
    let graph = WorkspaceGraph {
        packages: fixture
            .packages
            .into_iter()
            .map(|package| PackageSpec {
                name: package.name,
                relative_path: package.path,
            })
            .collect(),
        edges: fixture
            .edges
            .into_iter()
            .map(|edge| WorkspaceEdge {
                from: edge.from,
                to: edge.to,
                kind: edge.kind.into(),
            })
            .collect(),
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
    const PRODUCT_PACKAGES: [&str; 10] = [
        "stab-algebra",
        "stab-analysis",
        "stab-bits",
        "stab-cli",
        "stab-core",
        "stab-decoder",
        "stab-engine",
        "stab-kernels-simd",
        "stab-model",
        "stab-records",
    ];
    const PERMITTED_EDGES: [(&str, &str); 23] = [
        ("stab-bits", "stab-kernels-simd"),
        ("stab-records", "stab-bits"),
        ("stab-algebra", "stab-bits"),
        ("stab-algebra", "stab-kernels-simd"),
        ("stab-model", "stab-algebra"),
        ("stab-analysis", "stab-model"),
        ("stab-analysis", "stab-algebra"),
        ("stab-engine", "stab-model"),
        ("stab-engine", "stab-records"),
        ("stab-engine", "stab-algebra"),
        ("stab-engine", "stab-analysis"),
        ("stab-engine", "stab-kernels-simd"),
        ("stab-decoder", "stab-model"),
        ("stab-decoder", "stab-records"),
        ("stab-core", "stab-algebra"),
        ("stab-core", "stab-analysis"),
        ("stab-core", "stab-bits"),
        ("stab-core", "stab-decoder"),
        ("stab-core", "stab-engine"),
        ("stab-core", "stab-kernels-simd"),
        ("stab-core", "stab-model"),
        ("stab-core", "stab-records"),
        ("stab-cli", "stab-core"),
    ];

    let packages = PRODUCT_PACKAGES
        .iter()
        .map(|name| PackageSpec {
            name: (*name).to_string(),
            relative_path: PathBuf::from("crates").join(name),
        })
        .collect::<Vec<_>>();
    for from in PRODUCT_PACKAGES {
        for to in PRODUCT_PACKAGES {
            for kind in FixtureDependencyKind::ALL {
                let graph = WorkspaceGraph {
                    packages: packages.clone(),
                    edges: vec![WorkspaceEdge {
                        from: from.to_string(),
                        to: to.to_string(),
                        kind: kind.into(),
                    }],
                };
                let report = validate_graph(&graph);
                let is_permitted = PERMITTED_EDGES.contains(&(from, to));
                let forbidden_edges = report
                    .violations
                    .iter()
                    .filter(|violation| violation.code == "forbidden-product-edge")
                    .count();
                assert_eq!(
                    forbidden_edges,
                    usize::from(!is_permitted),
                    "{kind:?} product edge {from} -> {to}"
                );
                assert!(
                    report.migration_allowances.is_empty(),
                    "{kind:?} product edge {from} -> {to} must not use a migration allowance"
                );
            }
        }
    }
}
