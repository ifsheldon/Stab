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
