use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::artifact::MAX_REPORT_BYTES;
use super::artifact::QualificationOutputDir;
use super::artifact_locator::ReportRootRelativePath;
use super::model::{
    Comparator, DeferredProduct, EvidenceSelector, ExecutionTier, FeatureId, SelectorKind,
    UpstreamDisposition,
};
use super::receipt::{
    ExecutionReceipt, ExecutionVerdict, RunCompletionReceipt, StatisticalAttemptReceipt,
    StatisticalAttemptVerdict,
};

mod support;

use support::{canonical_json, completed_cases, expected_case_counts, is_sha256, sha256};
pub(super) use support::{regenerate, selector_sha256, validate_preflight};

const REPORT_SCHEMA_VERSION: u32 = 7;
const PREFLIGHT_SCHEMA_VERSION: u32 = 7;
const MAX_CASE_RESULTS: usize = 8_192;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct QualificationReport {
    pub(super) schema_version: u32,
    pub(super) qualification_manifest_digest: String,
    pub(super) run_request_sha256: String,
    pub(super) stab_commit: String,
    pub(super) local_modifications: bool,
    pub(super) stim_tag: String,
    pub(super) stim_commit: String,
    pub(super) rust_toolchain: String,
    pub(super) target_triple: String,
    pub(super) operating_system: String,
    pub(super) architecture: String,
    pub(super) tier: ExecutionTier,
    pub(super) feature_filters: Vec<FeatureId>,
    pub(super) case_filters: Vec<String>,
    pub(super) allow_deferred: bool,
    pub(super) selected_count: usize,
    pub(super) planned_count: usize,
    pub(super) deferred_count: usize,
    pub(super) passed_count: usize,
    pub(super) failed_count: usize,
    pub(super) selection_complete: bool,
    pub(super) statistical_declared_budget: ProbabilityBound,
    pub(super) statistical_consumed_bound: ProbabilityBound,
    pub(super) statistical_planned_shots: u64,
    pub(super) statistical_planned_seeds: BTreeMap<String, Vec<u64>>,
    pub(super) statistical_shots: u64,
    pub(super) statistical_seeds: BTreeMap<String, Vec<u64>>,
    pub(super) statistical_attempts: Vec<StatisticalAttempt>,
    pub(super) property_corpus_ids: Vec<String>,
    pub(super) resource_case_count: usize,
    pub(super) upstream_dispositions: Vec<DomainDispositionCount>,
    pub(super) deferred_products: BTreeMap<String, usize>,
    pub(super) case_counts: Vec<DomainComparatorCount>,
    pub(super) resource_contracts: Vec<ResourceExecution>,
    pub(super) results: Vec<CaseResult>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DomainComparatorCount {
    pub(super) feature_id: FeatureId,
    pub(super) comparator: Comparator,
    pub(super) passed: usize,
    pub(super) failed: usize,
    pub(super) planned: usize,
    pub(super) deferred: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DomainDispositionCount {
    pub(super) feature_id: FeatureId,
    pub(super) disposition: UpstreamDisposition,
    pub(super) count: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ResourceExecution {
    pub(super) case_id: String,
    pub(super) kind: String,
    pub(super) detail: String,
    pub(super) negative_axes: Vec<String>,
    pub(super) timeout_ms: u64,
    pub(super) stdout_limit_bytes: usize,
    pub(super) stderr_limit_bytes: usize,
    pub(super) artifact_limit_bytes: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StatisticalAttempt {
    pub(super) case_id: String,
    pub(super) seed: u64,
    pub(super) completed_shots: u64,
    pub(super) completed_comparisons: u32,
    pub(super) completed_batches: u32,
    pub(super) outcome: CaseOutcome,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ArtifactRecord {
    pub(super) path: ReportRootRelativePath,
    pub(super) bytes: usize,
    pub(super) sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CaseResult {
    pub(super) case_id: String,
    pub(super) feature_id: FeatureId,
    pub(super) comparator: Comparator,
    pub(super) selector: EvidenceSelector,
    pub(super) selector_sha256: String,
    pub(super) execution_receipt_sha256: String,
    pub(super) outcome: CaseOutcome,
    pub(super) exact_test_count: Option<usize>,
    pub(super) stdout_sha256: Option<String>,
    pub(super) stderr_sha256: Option<String>,
    pub(super) artifacts: Vec<ArtifactRecord>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum CaseOutcome {
    Passed,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct ProbabilityBound(f64);

impl Eq for ProbabilityBound {}

impl ProbabilityBound {
    pub(super) fn try_new(value: f64) -> Result<Self, ReportError> {
        if value.is_finite() && !value.is_sign_negative() && (0.0..=1.0).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ReportError::InvalidProbability(value))
        }
    }

    pub(super) const fn zero() -> Self {
        Self(0.0)
    }

    pub(super) const fn get(self) -> f64 {
        self.0
    }
}

impl Serialize for ProbabilityBound {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&format!("{:.17e}", self.0))
    }
}

impl<'de> Deserialize<'de> for ProbabilityBound {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        if value.len() > 32 {
            return Err(serde::de::Error::custom(
                "probability bound exceeds 32 canonical bytes",
            ));
        }
        let parsed = value.parse::<f64>().map_err(serde::de::Error::custom)?;
        Self::try_new(parsed).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct CorrectnessPreflight {
    schema_version: u32,
    report_sha256: String,
    completion_sha256: String,
    qualification_manifest_digest: String,
    run_request_sha256: String,
    stab_commit: String,
    local_modifications: bool,
    stim_commit: String,
    tier: ExecutionTier,
    allow_deferred: bool,
    selection_complete: bool,
    deferred_count: usize,
    cases: BTreeMap<String, CorrectnessPreflightCase>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct CorrectnessPreflightCase {
    outcome: CaseOutcome,
    selector_sha256: String,
    execution_receipt_sha256: String,
    stdout_sha256: Option<String>,
    stderr_sha256: Option<String>,
}

#[derive(Debug, Error)]
pub(crate) enum ReportError {
    #[error(transparent)]
    Artifact(#[from] super::artifact::ArtifactError),

    #[error("qualification report JSON is invalid: {0}")]
    Json(#[from] serde_json::Error),

    #[error("qualification report probability {0} is outside [0, 1]")]
    InvalidProbability(f64),

    #[error("qualification report validation failed:\n{0}")]
    Validation(Box<str>),

    #[error("correctness preflight failed:\n{0}")]
    Preflight(Box<str>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PublicationDigests {
    pub(super) report_sha256: String,
    pub(super) completion_sha256: String,
}

impl QualificationReport {
    pub(super) fn new(metadata: ReportMetadata, selection: SelectionSummary) -> Self {
        Self {
            schema_version: REPORT_SCHEMA_VERSION,
            qualification_manifest_digest: metadata.qualification_manifest_digest,
            run_request_sha256: String::new(),
            stab_commit: metadata.stab_commit,
            local_modifications: metadata.local_modifications,
            stim_tag: metadata.stim_tag,
            stim_commit: metadata.stim_commit,
            rust_toolchain: metadata.rust_toolchain,
            target_triple: metadata.target_triple,
            operating_system: metadata.operating_system,
            architecture: metadata.architecture,
            tier: selection.tier,
            feature_filters: selection.feature_filters,
            case_filters: selection.case_filters,
            allow_deferred: selection.allow_deferred,
            selected_count: selection.selected_count,
            planned_count: selection.planned_count,
            deferred_count: selection.deferred_count,
            passed_count: 0,
            failed_count: 0,
            selection_complete: false,
            statistical_declared_budget: ProbabilityBound::zero(),
            statistical_consumed_bound: ProbabilityBound::zero(),
            statistical_planned_shots: 0,
            statistical_planned_seeds: BTreeMap::new(),
            statistical_shots: 0,
            statistical_seeds: BTreeMap::new(),
            statistical_attempts: Vec::new(),
            property_corpus_ids: Vec::new(),
            resource_case_count: 0,
            upstream_dispositions: Vec::new(),
            deferred_products: BTreeMap::new(),
            case_counts: Vec::new(),
            resource_contracts: Vec::new(),
            results: Vec::new(),
        }
    }

    pub(super) fn finish(&mut self) {
        self.passed_count = self
            .results
            .iter()
            .filter(|result| result.outcome == CaseOutcome::Passed)
            .count();
        self.failed_count = self.results.len().saturating_sub(self.passed_count);
        self.selection_complete = self.results.len() == self.selected_count;
    }

    pub(super) fn validate(
        &self,
        output: &QualificationOutputDir,
        expectation: &ReportExpectation,
    ) -> Result<(), ReportError> {
        let mut violations = Vec::new();
        if self.schema_version != REPORT_SCHEMA_VERSION {
            violations.push(format!(
                "schema_version is {}, expected {REPORT_SCHEMA_VERSION}",
                self.schema_version
            ));
        }
        if self.results.len() > MAX_CASE_RESULTS {
            violations.push(format!(
                "report has {} results; limit is {MAX_CASE_RESULTS}",
                self.results.len()
            ));
        }
        if self.qualification_manifest_digest != expectation.metadata.qualification_manifest_digest
            || self.stab_commit != expectation.metadata.stab_commit
            || self.local_modifications != expectation.metadata.local_modifications
            || self.stim_tag != expectation.metadata.stim_tag
            || self.stim_commit != expectation.metadata.stim_commit
            || self.rust_toolchain != expectation.metadata.rust_toolchain
            || self.target_triple != expectation.metadata.target_triple
            || self.operating_system != expectation.metadata.operating_system
            || self.architecture != expectation.metadata.architecture
        {
            violations.push(
                "report environment metadata disagrees with the checked manifest or current repository"
                    .to_string(),
            );
        }
        if self.run_request_sha256 != expectation.run_request_sha256
            || !is_sha256(&self.run_request_sha256)
        {
            violations.push("report run-request digest is stale or malformed".to_string());
        }
        if self.tier != expectation.tier
            || self.feature_filters != expectation.feature_filters
            || self.case_filters != expectation.case_filters
            || self.allow_deferred != expectation.allow_deferred
        {
            violations.push(
                "report selection metadata disagrees with reconstructed manifest selection"
                    .to_string(),
            );
        }
        if self.selected_count != expectation.selected_cases.len()
            || self.planned_count != expectation.planned_cases.len()
            || self.deferred_count != expectation.deferred_cases.len()
        {
            violations.push(
                "report selection counts disagree with reconstructed manifest selection"
                    .to_string(),
            );
        }
        if self.statistical_declared_budget.get() != expectation.statistical_declared_budget
            || self.statistical_planned_shots != expectation.statistical_planned_shots
            || self.statistical_planned_seeds != expectation.statistical_planned_seeds
        {
            violations
                .push("report statistical plan disagrees with frozen source plans".to_string());
        }
        if self.property_corpus_ids != expectation.property_corpus_ids {
            violations.push("report property corpus ids disagree with selected cases".to_string());
        }
        if self.resource_case_count != expectation.resource_contracts.len()
            || self.resource_contracts != expectation.resource_contracts
        {
            violations.push("report resource contracts disagree with selected cases".to_string());
        }
        if self.upstream_dispositions != expectation.upstream_dispositions {
            violations.push("report upstream dispositions disagree with the manifest".to_string());
        }
        if self.deferred_products != expectation.deferred_products {
            violations.push(
                "report deferred products disagree with selected deferred evidence".to_string(),
            );
        }
        if self.resource_case_count != self.resource_contracts.len() {
            violations
                .push("resource_case_count disagrees with resource contract evidence".to_string());
        }
        let mut ids = BTreeSet::new();
        for result in &self.results {
            if !ids.insert(result.case_id.as_str()) {
                violations.push(format!("duplicate case result {:?}", result.case_id));
            }
            if !is_sha256(&result.selector_sha256) {
                violations.push(format!(
                    "case {:?} selector digest is malformed",
                    result.case_id
                ));
            }
            let expected_case = expectation.selected_cases.get(&result.case_id);
            match expected_case {
                Some(expected)
                    if result.feature_id == expected.feature_id
                        && result.comparator == expected.comparator
                        && result.selector == expected.selector
                        && result.selector_sha256 == expected.selector_sha256 => {}
                Some(_) => violations.push(format!(
                    "case {:?} metadata or selector disagrees with the checked manifest",
                    result.case_id
                )),
                None => violations.push(format!(
                    "case {:?} is not selected by the checked manifest and report filters",
                    result.case_id
                )),
            }
            if result.outcome == CaseOutcome::Passed
                && (!result.stdout_sha256.as_deref().is_some_and(is_sha256)
                    || !result.stderr_sha256.as_deref().is_some_and(is_sha256))
            {
                violations.push(format!(
                    "passed case {:?} is missing a valid stdout or stderr digest",
                    result.case_id
                ));
            }
            match ExecutionReceipt::read(output, &result.case_id) {
                Ok((receipt, digest)) => {
                    if !receipt.schema_is_current()
                        || digest != result.execution_receipt_sha256
                        || !is_sha256(&result.execution_receipt_sha256)
                        || receipt.run_request_sha256 != self.run_request_sha256
                        || receipt.case_id != result.case_id
                        || receipt.selector_sha256 != result.selector_sha256
                        || receipt.executables != expectation.executables
                        || receipt.execution_environment_sha256
                            != expectation.execution_environment_sha256
                        || receipt.exact_test_count != result.exact_test_count
                    {
                        violations.push(format!(
                            "case {:?} execution receipt is stale or disagrees with its report binding",
                            result.case_id
                        ));
                    }
                    let receipt_outcome = match receipt.verdict {
                        ExecutionVerdict::Accepted => CaseOutcome::Passed,
                        ExecutionVerdict::Rejected | ExecutionVerdict::InfrastructureFailure => {
                            CaseOutcome::Failed
                        }
                    };
                    if receipt_outcome != result.outcome
                        || (receipt.verdict == ExecutionVerdict::Accepted
                            && expected_case.is_none_or(|expected| {
                                receipt.exit_status != Some(expected.expected_exit_status)
                            }))
                        || (receipt.verdict == ExecutionVerdict::InfrastructureFailure
                            && receipt.exit_status.is_some())
                    {
                        violations.push(format!(
                            "case {:?} outcome is not derivable from its execution receipt",
                            result.case_id
                        ));
                    }
                    if result.selector.kind == SelectorKind::CargoTest
                        && receipt.verdict == ExecutionVerdict::Accepted
                        && receipt.exact_test_count != Some(1)
                    {
                        violations.push(format!(
                            "case {:?} accepted Cargo receipt does not prove exactly one test",
                            result.case_id
                        ));
                    }
                    let expected_attempts = self
                        .statistical_attempts
                        .iter()
                        .filter(|attempt| attempt.case_id == result.case_id)
                        .map(|attempt| StatisticalAttemptReceipt {
                            seed: attempt.seed,
                            verdict: match attempt.outcome {
                                CaseOutcome::Passed => StatisticalAttemptVerdict::Passed,
                                CaseOutcome::Failed => StatisticalAttemptVerdict::Failed,
                            },
                            completed_shots: attempt.completed_shots,
                            completed_comparisons: attempt.completed_comparisons,
                            completed_batches: attempt.completed_batches,
                        })
                        .collect::<Vec<_>>();
                    if receipt.statistical_attempts != expected_attempts {
                        violations.push(format!(
                            "case {:?} statistical attempts disagree with its execution receipt",
                            result.case_id
                        ));
                    }
                    for (label, stream, reported) in [
                        (
                            "stdout",
                            receipt.stdout.as_ref(),
                            result.stdout_sha256.as_ref(),
                        ),
                        (
                            "stderr",
                            receipt.stderr.as_ref(),
                            result.stderr_sha256.as_ref(),
                        ),
                    ] {
                        if stream.map(|value| &value.sha256) != reported
                            || stream.is_some_and(|value| !is_sha256(&value.sha256))
                            || (receipt.verdict != ExecutionVerdict::InfrastructureFailure
                                && stream.is_some_and(|value| !value.complete))
                        {
                            violations.push(format!(
                                "case {:?} {label} digest or completion disagrees with its execution receipt",
                                result.case_id
                            ));
                        }
                        let maximum = expected_case.map_or(0, |expected| match label {
                            "stdout" => expected.stdout_receipt_limit_bytes,
                            "stderr" => expected.stderr_receipt_limit_bytes,
                            _ => 0,
                        });
                        if stream.is_some_and(|value| value.bytes > maximum) {
                            violations.push(format!(
                                "case {:?} {label} receipt exceeds its {maximum}-byte execution contract",
                                result.case_id
                            ));
                        }
                    }
                    let receipt_artifacts_match = receipt.auxiliary_outputs.len()
                        == result.artifacts.len()
                        && receipt.auxiliary_outputs.iter().zip(&result.artifacts).all(
                            |(receipt, reported)| {
                                receipt.path == reported.path
                                    && receipt.bytes == reported.bytes
                                    && receipt.sha256 == reported.sha256
                            },
                        );
                    if !receipt_artifacts_match {
                        violations.push(format!(
                            "case {:?} auxiliary outputs disagree with its execution receipt",
                            result.case_id
                        ));
                    }
                }
                Err(source) => violations.push(format!(
                    "case {:?} execution receipt cannot be validated: {source}",
                    result.case_id
                )),
            }
            let mut artifact_bytes = 0_usize;
            let mut remaining_artifact_bytes = expected_case
                .map(|expected| expected.artifact_limit_bytes)
                .unwrap_or(0);
            let mut artifact_paths = BTreeSet::new();
            let case_artifact_root = Path::new("cases").join(result.case_id.as_str());
            for artifact in &result.artifacts {
                if !artifact.path.as_path().starts_with(&case_artifact_root) {
                    violations.push(format!(
                        "case {:?} has unsafe artifact path {:?}",
                        result.case_id, artifact.path
                    ));
                    continue;
                }
                if !artifact_paths.insert(artifact.path.as_path()) {
                    violations.push(format!(
                        "case {:?} repeats artifact path {:?}",
                        result.case_id, artifact.path
                    ));
                }
                if !is_sha256(&artifact.sha256) {
                    violations.push(format!(
                        "case {:?} artifact {:?} has a malformed digest",
                        result.case_id, artifact.path
                    ));
                }
                if artifact.bytes > remaining_artifact_bytes {
                    violations.push(format!(
                        "case {:?} artifact {:?} claims {} bytes with only {} bytes remaining in its contract",
                        result.case_id,
                        artifact.path,
                        artifact.bytes,
                        remaining_artifact_bytes
                    ));
                    continue;
                }
                let Some(next_total) = artifact_bytes.checked_add(artifact.bytes) else {
                    violations.push(format!(
                        "case {:?} artifact byte count overflowed",
                        result.case_id
                    ));
                    continue;
                };
                artifact_bytes = next_total;
                remaining_artifact_bytes -= artifact.bytes;
                match output.read(artifact.path.as_path(), artifact.bytes) {
                    Ok(bytes)
                        if bytes.len() == artifact.bytes && sha256(&bytes) == artifact.sha256 => {}
                    Ok(_) => violations.push(format!(
                        "case {:?} artifact {:?} disagrees with its size or digest",
                        result.case_id, artifact.path
                    )),
                    Err(source) => violations.push(format!(
                        "case {:?} artifact {:?} cannot be validated: {source}",
                        result.case_id, artifact.path
                    )),
                }
            }
            if result.outcome == CaseOutcome::Passed && !result.artifacts.is_empty() {
                violations.push(format!(
                    "passing case {:?} unexpectedly retained failure artifacts",
                    result.case_id
                ));
            }
            if let Some(expected) = expected_case
                && artifact_bytes > expected.artifact_limit_bytes
            {
                violations.push(format!(
                    "case {:?} retained {artifact_bytes} artifact bytes, exceeding its {}-byte contract",
                    result.case_id, expected.artifact_limit_bytes
                ));
            }
        }
        if ids
            != expectation
                .selected_cases
                .keys()
                .map(String::as_str)
                .collect()
        {
            violations.push(
                "report results do not exactly cover the reconstructed selected case ids"
                    .to_string(),
            );
        }
        let passed = self
            .results
            .iter()
            .filter(|result| result.outcome == CaseOutcome::Passed)
            .count();
        let failed = self.results.len().saturating_sub(passed);
        if passed != self.passed_count || failed != self.failed_count {
            violations.push("reported pass/fail counts disagree with case outcomes".to_string());
        }
        let counted = self.case_counts.iter().fold([0usize; 4], |mut total, row| {
            total[0] += row.passed;
            total[1] += row.failed;
            total[2] += row.planned;
            total[3] += row.deferred;
            total
        });
        if counted
            != [
                self.passed_count,
                self.failed_count,
                self.planned_count,
                self.deferred_count,
            ]
        {
            violations.push("domain/comparator counts disagree with report totals".to_string());
        }
        if self.case_counts != expected_case_counts(self, expectation) {
            violations.push(
                "domain/comparator counts disagree with reconstructed case ownership".to_string(),
            );
        }
        let complete = self.results.len() == self.selected_count;
        if complete != self.selection_complete {
            violations.push("selection_complete disagrees with selected case results".to_string());
        }
        if !self.case_filters.is_empty()
            && self.case_filters.len() != self.selected_count.saturating_add(self.deferred_count)
        {
            violations.push(
                "explicit case filters do not match selected plus permitted deferred cases"
                    .to_string(),
            );
        }
        for (label, bound) in [
            ("declared", self.statistical_declared_budget.get()),
            ("consumed", self.statistical_consumed_bound.get()),
        ] {
            if !bound.is_finite() || !(0.0..=1e-4).contains(&bound) {
                violations.push(format!(
                    "statistical suite {label} bound is outside 0..=1e-4"
                ));
            }
        }
        if self.statistical_consumed_bound.get() > self.statistical_declared_budget.get() {
            violations.push("statistical consumed bound exceeds the declared budget".to_string());
        }
        for (label, panels) in [
            ("planned", &self.statistical_planned_seeds),
            ("executed", &self.statistical_seeds),
        ] {
            for (case_id, seeds) in panels {
                let unique = seeds.iter().collect::<BTreeSet<_>>();
                if seeds.is_empty() || unique.len() != seeds.len() {
                    violations.push(format!(
                        "statistical case {case_id:?} has an empty or duplicate {label} seed panel"
                    ));
                }
                if !self.results.iter().any(|result| {
                    result.case_id == *case_id && result.comparator == Comparator::Statistical
                }) {
                    violations.push(format!(
                        "statistical {label} seed panel {case_id:?} has no selected statistical result"
                    ));
                }
            }
        }
        let mut attempted_seeds = BTreeMap::<String, Vec<u64>>::new();
        let mut attempted_shots = 0_u64;
        let mut consumed_bound = 0.0_f64;
        let mut attempt_keys = BTreeSet::new();
        let mut terminal_failures = BTreeSet::new();
        for attempt in &self.statistical_attempts {
            if !attempt_keys.insert((attempt.case_id.as_str(), attempt.seed)) {
                violations.push(format!(
                    "statistical case {:?} repeats executed seed {}",
                    attempt.case_id, attempt.seed
                ));
            }
            let contract = (
                expectation
                    .statistical_shots_per_batch
                    .get(&attempt.case_id),
                expectation
                    .statistical_comparisons_per_attempt
                    .get(&attempt.case_id),
                expectation
                    .statistical_batches_per_attempt
                    .get(&attempt.case_id),
                expectation
                    .statistical_shots_per_attempt
                    .get(&attempt.case_id),
                expectation
                    .statistical_exact_bound_per_attempt
                    .get(&attempt.case_id),
            );
            match contract {
                (
                    Some(shots_per_batch),
                    Some(comparisons_per_attempt),
                    Some(batches_per_attempt),
                    Some(shots_per_attempt),
                    Some(exact_bound_per_attempt),
                ) => {
                    let batches_per_comparison = if *comparisons_per_attempt != 0
                        && batches_per_attempt.is_multiple_of(*comparisons_per_attempt)
                    {
                        Some(*batches_per_attempt / *comparisons_per_attempt)
                    } else {
                        None
                    };
                    let expected_completed_batches = batches_per_comparison
                        .and_then(|batches| attempt.completed_comparisons.checked_mul(batches));
                    let expected_completed_shots =
                        shots_per_batch.checked_mul(u64::from(attempt.completed_batches));
                    let completion_valid = expected_completed_shots
                        == Some(attempt.completed_shots)
                        && expected_completed_batches == Some(attempt.completed_batches)
                        && attempt.completed_comparisons <= *comparisons_per_attempt
                        && attempt.completed_batches <= *batches_per_attempt
                        && (attempt.outcome == CaseOutcome::Failed
                            || (attempt.completed_shots == *shots_per_attempt
                                && attempt.completed_comparisons == *comparisons_per_attempt
                                && attempt.completed_batches == *batches_per_attempt));
                    if completion_valid {
                        if let Some(total) = attempted_shots.checked_add(attempt.completed_shots) {
                            attempted_shots = total;
                        } else {
                            violations
                                .push("statistical attempted shot total overflowed".to_string());
                        }
                        consumed_bound += exact_bound_per_attempt
                            * f64::from(attempt.completed_comparisons)
                            / f64::from(*comparisons_per_attempt);
                    } else {
                        violations.push(format!(
                            "statistical case {:?} reports completion outside its frozen work contract",
                            attempt.case_id
                        ));
                    }
                }
                _ => violations.push(format!(
                    "statistical attempt for {:?} has no complete frozen work contract",
                    attempt.case_id
                )),
            }
            attempted_seeds
                .entry(attempt.case_id.clone())
                .or_default()
                .push(attempt.seed);
            if terminal_failures.contains(attempt.case_id.as_str()) {
                violations.push(format!(
                    "statistical case {:?} has an attempt after a terminal failure",
                    attempt.case_id
                ));
            }
            if attempt.outcome == CaseOutcome::Failed {
                terminal_failures.insert(attempt.case_id.as_str());
            }
        }
        if attempted_shots != self.statistical_shots || attempted_seeds != self.statistical_seeds {
            violations.push(
                "statistical executed shots or seeds disagree with per-attempt evidence"
                    .to_string(),
            );
        }
        if consumed_bound != self.statistical_consumed_bound.get() {
            violations.push(format!(
                "statistical consumed bound disagrees with completed per-attempt evidence: report={:.17e} recomputed={consumed_bound:.17e}",
                self.statistical_consumed_bound.get()
            ));
        }
        if self.statistical_shots > self.statistical_planned_shots {
            violations.push("statistical executed shots exceed the frozen plan".to_string());
        }
        for (case_id, actual) in &self.statistical_seeds {
            let Some(planned) = self.statistical_planned_seeds.get(case_id) else {
                violations.push(format!(
                    "statistical executed case {case_id:?} has no frozen seed panel"
                ));
                continue;
            };
            if !planned.starts_with(actual) {
                violations.push(format!(
                    "statistical executed seeds for {case_id:?} are not a prefix of the frozen panel"
                ));
            }
        }
        for result in self
            .results
            .iter()
            .filter(|result| result.comparator == Comparator::Statistical)
        {
            if result.outcome == CaseOutcome::Passed
                && terminal_failures.contains(result.case_id.as_str())
            {
                violations.push(format!(
                    "passing statistical case {:?} contains a failed attempt",
                    result.case_id
                ));
            }
            if result.outcome == CaseOutcome::Passed
                && self.statistical_seeds.get(&result.case_id)
                    != self.statistical_planned_seeds.get(&result.case_id)
            {
                violations.push(format!(
                    "passing statistical case {:?} did not execute its complete frozen seed panel",
                    result.case_id
                ));
            }
        }
        if violations.is_empty() {
            Ok(())
        } else {
            Err(ReportError::Validation(
                violations.join("\n").into_boxed_str(),
            ))
        }
    }

    pub(super) fn publish(
        &self,
        output: &QualificationOutputDir,
        expectation: &ReportExpectation,
    ) -> Result<PublicationDigests, ReportError> {
        self.validate(output, expectation)?;
        let json = canonical_json(self)?;
        let report_sha256 = sha256(&json);
        output.write(Path::new("report.json"), &json)?;
        output.write(Path::new("report.md"), self.markdown().as_bytes())?;
        let completion = RunCompletionReceipt::new(
            self.run_request_sha256.clone(),
            report_sha256.clone(),
            completed_cases(self),
        );
        let completion_sha256 = completion
            .publish(output)
            .map_err(|source| ReportError::Validation(source.to_string().into_boxed_str()))?;
        let preflight = CorrectnessPreflight::from_report(
            self,
            report_sha256.clone(),
            completion_sha256.clone(),
        );
        let bytes = canonical_json(&preflight)?;
        output.write(Path::new("preflight.json"), &bytes)?;
        Ok(PublicationDigests {
            report_sha256,
            completion_sha256,
        })
    }

    pub(super) fn read(output: &QualificationOutputDir) -> Result<Self, ReportError> {
        let bytes = output.read(Path::new("report.json"), MAX_REPORT_BYTES)?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    pub(super) fn markdown(&self) -> String {
        let mut text = String::new();
        text.push_str("# Stab Correctness Qualification Report\n\n");
        text.push_str(&format!("- Tier: `{}`\n", self.tier.as_str()));
        text.push_str(&format!("- Stab commit: `{}`\n", self.stab_commit));
        text.push_str(&format!(
            "- Local modifications: `{}`\n",
            self.local_modifications
        ));
        text.push_str(&format!(
            "- Stim: `{}` at `{}`\n",
            self.stim_tag, self.stim_commit
        ));
        text.push_str(&format!(
            "- Selection: {} selected, {} passed, {} failed, {} planned, {} deferred\n",
            self.selected_count,
            self.passed_count,
            self.failed_count,
            self.planned_count,
            self.deferred_count
        ));
        text.push_str(&format!(
            "- Statistical bound: declared `{:.6e}`, consumed `{:.6e}`\n",
            self.statistical_declared_budget.get(),
            self.statistical_consumed_bound.get()
        ));
        text.push_str(&format!(
            "- Statistical shots: `{}` executed of `{}` planned\n\n",
            self.statistical_shots, self.statistical_planned_shots
        ));
        text.push_str("## Inventory\n\n");
        for row in &self.upstream_dispositions {
            text.push_str(&format!(
                "- `{}` `{}`: `{}`\n",
                row.feature_id.as_str(),
                row.disposition.as_str(),
                row.count
            ));
        }
        text.push('\n');
        text.push_str("## Cases\n\n");
        for result in &self.results {
            let outcome = match result.outcome {
                CaseOutcome::Passed => "PASS",
                CaseOutcome::Failed => "FAIL",
            };
            text.push_str(&format!(
                "- `{outcome}` `{}` `{}` `{:?}`\n",
                result.case_id,
                result.feature_id.as_str(),
                result.comparator
            ));
        }
        text
    }
}

impl CorrectnessPreflight {
    fn from_report(
        report: &QualificationReport,
        report_sha256: String,
        completion_sha256: String,
    ) -> Self {
        Self {
            schema_version: PREFLIGHT_SCHEMA_VERSION,
            report_sha256,
            completion_sha256,
            qualification_manifest_digest: report.qualification_manifest_digest.clone(),
            run_request_sha256: report.run_request_sha256.clone(),
            stab_commit: report.stab_commit.clone(),
            local_modifications: report.local_modifications,
            stim_commit: report.stim_commit.clone(),
            tier: report.tier,
            allow_deferred: report.allow_deferred,
            selection_complete: report.selection_complete,
            deferred_count: report.deferred_count,
            cases: report
                .results
                .iter()
                .map(|result| {
                    (
                        result.case_id.clone(),
                        CorrectnessPreflightCase {
                            outcome: result.outcome,
                            selector_sha256: result.selector_sha256.clone(),
                            execution_receipt_sha256: result.execution_receipt_sha256.clone(),
                            stdout_sha256: result.stdout_sha256.clone(),
                            stderr_sha256: result.stderr_sha256.clone(),
                        },
                    )
                })
                .collect(),
        }
    }

    fn validate_cases(
        &self,
        expectation: &PreflightExpectation,
        expected_report_sha256: &str,
    ) -> Result<(), ReportError> {
        let mut violations = Vec::new();
        if self.schema_version != PREFLIGHT_SCHEMA_VERSION {
            violations.push("preflight schema version is stale".to_string());
        }
        if self.report_sha256 != expected_report_sha256 {
            violations.push("preflight report digest is stale".to_string());
        }
        if self.qualification_manifest_digest != expectation.manifest_digest {
            violations.push("preflight qualification manifest digest is stale".to_string());
        }
        if self.run_request_sha256 != expectation.run_request_sha256 {
            violations.push("preflight run-request digest is stale".to_string());
        }
        if self.stab_commit != expectation.stab_commit {
            violations.push("preflight Stab commit is stale".to_string());
        }
        if self.stim_commit != expectation.stim_commit {
            violations.push("preflight Stim commit is stale".to_string());
        }
        if self.local_modifications && !expectation.allow_dirty {
            violations.push("preflight report was recorded with local modifications".to_string());
        }
        if expectation.current_worktree_dirty && !expectation.allow_dirty {
            violations.push("current worktree has local modifications".to_string());
        }
        if !self.selection_complete {
            violations.push("preflight report is a partial run".to_string());
        }
        if self.allow_deferred {
            violations.push("preflight report used --allow-deferred".to_string());
        }
        if self.deferred_count != 0 {
            violations.push("preflight report contains deferred cases".to_string());
        }
        for case in &expectation.cases {
            match self.cases.get(case) {
                Some(result) if result.outcome == CaseOutcome::Passed => {
                    match expectation.selectors.get(case) {
                        Some(expected) if result.selector_sha256 == *expected => {}
                        Some(_) => violations.push(format!(
                            "correctness prerequisite {case:?} has a stale selector digest"
                        )),
                        None => violations.push(format!(
                            "correctness prerequisite {case:?} is absent from the checked manifest"
                        )),
                    }
                    if !result.stdout_sha256.as_deref().is_some_and(is_sha256)
                        || !result.stderr_sha256.as_deref().is_some_and(is_sha256)
                    {
                        violations.push(format!(
                            "correctness prerequisite {case:?} lacks valid output digests"
                        ));
                    }
                }
                Some(_) => {
                    violations.push(format!("correctness prerequisite {case:?} failed"));
                }
                None => violations.push(format!(
                    "correctness prerequisite {case:?} is missing from the report"
                )),
            }
        }
        if violations.is_empty() {
            Ok(())
        } else {
            Err(ReportError::Preflight(
                violations.join("\n").into_boxed_str(),
            ))
        }
    }
}

pub(super) struct PreflightExpectation {
    pub(super) manifest_digest: String,
    pub(super) run_request_sha256: String,
    pub(super) stab_commit: String,
    pub(super) stim_commit: String,
    pub(super) selectors: BTreeMap<String, String>,
    pub(super) current_worktree_dirty: bool,
    pub(super) allow_dirty: bool,
    pub(super) cases: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ReportMetadata {
    pub(super) qualification_manifest_digest: String,
    pub(super) stab_commit: String,
    pub(super) local_modifications: bool,
    pub(super) stim_tag: String,
    pub(super) stim_commit: String,
    pub(super) rust_toolchain: String,
    pub(super) target_triple: String,
    pub(super) operating_system: String,
    pub(super) architecture: String,
}

pub(super) struct SelectionSummary {
    pub(super) tier: ExecutionTier,
    pub(super) feature_filters: Vec<FeatureId>,
    pub(super) case_filters: Vec<String>,
    pub(super) allow_deferred: bool,
    pub(super) selected_count: usize,
    pub(super) planned_count: usize,
    pub(super) deferred_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ExpectedCase {
    pub(super) feature_id: FeatureId,
    pub(super) comparator: Comparator,
    pub(super) selector: EvidenceSelector,
    pub(super) selector_sha256: String,
    pub(super) expected_exit_status: i32,
    pub(super) artifact_limit_bytes: usize,
    pub(super) stdout_receipt_limit_bytes: u64,
    pub(super) stderr_receipt_limit_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ExpectedDispositionCase {
    pub(super) feature_id: FeatureId,
    pub(super) comparator: Comparator,
    pub(super) deferred_product: Option<DeferredProduct>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ReportExpectation {
    pub(super) metadata: ReportMetadata,
    pub(super) run_request_sha256: String,
    pub(super) executables: Vec<super::receipt::ExecutableIdentity>,
    pub(super) execution_environment_sha256: String,
    pub(super) tier: ExecutionTier,
    pub(super) feature_filters: Vec<FeatureId>,
    pub(super) case_filters: Vec<String>,
    pub(super) allow_deferred: bool,
    pub(super) selected_cases: BTreeMap<String, ExpectedCase>,
    pub(super) planned_cases: Vec<ExpectedDispositionCase>,
    pub(super) deferred_cases: Vec<ExpectedDispositionCase>,
    pub(super) statistical_declared_budget: f64,
    pub(super) statistical_planned_shots: u64,
    pub(super) statistical_planned_seeds: BTreeMap<String, Vec<u64>>,
    pub(super) statistical_shots_per_batch: BTreeMap<String, u64>,
    pub(super) statistical_comparisons_per_attempt: BTreeMap<String, u32>,
    pub(super) statistical_batches_per_attempt: BTreeMap<String, u32>,
    pub(super) statistical_shots_per_attempt: BTreeMap<String, u64>,
    pub(super) statistical_exact_bound_per_attempt: BTreeMap<String, f64>,
    pub(super) property_corpus_ids: Vec<String>,
    pub(super) resource_contracts: Vec<ResourceExecution>,
    pub(super) upstream_dispositions: Vec<DomainDispositionCount>,
    pub(super) deferred_products: BTreeMap<String, usize>,
}

#[cfg(test)]
#[path = "report/tests.rs"]
mod tests;
