use std::collections::BTreeMap;
use std::path::{Component, PathBuf};

use crate::{MigrationAllowance, Violation};

const KNOWN_PRODUCT_PACKAGES: &[&str] = &[
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

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DependencyKind {
    Normal,
    Development,
    Build,
    Unknown,
}

impl DependencyKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Development => "development",
            Self::Build => "build",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PackageSpec {
    pub name: String,
    pub relative_path: PathBuf,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct WorkspaceEdge {
    pub from: String,
    pub to: String,
    pub kind: DependencyKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceGraph {
    pub packages: Vec<PackageSpec>,
    pub edges: Vec<WorkspaceEdge>,
}

#[derive(Debug)]
pub(super) struct PolicyReport {
    pub violations: Vec<Violation>,
    pub migration_allowances: Vec<MigrationAllowance>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PackageClass {
    Product,
    Ops,
    Unclassified,
}

pub(super) fn validate_graph(graph: &WorkspaceGraph) -> PolicyReport {
    let mut violations = Vec::new();
    let mut migration_allowances = Vec::new();
    let packages = graph
        .packages
        .iter()
        .map(|package| (package.name.as_str(), package))
        .collect::<BTreeMap<_, _>>();
    let classes = graph
        .packages
        .iter()
        .map(|package| (package.name.as_str(), classify_path(&package.relative_path)))
        .collect::<BTreeMap<_, _>>();

    for package in &graph.packages {
        let class = classify_path(&package.relative_path);
        if class == PackageClass::Unclassified {
            violations.push(Violation::new(
                "unclassified-package",
                format!(
                    "workspace package {} at {} is neither under crates/ nor ops/",
                    package.name,
                    package.relative_path.display()
                ),
            ));
            continue;
        }
        if class == PackageClass::Product {
            validate_product_identity(package, &mut violations);
        }
    }

    for edge in &graph.edges {
        let Some(from) = packages.get(edge.from.as_str()) else {
            violations.push(Violation::new(
                "unknown-edge-source",
                format!(
                    "{} dependency edge names missing source package {}",
                    edge.kind.as_str(),
                    edge.from
                ),
            ));
            continue;
        };
        let Some(to) = packages.get(edge.to.as_str()) else {
            violations.push(Violation::new(
                "unknown-edge-target",
                format!(
                    "{} dependency edge from {} names missing target package {}",
                    edge.kind.as_str(),
                    edge.from,
                    edge.to
                ),
            ));
            continue;
        };
        let from_class = classes
            .get(from.name.as_str())
            .copied()
            .unwrap_or(PackageClass::Unclassified);
        let to_class = classes
            .get(to.name.as_str())
            .copied()
            .unwrap_or(PackageClass::Unclassified);

        if from_class != PackageClass::Product {
            continue;
        }
        if to_class == PackageClass::Ops {
            if is_legacy_test_support_edge(edge) {
                migration_allowances.push(MigrationAllowance::new(
                    "legacy-product-test-support",
                    format!(
                        "{} retains its exact dev-only dependency on {} until the compatibility corpus moves to the records boundary",
                        edge.from, edge.to
                    ),
                ));
            } else {
                violations.push(Violation::new(
                    "product-to-ops",
                    format!(
                        "product package {} has a {} dependency on ops package {}",
                        edge.from,
                        edge.kind.as_str(),
                        edge.to
                    ),
                ));
            }
            continue;
        }
        if to_class == PackageClass::Product && !is_permitted_product_edge(&edge.from, &edge.to) {
            violations.push(Violation::new(
                "forbidden-product-edge",
                format!(
                    "target architecture forbids {} dependency {} -> {}",
                    edge.kind.as_str(),
                    edge.from,
                    edge.to
                ),
            ));
        }
    }

    violations.sort();
    violations.dedup();
    migration_allowances.sort();
    migration_allowances.dedup();
    PolicyReport {
        violations,
        migration_allowances,
    }
}

fn classify_path(path: &std::path::Path) -> PackageClass {
    match path.components().next() {
        Some(Component::Normal(component)) if component == "crates" => PackageClass::Product,
        Some(Component::Normal(component)) if component == "ops" => PackageClass::Ops,
        _ => PackageClass::Unclassified,
    }
}

fn validate_product_identity(package: &PackageSpec, violations: &mut Vec<Violation>) {
    if !KNOWN_PRODUCT_PACKAGES.contains(&package.name.as_str()) {
        violations.push(Violation::new(
            "unknown-product-package",
            format!(
                "product package {} at {} has no target architecture role",
                package.name,
                package.relative_path.display()
            ),
        ));
        return;
    }
    let expected_path = PathBuf::from("crates").join(&package.name);
    if package.relative_path != expected_path {
        violations.push(Violation::new(
            "product-path-mismatch",
            format!(
                "product package {} must live at {}, found {}",
                package.name,
                expected_path.display(),
                package.relative_path.display()
            ),
        ));
    }
}

fn is_permitted_product_edge(from: &str, to: &str) -> bool {
    match from {
        "stab-kernels-simd" => false,
        "stab-bits" => to == "stab-kernels-simd",
        "stab-records" => to == "stab-bits",
        "stab-algebra" => matches!(to, "stab-bits" | "stab-kernels-simd"),
        "stab-model" => to == "stab-algebra",
        "stab-analysis" => matches!(to, "stab-model" | "stab-algebra"),
        "stab-engine" => matches!(
            to,
            "stab-model" | "stab-records" | "stab-algebra" | "stab-analysis" | "stab-kernels-simd"
        ),
        "stab-decoder" => matches!(to, "stab-model" | "stab-records"),
        "stab-core" => KNOWN_PRODUCT_PACKAGES
            .iter()
            .copied()
            .any(|package| package == to && package != "stab-core" && package != "stab-cli"),
        "stab-cli" => to == "stab-core",
        _ => false,
    }
}

fn is_legacy_test_support_edge(edge: &WorkspaceEdge) -> bool {
    edge.kind == DependencyKind::Development
        && edge.to == "stab-compat-corpus"
        && matches!(edge.from.as_str(), "stab-core" | "stab-cli")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn package(name: &str, prefix: &str) -> PackageSpec {
        PackageSpec {
            name: name.to_owned(),
            relative_path: PathBuf::from(prefix).join(name),
        }
    }

    #[test]
    fn exact_legacy_corpus_edges_are_development_only() {
        let graph = WorkspaceGraph {
            packages: vec![
                package("stab-core", "crates"),
                PackageSpec {
                    name: "stab-compat-corpus".to_owned(),
                    relative_path: PathBuf::from("ops/compat-corpus"),
                },
            ],
            edges: vec![WorkspaceEdge {
                from: "stab-core".to_owned(),
                to: "stab-compat-corpus".to_owned(),
                kind: DependencyKind::Development,
            }],
        };
        let report = validate_graph(&graph);
        assert!(report.violations.is_empty());
        assert_eq!(report.migration_allowances.len(), 1);

        let legacy_edge = graph
            .edges
            .first()
            .expect("fixture should contain its legacy edge")
            .clone();
        let normal_graph = WorkspaceGraph {
            edges: vec![WorkspaceEdge {
                kind: DependencyKind::Normal,
                ..legacy_edge
            }],
            ..graph
        };
        let report = validate_graph(&normal_graph);
        assert_eq!(
            report
                .violations
                .first()
                .expect("normal product-to-ops edge should fail")
                .code,
            "product-to-ops"
        );
        assert!(report.migration_allowances.is_empty());
    }

    #[test]
    fn new_product_packages_require_an_explicit_role() {
        let graph = WorkspaceGraph {
            packages: vec![package("stab-plugin", "crates")],
            edges: Vec::new(),
        };
        let report = validate_graph(&graph);
        assert_eq!(
            report
                .violations
                .first()
                .expect("unknown product package should fail")
                .code,
            "unknown-product-package"
        );
    }
}
