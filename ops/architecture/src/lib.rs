//! Repository architecture checks for Stab.

#![cfg_attr(
    test,
    allow(
        clippy::expect_used,
        clippy::panic,
        clippy::unwrap_used,
        reason = "unit tests use direct assertions for concise fixture diagnostics"
    )
)]

mod consumer;
mod markdown;
mod metadata;
mod policy;
mod source_scan;
mod workflow_actions;

use std::path::{Path, PathBuf};

use thiserror::Error;

pub use consumer::{ConsumerCheckError, ConsumerCheckSummary, check_external_consumers};
pub use markdown::{DocsCheckError, DocsCheckReport, DocsViolation, check_markdown_docs};
pub use policy::{
    DeclaredPathDependency, DependencyKind, PackageSpec, WorkspaceEdge, WorkspaceGraph,
};

const PREFIX: &str = "stab-architecture";

/// A policy violation found in the workspace.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Violation {
    pub code: &'static str,
    pub message: String,
}

impl Violation {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

/// A pre-existing migration exception recognized by exact identity.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MigrationAllowance {
    pub code: &'static str,
    pub message: String,
}

/// Successful architecture-check counts and reported migration debt.
#[derive(Debug)]
pub struct CheckSummary {
    pub package_count: usize,
    pub dependency_edge_count: usize,
    pub rust_source_count: usize,
    pub workflow_action_count: usize,
    pub migration_allowances: Vec<MigrationAllowance>,
}

impl CheckSummary {
    pub fn print(&self) {
        for allowance in &self.migration_allowances {
            eprintln!(
                "[{PREFIX}] migration allowance [{}]: {}",
                allowance.code, allowance.message
            );
        }
        println!(
            "[{PREFIX}] checked {} workspace packages, {} workspace dependency edges, {} Rust source files, and {} workflow action uses",
            self.package_count,
            self.dependency_edge_count,
            self.rust_source_count,
            self.workflow_action_count
        );
    }
}

/// A completed architecture check containing all deterministic violations.
#[derive(Debug)]
pub struct CheckReport {
    pub summary: CheckSummary,
    pub violations: Vec<Violation>,
}

impl CheckReport {
    pub fn passed(&self) -> bool {
        self.violations.is_empty()
    }

    pub fn print(&self) {
        self.summary.print();
        for violation in &self.violations {
            eprintln!(
                "[{PREFIX}] violation [{}]: {}",
                violation.code, violation.message
            );
        }
    }
}

/// Errors that prevent the architecture policy from being evaluated.
#[derive(Debug, Error)]
pub enum CheckError {
    #[error("failed to resolve repository root {path}: {source}")]
    ResolveRoot {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to read Cargo workspace metadata: {0}")]
    Metadata(#[from] cargo_metadata::Error),

    #[error("Cargo metadata did not contain a dependency graph")]
    MissingResolve,

    #[error("workspace package {package} has no manifest parent")]
    MissingManifestParent { package: String },

    #[error("workspace package {package} is outside repository root {root}: {path}")]
    PackageOutsideRoot {
        package: String,
        root: PathBuf,
        path: PathBuf,
    },

    #[error("workspace dependency graph references unknown package id {0}")]
    UnknownPackageId(String),

    #[error("failed to inspect architecture source path {path}: {source}")]
    InspectSource {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to read architecture source file {path}: {source}")]
    ReadSource {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to inspect workflow directory {path}: {source}")]
    InspectWorkflowDirectory {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to read workflow file {path}: {source}")]
    ReadWorkflow {
        path: PathBuf,
        source: std::io::Error,
    },
}

/// Checks the Cargo dependency graph and direct portable-SIMD source boundary.
pub fn check_workspace(root: &Path) -> Result<CheckReport, CheckError> {
    let root = std::fs::canonicalize(root).map_err(|source| CheckError::ResolveRoot {
        path: root.to_path_buf(),
        source,
    })?;
    let graph = metadata::load_workspace_graph(&root)?;
    let mut policy_report = policy::validate_graph(&graph);
    let source_report = source_scan::scan_workspace_sources(&root, &graph.packages)?;
    let workflow_report = workflow_actions::scan_workflow_actions(&root)?;

    policy_report.violations.extend(source_report.violations);
    policy_report.violations.extend(workflow_report.violations);
    policy_report
        .migration_allowances
        .extend(source_report.migration_allowances);
    policy_report.violations.sort();
    policy_report.violations.dedup();
    policy_report.migration_allowances.sort();
    policy_report.migration_allowances.dedup();

    Ok(CheckReport {
        summary: CheckSummary {
            package_count: graph.packages.len(),
            dependency_edge_count: graph.edges.len(),
            rust_source_count: source_report.rust_source_count,
            workflow_action_count: workflow_report.action_use_count,
            migration_allowances: policy_report.migration_allowances,
        },
        violations: policy_report.violations,
    })
}

#[cfg(test)]
mod fixture_tests;
