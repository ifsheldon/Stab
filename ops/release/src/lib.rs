//! Release preflight and artifact packaging for Stab.

#![cfg_attr(
    test,
    allow(
        clippy::expect_used,
        clippy::unwrap_used,
        reason = "unit tests use concise fixture assertions"
    )
)]

mod artifact;
mod error;
mod package;
mod repository;
mod workspace;

use std::path::{Path, PathBuf};

pub use error::ReleaseError;

pub const RELEASE_VERSION: &str = "0.2.0";
pub const RELEASE_TAG: &str = "v0.2.0";
pub const PRODUCT_PACKAGE_ORDER: &[&str] = &[
    "stab-kernels-simd",
    "stab-model",
    "stab-bits",
    "stab-records",
    "stab-algebra",
    "stab-analysis",
    "stab-decoder",
    "stab-engine",
    "stab-core",
    "stab-cli",
];

pub fn check(output: &Path) -> Result<(), ReleaseError> {
    let root = repository_root()?;
    package::check(&root, output)?;
    Ok(())
}

pub fn print_publish_order() -> Result<(), ReleaseError> {
    let root = repository_root()?;
    let workspace = workspace::inspect(&root)?;
    for (index, package) in workspace.packages.iter().enumerate() {
        println!("{}. {} {}", index + 1, package.name, package.version);
    }
    Ok(())
}

pub fn package_binary(
    binary: &Path,
    target: &str,
    output: &Path,
    tag: &str,
) -> Result<(), ReleaseError> {
    let root = repository_root()?;
    repository::require_clean_tag(&root, tag)?;
    let packaged = artifact::package_binary(&root, binary, target, output)?;
    println!(
        "[stab-release] wrote {} and {}",
        packaged.binary.display(),
        packaged.checksum.display()
    );
    Ok(())
}

fn repository_root() -> Result<PathBuf, ReleaseError> {
    let candidate = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::canonicalize(&candidate).map_err(|source| ReleaseError::ResolveRoot {
        path: candidate,
        source,
    })
}
