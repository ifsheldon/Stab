use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

pub(super) const SUITE_SCHEMA_VERSION: u16 = 1;
pub(super) const MAX_RELEASE_FAMILIES: usize = 12;
pub(super) const MAX_RELEASE_CASES: usize = 30;
pub(super) const PARITY_MAX_RATIO: f64 = 1.25;
pub(super) const SELF_REGRESSION_MAX_RATIO: f64 = 1.15;
pub(super) const TIMING_BOUNDARY: &str = "e2e-user-workflow-v2";
pub(super) const STIM_VERSION: &str = "v1.16.0";
pub(super) const STIM_COMMIT: &str = "e2fc1eca7fd21684d433aa5f10f4504ea4860d07";
pub(super) const BOOTSTRAP_SEED: u64 = 6_004_497_137_074_745_393;
pub(super) const BOOTSTRAP_RESAMPLES: usize = 10_000;
pub(super) const CONFIDENCE_LEVEL: f64 = 0.95;
pub(super) const MAXIMUM_TEMPERATURE_MILLIDEGREES: u64 = 100_000;
pub(super) const MAXIMUM_OUTPUT_BYTES: usize = 64 << 20;
pub(super) const COMMAND_TIMEOUT_SECONDS: u64 = 600;
pub(super) const MAXIMUM_CASE_RSS_BYTES: u64 = 512 << 20;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Suite {
    pub(super) schema_version: u16,
    pub(super) stim: StimIdentity,
    pub(super) policy: Policy,
    pub(super) tiers: BTreeMap<Tier, TierPolicy>,
    pub(super) expected_release_families: usize,
    pub(super) expected_release_cases: usize,
    #[serde(default)]
    pub(super) self_baselines: Vec<SelfBaseline>,
    pub(super) families: Vec<Family>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StimIdentity {
    pub(super) version: String,
    pub(super) commit: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Policy {
    pub(super) timing_boundary: String,
    pub(super) parity_max_ratio: f64,
    pub(super) self_regression_max_ratio: f64,
    pub(super) bootstrap_seed: u64,
    pub(super) bootstrap_resamples: usize,
    pub(super) confidence_level: f64,
    pub(super) maximum_temperature_millidegrees: u64,
    pub(super) maximum_output_bytes: usize,
    pub(super) command_timeout_seconds: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub(super) enum Tier {
    Smoke,
    Full,
    Soak,
}

impl fmt::Display for Tier {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Smoke => "smoke",
            Self::Full => "full",
            Self::Soak => "soak",
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TierPolicy {
    pub(super) warmups: usize,
    pub(super) samples: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Family {
    pub(super) id: String,
    pub(super) description: String,
    pub(super) runner: RunnerKind,
    pub(super) comparator: Comparator,
    pub(super) prerequisites: Vec<String>,
    pub(super) cases: Vec<Case>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum RunnerKind {
    Cli,
    Rust,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum Comparator {
    StimCli,
    SelfOnly,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Case {
    pub(super) id: String,
    pub(super) size_class: SizeClass,
    pub(super) batch: u32,
    pub(super) work: SemanticWork,
    pub(super) maximum_stab_peak_rss_bytes: u64,
    pub(super) workload: Workload,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum SizeClass {
    Small,
    Medium,
    Large,
    Narrow,
    Wide,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SemanticWork {
    pub(super) amount: u64,
    pub(super) unit: WorkUnit,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum WorkUnit {
    GeneratedBytes,
    InputBytes,
    Records,
    Shots,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum Workload {
    Cli {
        args: Vec<String>,
        stdin: DataRecipe,
        #[serde(default)]
        files: Vec<FileSpec>,
        stdout: OutputContract,
    },
    CliPipeline {
        steps: Vec<PipelineStep>,
        stdout: OutputContract,
    },
    RustPipeline {
        shots: u64,
        minimum_logical_failures: u64,
        maximum_logical_failures: u64,
        seed: u64,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PipelineStep {
    pub(super) args: Vec<String>,
    pub(super) stdin: PipelineInput,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum PipelineInput {
    Empty,
    Previous,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FileSpec {
    pub(super) name: String,
    pub(super) role: FileRole,
    #[serde(default)]
    pub(super) data: Option<DataRecipe>,
    #[serde(default)]
    pub(super) output: Option<OutputContract>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum FileRole {
    Input,
    Output,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum DataRecipe {
    Empty,
    GeneratedCircuit {
        args: Vec<String>,
    },
    FoldedCircuit {
        qubits: usize,
        repeat_blocks: usize,
        repeat_count: u64,
        error_probability: f64,
    },
    Records {
        format: RecordFormat,
        records: usize,
        bits: usize,
        pattern: RecordPattern,
    },
    TypedDets {
        records: usize,
        detectors: usize,
        observables: usize,
        detector_hits: usize,
        observable_hits: usize,
    },
    M2dCircuit {
        bits: usize,
    },
    Dem {
        detectors: usize,
        mechanisms: usize,
        repeat_count: u64,
        error_probability: f64,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum RecordFormat {
    ZeroOne,
    B8,
    R8,
    Hits,
    Dets,
    Ptb64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum RecordPattern {
    Alternating,
    Sparse,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum OutputContract {
    Exact {
        minimum_bytes: usize,
    },
    Records {
        format: RecordFormat,
        records: usize,
        bits: usize,
        minimum_one_bits: u64,
        maximum_one_fraction: f64,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SelfBaseline {
    pub(super) architecture: String,
    pub(super) cpu_model: String,
    pub(super) rustc: String,
    pub(super) target: String,
    pub(super) timing_boundary: String,
    pub(super) case_id: String,
    pub(super) case_digest: String,
    pub(super) median_seconds_per_work: f64,
    pub(super) upper_seconds_per_work: f64,
}

impl Suite {
    pub(super) fn validate(&self) -> Result<(), String> {
        let mut errors = Vec::new();
        if self.schema_version != SUITE_SCHEMA_VERSION {
            errors.push(format!(
                "schema_version is {}, expected {SUITE_SCHEMA_VERSION}",
                self.schema_version
            ));
        }
        if self.stim.version != STIM_VERSION || self.stim.commit != STIM_COMMIT {
            errors.push(format!(
                "Stim identity must remain {STIM_VERSION} at {STIM_COMMIT}"
            ));
        }
        validate_policy(&self.policy, &mut errors);
        for (tier, expected_warmups, expected_samples) in
            [(Tier::Smoke, 0, 1), (Tier::Full, 2, 9), (Tier::Soak, 3, 21)]
        {
            match self.tiers.get(&tier) {
                Some(policy)
                    if policy.warmups == expected_warmups
                        && policy.samples == expected_samples => {}
                Some(policy) => errors.push(format!(
                    "tier {tier} must use {expected_warmups} warmups and {expected_samples} samples, found {} and {}",
                    policy.warmups, policy.samples
                )),
                None => errors.push(format!("tier {tier} is missing")),
            }
        }
        if self.families.len() > MAX_RELEASE_FAMILIES {
            errors.push(format!(
                "release family count {} exceeds {MAX_RELEASE_FAMILIES}",
                self.families.len()
            ));
        }
        if self.expected_release_families != self.families.len() {
            errors.push(format!(
                "expected_release_families is {}, actual count is {}",
                self.expected_release_families,
                self.families.len()
            ));
        }
        let case_count = self
            .families
            .iter()
            .map(|family| family.cases.len())
            .sum::<usize>();
        if case_count > MAX_RELEASE_CASES {
            errors.push(format!(
                "release case count {case_count} exceeds {MAX_RELEASE_CASES}"
            ));
        }
        if self.expected_release_cases != case_count {
            errors.push(format!(
                "expected_release_cases is {}, actual count is {case_count}",
                self.expected_release_cases
            ));
        }
        validate_families(&self.families, &mut errors);
        validate_baselines(&self.self_baselines, &self.families, &mut errors);
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("\n"))
        }
    }

    pub(super) fn case_count(&self) -> usize {
        self.families.iter().map(|family| family.cases.len()).sum()
    }
}

fn validate_policy(policy: &Policy, errors: &mut Vec<String>) {
    if policy.timing_boundary != TIMING_BOUNDARY {
        errors.push(format!("timing_boundary must be {TIMING_BOUNDARY}"));
    }
    if policy.parity_max_ratio != PARITY_MAX_RATIO {
        errors.push(format!("parity_max_ratio must remain {PARITY_MAX_RATIO}"));
    }
    if policy.self_regression_max_ratio != SELF_REGRESSION_MAX_RATIO {
        errors.push(format!(
            "self_regression_max_ratio must remain {SELF_REGRESSION_MAX_RATIO}"
        ));
    }
    if policy.bootstrap_seed != BOOTSTRAP_SEED {
        errors.push(format!("bootstrap_seed must remain {BOOTSTRAP_SEED}"));
    }
    if policy.bootstrap_resamples != BOOTSTRAP_RESAMPLES {
        errors.push(format!(
            "bootstrap_resamples must remain {BOOTSTRAP_RESAMPLES}"
        ));
    }
    if policy.confidence_level != CONFIDENCE_LEVEL {
        errors.push(format!("confidence_level must remain {CONFIDENCE_LEVEL}"));
    }
    if policy.maximum_temperature_millidegrees != MAXIMUM_TEMPERATURE_MILLIDEGREES {
        errors.push(format!(
            "maximum_temperature_millidegrees must remain {MAXIMUM_TEMPERATURE_MILLIDEGREES}"
        ));
    }
    if policy.maximum_output_bytes != MAXIMUM_OUTPUT_BYTES {
        errors.push(format!(
            "maximum_output_bytes must remain {MAXIMUM_OUTPUT_BYTES}"
        ));
    }
    if policy.command_timeout_seconds != COMMAND_TIMEOUT_SECONDS {
        errors.push(format!(
            "command_timeout_seconds must remain {COMMAND_TIMEOUT_SECONDS}"
        ));
    }
}

fn validate_families(families: &[Family], errors: &mut Vec<String>) {
    let mut family_ids = BTreeSet::new();
    let mut case_ids = BTreeSet::new();
    for family in families {
        validate_id("family", &family.id, errors);
        if !family_ids.insert(family.id.as_str()) {
            errors.push(format!("duplicate family id {}", family.id));
        }
        if family.description.trim().is_empty() {
            errors.push(format!("family {} has an empty description", family.id));
        }
        if family.prerequisites.is_empty() {
            errors.push(format!(
                "family {} has no correctness prerequisites",
                family.id
            ));
        }
        let mut prerequisites = BTreeSet::new();
        for prerequisite in &family.prerequisites {
            if !prerequisites.insert(prerequisite) {
                errors.push(format!(
                    "family {} repeats prerequisite {prerequisite}",
                    family.id
                ));
            }
        }
        match (family.runner, family.comparator) {
            (RunnerKind::Cli, Comparator::StimCli) | (RunnerKind::Rust, Comparator::SelfOnly) => {}
            _ => errors.push(format!(
                "family {} has inconsistent runner and comparator",
                family.id
            )),
        }
        if family.cases.is_empty() {
            errors.push(format!("family {} has no cases", family.id));
        }
        for case in &family.cases {
            validate_id("case", &case.id, errors);
            let qualified = format!("{}.{}", family.id, case.id);
            if !case_ids.insert(qualified.clone()) {
                errors.push(format!("duplicate case id {qualified}"));
            }
            if case.batch == 0 {
                errors.push(format!("case {qualified} has a zero batch"));
            }
            if case.work.amount == 0 {
                errors.push(format!("case {qualified} has zero semantic work"));
            }
            if case.maximum_stab_peak_rss_bytes == 0 {
                errors.push(format!("case {qualified} has a zero RSS limit"));
            } else if case.maximum_stab_peak_rss_bytes > MAXIMUM_CASE_RSS_BYTES {
                errors.push(format!(
                    "case {qualified} exceeds the {MAXIMUM_CASE_RSS_BYTES}-byte RSS policy cap"
                ));
            }
            validate_workload(family, case, &qualified, errors);
        }
    }
}

fn validate_workload(family: &Family, case: &Case, qualified: &str, errors: &mut Vec<String>) {
    match (&case.workload, family.runner) {
        (
            Workload::Cli {
                args,
                stdin,
                files,
                stdout,
            },
            RunnerKind::Cli,
        ) => {
            validate_args(args, qualified, errors);
            validate_files(files, qualified, errors);
            validate_file_placeholders(args, files, qualified, errors);
            validate_data(stdin, qualified, errors);
            validate_output(stdout, qualified, errors);
            validate_cli_semantic_work(case, args, qualified, errors);
        }
        (Workload::CliPipeline { steps, stdout }, RunnerKind::Cli) => {
            if steps.len() < 2 {
                errors.push(format!(
                    "case {qualified} pipeline needs at least two steps"
                ));
            }
            for step in steps {
                validate_args(&step.args, qualified, errors);
            }
            if steps
                .first()
                .is_some_and(|step| step.stdin != PipelineInput::Empty)
            {
                errors.push(format!(
                    "case {qualified} first pipeline step needs empty stdin"
                ));
            }
            if steps
                .iter()
                .skip(1)
                .any(|step| step.stdin != PipelineInput::Previous)
            {
                errors.push(format!(
                    "case {qualified} later pipeline steps need previous stdout"
                ));
            }
            if let Some(last) = steps.last() {
                validate_cli_semantic_work(case, &last.args, qualified, errors);
            }
            validate_output(stdout, qualified, errors);
        }
        (
            Workload::RustPipeline {
                shots,
                minimum_logical_failures,
                maximum_logical_failures,
                ..
            },
            RunnerKind::Rust,
        ) => {
            if *shots == 0
                || *minimum_logical_failures == 0
                || minimum_logical_failures > maximum_logical_failures
                || maximum_logical_failures >= shots
                || case.work.unit != WorkUnit::Shots
                || case.work.amount != *shots
            {
                errors.push(format!(
                    "case {qualified} Rust pipeline needs matching positive work and a nontrivial logical-failure band"
                ));
            }
        }
        _ => errors.push(format!(
            "case {qualified} workload does not match its family runner"
        )),
    }
}

fn validate_data(recipe: &DataRecipe, qualified: &str, errors: &mut Vec<String>) {
    let probability_is_valid = |value: f64| value.is_finite() && (0.0..=1.0).contains(&value);
    match recipe {
        DataRecipe::Empty => {}
        DataRecipe::GeneratedCircuit { args } => validate_args(args, qualified, errors),
        DataRecipe::FoldedCircuit {
            qubits,
            repeat_blocks,
            repeat_count,
            error_probability,
        } => {
            if *qubits == 0
                || *repeat_blocks == 0
                || *repeat_count == 0
                || !probability_is_valid(*error_probability)
            {
                errors.push(format!(
                    "case {qualified} has an invalid folded-circuit recipe"
                ));
            }
        }
        DataRecipe::Records {
            format,
            records,
            bits,
            ..
        } => {
            if *records == 0
                || *bits == 0
                || (*format == RecordFormat::Ptb64 && !records.is_multiple_of(64))
                || *format == RecordFormat::Dets
            {
                errors.push(format!("case {qualified} has an invalid record recipe"));
            }
        }
        DataRecipe::TypedDets {
            records,
            detectors,
            observables,
            detector_hits,
            observable_hits,
        } => {
            if *records == 0
                || *detectors == 0
                || *observables == 0
                || detector_hits > detectors
                || observable_hits > observables
            {
                errors.push(format!("case {qualified} has an invalid typed-DETS recipe"));
            }
        }
        DataRecipe::M2dCircuit { bits } => {
            if *bits == 0 {
                errors.push(format!("case {qualified} has a zero-width m2d circuit"));
            }
        }
        DataRecipe::Dem {
            detectors,
            mechanisms,
            repeat_count,
            error_probability,
        } => {
            if *detectors == 0
                || *mechanisms == 0
                || *repeat_count == 0
                || !probability_is_valid(*error_probability)
            {
                errors.push(format!("case {qualified} has an invalid DEM recipe"));
            }
        }
    }
}

fn validate_output(output: &OutputContract, qualified: &str, errors: &mut Vec<String>) {
    match output {
        OutputContract::Exact { minimum_bytes } => {
            if *minimum_bytes == 0 {
                errors.push(format!(
                    "case {qualified} has a vacuous exact-output contract"
                ));
            }
        }
        OutputContract::Records {
            format,
            records,
            bits,
            minimum_one_bits,
            maximum_one_fraction,
        } => {
            let total_bits = u64::try_from(*records).ok().and_then(|records| {
                u64::try_from(*bits)
                    .ok()
                    .and_then(|bits| records.checked_mul(bits))
            });
            if *records == 0
                || *bits == 0
                || (*format == RecordFormat::Ptb64 && !records.is_multiple_of(64))
                || *format == RecordFormat::Dets
                || total_bits.is_none_or(|total| *minimum_one_bits > total)
                || !maximum_one_fraction.is_finite()
                || !(0.0..=1.0).contains(maximum_one_fraction)
            {
                errors.push(format!(
                    "case {qualified} has an invalid record-output contract"
                ));
            }
        }
    }
}

fn validate_cli_semantic_work(
    case: &Case,
    measured_args: &[String],
    qualified: &str,
    errors: &mut Vec<String>,
) {
    let declared_output_records = match &case.workload {
        Workload::Cli { stdout, .. } | Workload::CliPipeline { stdout, .. } => {
            output_record_count(stdout)
        }
        Workload::RustPipeline { .. } => None,
    };
    match case.work.unit {
        WorkUnit::GeneratedBytes | WorkUnit::InputBytes => {}
        WorkUnit::Records => {
            let declared_input_records = match &case.workload {
                Workload::Cli { stdin, .. } => recipe_record_count(stdin),
                Workload::CliPipeline { .. } | Workload::RustPipeline { .. } => None,
            };
            if declared_input_records != Some(case.work.amount) {
                errors.push(format!(
                    "case {qualified} record work does not equal its input recipe"
                ));
            }
        }
        WorkUnit::Shots => {
            if argument_u64(measured_args, "--shots") != Some(case.work.amount) {
                errors.push(format!(
                    "case {qualified} shot work does not equal its --shots argument"
                ));
            }
            if declared_output_records != Some(case.work.amount) {
                errors.push(format!(
                    "case {qualified} shot work does not equal its output record count"
                ));
            }
        }
    }
}

fn recipe_record_count(recipe: &DataRecipe) -> Option<u64> {
    let count = match recipe {
        DataRecipe::Records { records, .. } | DataRecipe::TypedDets { records, .. } => *records,
        _ => return None,
    };
    u64::try_from(count).ok()
}

fn output_record_count(output: &OutputContract) -> Option<u64> {
    let OutputContract::Records { records, .. } = output else {
        return None;
    };
    u64::try_from(*records).ok()
}

fn argument_u64(args: &[String], name: &str) -> Option<u64> {
    let mut values = args
        .windows(2)
        .filter_map(|pair| (pair.first()?.as_str() == name).then_some(pair.get(1)?));
    let value = values.next()?.parse::<u64>().ok()?;
    values.next().is_none().then_some(value)
}

fn validate_args(args: &[String], qualified: &str, errors: &mut Vec<String>) {
    if args.is_empty() || args.first().is_none_or(|arg| arg.starts_with('-')) {
        errors.push(format!(
            "case {qualified} command args need a named subcommand"
        ));
    }
    if args.iter().any(|arg| arg.contains('\0')) {
        errors.push(format!("case {qualified} command args contain NUL"));
    }
}

fn validate_files(files: &[FileSpec], qualified: &str, errors: &mut Vec<String>) {
    let mut names = BTreeSet::new();
    for file in files {
        validate_id("file", &file.name, errors);
        if !names.insert(file.name.as_str()) {
            errors.push(format!("case {qualified} repeats file {}", file.name));
        }
        match file.role {
            FileRole::Input if file.data.is_none() || file.output.is_some() => {
                errors.push(format!(
                    "case {qualified} input file {} needs data and no output contract",
                    file.name
                ))
            }
            FileRole::Output if file.data.is_some() || file.output.is_none() => {
                errors.push(format!(
                    "case {qualified} output file {} needs an output contract and no data",
                    file.name
                ))
            }
            _ => {}
        }
        if let Some(data) = &file.data {
            validate_data(data, qualified, errors);
        }
        if let Some(output) = &file.output {
            validate_output(output, qualified, errors);
        }
    }
}

fn validate_file_placeholders(
    args: &[String],
    files: &[FileSpec],
    qualified: &str,
    errors: &mut Vec<String>,
) {
    let expected = files
        .iter()
        .map(|file| format!("{{file:{}}}", file.name))
        .collect::<BTreeSet<_>>();
    let actual = args
        .iter()
        .filter(|arg| arg.starts_with("{file:") || arg.contains("{file:"))
        .cloned()
        .collect::<BTreeSet<_>>();
    if actual
        .iter()
        .any(|placeholder| !expected.contains(placeholder))
    {
        errors.push(format!(
            "case {qualified} has an unknown or partial file placeholder"
        ));
    }
    for file in files {
        let placeholder = format!("{{file:{}}}", file.name);
        if !args.contains(&placeholder) {
            errors.push(format!(
                "case {qualified} does not route file {} into its command",
                file.name
            ));
        }
    }
}

fn validate_baselines(baselines: &[SelfBaseline], families: &[Family], errors: &mut Vec<String>) {
    let cases = families
        .iter()
        .flat_map(|family| {
            family.cases.iter().map(|case| {
                (
                    format!("{}.{}", family.id, case.id),
                    case_digest(&family.id, case),
                )
            })
        })
        .collect::<BTreeMap<_, _>>();
    let mut keys = BTreeSet::new();
    for baseline in baselines {
        let key = (
            baseline.architecture.as_str(),
            baseline.cpu_model.as_str(),
            baseline.rustc.as_str(),
            baseline.target.as_str(),
            baseline.timing_boundary.as_str(),
            baseline.case_id.as_str(),
            baseline.case_digest.as_str(),
        );
        if !keys.insert(key) {
            errors.push(format!(
                "duplicate self baseline for {} on {}",
                baseline.case_id, baseline.architecture
            ));
        }
        match cases.get(&baseline.case_id) {
            Some(Ok(digest)) if digest == &baseline.case_digest => {}
            Some(Ok(_)) => errors.push(format!(
                "self baseline {} has a stale case digest",
                baseline.case_id
            )),
            Some(Err(reason)) => errors.push(format!(
                "self baseline {} cannot digest its case: {reason}",
                baseline.case_id
            )),
            None => errors.push(format!(
                "self baseline {} has no active release case",
                baseline.case_id
            )),
        }
        if baseline.architecture.is_empty()
            || baseline.cpu_model.is_empty()
            || baseline.rustc.is_empty()
            || baseline.target.is_empty()
            || baseline.timing_boundary != TIMING_BOUNDARY
            || baseline.case_digest.len() != 64
        {
            errors.push(format!(
                "self baseline {} has an incomplete identity",
                baseline.case_id
            ));
        }
        for (name, value) in [
            ("median", baseline.median_seconds_per_work),
            ("upper", baseline.upper_seconds_per_work),
        ] {
            if !value.is_finite() || value <= 0.0 {
                errors.push(format!(
                    "self baseline {} has invalid {name}",
                    baseline.case_id
                ));
            }
        }
        if baseline.upper_seconds_per_work < baseline.median_seconds_per_work {
            errors.push(format!(
                "self baseline {} upper bound is below its median",
                baseline.case_id
            ));
        }
    }
}

pub(super) fn case_digest(family_id: &str, case: &Case) -> Result<String, String> {
    let bytes = serde_json::to_vec(&(family_id, case)).map_err(|source| source.to_string())?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

pub(super) fn suite_contract_digest(suite: &Suite) -> Result<String, String> {
    let bytes = serde_json::to_vec(&(
        suite.schema_version,
        &suite.stim,
        &suite.policy,
        &suite.tiers,
        suite.expected_release_families,
        suite.expected_release_cases,
        &suite.families,
    ))
    .map_err(|source| source.to_string())?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

fn validate_id(kind: &str, value: &str, errors: &mut Vec<String>) {
    if value.is_empty()
        || value.starts_with('-')
        || value.ends_with('-')
        || value
            .bytes()
            .any(|byte| !byte.is_ascii_lowercase() && !byte.is_ascii_digit() && byte != b'-')
    {
        errors.push(format!("{kind} id {value:?} is not lowercase kebab-case"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_suite() -> Suite {
        Suite {
            schema_version: SUITE_SCHEMA_VERSION,
            stim: StimIdentity {
                version: STIM_VERSION.to_string(),
                commit: STIM_COMMIT.to_string(),
            },
            policy: Policy {
                timing_boundary: TIMING_BOUNDARY.to_string(),
                parity_max_ratio: 1.25,
                self_regression_max_ratio: 1.15,
                bootstrap_seed: BOOTSTRAP_SEED,
                bootstrap_resamples: BOOTSTRAP_RESAMPLES,
                confidence_level: CONFIDENCE_LEVEL,
                maximum_temperature_millidegrees: MAXIMUM_TEMPERATURE_MILLIDEGREES,
                maximum_output_bytes: MAXIMUM_OUTPUT_BYTES,
                command_timeout_seconds: COMMAND_TIMEOUT_SECONDS,
            },
            tiers: BTreeMap::from([
                (
                    Tier::Smoke,
                    TierPolicy {
                        warmups: 0,
                        samples: 1,
                    },
                ),
                (
                    Tier::Full,
                    TierPolicy {
                        warmups: 2,
                        samples: 9,
                    },
                ),
                (
                    Tier::Soak,
                    TierPolicy {
                        warmups: 3,
                        samples: 21,
                    },
                ),
            ]),
            expected_release_families: 1,
            expected_release_cases: 1,
            self_baselines: Vec::new(),
            families: vec![Family {
                id: "sample".to_string(),
                description: "sample records".to_string(),
                runner: RunnerKind::Cli,
                comparator: Comparator::StimCli,
                prerequisites: vec!["cli.sample".to_string()],
                cases: vec![Case {
                    id: "small".to_string(),
                    size_class: SizeClass::Small,
                    batch: 1,
                    work: SemanticWork {
                        amount: 64,
                        unit: WorkUnit::Shots,
                    },
                    maximum_stab_peak_rss_bytes: MAXIMUM_CASE_RSS_BYTES,
                    workload: Workload::Cli {
                        args: vec![
                            "sample".to_string(),
                            "--shots".to_string(),
                            "64".to_string(),
                        ],
                        stdin: DataRecipe::Empty,
                        files: Vec::new(),
                        stdout: OutputContract::Records {
                            format: RecordFormat::B8,
                            records: 64,
                            bits: 1,
                            minimum_one_bits: 0,
                            maximum_one_fraction: 1.0,
                        },
                    },
                }],
            }],
        }
    }

    #[test]
    fn suite_rejects_duplicate_ids_caps_and_inconsistent_work() {
        let mut suite = minimal_suite();
        assert_eq!(suite.validate(), Ok(()));

        let duplicate = suite.families[0].cases[0].clone();
        suite.families[0].cases.push(duplicate);
        suite.expected_release_cases = 2;
        let error = suite.validate().expect_err("duplicate case");
        assert!(error.contains("duplicate case id"));

        let mut suite = minimal_suite();
        if let Workload::Cli { args, .. } = &mut suite.families[0].cases[0].workload {
            args.clear();
        }
        assert!(
            suite
                .validate()
                .expect_err("empty command")
                .contains("named subcommand")
        );
    }

    #[test]
    fn semantic_work_must_match_cli_records_and_shots() {
        let mut suite = minimal_suite();
        suite.families[0].cases[0].work.amount = 63;
        let error = suite.validate().expect_err("mismatched shots");
        assert!(error.contains("--shots"));
        assert!(error.contains("output record count"));

        let case = &mut suite.families[0].cases[0];
        case.work.unit = WorkUnit::Records;
        let error = suite
            .validate()
            .expect_err("records without an input recipe");
        assert!(error.contains("input recipe"));
    }

    #[test]
    fn contract_digest_ignores_baseline_values_but_owns_workloads() {
        let mut suite = minimal_suite();
        let initial = suite_contract_digest(&suite).expect("initial digest");
        let case_digest = case_digest("sample", &suite.families[0].cases[0]).expect("case digest");
        suite.self_baselines.push(SelfBaseline {
            architecture: "aarch64".to_string(),
            cpu_model: "test".to_string(),
            rustc: "rustc test".to_string(),
            target: "aarch64-test".to_string(),
            timing_boundary: TIMING_BOUNDARY.to_string(),
            case_id: "sample.small".to_string(),
            case_digest,
            median_seconds_per_work: 0.1,
            upper_seconds_per_work: 0.2,
        });
        assert_eq!(
            suite_contract_digest(&suite).expect("seeded digest"),
            initial
        );

        suite.families[0].cases[0].batch = 2;
        assert_ne!(
            suite_contract_digest(&suite).expect("changed digest"),
            initial
        );
    }

    #[test]
    fn suite_rejects_weakened_evidence_policy() {
        let mut suite = minimal_suite();
        suite.stim.commit = "a".repeat(40);
        suite.tiers.get_mut(&Tier::Full).expect("full tier").samples = 1;
        suite.policy.bootstrap_resamples = 1;
        suite.families[0].cases[0].maximum_stab_peak_rss_bytes = MAXIMUM_CASE_RSS_BYTES + 1;
        let error = suite.validate().expect_err("weakened policy");
        assert!(error.contains("Stim identity"));
        assert!(error.contains("tier full"));
        assert!(error.contains("bootstrap_resamples"));
        assert!(error.contains("RSS policy cap"));
    }
}
