use std::collections::BTreeMap;
use std::path::{Component, PathBuf};

use cargo_metadata::semver::{Op, Version, VersionReq};

use crate::Violation;

const RELEASE_MAJOR: u64 = 0;
const RELEASE_MINOR: u64 = 2;
const RELEASE_PATCH: u64 = 0;
const MINIMUM_RUST_MAJOR: u64 = 1;
const MINIMUM_RUST_MINOR: u64 = 100;
const MINIMUM_RUST_PATCH: u64 = 0;
pub(crate) const PRODUCT_PACKAGE_CONTRACTS: &[ProductPackageContract] = &[
    ProductPackageContract::library(
        "stab-algebra",
        &["stab-bits", "stab-foundation", "stab-kernels-simd"],
    ),
    ProductPackageContract::library("stab-analysis", &["stab-algebra", "stab-model"]),
    ProductPackageContract::library("stab-bits", &["stab-kernels-simd"]),
    ProductPackageContract::binary(
        "stab-cli",
        &[
            "stab-analysis",
            "stab-bits",
            "stab-engine",
            "stab-model",
            "stab-records",
        ],
        &["stab"],
    ),
    ProductPackageContract::library(
        "stab-core",
        &[
            "stab-algebra",
            "stab-analysis",
            "stab-decoder",
            "stab-engine",
            "stab-model",
            "stab-records",
        ],
    ),
    ProductPackageContract::library("stab-decoder", &["stab-model", "stab-records"]),
    ProductPackageContract::library(
        "stab-engine",
        &[
            "stab-algebra",
            "stab-analysis",
            "stab-bits",
            "stab-model",
            "stab-records",
        ],
    ),
    ProductPackageContract::library("stab-foundation", &[]),
    ProductPackageContract::library("stab-kernels-simd", &[]),
    ProductPackageContract::library("stab-model", &["stab-foundation"]),
    ProductPackageContract::library("stab-records", &["stab-bits", "stab-foundation"]),
];
/// Test-support packages that may depend on product components, with the exact
/// non-optional edges each one earned. Every other test-support package may not
/// depend on product or operational packages at all.
pub(crate) const TEST_SUPPORT_PACKAGE_CONTRACTS: &[TestSupportPackageContract] = &[
    TestSupportPackageContract {
        name: "stab-reference-decoder",
        normal_dependencies: &["stab-decoder", "stab-model", "stab-records"],
        development_dependencies: &["stab-analysis", "stab-engine"],
    },
    TestSupportPackageContract {
        name: "stab-reference-noise-pass",
        normal_dependencies: &["stab-analysis", "stab-model"],
        development_dependencies: &[],
    },
];
const SIMD_KERNEL_PACKAGE: &str = "stab-kernels-simd";
const SIMD_LIBRARY_PACKAGES: &[&str] = &["fearless_simd", "fearless_simd_macros"];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProductPackageContract {
    pub(crate) name: &'static str,
    pub(crate) allowed_dependencies: &'static [&'static str],
    pub(crate) binary_targets: &'static [&'static str],
}

impl ProductPackageContract {
    const fn library(name: &'static str, allowed_dependencies: &'static [&'static str]) -> Self {
        Self {
            name,
            allowed_dependencies,
            binary_targets: &[],
        }
    }

    const fn binary(
        name: &'static str,
        allowed_dependencies: &'static [&'static str],
        binary_targets: &'static [&'static str],
    ) -> Self {
        Self {
            name,
            allowed_dependencies,
            binary_targets,
        }
    }
}

/// A Stable test-support proof package and the product edges it may use.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TestSupportPackageContract {
    pub(crate) name: &'static str,
    pub(crate) normal_dependencies: &'static [&'static str],
    pub(crate) development_dependencies: &'static [&'static str],
}

impl TestSupportPackageContract {
    fn permits(&self, edge: &WorkspaceEdge) -> bool {
        let allowed = match edge.kind {
            DependencyKind::Normal => self.normal_dependencies,
            DependencyKind::Development => self.development_dependencies,
            DependencyKind::Build | DependencyKind::Unknown => &[],
        };
        !edge.optional && allowed.contains(&edge.to.as_str())
    }
}

pub(crate) fn product_contract(name: &str) -> Option<&'static ProductPackageContract> {
    PRODUCT_PACKAGE_CONTRACTS
        .iter()
        .find(|contract| contract.name == name)
}

pub(crate) fn test_support_contract(name: &str) -> Option<&'static TestSupportPackageContract> {
    TEST_SUPPORT_PACKAGE_CONTRACTS
        .iter()
        .find(|contract| contract.name == name)
}

/// Every product package and the Stable test-support proofs must declare the
/// workspace minimum Rust version and must not enable unstable Rust features.
pub(crate) fn requires_minimum_rust(name: &str) -> bool {
    product_contract(name).is_some() || test_support_contract(name).is_some()
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
    pub rust_version: Option<Version>,
    pub version: Version,
    pub publish: Option<Vec<String>>,
    pub binary_targets: Vec<String>,
    /// Every Cargo target root of the package, as Cargo metadata resolved it.
    pub targets: Vec<TargetSpec>,
}

/// One Cargo target root (`src_path`) reported by Cargo metadata.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct TargetSpec {
    pub name: String,
    pub library: bool,
    /// The canonical absolute target root.
    pub src_path: PathBuf,
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
                if product_contract(&package.name).is_some() {
                    validate_minimum_rust_version(package, &mut violations);
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
            PackageClass::TestSupport => {
                if package_is_publishable(package) {
                    violations.push(Violation::new(
                        "test-support-package-publishable",
                        format!(
                            "test-support package {} at {} must set publish = false",
                            package.name,
                            package.relative_path.display()
                        ),
                    ));
                }
                if test_support_contract(&package.name).is_some() {
                    validate_minimum_rust_version(package, &mut violations);
                }
            }
            PackageClass::Ops | PackageClass::Unclassified => {}
        }
    }

    for dependency in &graph.resolved_dependencies {
        if SIMD_LIBRARY_PACKAGES.contains(&dependency.to_package.as_str())
            && dependency.from_package != SIMD_KERNEL_PACKAGE
        {
            violations.push(Violation::new(
                "simd-library-outside-kernel",
                format!(
                    "{} package {} ({}) depends on {} ({}) as {}; only {} may depend on the SIMD library",
                    dependency.kind.as_str(),
                    dependency.from_package,
                    dependency.from_package_id,
                    dependency.to_package,
                    dependency.to_package_id,
                    dependency.dependency_name,
                    SIMD_KERNEL_PACKAGE,
                ),
            ));
        }
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
        } else if is_protected_support_identity(&dependency.to_package, &classes)
            && !resolves_to_local_workspace_package(dependency, &packages)
        {
            violations.push(Violation::new(
                "external-workspace-package-dependency",
                format!(
                    "{} package {} ({}) resolves dependency {} to external package {} ({}), which shadows the repository's operational or test-support package; it must resolve to the local workspace package",
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
            if matches!(to_class, PackageClass::Product | PackageClass::Ops)
                && !is_permitted_test_support_component_edge(edge)
            {
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
    PolicyReport { violations }
}

fn is_permitted_test_support_component_edge(edge: &WorkspaceEdge) -> bool {
    test_support_contract(&edge.from).is_some_and(|contract| contract.permits(edge))
}

/// Operational and test-support package names are repository identities too: a
/// dependency that names one must resolve to the workspace member, never to an
/// external path, git, or registry copy that shadows it.
fn resolves_to_local_workspace_package(
    dependency: &ResolvedDependencyIdentity,
    packages: &BTreeMap<&str, &PackageSpec>,
) -> bool {
    dependency.to_workspace_package.as_deref() == Some(dependency.to_package.as_str())
        && packages.contains_key(dependency.to_package.as_str())
}

fn is_protected_support_identity(name: &str, classes: &BTreeMap<&str, PackageClass>) -> bool {
    matches!(
        classes.get(name),
        Some(PackageClass::Ops | PackageClass::TestSupport)
    ) || test_support_contract(name).is_some()
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

fn validate_minimum_rust_version(package: &PackageSpec, violations: &mut Vec<Violation>) {
    let expected = minimum_rust_version();
    if package.rust_version.as_ref() == Some(&expected) {
        return;
    }
    let actual = package
        .rust_version
        .as_ref()
        .map_or_else(|| "not declared".to_owned(), ToString::to_string);
    violations.push(Violation::new(
        "minimum-rust-version",
        format!(
            "package {} must declare the workspace minimum rust-version = {}, found {}",
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

pub(crate) fn minimum_rust_version() -> Version {
    Version::new(MINIMUM_RUST_MAJOR, MINIMUM_RUST_MINOR, MINIMUM_RUST_PATCH)
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
mod tests;
