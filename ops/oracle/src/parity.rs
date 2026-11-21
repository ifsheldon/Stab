use std::collections::BTreeMap;
use std::path::Path;

use clap::Subcommand;
use thiserror::Error;

use crate::safe_file::{self, SafeFileError};
use crate::{OracleError, RepoRoot};

mod commands;
mod expected;
mod render;
mod routes;
mod runner;
mod schema;
mod source;
mod validate;

use commands::{is_expected_command_option, validate_command_surfaces};
#[cfg(test)]
use expected::parse_stim_gate_doc;
use expected::{
    EXPECTED_COMMAND_SURFACES, EXPECTED_COMMANDS, EXPECTED_FORMAT_ROUTES, EXPECTED_FORMATS,
    ExpectedCoverage, GENERATED_DOC_PATH, LEDGER_PATH, LEDGER_SCHEMA_VERSION,
    MAX_GENERATED_DOC_BYTES, MAX_LEDGER_BYTES, PRODUCT_OWNERS, TEST_PACKAGES, expected_coverage,
};
#[cfg(test)]
use render::render;
use render::render_document;
use routes::validate_format_routes;
#[cfg(test)]
use routes::{expected_format_route_exceptions, expected_format_route_shape};
use runner::{check_owner_selectors, run_owner_tests};
#[cfg(test)]
use runner::{collect_owner_tests, require_one_listing_match};
use schema::{
    Area, CargoTestTarget, CommandSurface, DetsObservableOrder, Disposition, DivergenceKind,
    Evidence, EvidenceStatus, Family, FormatRoute, FormatRouteRole, Ledger, Milestone, RecordType,
    RouteOutcome, Status, StimIdentity, TestOwner,
};
use validate::{is_slug, is_test_name, validate, validate_stim_reference};

#[derive(Clone, Debug, Subcommand)]
pub(crate) enum Command {
    /// Validate the source-owned parity ledger and print its summary.
    Check,
    /// Run all canonical owner tests.
    Run,
    /// Render the generated parity document or check that it is current.
    Render {
        #[arg(long)]
        check: bool,
    },
}

#[derive(Debug, Error)]
pub(crate) enum ParityError {
    #[error("failed to {action} {path}: {source}")]
    SafeFile {
        action: &'static str,
        path: Box<Path>,
        source: SafeFileError,
    },
    #[error("failed to parse {path}: {source}")]
    Parse {
        path: Box<Path>,
        source: toml::de::Error,
    },
    #[error("parity ledger validation failed:\n{0}")]
    InvalidLedger(Box<str>),
    #[error("generated parity document differs from {0}")]
    GeneratedDocumentDiffers(Box<Path>),
}

pub(crate) fn run(root: &RepoRoot, command: Command) -> Result<(), OracleError> {
    let ledger = read_and_validate(root)?;
    match command {
        Command::Check => {
            check_owner_selectors(root, &ledger)?;
            print_summary(&ledger);
        }
        Command::Run => run_owner_tests(root, &ledger)?,
        Command::Render { check } => render_document(root, &ledger, check)?,
    }
    Ok(())
}

fn read_and_validate(root: &RepoRoot) -> Result<Ledger, ParityError> {
    let ledger = source::read(root)?;
    let expected = expected_coverage(root)?;
    validate(root, &ledger, &expected)?;
    Ok(ledger)
}

fn read_regular_file_bounded(path: &Path, limit: usize) -> Result<Vec<u8>, ParityError> {
    safe_file::read_regular_file_bounded(path, limit).map_err(|source| ParityError::SafeFile {
        action: "read",
        path: path.to_path_buf().into_boxed_path(),
        source,
    })
}

fn print_summary(ledger: &Ledger) {
    let mut counts = BTreeMap::<Status, usize>::new();
    let mut evidence_counts = BTreeMap::<EvidenceStatus, usize>::new();
    for family in &ledger.families {
        *counts.entry(family.status()).or_default() += 1;
        *evidence_counts.entry(family.evidence_status()).or_default() += 1;
    }
    println!(
        "[stab-oracle] parity schema={} stim={} commit={} families={} done={} missing={} deferred={} divergence={} verified={} needs_owner={}",
        ledger.schema_version,
        ledger.stim.version,
        ledger.stim.commit,
        ledger.families.len(),
        counts.get(&Status::Done).copied().unwrap_or_default(),
        counts.get(&Status::Missing).copied().unwrap_or_default(),
        counts.get(&Status::Deferred).copied().unwrap_or_default(),
        counts.get(&Status::Divergence).copied().unwrap_or_default(),
        evidence_counts
            .get(&EvidenceStatus::Verified)
            .copied()
            .unwrap_or_default(),
        evidence_counts
            .get(&EvidenceStatus::NeedsOwner)
            .copied()
            .unwrap_or_default(),
    );
}

fn invalid(message: String) -> ParityError {
    ParityError::InvalidLedger(message.into_boxed_str())
}

#[cfg(test)]
mod tests;
