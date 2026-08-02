//! Release preflight and artifact packaging for Stab.

#![cfg_attr(
    test,
    allow(
        clippy::expect_used,
        clippy::unwrap_used,
        reason = "unit tests use concise fixture assertions"
    )
)]

mod archive;
mod artifact;
mod authorization;
mod cancellation;
mod cargo;
mod error;
mod github;
mod package;
mod process;
mod publish;
mod registry;
mod repository;
mod safe_fs;
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

#[doc(hidden)]
#[allow(
    clippy::too_many_arguments,
    reason = "hidden exec boundary mirrors its typed CLI fields"
)]
pub fn execute_isolated_cargo(
    cargo: &Path,
    rustc: &Path,
    rustdoc: &Path,
    home: &Path,
    cargo_home: &Path,
    target: &Path,
    temporary: &Path,
    config: &Path,
    cargo_args: &[std::ffi::OsString],
) -> Result<(), ReleaseError> {
    cargo::execute_isolated_cargo(
        cargo, rustc, rustdoc, home, cargo_home, target, temporary, config, cargo_args,
    )
}

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

pub fn publish_reviewed(preflight: &Path, confirmation: &str) -> Result<(), ReleaseError> {
    let root = repository_root()?;
    publish::publish_reviewed(&root, preflight, confirmation)
}

pub fn build_binary(target: &str, output: &Path, tag: &str) -> Result<(), ReleaseError> {
    let root = repository_root()?;
    let packaged = artifact::build_binary(&root, target, output, tag)?;
    println!(
        "[stab-release] wrote {}, {}, and {}",
        packaged.binary.display(),
        packaged.checksum.display(),
        packaged.manifest.display()
    );
    Ok(())
}

pub fn verify_assets(assets: &Path, tag: &str) -> Result<(), ReleaseError> {
    let root = repository_root()?;
    artifact::verify_assets(&root, assets, tag)?;
    println!(
        "[stab-release] verified release assets in {}",
        assets.display()
    );
    Ok(())
}

pub fn create_verified_draft(
    assets: &Path,
    tag: &str,
    confirmation: &str,
) -> Result<(), ReleaseError> {
    let root = repository_root()?;
    github::create_verified_draft(&root, assets, tag, confirmation)?;
    println!("[stab-release] created and verified private draft {tag} with exact reviewed assets");
    Ok(())
}

fn repository_root() -> Result<PathBuf, ReleaseError> {
    let candidate = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::canonicalize(&candidate).map_err(|source| ReleaseError::ResolveRoot {
        path: candidate,
        source,
    })
}
