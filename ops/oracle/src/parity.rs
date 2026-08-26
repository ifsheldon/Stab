use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path};

use clap::{Subcommand, ValueEnum};
use serde::Deserialize;
use thiserror::Error;

use crate::safe_file::{self, SafeFileError};
use crate::{OracleError, RepoRoot, STIM_COMMIT, STIM_TAG};

mod commands;
mod render;
mod routes;
mod runner;
mod source;

use commands::{is_expected_command_option, validate_command_surfaces};
#[cfg(test)]
use render::render;
use render::render_document;
use routes::validate_format_routes;
#[cfg(test)]
use routes::{expected_format_route_exceptions, expected_format_route_shape};
use runner::{check_owner_selectors, run_owner_tests};
#[cfg(test)]
use runner::{collect_owner_tests, require_one_listing_match};

const LEDGER_PATH: &str = "oracle/stim-v1.16-parity.toml";
const GENERATED_DOC_PATH: &str = "docs/stim-parity.md";
const STIM_GATE_DOC_PATH: &str = "doc/gates.md";
const LEDGER_SCHEMA_VERSION: u32 = 1;
const MAX_LEDGER_BYTES: usize = 2 * 1024 * 1024;
const MAX_GENERATED_DOC_BYTES: usize = 4 * 1024 * 1024;
const PRODUCT_OWNERS: [&str; 10] = [
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
const TEST_PACKAGES: [&str; 14] = [
    "stab-algebra",
    "stab-analysis",
    "stab-bits",
    "stab-cli",
    "stab-core",
    "stab-decoder",
    "stab-engine",
    "stab-kernels-simd",
    "stab-model",
    "stab-oracle",
    "stab-records",
    "stab-reference-decoder",
    "stab-reference-noise-pass",
    "stab-compat-corpus",
];
const EXPECTED_DIALECTS: [&str; 2] = ["stim", "dem"];
const EXPECTED_FORMATS: [&str; 6] = ["01", "b8", "r8", "hits", "dets", "ptb64"];
const EXPECTED_COMMANDS: [&str; 11] = [
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
const EXPECTED_COMMAND_SURFACES: [&str; 7] = [
    "analyze_errors",
    "convert",
    "detect",
    "gen",
    "m2d",
    "sample",
    "sample_dem",
];
const EXPECTED_FORMAT_ROUTES: [&str; 14] = [
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
const EXPECTED_TARGET_FAMILIES: [&str; 8] = [
    "combiner",
    "inverted-pauli",
    "inverted-qubit",
    "measurement-record",
    "pad-bit",
    "pauli",
    "qubit",
    "sweep-bit",
];
const EXPECTED_CONSUMERS: [&str; 5] = ["analysis", "detection", "flow", "sampling", "transform"];
const EXPECTED_STABLE_CAPABILITIES: [&str; 10] = [
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
const EXPECTED_EXTENSIONS: [&str; 7] = [
    "agent-capabilities",
    "agent-inspect",
    "agent-plan",
    "circuit-pass",
    "decoder-session",
    "json-diagnostics",
    "stim-canonical-convert",
];
const EXPECTED_OBSOLETE_SURFACES: [&str; 7] = [
    "analyze-detector-hypergraph",
    "detect-prepend-observables",
    "help-frame0",
    "legacy-dispatch",
    "sample-dem-append-observables",
    "sample-dem-prepend-observables",
    "sample-frame0",
];

#[derive(Clone, Debug, Subcommand)]
pub(crate) enum Command {
    /// Validate the source-owned parity ledger and print its summary.
    Check,
    /// Run canonical owner tests for the selected tier.
    Run {
        #[arg(long, value_enum)]
        tier: Tier,
    },
    /// Render the generated parity document or check that it is current.
    Render {
        #[arg(long)]
        check: bool,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Tier {
    Pr,
    Full,
    Soak,
}

impl Tier {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Pr => "pr",
            Self::Full => "full",
            Self::Soak => "soak",
        }
    }
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Ledger {
    schema_version: u32,
    stim: StimIdentity,
    command_surfaces: Vec<CommandSurface>,
    format_routes: Vec<FormatRoute>,
    families: Vec<Family>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CommandSurface {
    command: String,
    options: Vec<String>,
    stim_refs: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StimIdentity {
    version: String,
    commit: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FormatRoute {
    id: String,
    command: String,
    role: FormatRouteRole,
    record_types: Vec<RecordType>,
    accepted_formats: Vec<String>,
    #[serde(default)]
    rejected_formats: Vec<String>,
    #[serde(default)]
    stim_bug_divergences: Vec<String>,
    #[serde(default)]
    dets_observable_order: Option<DetsObservableOrder>,
    stim_refs: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "kebab-case")]
enum FormatRouteRole {
    Input,
    Output,
    SideOutput,
    ReplayInput,
}

impl FormatRouteRole {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::Output => "output",
            Self::SideOutput => "side output",
            Self::ReplayInput => "replay input",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
enum RecordType {
    M,
    D,
    L,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum DetsObservableOrder {
    PrependByDefault,
}

impl DetsObservableOrder {
    const fn as_str(self) -> &'static str {
        match self {
            Self::PrependByDefault => "prepend by default",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RouteOutcome {
    Accepted,
    Rejected,
    StimBugDivergence,
}

impl RecordType {
    const fn as_str(self) -> &'static str {
        match self {
            Self::M => "M",
            Self::D => "D",
            Self::L => "L",
        }
    }
}

#[derive(Debug, Deserialize)]
struct Family {
    id: String,
    area: Area,
    contract: String,
    stim_refs: Vec<String>,
    #[serde(default)]
    coverage: Vec<String>,
    #[serde(flatten)]
    disposition: Disposition,
}

impl Family {
    const fn status(&self) -> Status {
        match &self.disposition {
            Disposition::Done { .. } => Status::Done,
            Disposition::Missing { .. } => Status::Missing,
            Disposition::Deferred { .. } => Status::Deferred,
            Disposition::Divergence { .. } => Status::Divergence,
        }
    }

    fn owner(&self) -> Option<&str> {
        match &self.disposition {
            Disposition::Done { owner, .. }
            | Disposition::Missing { owner, .. }
            | Disposition::Divergence { owner, .. } => Some(owner),
            Disposition::Deferred { .. } => None,
        }
    }

    const fn test(&self) -> Option<&TestOwner> {
        match &self.disposition {
            Disposition::Done {
                evidence: Evidence::Verified { test, .. },
                ..
            }
            | Disposition::Divergence {
                evidence: Evidence::Verified { test, .. },
                ..
            } => Some(test),
            Disposition::Done {
                evidence: Evidence::NeedsOwner { .. },
                ..
            }
            | Disposition::Divergence {
                evidence: Evidence::NeedsOwner { .. },
                ..
            }
            | Disposition::Missing { .. }
            | Disposition::Deferred { .. } => None,
        }
    }

    const fn stim_reproduction(&self) -> Option<&TestOwner> {
        match &self.disposition {
            Disposition::Divergence {
                evidence:
                    Evidence::Verified {
                        stim_reproduction: Some(test),
                        ..
                    },
                ..
            } => Some(test),
            _ => None,
        }
    }

    const fn evidence_status(&self) -> EvidenceStatus {
        match &self.disposition {
            Disposition::Done {
                evidence: Evidence::Verified { .. },
                ..
            }
            | Disposition::Divergence {
                evidence: Evidence::Verified { .. },
                ..
            } => EvidenceStatus::Verified,
            Disposition::Done {
                evidence: Evidence::NeedsOwner { .. },
                ..
            }
            | Disposition::Divergence {
                evidence: Evidence::NeedsOwner { .. },
                ..
            } => EvidenceStatus::NeedsOwner,
            Disposition::Missing { .. } | Disposition::Deferred { .. } => {
                EvidenceStatus::NotApplicable
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "kebab-case")]
enum Area {
    Algebra,
    Analysis,
    CircuitModel,
    Cli,
    DemModel,
    Detection,
    GateCatalog,
    Generation,
    ProductSurface,
    ResourceSafety,
    ResultFormats,
    Sampling,
    Search,
    StabExtensions,
    Transforms,
}

impl Area {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Algebra => "algebra",
            Self::Analysis => "analysis",
            Self::CircuitModel => "circuit-model",
            Self::Cli => "cli",
            Self::DemModel => "dem-model",
            Self::Detection => "detection",
            Self::GateCatalog => "gate-catalog",
            Self::Generation => "generation",
            Self::ProductSurface => "product-surface",
            Self::ResourceSafety => "resource-safety",
            Self::ResultFormats => "result-formats",
            Self::Sampling => "sampling",
            Self::Search => "search",
            Self::StabExtensions => "stab-extensions",
            Self::Transforms => "transforms",
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "status", rename_all = "kebab-case", deny_unknown_fields)]
enum Disposition {
    Done {
        owner: String,
        evidence: Evidence,
    },
    Missing {
        owner: String,
        milestone: Milestone,
    },
    Deferred {
        rationale: String,
    },
    Divergence {
        owner: String,
        divergence_kind: DivergenceKind,
        rationale: String,
        evidence: Evidence,
    },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "status", rename_all = "kebab-case", deny_unknown_fields)]
enum Evidence {
    Verified {
        test: TestOwner,
        #[serde(default)]
        stim_reproduction: Option<TestOwner>,
    },
    NeedsOwner {
        milestone: Milestone,
    },
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum EvidenceStatus {
    Verified,
    NeedsOwner,
    NotApplicable,
}

impl EvidenceStatus {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Verified => "verified",
            Self::NeedsOwner => "needs-owner",
            Self::NotApplicable => "not-applicable",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "kebab-case")]
enum Status {
    Done,
    Missing,
    Deferred,
    Divergence,
}

impl Status {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Done => "done",
            Self::Missing => "missing",
            Self::Deferred => "deferred",
            Self::Divergence => "divergence",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
enum Milestone {
    P1,
    P2,
    P3,
    P4,
    P5,
    P6,
}

impl Milestone {
    const fn as_str(self) -> &'static str {
        match self {
            Self::P1 => "P1",
            Self::P2 => "P2",
            Self::P3 => "P3",
            Self::P4 => "P4",
            Self::P5 => "P5",
            Self::P6 => "P6",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum DivergenceKind {
    StimBug,
    ResourceLimit,
    StabExtension,
}

impl DivergenceKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::StimBug => "Stim bug",
            Self::ResourceLimit => "resource limit",
            Self::StabExtension => "Stab extension",
        }
    }
}

#[derive(Debug, Deserialize)]
struct TestOwner {
    package: String,
    #[serde(flatten)]
    target: CargoTestTarget,
    name: String,
    minimum_tier: Tier,
}

impl TestOwner {
    fn display(&self) -> String {
        match &self.target {
            CargoTestTarget::Lib => format!(
                "cargo test -p {} --lib -- {} --exact --include-ignored",
                self.package, self.name
            ),
            CargoTestTarget::Bin { target } => format!(
                "cargo test -p {} --bin {} -- {} --exact --include-ignored",
                self.package, target, self.name
            ),
            CargoTestTarget::Test { target } => format!(
                "cargo test -p {} --test {} -- {} --exact --include-ignored",
                self.package, target, self.name
            ),
        }
    }

    fn listing_group_key(&self) -> String {
        match &self.target {
            CargoTestTarget::Lib => format!("{}|lib", self.package),
            CargoTestTarget::Bin { target } => format!("{}|bin|{target}", self.package),
            CargoTestTarget::Test { target } => format!("{}|test|{target}", self.package),
        }
    }

    fn listing_all_args(&self) -> Vec<&str> {
        let mut args = vec!["test", "-p", self.package.as_str()];
        match &self.target {
            CargoTestTarget::Lib => args.push("--lib"),
            CargoTestTarget::Bin { target } => {
                args.extend(["--bin", target.as_str()]);
            }
            CargoTestTarget::Test { target } => {
                args.extend(["--test", target.as_str()]);
            }
        }
        args.extend(["--quiet", "--", "--list"]);
        args
    }

    fn run_args(&self) -> Vec<&str> {
        let mut args = vec!["test", "-p", self.package.as_str()];
        match &self.target {
            CargoTestTarget::Lib => args.push("--lib"),
            CargoTestTarget::Bin { target } => {
                args.extend(["--bin", target.as_str()]);
            }
            CargoTestTarget::Test { target } => {
                args.extend(["--test", target.as_str()]);
            }
        }
        args.extend([
            "--quiet",
            "--",
            self.name.as_str(),
            "--exact",
            "--include-ignored",
        ]);
        args
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum CargoTestTarget {
    Lib,
    Bin { target: String },
    Test { target: String },
}

#[derive(Debug)]
struct ExpectedCoverage {
    members: BTreeSet<String>,
    canonical_gates: BTreeSet<String>,
    aliases: BTreeSet<String>,
}

pub(crate) fn run(root: &RepoRoot, command: Command) -> Result<(), OracleError> {
    let ledger = read_and_validate(root)?;
    match command {
        Command::Check => {
            check_owner_selectors(root, &ledger)?;
            print_summary(&ledger);
        }
        Command::Run { tier } => run_owner_tests(root, &ledger, tier)?,
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

fn expected_coverage(root: &RepoRoot) -> Result<ExpectedCoverage, ParityError> {
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

fn extend_dimension<'a, I>(members: &mut BTreeSet<String>, dimension: &str, values: I)
where
    I: IntoIterator<Item = &'a str>,
{
    members.extend(
        values
            .into_iter()
            .map(|value| format!("{dimension}:{value}")),
    );
}

fn parse_stim_gate_doc(text: &str) -> Result<(BTreeSet<String>, BTreeSet<String>), ParityError> {
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

fn validate(
    root: &RepoRoot,
    ledger: &Ledger,
    expected: &ExpectedCoverage,
) -> Result<(), ParityError> {
    let mut errors = Vec::new();
    if ledger.schema_version != LEDGER_SCHEMA_VERSION {
        errors.push(format!(
            "schema_version is {}, expected {LEDGER_SCHEMA_VERSION}",
            ledger.schema_version
        ));
    }
    if ledger.stim.version != STIM_TAG {
        errors.push(format!(
            "stim.version is {}, expected {STIM_TAG}",
            ledger.stim.version
        ));
    }
    if ledger.stim.commit != STIM_COMMIT {
        errors.push(format!(
            "stim.commit is {}, expected {STIM_COMMIT}",
            ledger.stim.commit
        ));
    }
    if ledger.families.is_empty() {
        errors.push("families must not be empty".to_string());
    }

    validate_format_routes(root, ledger, &mut errors);
    validate_command_surfaces(root, ledger, &mut errors);
    let mut expected_members = expected.members.clone();
    for surface in &ledger.command_surfaces {
        for option in &surface.options {
            expected_members.insert(format!("option:{}/{}", surface.command, option));
        }
    }
    let mut route_outcomes = BTreeMap::new();
    for route in &ledger.format_routes {
        for (formats, outcome) in [
            (&route.accepted_formats, RouteOutcome::Accepted),
            (&route.rejected_formats, RouteOutcome::Rejected),
            (&route.stim_bug_divergences, RouteOutcome::StimBugDivergence),
        ] {
            for format in formats {
                let member = format!("route:{}/{format}", route.id);
                expected_members.insert(member.clone());
                route_outcomes.insert(member, outcome);
            }
        }
    }

    let mut ids = BTreeSet::new();
    let mut coverage = BTreeMap::<String, String>::new();
    let mut previous_id: Option<&str> = None;
    for family in &ledger.families {
        validate_family(root, family, &mut errors);
        if !ids.insert(family.id.as_str()) {
            errors.push(format!("family id {} is duplicated", family.id));
        }
        if previous_id.is_some_and(|previous| previous >= family.id.as_str()) {
            errors.push(format!(
                "family {} is out of order; families must be sorted by id",
                family.id
            ));
        }
        previous_id = Some(&family.id);
        for member in &family.coverage {
            if !expected_members.contains(member) {
                errors.push(format!(
                    "family {} names unknown coverage member {member}",
                    family.id
                ));
                continue;
            }
            if let Some(first) = coverage.insert(member.clone(), family.id.clone()) {
                errors.push(format!(
                    "coverage member {member} is owned by both {first} and {}",
                    family.id
                ));
            }
            if matches!(family.status(), Status::Done | Status::Divergence) {
                validate_current_capability(family, member, expected, &route_outcomes, &mut errors);
            }
        }
    }

    for missing in expected_members
        .iter()
        .filter(|member| !coverage.contains_key(*member))
    {
        errors.push(format!("coverage member {missing} has no family owner"));
    }

    if errors.is_empty() {
        Ok(())
    } else {
        errors.sort();
        errors.dedup();
        Err(invalid(errors.join("\n")))
    }
}

fn validate_family(root: &RepoRoot, family: &Family, errors: &mut Vec<String>) {
    if !is_slug(&family.id, true) {
        errors.push(format!(
            "family id {} is not a lowercase dotted slug",
            family.id
        ));
    }
    validate_clean_text(&family.id, "contract", &family.contract, 600, errors);
    if family.stim_refs.is_empty() {
        errors.push(format!(
            "family {} has no pinned Stim references",
            family.id
        ));
    }
    let mut refs = BTreeSet::new();
    for reference in &family.stim_refs {
        if !refs.insert(reference.as_str()) {
            errors.push(format!(
                "family {} repeats Stim reference {reference}",
                family.id
            ));
        }
        validate_stim_reference(root, &family.id, reference, errors);
    }
    if family.coverage.iter().collect::<BTreeSet<_>>().len() != family.coverage.len() {
        errors.push(format!("family {} repeats a coverage member", family.id));
    }

    match &family.disposition {
        Disposition::Done { owner, evidence } => {
            validate_owner(&family.id, owner, errors);
            match evidence {
                Evidence::Verified {
                    test,
                    stim_reproduction,
                } => {
                    validate_test_owner(&family.id, test, errors);
                    if stim_reproduction.is_some() {
                        errors.push(format!(
                            "done family {} cannot carry a Stim-bug reproduction",
                            family.id
                        ));
                    }
                }
                Evidence::NeedsOwner { milestone } if *milestone != Milestone::P1 => {
                    errors.push(format!(
                        "done family {} must receive its lean canonical owner in P1",
                        family.id
                    ));
                }
                Evidence::NeedsOwner { .. } => {}
            }
        }
        Disposition::Missing { owner, .. } => {
            validate_owner(&family.id, owner, errors);
        }
        Disposition::Deferred { rationale } => {
            validate_clean_text(&family.id, "rationale", rationale, 600, errors);
        }
        Disposition::Divergence {
            owner,
            divergence_kind,
            rationale,
            evidence,
            ..
        } => {
            validate_owner(&family.id, owner, errors);
            validate_clean_text(&family.id, "rationale", rationale, 600, errors);
            match evidence {
                Evidence::Verified {
                    test,
                    stim_reproduction,
                } => {
                    validate_test_owner(&family.id, test, errors);
                    match (divergence_kind, stim_reproduction) {
                        (DivergenceKind::StimBug, Some(reproduction)) => {
                            validate_test_owner(&family.id, reproduction, errors);
                            if reproduction.package != "stab-oracle" {
                                errors.push(format!(
                                    "Stim-bug family {} reproduction must be owned by stab-oracle",
                                    family.id
                                ));
                            }
                            if test.display() == reproduction.display() {
                                errors.push(format!(
                                    "Stim-bug family {} reproduction must be distinct from its Stab regression",
                                    family.id
                                ));
                            }
                        }
                        (DivergenceKind::StimBug, None) => errors.push(format!(
                            "Stim-bug family {} has no independent pinned reproduction",
                            family.id
                        )),
                        (_, Some(_)) => errors.push(format!(
                            "non-bug divergence family {} cannot carry a Stim-bug reproduction",
                            family.id
                        )),
                        (_, None) => {}
                    }
                }
                Evidence::NeedsOwner { .. } if *divergence_kind == DivergenceKind::StimBug => {
                    errors.push(format!(
                        "Stim-bug divergence family {} must have verified independent evidence",
                        family.id
                    ));
                }
                Evidence::NeedsOwner { milestone } if *milestone != Milestone::P1 => {
                    errors.push(format!(
                        "implemented divergence family {} must receive its lean canonical owner in P1",
                        family.id
                    ));
                }
                Evidence::NeedsOwner { .. } => {}
            }
        }
    }
}

fn validate_owner(family_id: &str, owner: &str, errors: &mut Vec<String>) {
    if !PRODUCT_OWNERS.contains(&owner) {
        errors.push(format!(
            "family {family_id} has unknown product owner {owner}"
        ));
    }
}

fn validate_test_owner(family_id: &str, test: &TestOwner, errors: &mut Vec<String>) {
    if !TEST_PACKAGES.contains(&test.package.as_str()) {
        errors.push(format!(
            "family {family_id} uses unknown test package {}",
            test.package
        ));
    }
    if !is_test_name(&test.name) {
        errors.push(format!(
            "family {family_id} has invalid exact test name {}",
            test.name
        ));
    }
    if let CargoTestTarget::Bin { target } | CargoTestTarget::Test { target } = &test.target
        && !is_test_name(target)
    {
        errors.push(format!(
            "family {family_id} has invalid integration-test target {target}"
        ));
    }
}

fn is_test_name(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('-')
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-' | b':')
        })
}

fn validate_stim_reference(
    root: &RepoRoot,
    family_id: &str,
    reference: &str,
    errors: &mut Vec<String>,
) {
    let path = Path::new(reference);
    if reference.is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        errors.push(format!(
            "family {family_id} has unsafe Stim reference {reference}"
        ));
        return;
    }
    let resolved = root.stim_source().join(path);
    if safe_file::open_regular_file(&resolved).is_err() {
        errors.push(format!(
            "family {family_id} names missing or unsafe Stim reference {reference}"
        ));
    }
}

fn validate_current_capability(
    family: &Family,
    member: &str,
    expected: &ExpectedCoverage,
    route_outcomes: &BTreeMap<String, RouteOutcome>,
    errors: &mut Vec<String>,
) {
    let Some((dimension, value)) = member.split_once(':') else {
        return;
    };
    match dimension {
        "gate" => {
            let gate = stab_model::Gate::from_name(value);
            if gate.is_err()
                || gate
                    .as_ref()
                    .is_ok_and(|gate| gate.canonical_name() != value)
                || !expected.canonical_gates.contains(value)
            {
                errors.push(format!(
                    "family {} claims unavailable canonical gate {value}",
                    family.id
                ));
            }
        }
        "alias" => {
            if stab_model::Gate::from_name(value).is_err() || !expected.aliases.contains(value) {
                errors.push(format!(
                    "family {} claims unavailable gate alias {value}",
                    family.id
                ));
            }
        }
        "format" => {
            if !stab_records::RecordFormat::all().any(|format| format.as_str() == value) {
                errors.push(format!(
                    "family {} claims unavailable result format {value}",
                    family.id
                ));
            }
        }
        "dialect" => {
            let capability_name = match value {
                "stim" => "stim-circuit",
                "dem" => "detector-error-model",
                _ => value,
            };
            if !stab_model::ModelDialect::all().any(|dialect| dialect.as_str() == capability_name) {
                errors.push(format!(
                    "family {} claims unavailable model dialect {value}",
                    family.id
                ));
            }
        }
        "option" => {
            let valid = value
                .split_once('/')
                .is_some_and(|(command, option)| is_expected_command_option(command, option));
            if !valid {
                errors.push(format!(
                    "family {} claims unknown command option {value}",
                    family.id
                ));
            }
        }
        "command" | "consumer" | "target" | "capability" | "extension" | "obsolete" => {}
        "route" => {
            let outcome = route_outcomes.get(member);
            let is_stim_bug = matches!(
                family.disposition,
                Disposition::Divergence {
                    divergence_kind: DivergenceKind::StimBug,
                    ..
                }
            );
            if outcome == Some(&RouteOutcome::StimBugDivergence) && !is_stim_bug {
                errors.push(format!(
                    "family {} owns Stim-bug route member {member} without a Stim-bug divergence disposition",
                    family.id
                ));
            } else if outcome != Some(&RouteOutcome::StimBugDivergence) && is_stim_bug {
                errors.push(format!(
                    "family {} assigns Stim-bug divergence to non-bug route member {member}",
                    family.id
                ));
            }
        }
        _ => errors.push(format!(
            "family {} uses unsupported coverage dimension {dimension}",
            family.id
        )),
    }
}

fn is_slug(value: &str, allow_dot: bool) -> bool {
    !value.is_empty()
        && !value.starts_with(['-', '.'])
        && !value.ends_with(['-', '.'])
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || byte == b'-'
                || (allow_dot && byte == b'.')
        })
}

fn validate_clean_text(
    family_id: &str,
    field: &str,
    value: &str,
    maximum_bytes: usize,
    errors: &mut Vec<String>,
) {
    if value.is_empty()
        || value.trim() != value
        || value.contains(['\r', '\n'])
        || value.len() > maximum_bytes
    {
        errors.push(format!(
            "family {family_id} has invalid {field}; it must be one trimmed line of at most {maximum_bytes} bytes"
        ));
    }
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
