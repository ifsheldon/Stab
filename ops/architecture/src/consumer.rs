use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use cargo_metadata::{Metadata, MetadataCommand, PackageId};
use thiserror::Error;

const PREFIX: &str = "stab-architecture";
const STABLE_TOOLCHAIN: &str = "+1.97.1";
const FEATURE_CONTRACTS: [FeatureContract; 6] = [
    FeatureContract {
        package: "stab-bits",
        default: &[],
        portable: &["dep:stab-kernels-simd"],
    },
    FeatureContract {
        package: "stab-algebra",
        default: &[],
        portable: &["dep:stab-kernels-simd", "stab-bits/portable-simd"],
    },
    FeatureContract {
        package: "stab-core",
        default: &[],
        portable: &["stab-algebra/portable-simd", "stab-bits/portable-simd"],
    },
    FeatureContract {
        package: "stab-cli",
        default: &[],
        portable: &["stab-bits/portable-simd"],
    },
    FeatureContract {
        package: "stab-oracle",
        default: &[],
        portable: &["stab-core/portable-simd"],
    },
    FeatureContract {
        package: "stab-bench",
        default: &[],
        portable: &["stab-cli/portable-simd", "stab-core/portable-simd"],
    },
];

const FIXTURES: [ConsumerFixture; 4] = [
    ConsumerFixture {
        id: "stable",
        relative_manifest: "test-support/consumers/stable/Cargo.toml",
        toolchain: Some(STABLE_TOOLCHAIN),
        cargo_subcommand: "test",
        portable: false,
        requires_core: false,
    },
    ConsumerFixture {
        id: "scalar-facade",
        relative_manifest: "test-support/consumers/scalar-facade/Cargo.toml",
        toolchain: None,
        cargo_subcommand: "check",
        portable: false,
        requires_core: true,
    },
    ConsumerFixture {
        id: "nightly-facade",
        relative_manifest: "test-support/consumers/nightly-facade/Cargo.toml",
        toolchain: None,
        cargo_subcommand: "check",
        portable: true,
        requires_core: true,
    },
    ConsumerFixture {
        id: "mixed",
        relative_manifest: "test-support/consumers/mixed/Cargo.toml",
        toolchain: None,
        cargo_subcommand: "check",
        portable: true,
        requires_core: true,
    },
];

#[derive(Clone, Copy, Debug)]
struct ConsumerFixture {
    id: &'static str,
    relative_manifest: &'static str,
    toolchain: Option<&'static str>,
    cargo_subcommand: &'static str,
    portable: bool,
    requires_core: bool,
}

#[derive(Clone, Copy, Debug)]
struct FeatureContract {
    package: &'static str,
    default: &'static [&'static str],
    portable: &'static [&'static str],
}

#[derive(Debug)]
pub struct ConsumerCheckSummary {
    fixture_count: usize,
    feature_contract_count: usize,
}

impl ConsumerCheckSummary {
    pub fn print(&self) {
        println!(
            "[{PREFIX}] checked {} explicit feature contracts and {} external component and facade consumer fixtures",
            self.feature_contract_count, self.fixture_count
        );
    }
}

#[derive(Debug, Error)]
pub enum ConsumerCheckError {
    #[error("failed to resolve repository root {path}: {source}")]
    ResolveRoot {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to start Cargo for external consumer {fixture}: {source}")]
    StartCargo {
        fixture: &'static str,
        source: std::io::Error,
    },

    #[error("external consumer {fixture} failed its Cargo validation with status {status}")]
    CargoFailed {
        fixture: &'static str,
        status: std::process::ExitStatus,
    },

    #[error("failed to resolve Cargo metadata for external consumer {fixture}: {source}")]
    Metadata {
        fixture: &'static str,
        source: cargo_metadata::Error,
    },

    #[error("external consumer {fixture} metadata has no resolved dependency graph")]
    MissingResolve { fixture: &'static str },

    #[error(
        "external consumer {fixture} resolved {actual} stab-kernels-simd packages; expected {expected}"
    )]
    KernelCount {
        fixture: &'static str,
        actual: usize,
        expected: usize,
    },

    #[error(
        "external consumer {fixture} resolved package {package} with portable-simd={actual}; expected {expected}"
    )]
    PortableFeature {
        fixture: &'static str,
        package: &'static str,
        actual: bool,
        expected: bool,
    },

    #[error("external consumer {fixture} unexpectedly resolved package {package}")]
    UnexpectedPackage {
        fixture: &'static str,
        package: &'static str,
    },

    #[error("workspace package {package} declares {feature}={actual:?}; expected {expected:?}")]
    FeatureIntent {
        package: &'static str,
        feature: &'static str,
        actual: Vec<String>,
        expected: Vec<String>,
    },

    #[error("workspace metadata does not contain package {0}")]
    MissingWorkspacePackage(&'static str),

    #[error(
        "external consumer {fixture} has publishable workspace packages {packages:?}; every test-support package must set publish = false"
    )]
    PublishableFixturePackages {
        fixture: &'static str,
        packages: Vec<String>,
    },
}

pub fn check_external_consumers(root: &Path) -> Result<ConsumerCheckSummary, ConsumerCheckError> {
    let root = std::fs::canonicalize(root).map_err(|source| ConsumerCheckError::ResolveRoot {
        path: root.to_path_buf(),
        source,
    })?;
    validate_workspace_feature_intent(&root)?;
    for fixture in FIXTURES {
        validate_fixture_execution(&root, fixture)?;
        validate_fixture_metadata(&root, fixture)?;
    }
    Ok(ConsumerCheckSummary {
        fixture_count: FIXTURES.len(),
        feature_contract_count: FEATURE_CONTRACTS.len(),
    })
}

fn validate_workspace_feature_intent(root: &Path) -> Result<(), ConsumerCheckError> {
    let metadata = MetadataCommand::new()
        .current_dir(root)
        .manifest_path(root.join("Cargo.toml"))
        .other_options(vec!["--locked".to_string()])
        .exec()
        .map_err(|source| ConsumerCheckError::Metadata {
            fixture: "workspace-feature-intent",
            source,
        })?;
    for contract in FEATURE_CONTRACTS {
        let package = metadata
            .packages
            .iter()
            .find(|package| package.name.as_str() == contract.package)
            .ok_or(ConsumerCheckError::MissingWorkspacePackage(
                contract.package,
            ))?;
        require_feature_declaration(package, contract.package, "default", contract.default)?;
        require_feature_declaration(
            package,
            contract.package,
            "portable-simd",
            contract.portable,
        )?;
    }
    Ok(())
}

fn require_feature_declaration(
    package: &cargo_metadata::Package,
    package_name: &'static str,
    feature: &'static str,
    expected: &[&str],
) -> Result<(), ConsumerCheckError> {
    let actual = package
        .features
        .get(feature)
        .into_iter()
        .flatten()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let expected = expected
        .iter()
        .map(|value| (*value).to_string())
        .collect::<Vec<_>>();
    if actual != expected {
        return Err(ConsumerCheckError::FeatureIntent {
            package: package_name,
            feature,
            actual,
            expected,
        });
    }
    Ok(())
}

fn validate_fixture_execution(
    root: &Path,
    fixture: ConsumerFixture,
) -> Result<(), ConsumerCheckError> {
    let manifest = root.join(fixture.relative_manifest);
    let target = root
        .join("target")
        .join("architecture-consumers")
        .join(fixture.id);
    let mut command = Command::new("cargo");
    if let Some(toolchain) = fixture.toolchain {
        command.arg(toolchain);
    }
    let status = command
        .arg(fixture.cargo_subcommand)
        .args(["--locked", "--manifest-path"])
        .arg(manifest)
        .arg("--target-dir")
        .arg(target)
        .current_dir(root)
        .status()
        .map_err(|source| ConsumerCheckError::StartCargo {
            fixture: fixture.id,
            source,
        })?;
    if !status.success() {
        return Err(ConsumerCheckError::CargoFailed {
            fixture: fixture.id,
            status,
        });
    }
    Ok(())
}

fn validate_fixture_metadata(
    root: &Path,
    fixture: ConsumerFixture,
) -> Result<(), ConsumerCheckError> {
    let metadata = MetadataCommand::new()
        .current_dir(root)
        .manifest_path(root.join(fixture.relative_manifest))
        .other_options(vec!["--locked".to_string()])
        .exec()
        .map_err(|source| ConsumerCheckError::Metadata {
            fixture: fixture.id,
            source,
        })?;
    require_unpublished_fixture_packages(&metadata, fixture.id)?;
    let features = resolved_features(&metadata, fixture.id)?;
    let kernel_count = features.count("stab-kernels-simd");
    let expected_kernel_count = usize::from(fixture.portable);
    if kernel_count != expected_kernel_count {
        return Err(ConsumerCheckError::KernelCount {
            fixture: fixture.id,
            actual: kernel_count,
            expected: expected_kernel_count,
        });
    }
    for package in ["stab-bits", "stab-algebra"] {
        require_portable_feature(&features, fixture, package, fixture.portable)?;
    }
    if fixture.requires_core {
        require_portable_feature(&features, fixture, "stab-core", fixture.portable)?;
    } else if features.contains("stab-core") {
        return Err(ConsumerCheckError::UnexpectedPackage {
            fixture: fixture.id,
            package: "stab-core",
        });
    }
    Ok(())
}

fn require_unpublished_fixture_packages(
    metadata: &Metadata,
    fixture: &'static str,
) -> Result<(), ConsumerCheckError> {
    let workspace_members = metadata.workspace_members.iter().collect::<BTreeSet<_>>();
    let mut publishable_packages = metadata
        .packages
        .iter()
        .filter(|package| workspace_members.contains(&package.id))
        .filter(|package| !matches!(&package.publish, Some(registries) if registries.is_empty()))
        .map(|package| package.name.to_string())
        .collect::<Vec<_>>();
    publishable_packages.sort();
    if !publishable_packages.is_empty() {
        return Err(ConsumerCheckError::PublishableFixturePackages {
            fixture,
            packages: publishable_packages,
        });
    }
    Ok(())
}

#[derive(Debug)]
struct ResolvedPackages {
    counts: BTreeMap<String, usize>,
    features: BTreeMap<String, BTreeSet<String>>,
}

impl ResolvedPackages {
    fn count(&self, package: &str) -> usize {
        self.counts.get(package).copied().unwrap_or_default()
    }

    fn contains(&self, package: &str) -> bool {
        self.counts.contains_key(package)
    }

    fn has_feature(&self, package: &str, feature: &str) -> bool {
        self.features
            .get(package)
            .is_some_and(|features| features.contains(feature))
    }
}

fn resolved_features(
    metadata: &Metadata,
    fixture: &'static str,
) -> Result<ResolvedPackages, ConsumerCheckError> {
    let resolve = metadata
        .resolve
        .as_ref()
        .ok_or(ConsumerCheckError::MissingResolve { fixture })?;
    let package_names = metadata
        .packages
        .iter()
        .map(|package| (package.id.clone(), package.name.to_string()))
        .collect::<BTreeMap<PackageId, String>>();
    let mut counts = BTreeMap::new();
    let mut features = BTreeMap::<String, BTreeSet<String>>::new();
    for node in &resolve.nodes {
        let Some(name) = package_names.get(&node.id) else {
            continue;
        };
        *counts.entry(name.clone()).or_default() += 1;
        features
            .entry(name.clone())
            .or_default()
            .extend(node.features.iter().map(ToString::to_string));
    }
    Ok(ResolvedPackages { counts, features })
}

fn require_portable_feature(
    features: &ResolvedPackages,
    fixture: ConsumerFixture,
    package: &'static str,
    expected: bool,
) -> Result<(), ConsumerCheckError> {
    let actual = features.has_feature(package, "portable-simd");
    if actual != expected {
        return Err(ConsumerCheckError::PortableFeature {
            fixture: fixture.id,
            package,
            actual,
            expected,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publishable_external_fixture_packages_are_rejected_from_metadata() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/publication-contract-workspace");
        let metadata = MetadataCommand::new()
            .current_dir(&root)
            .manifest_path(root.join("Cargo.toml"))
            .other_options(vec!["--locked".to_owned()])
            .exec()
            .expect("load publication-contract fixture");

        let error = require_unpublished_fixture_packages(&metadata, "publishable-fixture")
            .expect_err("publishable fixture packages should fail");
        match error {
            ConsumerCheckError::PublishableFixturePackages { fixture, packages } => {
                assert_eq!(fixture, "publishable-fixture");
                assert_eq!(
                    packages,
                    ["fixture-ops", "fixture-support", "stab-cli", "stab-model"]
                );
            }
            other => panic!("unexpected fixture-publication error: {other}"),
        }
    }
}
