use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use cargo_metadata::{Metadata, MetadataCommand, PackageId};
use thiserror::Error;

use crate::policy::product_contract;

const PREFIX: &str = "stab-architecture";
const KERNEL_PACKAGE: &str = "stab-kernels-simd";
const FACADE_PACKAGE: &str = "stab-core";

const FIXTURES: [ConsumerFixture; 3] = [
    ConsumerFixture {
        id: "stable",
        relative_manifest: "test-support/consumers/stable/Cargo.toml",
        requires_core: false,
    },
    ConsumerFixture {
        id: "facade",
        relative_manifest: "test-support/consumers/facade/Cargo.toml",
        requires_core: true,
    },
    ConsumerFixture {
        id: "mixed",
        relative_manifest: "test-support/consumers/mixed/Cargo.toml",
        requires_core: true,
    },
];

#[derive(Clone, Copy, Debug)]
struct ConsumerFixture {
    id: &'static str,
    relative_manifest: &'static str,
    requires_core: bool,
}

#[derive(Debug)]
pub struct ConsumerCheckSummary {
    fixture_count: usize,
}

impl ConsumerCheckSummary {
    pub fn print(&self) {
        println!(
            "[{PREFIX}] checked {} external component and facade consumer fixtures, each resolving exactly one {KERNEL_PACKAGE} package",
            self.fixture_count
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

    #[error("external consumer {fixture} did not resolve required package {package}")]
    MissingPackage {
        fixture: &'static str,
        package: &'static str,
    },

    #[error("external consumer {fixture} unexpectedly resolved package {package}")]
    UnexpectedPackage {
        fixture: &'static str,
        package: &'static str,
    },

    #[error(
        "external consumer {fixture} resolved product package {package} from {location}; every product package must be a path package at crates/{package} in this repository"
    )]
    ForeignProductPackage {
        fixture: &'static str,
        package: String,
        location: String,
    },

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
    for fixture in FIXTURES {
        validate_fixture_execution(&root, fixture)?;
        validate_fixture_metadata(&root, fixture)?;
    }
    Ok(ConsumerCheckSummary {
        fixture_count: FIXTURES.len(),
    })
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
    let status = Command::new("cargo")
        .arg("test")
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
    let packages = resolved_packages(root, &metadata, fixture.id)?;
    validate_resolved_packages(fixture, &packages)
}

fn validate_resolved_packages(
    fixture: ConsumerFixture,
    packages: &ResolvedPackages,
) -> Result<(), ConsumerCheckError> {
    if let Some(foreign) = packages.foreign_products.first() {
        return Err(ConsumerCheckError::ForeignProductPackage {
            fixture: fixture.id,
            package: foreign.name.clone(),
            location: foreign.location.clone(),
        });
    }
    let kernel_count = packages.count(KERNEL_PACKAGE);
    if kernel_count != 1 {
        return Err(ConsumerCheckError::KernelCount {
            fixture: fixture.id,
            actual: kernel_count,
            expected: 1,
        });
    }
    match (fixture.requires_core, packages.contains(FACADE_PACKAGE)) {
        (true, false) => Err(ConsumerCheckError::MissingPackage {
            fixture: fixture.id,
            package: FACADE_PACKAGE,
        }),
        (false, true) => Err(ConsumerCheckError::UnexpectedPackage {
            fixture: fixture.id,
            package: FACADE_PACKAGE,
        }),
        (true, true) | (false, false) => Ok(()),
    }
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
    foreign_products: Vec<ForeignProduct>,
}

/// A resolved package that carries a product name but is not this repository's
/// path package, so name counts alone would accept a shadowing copy.
#[derive(Debug)]
struct ForeignProduct {
    name: String,
    location: String,
}

impl ResolvedPackages {
    fn count(&self, package: &str) -> usize {
        self.counts.get(package).copied().unwrap_or_default()
    }

    fn contains(&self, package: &str) -> bool {
        self.counts.contains_key(package)
    }
}

fn resolved_packages(
    root: &Path,
    metadata: &Metadata,
    fixture: &'static str,
) -> Result<ResolvedPackages, ConsumerCheckError> {
    let resolve = metadata
        .resolve
        .as_ref()
        .ok_or(ConsumerCheckError::MissingResolve { fixture })?;
    let packages = metadata
        .packages
        .iter()
        .map(|package| (package.id.clone(), package))
        .collect::<BTreeMap<PackageId, &cargo_metadata::Package>>();
    let mut counts = BTreeMap::new();
    let mut foreign_products = Vec::new();
    for node in &resolve.nodes {
        let Some(package) = packages.get(&node.id) else {
            continue;
        };
        let name = package.name.to_string();
        if product_contract(&name).is_some() && !is_repository_product(root, package) {
            foreign_products.push(ForeignProduct {
                location: package
                    .source
                    .as_ref()
                    .map_or_else(|| package.manifest_path.to_string(), ToString::to_string),
                name: name.clone(),
            });
        }
        *counts.entry(name).or_default() += 1;
    }
    foreign_products
        .sort_by(|left, right| (&left.name, &left.location).cmp(&(&right.name, &right.location)));
    Ok(ResolvedPackages {
        counts,
        foreign_products,
    })
}

fn is_repository_product(root: &Path, package: &cargo_metadata::Package) -> bool {
    if package.source.is_some() {
        return false;
    }
    let expected = root.join("crates").join(package.name.as_str());
    package
        .manifest_path
        .parent()
        .and_then(|directory| std::fs::canonicalize(directory.as_std_path()).ok())
        .is_some_and(|directory| directory == expected)
}

#[cfg(test)]
mod tests {
    use std::assert_matches;

    use super::*;

    fn resolved(packages: &[(&str, usize)]) -> ResolvedPackages {
        ResolvedPackages {
            counts: packages
                .iter()
                .map(|(name, count)| ((*name).to_owned(), *count))
                .collect(),
            foreign_products: Vec::new(),
        }
    }

    #[test]
    fn every_consumer_graph_requires_exactly_one_kernel_package() {
        for fixture in FIXTURES {
            let mut valid = vec![("stab-bits", 1), (KERNEL_PACKAGE, 1)];
            if fixture.requires_core {
                valid.push((FACADE_PACKAGE, 1));
            }
            validate_resolved_packages(fixture, &resolved(&valid))
                .unwrap_or_else(|error| panic!("{} should pass: {error}", fixture.id));

            for kernel_count in [0, 2] {
                let mut invalid = valid.clone();
                invalid.retain(|(name, _)| *name != KERNEL_PACKAGE);
                if kernel_count > 0 {
                    invalid.push((KERNEL_PACKAGE, kernel_count));
                }
                match validate_resolved_packages(fixture, &resolved(&invalid)) {
                    Err(ConsumerCheckError::KernelCount {
                        fixture: id,
                        actual,
                        expected: 1,
                    }) => {
                        assert_eq!(id, fixture.id);
                        assert_eq!(actual, kernel_count);
                    }
                    other => panic!(
                        "{} with {kernel_count} kernel packages should fail: {other:?}",
                        fixture.id
                    ),
                }
            }
        }
    }

    #[test]
    fn consumer_graphs_must_match_their_facade_expectation() {
        for fixture in FIXTURES {
            let mut flipped = vec![(KERNEL_PACKAGE, 1)];
            if !fixture.requires_core {
                flipped.push((FACADE_PACKAGE, 1));
            }
            let error = validate_resolved_packages(fixture, &resolved(&flipped))
                .expect_err("facade presence must match the fixture contract");
            assert_matches!(
                (fixture.requires_core, &error),
                (true, ConsumerCheckError::MissingPackage { .. })
                    | (false, ConsumerCheckError::UnexpectedPackage { .. }),
                "{}: {error}",
                fixture.id,
            );
        }
    }

    #[test]
    fn consumers_reject_product_packages_that_are_not_repository_path_packages() {
        let root = std::fs::canonicalize(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/external-product-copy-workspace"),
        )
        .expect("resolve external-product-copy fixture");
        let metadata = MetadataCommand::new()
            .current_dir(&root)
            .manifest_path(root.join("Cargo.toml"))
            .other_options(vec!["--locked".to_owned()])
            .exec()
            .expect("load external-product-copy fixture");

        let packages = resolved_packages(&root, &metadata, "shadowed").expect("resolve packages");
        assert_eq!(
            packages.count("stab-core"),
            2,
            "name counts alone cannot tell the copies apart"
        );
        let [foreign] = packages.foreign_products.as_slice() else {
            panic!(
                "only the external copy should be foreign: {:?}",
                packages.foreign_products
            );
        };
        assert_eq!(foreign.name, "stab-core");
        assert!(
            foreign.location.contains("external/stab-core"),
            "{}",
            foreign.location
        );

        let fixture = ConsumerFixture {
            id: "shadowed",
            relative_manifest: "Cargo.toml",
            requires_core: true,
        };
        match validate_resolved_packages(fixture, &packages) {
            Err(ConsumerCheckError::ForeignProductPackage { package, .. }) => {
                assert_eq!(package, "stab-core");
            }
            other => panic!("foreign product copy should fail first: {other:?}"),
        }
    }

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
