//! Source-owned suite validation: policy floors, ids, workloads, recipes, contracts, and
//! baselines.

use std::collections::{BTreeMap, BTreeSet};

use super::super::error::{
    BaselineBound, BaselineProblem, CaseProblem, FamilyProblem, IdKind, PolicyField, RecipeKind,
    SuiteViolation, SuiteViolations,
};
use super::{
    BOOTSTRAP_RESAMPLES, BOOTSTRAP_SEED, COMMAND_TIMEOUT_SECONDS, CONFIDENCE_LEVEL, Case, CaseId,
    Comparator, DataRecipe, Family, FileRole, FileSpec, MAX_RELEASE_CASES, MAX_RELEASE_FAMILIES,
    MAXIMUM_CASE_RSS_BYTES, MAXIMUM_OUTPUT_BYTES, MAXIMUM_TEMPERATURE_MILLIDEGREES, OutputContract,
    PARITY_MAX_RATIO, PipelineInput, Policy, RecordFormat, RunnerKind, SELF_REGRESSION_MAX_RATIO,
    STIM_COMMIT, STIM_VERSION, SUITE_SCHEMA_VERSION, SelfBaseline, Suite, TIMING_BOUNDARY, Tier,
    WorkUnit, Workload,
};

type Errors = Vec<SuiteViolation>;

/// Records a case-level violation.
fn case_issue(errors: &mut Errors, case: &CaseId, problem: CaseProblem) {
    errors.push(SuiteViolation::Case {
        case: case.clone(),
        problem,
    });
}

/// Records a family-level violation.
fn family_issue(errors: &mut Errors, family: &str, problem: FamilyProblem) {
    errors.push(SuiteViolation::Family {
        family: family.to_string(),
        problem,
    });
}

/// Records a baseline-level violation.
fn baseline_issue(errors: &mut Errors, case: &CaseId, problem: BaselineProblem) {
    errors.push(SuiteViolation::Baseline {
        case: case.clone(),
        problem,
    });
}

impl Suite {
    pub(in crate::e2e) fn validate(&self) -> Result<(), SuiteViolations> {
        let mut errors = Vec::new();
        if self.schema_version != SUITE_SCHEMA_VERSION {
            errors.push(SuiteViolation::SchemaVersion {
                actual: self.schema_version,
            });
        }
        if self.stim.version != STIM_VERSION || self.stim.commit != STIM_COMMIT {
            errors.push(SuiteViolation::StimIdentity);
        }
        validate_policy(&self.policy, &mut errors);
        for (tier, expected_warmups, expected_samples) in
            [(Tier::Smoke, 0, 1), (Tier::Full, 2, 9), (Tier::Soak, 3, 21)]
        {
            match self.tiers.get(&tier) {
                Some(policy)
                    if policy.warmups == expected_warmups && policy.samples == expected_samples => {
                }
                Some(policy) => errors.push(SuiteViolation::TierShape {
                    tier,
                    expected_warmups,
                    expected_samples,
                    warmups: policy.warmups,
                    samples: policy.samples,
                }),
                None => errors.push(SuiteViolation::TierMissing { tier }),
            }
        }
        if self.families.len() > MAX_RELEASE_FAMILIES {
            errors.push(SuiteViolation::FamilyCap {
                count: self.families.len(),
                maximum: MAX_RELEASE_FAMILIES,
            });
        }
        if self.expected_release_families != self.families.len() {
            errors.push(SuiteViolation::FamilyCount {
                declared: self.expected_release_families,
                actual: self.families.len(),
            });
        }
        let case_count = self
            .families
            .iter()
            .map(|family| family.cases.len())
            .sum::<usize>();
        if case_count > MAX_RELEASE_CASES {
            errors.push(SuiteViolation::CaseCap {
                count: case_count,
                maximum: MAX_RELEASE_CASES,
            });
        }
        if self.expected_release_cases != case_count {
            errors.push(SuiteViolation::CaseCount {
                declared: self.expected_release_cases,
                actual: case_count,
            });
        }
        validate_families(&self.families, &mut errors);
        validate_baselines(&self.self_baselines, &self.families, &mut errors);
        if errors.is_empty() {
            Ok(())
        } else {
            Err(SuiteViolations(errors))
        }
    }
}

fn validate_policy(policy: &Policy, errors: &mut Errors) {
    for (field, holds) in [
        (
            PolicyField::TimingBoundary,
            policy.timing_boundary == TIMING_BOUNDARY,
        ),
        (
            PolicyField::ParityMaxRatio,
            policy.parity_max_ratio == PARITY_MAX_RATIO,
        ),
        (
            PolicyField::SelfRegressionMaxRatio,
            policy.self_regression_max_ratio == SELF_REGRESSION_MAX_RATIO,
        ),
        (
            PolicyField::BootstrapSeed,
            policy.bootstrap_seed == BOOTSTRAP_SEED,
        ),
        (
            PolicyField::BootstrapResamples,
            policy.bootstrap_resamples == BOOTSTRAP_RESAMPLES,
        ),
        (
            PolicyField::ConfidenceLevel,
            policy.confidence_level == CONFIDENCE_LEVEL,
        ),
        (
            PolicyField::MaximumTemperature,
            policy.maximum_temperature_millidegrees == MAXIMUM_TEMPERATURE_MILLIDEGREES,
        ),
        (
            PolicyField::MaximumOutputBytes,
            policy.maximum_output_bytes == MAXIMUM_OUTPUT_BYTES,
        ),
        (
            PolicyField::CommandTimeout,
            policy.command_timeout_seconds == COMMAND_TIMEOUT_SECONDS,
        ),
    ] {
        if !holds {
            errors.push(SuiteViolation::Policy(field));
        }
    }
}

fn validate_families(families: &[Family], errors: &mut Errors) {
    let mut family_ids = BTreeSet::new();
    let mut case_ids = BTreeSet::new();
    for family in families {
        validate_id(IdKind::Family, &family.id, errors);
        if !family_ids.insert(family.id.as_str()) {
            errors.push(SuiteViolation::DuplicateFamily {
                family: family.id.clone(),
            });
        }
        if family.description.trim().is_empty() {
            family_issue(errors, &family.id, FamilyProblem::EmptyDescription);
        }
        if family.prerequisites.is_empty() {
            family_issue(errors, &family.id, FamilyProblem::NoPrerequisites);
        }
        let mut prerequisites = BTreeSet::new();
        for prerequisite in &family.prerequisites {
            if !prerequisites.insert(prerequisite) {
                family_issue(
                    errors,
                    &family.id,
                    FamilyProblem::RepeatedPrerequisite(prerequisite.clone()),
                );
            }
        }
        match (family.runner, family.comparator) {
            (RunnerKind::Cli, Comparator::StimCli) | (RunnerKind::Rust, Comparator::SelfOnly) => {}
            _ => family_issue(errors, &family.id, FamilyProblem::InconsistentRunner),
        }
        if family.cases.is_empty() {
            family_issue(errors, &family.id, FamilyProblem::NoCases);
        }
        for case in &family.cases {
            validate_id(IdKind::Case, &case.id, errors);
            let qualified = CaseId::new(&family.id, &case.id);
            if !case_ids.insert(qualified.clone()) {
                errors.push(SuiteViolation::DuplicateCase {
                    case: qualified.clone(),
                });
            }
            if case.batch == 0 {
                case_issue(errors, &qualified, CaseProblem::ZeroBatch);
            }
            if case.work.amount == 0 {
                case_issue(errors, &qualified, CaseProblem::ZeroWork);
            }
            if case.maximum_stab_peak_rss_bytes == 0 {
                case_issue(errors, &qualified, CaseProblem::ZeroRss);
            } else if case.maximum_stab_peak_rss_bytes > MAXIMUM_CASE_RSS_BYTES {
                case_issue(errors, &qualified, CaseProblem::RssCap);
            }
            validate_workload(family, case, &qualified, errors);
        }
    }
}

fn validate_workload(family: &Family, case: &Case, qualified: &CaseId, errors: &mut Errors) {
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
                case_issue(errors, qualified, CaseProblem::ShortPipeline);
            }
            for step in steps {
                validate_args(&step.args, qualified, errors);
            }
            if steps
                .first()
                .is_some_and(|step| step.stdin != PipelineInput::Empty)
            {
                case_issue(errors, qualified, CaseProblem::FirstStepStdin);
            }
            if steps
                .iter()
                .skip(1)
                .any(|step| step.stdin != PipelineInput::Previous)
            {
                case_issue(errors, qualified, CaseProblem::LaterStepStdin);
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
                case_issue(errors, qualified, CaseProblem::RustPipelineBand);
            }
        }
        _ => case_issue(errors, qualified, CaseProblem::RunnerMismatch),
    }
}

fn validate_data(recipe: &DataRecipe, qualified: &CaseId, errors: &mut Errors) {
    let probability_is_valid = |value: f64| value.is_finite() && (0.0..=1.0).contains(&value);
    match recipe {
        DataRecipe::Empty => {}
        DataRecipe::GeneratedCircuit { args } => validate_args(args, qualified, errors),
        DataRecipe::GeneratedDem { circuit, analyze } => {
            validate_args(circuit, qualified, errors);
            validate_args(analyze, qualified, errors);
            if analyze.first().map(String::as_str) != Some("analyze_errors") {
                case_issue(errors, qualified, CaseProblem::DemWithoutAnalysis);
            }
        }
        DataRecipe::SampledRecords {
            circuit,
            sample,
            records,
        } => {
            validate_args(circuit, qualified, errors);
            validate_args(sample, qualified, errors);
            let shots = argument_u64(sample, "--shots");
            if sample.first().map(String::as_str) != Some("sample")
                || argument_u64(sample, "--seed").is_none()
                || shots.is_none()
                || shots != u64::try_from(*records).ok()
                || *records == 0
            {
                case_issue(errors, qualified, CaseProblem::SampledRecords);
            }
        }
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
                case_issue(
                    errors,
                    qualified,
                    CaseProblem::InvalidRecipe(RecipeKind::FoldedCircuit),
                );
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
                case_issue(
                    errors,
                    qualified,
                    CaseProblem::InvalidRecipe(RecipeKind::Records),
                );
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
                case_issue(
                    errors,
                    qualified,
                    CaseProblem::InvalidRecipe(RecipeKind::TypedDets),
                );
            }
        }
        DataRecipe::M2dCircuit { bits } => {
            if *bits == 0 {
                case_issue(errors, qualified, CaseProblem::ZeroWidthM2d);
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
                case_issue(
                    errors,
                    qualified,
                    CaseProblem::InvalidRecipe(RecipeKind::Dem),
                );
            }
        }
    }
}

fn validate_output(output: &OutputContract, qualified: &CaseId, errors: &mut Errors) {
    match output {
        OutputContract::Exact { minimum_bytes } => {
            if *minimum_bytes == 0 {
                case_issue(errors, qualified, CaseProblem::VacuousExactOutput);
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
                case_issue(errors, qualified, CaseProblem::InvalidRecordOutput);
            }
        }
    }
}

fn validate_cli_semantic_work(
    case: &Case,
    measured_args: &[String],
    qualified: &CaseId,
    errors: &mut Errors,
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
                case_issue(errors, qualified, CaseProblem::RecordWork);
            }
        }
        WorkUnit::Shots => {
            if argument_u64(measured_args, "--shots") != Some(case.work.amount) {
                case_issue(errors, qualified, CaseProblem::ShotArgument);
            }
            if declared_output_records != Some(case.work.amount) {
                case_issue(errors, qualified, CaseProblem::ShotRecords);
            }
        }
    }
}

fn recipe_record_count(recipe: &DataRecipe) -> Option<u64> {
    let count = match recipe {
        DataRecipe::Records { records, .. }
        | DataRecipe::TypedDets { records, .. }
        | DataRecipe::SampledRecords { records, .. } => *records,
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

fn validate_args(args: &[String], qualified: &CaseId, errors: &mut Errors) {
    if args.is_empty() || args.first().is_none_or(|arg| arg.starts_with('-')) {
        case_issue(errors, qualified, CaseProblem::MissingSubcommand);
    }
    if args.iter().any(|arg| arg.contains('\0')) {
        case_issue(errors, qualified, CaseProblem::NulArgument);
    }
}

fn validate_files(files: &[FileSpec], qualified: &CaseId, errors: &mut Errors) {
    let mut names = BTreeSet::new();
    for file in files {
        validate_id(IdKind::File, &file.name, errors);
        if !names.insert(file.name.as_str()) {
            case_issue(
                errors,
                qualified,
                CaseProblem::RepeatedFile(file.name.clone()),
            );
        }
        match file.role {
            FileRole::Input if file.data.is_none() || file.output.is_some() => case_issue(
                errors,
                qualified,
                CaseProblem::InputFileShape(file.name.clone()),
            ),
            FileRole::Output if file.data.is_some() || file.output.is_none() => case_issue(
                errors,
                qualified,
                CaseProblem::OutputFileShape(file.name.clone()),
            ),
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
    qualified: &CaseId,
    errors: &mut Errors,
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
        case_issue(errors, qualified, CaseProblem::UnknownPlaceholder);
    }
    for file in files {
        let placeholder = format!("{{file:{}}}", file.name);
        if !args.contains(&placeholder) {
            case_issue(
                errors,
                qualified,
                CaseProblem::UnroutedFile(file.name.clone()),
            );
        }
    }
}

fn validate_baselines(baselines: &[SelfBaseline], families: &[Family], errors: &mut Errors) {
    let cases = families
        .iter()
        .flat_map(|family| {
            family.cases.iter().map(|case| {
                (
                    CaseId::new(&family.id, &case.id),
                    (family.id.as_str(), case),
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
        let case = &baseline.case_id;
        if !keys.insert(key) {
            errors.push(SuiteViolation::DuplicateBaseline {
                case: case.clone(),
                architecture: baseline.architecture.clone(),
            });
        }
        match cases
            .get(case)
            .map(|(family_id, definition)| definition.digest(family_id))
        {
            Some(Ok(digest)) if digest == baseline.case_digest => {}
            Some(Ok(_)) => baseline_issue(errors, case, BaselineProblem::StaleDigest),
            Some(Err(source)) => baseline_issue(errors, case, BaselineProblem::Digest(source)),
            None => baseline_issue(errors, case, BaselineProblem::NoCase),
        }
        if baseline.architecture.is_empty()
            || baseline.cpu_model.is_empty()
            || baseline.rustc.is_empty()
            || baseline.target.is_empty()
            || baseline.timing_boundary != TIMING_BOUNDARY
            || baseline.case_digest.len() != 64
        {
            baseline_issue(errors, case, BaselineProblem::IncompleteIdentity);
        }
        for (bound, value) in [
            (BaselineBound::Median, baseline.median_seconds_per_work),
            (BaselineBound::Upper, baseline.upper_seconds_per_work),
        ] {
            if !value.is_finite() || value <= 0.0 {
                baseline_issue(errors, case, BaselineProblem::InvalidBound(bound));
            }
        }
        if baseline.upper_seconds_per_work < baseline.median_seconds_per_work {
            baseline_issue(errors, case, BaselineProblem::UpperBelowMedian);
        }
    }
}

fn validate_id(kind: IdKind, value: &str, errors: &mut Errors) {
    if value.is_empty()
        || value.starts_with('-')
        || value.ends_with('-')
        || value
            .bytes()
            .any(|byte| !byte.is_ascii_lowercase() && !byte.is_ascii_digit() && byte != b'-')
    {
        errors.push(SuiteViolation::InvalidId {
            kind,
            value: value.to_string(),
        });
    }
}
