//! The coverage the ledger must declare: fixed dimensions plus the gates and aliases pinned
//! Stim documents.

use std::collections::BTreeSet;

use super::{ParityError, invalid, read_regular_file_bounded};
use crate::RepoRoot;

pub(super) const LEDGER_PATH: &str = "oracle/stim-v1.16-parity.toml";
pub(super) const GENERATED_DOC_PATH: &str = "docs/stim-parity.md";
pub(super) const STIM_GATE_DOC_PATH: &str = "doc/gates.md";
pub(super) const LEDGER_SCHEMA_VERSION: u32 = 3;
pub(super) const MAX_LEDGER_BYTES: usize = 2 * 1024 * 1024;
pub(super) const MAX_GENERATED_DOC_BYTES: usize = 4 * 1024 * 1024;
pub(super) const PRODUCT_OWNERS: [&str; 10] = [
    "stab-algebra",
    "stab-analysis",
    "stab-bits",
    "stab-cli",
    "stab-decoder",
    "stab-engine",
    "stab-foundation",
    "stab-kernels-simd",
    "stab-model",
    "stab-records",
];
pub(super) const TEST_PACKAGES: [&str; 15] = [
    "stab-algebra",
    "stab-analysis",
    "stab-bits",
    "stab-cli",
    "stab-core",
    "stab-decoder",
    "stab-engine",
    "stab-foundation",
    "stab-kernels-simd",
    "stab-model",
    "stab-oracle",
    "stab-records",
    "stab-reference-decoder",
    "stab-reference-noise-pass",
    "stab-compat-corpus",
];
pub(super) const EXPECTED_DIALECTS: [&str; 2] = ["stim", "dem"];
pub(super) const EXPECTED_FORMATS: [&str; 6] = ["01", "b8", "r8", "hits", "dets", "ptb64"];
pub(super) const EXPECTED_COMMANDS: [&str; 11] = [
    "analyze_errors",
    "convert",
    "detect",
    "diagram",
    "explain_errors",
    "gen",
    "help",
    "m2d",
    "repl",
    "sample",
    "sample_dem",
];
pub(super) const EXPECTED_COMMAND_SURFACES: [&str; 7] = [
    "analyze_errors",
    "convert",
    "detect",
    "gen",
    "m2d",
    "sample",
    "sample_dem",
];
pub(super) const EXPECTED_FORMAT_ROUTES: [&str; 14] = [
    "convert-input",
    "convert-observable-output",
    "convert-output",
    "detect-observable-output",
    "detect-output",
    "m2d-measurement-input",
    "m2d-observable-output",
    "m2d-output",
    "m2d-sweep-input",
    "sample-dem-error-output",
    "sample-dem-observable-output",
    "sample-dem-output",
    "sample-dem-replay-input",
    "sample-output",
];
pub(super) const EXPECTED_TARGET_FAMILIES: [&str; 8] = [
    "combiner",
    "inverted-pauli",
    "inverted-qubit",
    "measurement-record",
    "pad-bit",
    "pauli",
    "qubit",
    "sweep-bit",
];
pub(super) const EXPECTED_CONSUMERS: [&str; 5] =
    ["analysis", "detection", "flow", "sampling", "transform"];
pub(super) const EXPECTED_STABLE_CAPABILITIES: [&str; 10] = [
    "circuit-approximate-equality",
    "circuit-determined-measurements",
    "circuit-reference-sample",
    "circuit-reference-signs",
    "circuit-simplification",
    "circuit-unitary-inverse",
    "dem-approximate-equality",
    "gate-flow-metadata",
    "gate-tableau-metadata",
    "gate-unitary-metadata",
];
pub(super) const EXPECTED_EXTENSIONS: [&str; 8] = [
    "agent-capabilities",
    "agent-inspect",
    "agent-plan",
    "circuit-pass",
    "decoder-session",
    "json-diagnostics",
    "memory-budget",
    "stim-canonical-convert",
];
pub(super) const EXPECTED_OBSOLETE_SURFACES: [&str; 7] = [
    "analyze-detector-hypergraph",
    "detect-prepend-observables",
    "help-frame0",
    "legacy-dispatch",
    "sample-dem-append-observables",
    "sample-dem-prepend-observables",
    "sample-frame0",
];

#[derive(Debug)]
pub(super) struct ExpectedCoverage {
    pub(super) members: BTreeSet<String>,
    pub(super) canonical_gates: BTreeSet<String>,
    pub(super) aliases: BTreeSet<String>,
}

pub(super) fn expected_coverage(root: &RepoRoot) -> Result<ExpectedCoverage, ParityError> {
    let path = root.stim_source().join(STIM_GATE_DOC_PATH);
    let bytes = read_regular_file_bounded(&path, MAX_LEDGER_BYTES)?;
    let text = std::str::from_utf8(&bytes).map_err(|error| {
        ParityError::InvalidLedger(
            format!("{} is not UTF-8: {error}", path.display()).into_boxed_str(),
        )
    })?;
    let (canonical_gates, aliases) = parse_stim_gate_doc(text)?;
    let mut members = BTreeSet::new();
    extend_dimension(&mut members, "dialect", EXPECTED_DIALECTS);
    extend_dimension(&mut members, "format", EXPECTED_FORMATS);
    extend_dimension(&mut members, "command", EXPECTED_COMMANDS);
    extend_dimension(&mut members, "consumer", EXPECTED_CONSUMERS);
    extend_dimension(&mut members, "target", EXPECTED_TARGET_FAMILIES);
    extend_dimension(&mut members, "capability", EXPECTED_STABLE_CAPABILITIES);
    extend_dimension(&mut members, "extension", EXPECTED_EXTENSIONS);
    extend_dimension(&mut members, "obsolete", EXPECTED_OBSOLETE_SURFACES);
    extend_dimension(
        &mut members,
        "gate",
        canonical_gates.iter().map(String::as_str),
    );
    extend_dimension(&mut members, "alias", aliases.iter().map(String::as_str));
    Ok(ExpectedCoverage {
        members,
        canonical_gates,
        aliases,
    })
}

pub(super) fn extend_dimension<'a, I>(members: &mut BTreeSet<String>, dimension: &str, values: I)
where
    I: IntoIterator<Item = &'a str>,
{
    members.extend(
        values
            .into_iter()
            .map(|value| format!("{dimension}:{value}")),
    );
}

pub(super) fn parse_stim_gate_doc(
    text: &str,
) -> Result<(BTreeSet<String>, BTreeSet<String>), ParityError> {
    let mut gates = BTreeSet::new();
    let mut aliases = BTreeSet::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("### The '")
            && let Some((name, suffix)) = rest.split_once('\'')
            && (suffix == " Gate" || suffix == " Instruction")
            && !gates.insert(name.to_string())
        {
            return Err(invalid(format!(
                "pinned Stim gate documentation repeats canonical gate {name}"
            )));
        }
        if let Some(rest) = line.strip_prefix("Alternate name:")
            && let Some((_, after_open)) = rest.split_once('`')
            && let Some((name, _)) = after_open.split_once('`')
            && !aliases.insert(name.to_string())
        {
            return Err(invalid(format!(
                "pinned Stim gate documentation repeats alias {name}"
            )));
        }
    }
    if gates.len() != 81 {
        return Err(invalid(format!(
            "pinned Stim gate documentation yielded {} canonical gates instead of 81",
            gates.len()
        )));
    }
    if aliases.len() != 12 {
        return Err(invalid(format!(
            "pinned Stim gate documentation yielded {} aliases instead of 12",
            aliases.len()
        )));
    }
    Ok((gates, aliases))
}
