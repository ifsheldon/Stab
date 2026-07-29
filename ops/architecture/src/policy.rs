use std::collections::BTreeMap;
use std::path::{Component, PathBuf};

use cargo_metadata::semver::{Op, Version, VersionReq};

use crate::{MigrationAllowance, Violation};

const RELEASE_MAJOR: u64 = 0;
const RELEASE_MINOR: u64 = 2;
const RELEASE_PATCH: u64 = 0;
const STABLE_RUST_MAJOR: u64 = 1;
const STABLE_RUST_MINOR: u64 = 97;
const STABLE_RUST_PATCH: u64 = 1;
pub(crate) const PRODUCT_PACKAGE_CONTRACTS: &[ProductPackageContract] = &[
    ProductPackageContract::stable("stab-algebra", &["stab-bits", "stab-kernels-simd"]),
    ProductPackageContract::stable("stab-analysis", &["stab-algebra", "stab-model"]),
    ProductPackageContract::stable("stab-bits", &["stab-kernels-simd"]),
    ProductPackageContract::nightly("stab-cli", &["stab-core"], &["stab"]),
    ProductPackageContract::nightly(
        "stab-core",
        &[
            "stab-algebra",
            "stab-analysis",
            "stab-bits",
            "stab-decoder",
            "stab-engine",
            "stab-model",
            "stab-records",
        ],
        &[],
    ),
    ProductPackageContract::stable("stab-decoder", &["stab-model", "stab-records"]),
    ProductPackageContract::stable(
        "stab-engine",
        &[
            "stab-algebra",
            "stab-analysis",
            "stab-model",
            "stab-records",
        ],
    ),
    ProductPackageContract::nightly("stab-kernels-simd", &[], &[]),
    ProductPackageContract::stable("stab-model", &["stab-algebra"]),
    ProductPackageContract::stable("stab-records", &["stab-bits"]),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProductPackageContract {
    pub(crate) name: &'static str,
    pub(crate) stable_component: bool,
    pub(crate) allowed_dependencies: &'static [&'static str],
    pub(crate) binary_targets: &'static [&'static str],
}

impl ProductPackageContract {
    const fn stable(name: &'static str, allowed_dependencies: &'static [&'static str]) -> Self {
        Self {
            name,
            stable_component: true,
            allowed_dependencies,
            binary_targets: &[],
        }
    }

    const fn nightly(
        name: &'static str,
        allowed_dependencies: &'static [&'static str],
        binary_targets: &'static [&'static str],
    ) -> Self {
        Self {
            name,
            stable_component: false,
            allowed_dependencies,
            binary_targets,
        }
    }
}

pub(crate) fn product_contract(name: &str) -> Option<&'static ProductPackageContract> {
    PRODUCT_PACKAGE_CONTRACTS
        .iter()
        .find(|contract| contract.name == name)
}

pub(crate) fn is_stable_component(name: &str) -> bool {
    product_contract(name).is_some_and(|contract| contract.stable_component)
}

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
    pub default_features: Vec<String>,
    pub rust_version: Option<Version>,
    pub version: Version,
    pub publish: Option<Vec<String>>,
    pub binary_targets: Vec<String>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct WorkspaceEdge {
    pub from: String,
    pub to: String,
    pub kind: DependencyKind,
    pub optional: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeclaredPathDependency {
    pub from: String,
    pub to: String,
    pub kind: DependencyKind,
    pub version_req: VersionReq,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct ResolvedDependencyIdentity {
    pub from_package: String,
    pub from_package_id: String,
    pub dependency_name: String,
    pub to_package: String,
    pub to_package_id: String,
    pub to_workspace_package: Option<String>,
    pub kind: DependencyKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceGraph {
    pub packages: Vec<PackageSpec>,
    pub edges: Vec<WorkspaceEdge>,
    pub declared_path_dependencies: Vec<DeclaredPathDependency>,
    pub(crate) resolved_dependencies: Vec<ResolvedDependencyIdentity>,
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
    TestSupport,
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
                    "workspace package {} at {} is not under crates/, ops/, or test-support/",
                    package.name,
                    package.relative_path.display()
                ),
            ));
            continue;
        }

        match class {
            PackageClass::Product => {
                validate_product_identity(package, &mut violations);
                validate_product_publication(package, &mut violations);
                if is_stable_component(&package.name) {
                    validate_stable_rust_version(package, &mut violations);
                    if package
                        .default_features
                        .iter()
                        .any(|feature| feature.contains("portable-simd"))
                    {
                        violations.push(Violation::new(
                            "stable-default-reaches-nightly",
                            format!(
                                "Stable component {} enables portable SIMD through default features {:?}",
                                package.name, package.default_features
                            ),
                        ));
                    }
                }
            }
            PackageClass::Ops if package_is_publishable(package) => {
                violations.push(Violation::new(
                    "operational-package-publishable",
                    format!(
                        "operational package {} at {} must set publish = false",
                        package.name,
                        package.relative_path.display()
                    ),
                ));
            }
            PackageClass::TestSupport if package_is_publishable(package) => {
                violations.push(Violation::new(
                    "test-support-package-publishable",
                    format!(
                        "test-support package {} at {} must set publish = false",
                        package.name,
                        package.relative_path.display()
                    ),
                ));
            }
            PackageClass::Ops | PackageClass::TestSupport | PackageClass::Unclassified => {}
        }
    }

    for dependency in &graph.resolved_dependencies {
        if product_contract(&dependency.to_package).is_some()
            && !resolves_to_local_product_package(dependency, &packages)
        {
            violations.push(Violation::new(
                "external-product-dependency",
                format!(
                    "{} package {} ({}) resolves dependency {} to external product package {} ({}); it must resolve to the corresponding local workspace package",
                    dependency.kind.as_str(),
                    dependency.from_package,
                    dependency.from_package_id,
                    dependency.dependency_name,
                    dependency.to_package,
                    dependency.to_package_id,
                ),
            ));
        }
    }

    for dependency in &graph.declared_path_dependencies {
        let Some(from) = packages.get(dependency.from.as_str()) else {
            violations.push(Violation::new(
                "unknown-path-dependency-source",
                format!(
                    "{} path dependency names missing source package {}",
                    dependency.kind.as_str(),
                    dependency.from
                ),
            ));
            continue;
        };
        let Some(to) = packages.get(dependency.to.as_str()) else {
            violations.push(Violation::new(
                "unknown-path-dependency-target",
                format!(
                    "{} path dependency from {} names missing target package {}",
                    dependency.kind.as_str(),
                    dependency.from,
                    dependency.to
                ),
            ));
            continue;
        };
        if classify_path(&from.relative_path) == PackageClass::Product
            && classify_path(&to.relative_path) == PackageClass::Product
            && package_is_publishable(from)
            && package_is_publishable(to)
            && !is_exact_release_requirement(&dependency.version_req)
        {
            violations.push(Violation::new(
                "publishable-product-path-version",
                format!(
                    "publishable product {} has a {} path dependency on {} requiring {}; every publishable product path dependency must require exactly =0.2.0",
                    dependency.from,
                    dependency.kind.as_str(),
                    dependency.to,
                    dependency.version_req
                ),
            ));
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

        if from_class == PackageClass::TestSupport {
            if matches!(to_class, PackageClass::Product | PackageClass::Ops) {
                violations.push(Violation::new(
                    "test-support-upward-dependency",
                    format!(
                        "test-support package {} has a {} dependency on workspace package {}",
                        edge.from,
                        edge.kind.as_str(),
                        edge.to
                    ),
                ));
            }
            continue;
        }
        if from_class != PackageClass::Product {
            continue;
        }
        if to_class == PackageClass::TestSupport {
            if edge.kind != DependencyKind::Development {
                violations.push(Violation::new(
                    "product-test-support-runtime-edge",
                    format!(
                        "product package {} has a {} dependency on test-support package {}",
                        edge.from,
                        edge.kind.as_str(),
                        edge.to
                    ),
                ));
            }
            continue;
        }
        if to_class == PackageClass::Ops {
            violations.push(Violation::new(
                "product-to-ops",
                format!(
                    "product package {} has a {} dependency on ops package {}",
                    edge.from,
                    edge.kind.as_str(),
                    edge.to
                ),
            ));
            continue;
        }
        if to_class == PackageClass::Product
            && edge.to == "stab-kernels-simd"
            && matches!(edge.from.as_str(), "stab-bits" | "stab-algebra")
            && (edge.kind != DependencyKind::Normal || !edge.optional)
        {
            violations.push(Violation::new(
                "nightly-kernel-edge-not-optional",
                format!(
                    "{} -> {} must be an optional normal dependency, found {} optional={}",
                    edge.from,
                    edge.to,
                    edge.kind.as_str(),
                    edge.optional
                ),
            ));
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

fn resolves_to_local_product_package(
    dependency: &ResolvedDependencyIdentity,
    packages: &BTreeMap<&str, &PackageSpec>,
) -> bool {
    if dependency.to_workspace_package.as_deref() != Some(dependency.to_package.as_str()) {
        return false;
    }
    packages
        .get(dependency.to_package.as_str())
        .is_some_and(|package| {
            package.relative_path == PathBuf::from("crates").join(&dependency.to_package)
        })
}

fn classify_path(path: &std::path::Path) -> PackageClass {
    match path.components().next() {
        Some(Component::Normal(component)) if component == "crates" => PackageClass::Product,
        Some(Component::Normal(component)) if component == "ops" => PackageClass::Ops,
        Some(Component::Normal(component)) if component == "test-support" => {
            PackageClass::TestSupport
        }
        _ => PackageClass::Unclassified,
    }
}

fn validate_product_identity(package: &PackageSpec, violations: &mut Vec<Violation>) {
    if product_contract(&package.name).is_none() {
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

fn validate_product_publication(package: &PackageSpec, violations: &mut Vec<Violation>) {
    if package_is_publishable(package) && package.version != release_version() {
        violations.push(Violation::new(
            "publishable-product-version",
            format!(
                "publishable product package {} must have version {}, found {}",
                package.name,
                release_version(),
                package.version
            ),
        ));
    }
    let Some(contract) = product_contract(&package.name) else {
        return;
    };
    if package.binary_targets != contract.binary_targets {
        violations.push(Violation::new(
            "product-binary-targets",
            format!(
                "{} must expose binary targets {:?}, found {:?}",
                package.name, contract.binary_targets, package.binary_targets
            ),
        ));
    }
}

fn validate_stable_rust_version(package: &PackageSpec, violations: &mut Vec<Violation>) {
    let expected = stable_rust_version();
    if package.rust_version.as_ref() == Some(&expected) {
        return;
    }
    let actual = package
        .rust_version
        .as_ref()
        .map_or_else(|| "not declared".to_owned(), ToString::to_string);
    violations.push(Violation::new(
        "stable-component-rust-version",
        format!(
            "Stable component {} must declare rust-version = {}, found {}",
            package.name, expected, actual
        ),
    ));
}

fn package_is_publishable(package: &PackageSpec) -> bool {
    !matches!(&package.publish, Some(registries) if registries.is_empty())
}

fn release_version() -> Version {
    Version::new(RELEASE_MAJOR, RELEASE_MINOR, RELEASE_PATCH)
}

pub(crate) fn stable_rust_version() -> Version {
    Version::new(STABLE_RUST_MAJOR, STABLE_RUST_MINOR, STABLE_RUST_PATCH)
}

fn is_exact_release_requirement(requirement: &VersionReq) -> bool {
    matches!(
        requirement.comparators.as_slice(),
        [comparator]
            if comparator.op == Op::Exact
                && comparator.major == RELEASE_MAJOR
                && comparator.minor == Some(RELEASE_MINOR)
                && comparator.patch == Some(RELEASE_PATCH)
                && comparator.pre.is_empty()
    )
}

fn is_permitted_product_edge(from: &str, to: &str) -> bool {
    product_contract(from).is_some_and(|contract| contract.allowed_dependencies.contains(&to))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn package(name: &str, prefix: &str) -> PackageSpec {
        PackageSpec {
            name: name.to_owned(),
            relative_path: PathBuf::from(prefix).join(name),
            default_features: Vec::new(),
            rust_version: is_stable_component(name).then(stable_rust_version),
            version: Version::new(0, 2, 0),
            publish: if prefix == "crates" {
                None
            } else {
                Some(Vec::new())
            },
            binary_targets: if name == "stab-cli" {
                vec!["stab".to_owned()]
            } else {
                Vec::new()
            },
        }
    }

    fn graph(packages: Vec<PackageSpec>) -> WorkspaceGraph {
        WorkspaceGraph {
            packages,
            edges: Vec::new(),
            declared_path_dependencies: Vec::new(),
            resolved_dependencies: Vec::new(),
        }
    }

    #[test]
    fn test_support_edges_are_development_only_and_downward() {
        let mut graph = graph(vec![
            package("stab-core", "crates"),
            PackageSpec {
                name: "stab-compat-corpus".to_owned(),
                relative_path: PathBuf::from("test-support/compat-corpus"),
                default_features: Vec::new(),
                rust_version: None,
                version: Version::new(0, 2, 0),
                publish: Some(Vec::new()),
                binary_targets: Vec::new(),
            },
        ]);
        graph.edges = vec![WorkspaceEdge {
            from: "stab-core".to_owned(),
            to: "stab-compat-corpus".to_owned(),
            kind: DependencyKind::Development,
            optional: false,
        }];
        let report = validate_graph(&graph);
        assert!(report.violations.is_empty());
        assert!(report.migration_allowances.is_empty());

        let support_edge = graph
            .edges
            .first()
            .expect("fixture should contain its test-support edge")
            .clone();
        let normal_graph = WorkspaceGraph {
            edges: vec![WorkspaceEdge {
                kind: DependencyKind::Normal,
                ..support_edge
            }],
            ..graph.clone()
        };
        let report = validate_graph(&normal_graph);
        assert_eq!(
            report
                .violations
                .first()
                .expect("normal product-to-test-support edge should fail")
                .code,
            "product-test-support-runtime-edge"
        );
        assert!(report.migration_allowances.is_empty());

        let upward_graph = WorkspaceGraph {
            edges: vec![WorkspaceEdge {
                from: "stab-compat-corpus".to_owned(),
                to: "stab-core".to_owned(),
                kind: DependencyKind::Development,
                optional: false,
            }],
            ..graph
        };
        assert_eq!(
            validate_graph(&upward_graph)
                .violations
                .first()
                .expect("test support must not depend on product code")
                .code,
            "test-support-upward-dependency"
        );
    }

    #[test]
    fn new_product_packages_require_an_explicit_role() {
        let graph = graph(vec![package("stab-plugin", "crates")]);
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

    #[test]
    fn stable_component_defaults_cannot_reach_portable_simd() {
        let mut bits = package("stab-bits", "crates");
        bits.default_features = vec!["portable-simd".to_owned()];
        let report = validate_graph(&graph(vec![bits]));

        assert_eq!(report.violations.len(), 1);
        assert_eq!(
            report
                .violations
                .first()
                .expect("portable default should produce one violation")
                .code,
            "stable-default-reaches-nightly"
        );
    }

    #[test]
    fn stable_components_require_the_exact_rust_version() {
        for contract in PRODUCT_PACKAGE_CONTRACTS
            .iter()
            .filter(|contract| contract.stable_component)
        {
            let valid = graph(vec![package(contract.name, "crates")]);
            assert!(
                validate_graph(&valid).violations.is_empty(),
                "{} should accept the exact Stable MSRV",
                contract.name
            );

            for actual in [None, Some(Version::new(1, 97, 0))] {
                let mut invalid = valid.clone();
                invalid
                    .packages
                    .first_mut()
                    .expect("fixture should contain one package")
                    .rust_version = actual;
                let report = validate_graph(&invalid);
                let violation = report
                    .violations
                    .iter()
                    .find(|violation| violation.code == "stable-component-rust-version")
                    .expect("missing or mismatched Stable MSRV should fail");
                assert!(violation.message.contains(contract.name));
                assert!(violation.message.contains("1.97.1"));
            }
        }
    }

    #[test]
    fn external_product_dependencies_are_rejected_for_every_source_kind() {
        let packages = vec![
            package("stab-core", "crates"),
            package("fixture-consumer", "ops"),
        ];
        for to_package_id in [
            "path+file:///tmp/external/stab-core#9.9.9",
            "git+https://example.invalid/stab-core#01234567",
            "registry+https://example.invalid/index#stab-core@9.9.9",
        ] {
            let mut invalid = graph(packages.clone());
            invalid
                .resolved_dependencies
                .push(ResolvedDependencyIdentity {
                    from_package: "fixture-consumer".to_owned(),
                    from_package_id: "path+file:///workspace/ops/fixture-consumer#0.2.0".to_owned(),
                    dependency_name: "stab_core_copy".to_owned(),
                    to_package: "stab-core".to_owned(),
                    to_package_id: to_package_id.to_owned(),
                    to_workspace_package: None,
                    kind: DependencyKind::Normal,
                });
            let report = validate_graph(&invalid);
            let violation = report
                .violations
                .iter()
                .find(|violation| violation.code == "external-product-dependency")
                .expect("external product package copy should fail");
            assert!(violation.message.contains("fixture-consumer"));
            assert!(violation.message.contains("stab-core"));
            assert!(violation.message.contains(to_package_id));
        }

        let mut valid = graph(packages);
        valid
            .resolved_dependencies
            .push(ResolvedDependencyIdentity {
                from_package: "fixture-consumer".to_owned(),
                from_package_id: "path+file:///workspace/ops/fixture-consumer#0.2.0".to_owned(),
                dependency_name: "stab_core".to_owned(),
                to_package: "stab-core".to_owned(),
                to_package_id: "path+file:///workspace/crates/stab-core#0.2.0".to_owned(),
                to_workspace_package: Some("stab-core".to_owned()),
                kind: DependencyKind::Normal,
            });
        assert!(validate_graph(&valid).violations.is_empty());
    }

    #[test]
    fn publishable_product_packages_require_release_version() {
        let mut core = package("stab-core", "crates");
        core.version = Version::new(0, 2, 1);
        let report = validate_graph(&graph(vec![core.clone()]));
        assert_eq!(report.violations.len(), 1);
        let violation = report
            .violations
            .first()
            .expect("wrong release version should fail");
        assert_eq!(violation.code, "publishable-product-version");
        assert!(violation.message.contains("stab-core"));
        assert!(violation.message.contains("0.2.1"));
        assert!(violation.message.contains("0.2.0"));

        core.publish = Some(Vec::new());
        assert!(
            validate_graph(&graph(vec![core])).violations.is_empty(),
            "unpublished product packages are outside the publication-version contract"
        );
    }

    #[test]
    fn publishable_product_path_dependencies_require_exact_release_version() {
        let packages = vec![
            package("stab-records", "crates"),
            package("stab-bits", "crates"),
        ];
        for kind in [
            DependencyKind::Normal,
            DependencyKind::Development,
            DependencyKind::Build,
        ] {
            let mut exact = graph(packages.clone());
            exact.declared_path_dependencies = vec![DeclaredPathDependency {
                from: "stab-records".to_owned(),
                to: "stab-bits".to_owned(),
                kind,
                version_req: VersionReq::parse("=0.2.0")
                    .expect("exact release requirement should parse"),
            }];
            assert!(
                validate_graph(&exact).violations.is_empty(),
                "{kind:?} exact path requirement should pass"
            );

            for requirement in ["0.2.0", "=0.2", ">=0.2.0", "=0.2.0-alpha.1"] {
                let mut inexact = exact.clone();
                inexact
                    .declared_path_dependencies
                    .first_mut()
                    .expect("fixture should contain its path dependency")
                    .version_req =
                    VersionReq::parse(requirement).expect("fixture requirement should parse");
                let report = validate_graph(&inexact);
                let violation = report
                    .violations
                    .iter()
                    .find(|violation| violation.code == "publishable-product-path-version")
                    .expect("inexact product path requirement should fail");
                assert!(violation.message.contains("stab-records"));
                assert!(violation.message.contains("stab-bits"));
                assert!(violation.message.contains("=0.2.0"));
            }
        }
    }

    #[test]
    fn operational_and_test_support_packages_must_be_unpublished() {
        for (name, prefix, expected_code) in [
            (
                "stab-architecture",
                "ops",
                "operational-package-publishable",
            ),
            (
                "stab-compat-corpus",
                "test-support",
                "test-support-package-publishable",
            ),
        ] {
            let mut support = package(name, prefix);
            support.publish = None;
            let report = validate_graph(&graph(vec![support]));
            let violation = report
                .violations
                .first()
                .expect("publishable support package should fail");
            assert_eq!(violation.code, expected_code);
            assert!(violation.message.contains(name));
            assert!(violation.message.contains("publish = false"));
        }
    }

    #[test]
    fn cli_exposes_exactly_one_stab_binary() {
        let valid = package("stab-cli", "crates");
        assert!(
            validate_graph(&graph(vec![valid.clone()]))
                .violations
                .is_empty()
        );

        for targets in [
            Vec::new(),
            vec!["stab-cli".to_owned()],
            vec!["stab".to_owned(), "stab-helper".to_owned()],
        ] {
            let mut cli = valid.clone();
            cli.binary_targets = targets;
            let report = validate_graph(&graph(vec![cli]));
            let violation = report
                .violations
                .iter()
                .find(|violation| violation.code == "product-binary-targets")
                .expect("invalid CLI binary targets should fail");
            assert!(violation.message.contains("[\"stab\"]"));
        }
    }
}
