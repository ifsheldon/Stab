//! Source-owned suite validation: policy floors, ids, workloads, recipes, contracts, and
//! baselines.

use std::collections::{BTreeMap, BTreeSet};

use super::{
    BOOTSTRAP_RESAMPLES, BOOTSTRAP_SEED, COMMAND_TIMEOUT_SECONDS, CONFIDENCE_LEVEL, Case, CaseId,
    Comparator, DataRecipe, Family, FileRole, FileSpec, MAX_RELEASE_CASES, MAX_RELEASE_FAMILIES,
    MAXIMUM_CASE_RSS_BYTES, MAXIMUM_OUTPUT_BYTES, MAXIMUM_TEMPERATURE_MILLIDEGREES, OutputContract,
    PARITY_MAX_RATIO, PipelineInput, Policy, RecordFormat, RunnerKind, SELF_REGRESSION_MAX_RATIO,
    STIM_COMMIT, STIM_VERSION, SUITE_SCHEMA_VERSION, SelfBaseline, Suite, TIMING_BOUNDARY, Tier,
    WorkUnit, Workload,
};

impl Suite {
    pub(in crate::e2e) fn validate(&self) -> Result<(), String> {
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
        errors.is_empty().ok_or_else(|| errors.join("\n"))
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
            let qualified = CaseId::new(&family.id, &case.id).to_string();
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
        DataRecipe::GeneratedDem { circuit, analyze } => {
            validate_args(circuit, qualified, errors);
            validate_args(analyze, qualified, errors);
            if analyze.first().map(String::as_str) != Some("analyze_errors") {
                errors.push(format!(
                    "case {qualified} DEM recipe must run analyze_errors"
                ));
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
                errors.push(format!(
                    "case {qualified} sampled-record recipe needs seeded sample with --shots equal to records"
                ));
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
            family
                .cases
                .iter()
                .map(|case| (CaseId::new(&family.id, &case.id), case.digest(&family.id)))
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
