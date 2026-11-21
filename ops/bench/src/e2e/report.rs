use std::collections::{BTreeMap, BTreeSet};

use super::data::{OutputKey, OutputWitness};
use super::host::HostProfile;
use super::model::{Case, CaseId, Comparator, Suite, TIMING_BOUNDARY, Tier};
use super::statistics::{
    PairedTiming, StabTiming, TimingSummary, summarize_paired, summarize_stab,
};
use serde::{Deserialize, Serialize};

pub(super) const RUN_SCHEMA_VERSION: u16 = 1;
pub(super) const CORRECTNESS_SCHEMA_VERSION: u16 = 1;
pub(super) const SAMPLES_SCHEMA_VERSION: u16 = 1;
pub(super) const REPORT_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RunMetadata {
    pub(super) schema_version: u16,
    pub(super) suite_sha256: String,
    pub(super) source_commit: String,
    pub(super) source_clean: bool,
    pub(super) stim_commit: String,
    pub(super) stim_binary_sha256: String,
    pub(super) stab_binary_sha256: String,
    pub(super) bench_binary_sha256: String,
    pub(super) rustc: String,
    pub(super) target: String,
    pub(super) tier: Tier,
    pub(super) formal: bool,
    pub(super) host_before: HostProfile,
    pub(super) host_after: HostProfile,
    pub(super) selected_cases: Vec<CaseId>,
    /// Warmup rounds per case. Bundles written before this field existed omit it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) warmups: Option<usize>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CorrectnessReport {
    pub(super) schema_version: u16,
    pub(super) cases: Vec<CaseCorrectness>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CaseCorrectness {
    pub(super) case_id: CaseId,
    pub(super) input_sha256: String,
    pub(super) stim_outputs: BTreeMap<OutputKey, OutputWitness>,
    pub(super) stab_outputs: BTreeMap<OutputKey, OutputWitness>,
}

/// Output bytes one run measures: every witnessed output except pipeline intermediates.
fn measured_output_bytes(outputs: &BTreeMap<OutputKey, OutputWitness>) -> Option<u64> {
    outputs
        .iter()
        .filter(|(key, _)| key.is_measured())
        .try_fold(0_u64, |total, (_, witness)| {
            total.checked_add(witness.bytes)
        })
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawSamples {
    pub(super) schema_version: u16,
    pub(super) cases: Vec<CaseSamples>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CaseSamples {
    pub(super) case_id: CaseId,
    pub(super) case_digest: String,
    pub(super) paired: Vec<PairedTiming>,
    pub(super) stab_only: Vec<StabTiming>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum GateOutcome {
    Passed,
    Failed,
    Unseeded,
    Diagnostic,
    NotApplicable,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DerivedReport {
    pub(super) schema_version: u16,
    pub(super) suite_sha256: String,
    pub(super) source_commit: String,
    pub(super) tier: Tier,
    pub(super) architecture: String,
    pub(super) cpu_model: String,
    pub(super) cases: Vec<CaseReport>,
    pub(super) parity: GateOutcome,
    pub(super) self_regression: GateOutcome,
    pub(super) memory: GateOutcome,
    pub(super) passed: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CaseReport {
    pub(super) case_id: CaseId,
    pub(super) case_digest: String,
    pub(super) timing: TimingSummary,
    pub(super) parity: GateOutcome,
    pub(super) self_regression: GateOutcome,
    pub(super) memory: GateOutcome,
    pub(super) self_median_ratio: Option<f64>,
    pub(super) self_upper_ratio: Option<f64>,
}

impl DerivedReport {
    /// Derives the report from raw samples, after checking that every sample has the shape the
    /// suite and the correctness witnesses fix: the tier's sample count, the case's semantic work
    /// per sample, and the witnessed output bytes per sample. Recording and replay share it.
    pub(super) fn derive(
        suite: &Suite,
        metadata: &RunMetadata,
        correctness: &CorrectnessReport,
        samples: &RawSamples,
    ) -> Result<Self, String> {
        if metadata.schema_version != RUN_SCHEMA_VERSION {
            return Err(format!(
                "run schema is {}, expected {RUN_SCHEMA_VERSION}",
                metadata.schema_version
            ));
        }
        if samples.schema_version != SAMPLES_SCHEMA_VERSION {
            return Err(format!(
                "sample schema is {}, expected {SAMPLES_SCHEMA_VERSION}",
                samples.schema_version
            ));
        }
        let tier_policy = suite
            .tiers
            .get(&metadata.tier)
            .ok_or_else(|| format!("suite omits tier {}", metadata.tier))?;
        if metadata
            .warmups
            .is_some_and(|warmups| warmups != tier_policy.warmups)
        {
            return Err(format!(
                "run recorded {:?} warmups, tier {} requires {}",
                metadata.warmups, metadata.tier, tier_policy.warmups
            ));
        }
        let mut sample_map = samples
            .cases
            .iter()
            .map(|case| (case.case_id.as_str(), case))
            .collect::<BTreeMap<_, _>>();
        if sample_map.len() != samples.cases.len() {
            return Err("raw samples repeat a case id".to_string());
        }
        let witnesses = correctness
            .cases
            .iter()
            .map(|case| (case.case_id.as_str(), case))
            .collect::<BTreeMap<_, _>>();
        let selected = metadata
            .selected_cases
            .iter()
            .map(CaseId::as_str)
            .collect::<BTreeSet<_>>();
        if selected.is_empty() || selected.len() != metadata.selected_cases.len() {
            return Err("run selection is empty or repeats a case id".to_string());
        }
        let known = suite.cases().map(|(_, _, id)| id).collect::<BTreeSet<_>>();
        if let Some(unknown) = selected.iter().find(|case| !known.contains(**case)) {
            return Err(format!("run selection names unknown case {unknown}"));
        }
        let mut reports = Vec::new();
        for (family, case, case_id) in suite.cases() {
            if !selected.contains(case_id.as_str()) {
                continue;
            }
            let raw = sample_map
                .remove(case_id.as_str())
                .ok_or_else(|| format!("raw samples omit {case_id}"))?;
            let expected_digest = case.digest(&family.id)?;
            if raw.case_digest != expected_digest {
                return Err(format!("case digest mismatch for {case_id}"));
            }
            let witness = witnesses
                .get(case_id.as_str())
                .ok_or_else(|| format!("correctness report omits {case_id}"))?;
            let shape = SampleShape::new(&case_id, case, tier_policy.samples, witness)?;
            let timing = match family.comparator {
                Comparator::StimCli => {
                    if !raw.stab_only.is_empty() {
                        return Err(format!("paired case {case_id} has Stab-only rows"));
                    }
                    shape.check_paired(&raw.paired)?;
                    summarize_paired(
                        &raw.paired,
                        suite.policy.bootstrap_seed,
                        suite.policy.bootstrap_resamples,
                        suite.policy.confidence_level,
                    )?
                }
                Comparator::SelfOnly => {
                    if !raw.paired.is_empty() {
                        return Err(format!("self-only case {case_id} has paired rows"));
                    }
                    shape.check_stab_only(&raw.stab_only)?;
                    summarize_stab(
                        &raw.stab_only,
                        suite.policy.bootstrap_seed,
                        suite.policy.bootstrap_resamples,
                        suite.policy.confidence_level,
                    )?
                }
            };
            let parity = parity_outcome(suite, metadata.tier, family.comparator, &timing);
            let (self_regression, self_median_ratio, self_upper_ratio) =
                self_outcome(suite, metadata, &case_id, &expected_digest, &timing);
            let memory = if timing.stab_peak_rss_bytes <= case.maximum_stab_peak_rss_bytes {
                GateOutcome::Passed
            } else {
                GateOutcome::Failed
            };
            reports.push(CaseReport {
                case_id,
                case_digest: expected_digest,
                timing,
                parity,
                self_regression,
                memory,
                self_median_ratio,
                self_upper_ratio,
            });
        }
        if !sample_map.is_empty() {
            return Err(format!(
                "raw samples contain unselected cases: {}",
                sample_map.keys().copied().collect::<Vec<_>>().join(", ")
            ));
        }
        let parity = aggregate(reports.iter().map(|report| report.parity));
        let self_regression = aggregate(reports.iter().map(|report| report.self_regression));
        let memory = aggregate(reports.iter().map(|report| report.memory));
        let passed = match metadata.tier {
            Tier::Smoke => ![parity, self_regression, memory].contains(&GateOutcome::Failed),
            Tier::Full | Tier::Soak => {
                matches!(parity, GateOutcome::Passed | GateOutcome::NotApplicable)
                    && self_regression == GateOutcome::Passed
                    && memory == GateOutcome::Passed
            }
        };
        Ok(Self {
            schema_version: REPORT_SCHEMA_VERSION,
            suite_sha256: metadata.suite_sha256.clone(),
            source_commit: metadata.source_commit.clone(),
            tier: metadata.tier,
            architecture: metadata.host_before.architecture.clone(),
            cpu_model: metadata.host_before.cpu_model.clone(),
            cases: reports,
            parity,
            self_regression,
            memory,
            passed,
        })
    }
}

/// The shape every raw sample of one case must have.
struct SampleShape<'a> {
    case_id: &'a CaseId,
    samples: usize,
    work: u64,
    stim_output_bytes: Option<u64>,
    stab_output_bytes: u64,
}

impl<'a> SampleShape<'a> {
    fn new(
        case_id: &'a CaseId,
        case: &Case,
        samples: usize,
        witness: &CaseCorrectness,
    ) -> Result<Self, String> {
        let batch = u64::from(case.batch);
        let overflow = || format!("{case_id} expected sample shape overflows");
        let per_batch = |outputs: &BTreeMap<OutputKey, OutputWitness>| {
            measured_output_bytes(outputs)
                .and_then(|bytes| bytes.checked_mul(batch))
                .ok_or_else(overflow)
        };
        Ok(Self {
            case_id,
            samples,
            work: case.work.amount.checked_mul(batch).ok_or_else(overflow)?,
            stim_output_bytes: if witness.stim_outputs.is_empty() {
                None
            } else {
                Some(per_batch(&witness.stim_outputs)?)
            },
            stab_output_bytes: per_batch(&witness.stab_outputs)?,
        })
    }

    fn check_count(&self, actual: usize) -> Result<(), String> {
        if actual == self.samples {
            Ok(())
        } else {
            Err(format!(
                "{} has {actual} samples, its tier requires {}",
                self.case_id, self.samples
            ))
        }
    }

    fn check_paired(&self, samples: &[PairedTiming]) -> Result<(), String> {
        self.check_count(samples.len())?;
        let stim_output_bytes = self
            .stim_output_bytes
            .ok_or_else(|| format!("{} paired samples have no Stim witness", self.case_id))?;
        for sample in samples {
            if sample.stim_work != self.work || sample.stab_work != self.work {
                return Err(format!(
                    "{} paired sample {} records work {}/{}, the case requires {}",
                    self.case_id, sample.index, sample.stim_work, sample.stab_work, self.work
                ));
            }
            if sample.stim_output_bytes != stim_output_bytes
                || sample.stab_output_bytes != self.stab_output_bytes
            {
                return Err(format!(
                    "{} paired sample {} records output bytes {}/{}, the witnesses require {stim_output_bytes}/{}",
                    self.case_id,
                    sample.index,
                    sample.stim_output_bytes,
                    sample.stab_output_bytes,
                    self.stab_output_bytes
                ));
            }
        }
        Ok(())
    }

    fn check_stab_only(&self, samples: &[StabTiming]) -> Result<(), String> {
        self.check_count(samples.len())?;
        for sample in samples {
            if sample.work != self.work || sample.output_bytes != self.stab_output_bytes {
                return Err(format!(
                    "{} Stab sample {} records work {} and output bytes {}, the case and witness require {} and {}",
                    self.case_id,
                    sample.index,
                    sample.work,
                    sample.output_bytes,
                    self.work,
                    self.stab_output_bytes
                ));
            }
        }
        Ok(())
    }
}

fn parity_outcome(
    suite: &Suite,
    tier: Tier,
    comparator: Comparator,
    timing: &TimingSummary,
) -> GateOutcome {
    if comparator == Comparator::SelfOnly {
        return GateOutcome::NotApplicable;
    }
    if tier == Tier::Smoke {
        return GateOutcome::Diagnostic;
    }
    let Some(ratio) = &timing.parity_ratio else {
        return GateOutcome::Failed;
    };
    if ratio.median <= suite.policy.parity_max_ratio
        && ratio.confidence_upper <= suite.policy.parity_max_ratio
    {
        GateOutcome::Passed
    } else {
        GateOutcome::Failed
    }
}

fn self_outcome(
    suite: &Suite,
    metadata: &RunMetadata,
    case_id: &CaseId,
    digest: &str,
    timing: &TimingSummary,
) -> (GateOutcome, Option<f64>, Option<f64>) {
    if metadata.tier == Tier::Smoke {
        return (GateOutcome::Diagnostic, None, None);
    }
    let baseline = suite.self_baselines.iter().find(|baseline| {
        baseline.architecture == metadata.host_before.architecture
            && baseline.cpu_model == metadata.host_before.cpu_model
            && baseline.rustc == metadata.rustc
            && baseline.target == metadata.target
            && baseline.timing_boundary == TIMING_BOUNDARY
            && baseline.case_id == *case_id
            && baseline.case_digest == digest
    });
    let Some(baseline) = baseline else {
        return (GateOutcome::Unseeded, None, None);
    };
    let median = timing.stab_seconds_per_work.median / baseline.median_seconds_per_work;
    let upper = timing.stab_seconds_per_work.confidence_upper / baseline.upper_seconds_per_work;
    let outcome = if median <= suite.policy.self_regression_max_ratio
        && upper <= suite.policy.self_regression_max_ratio
    {
        GateOutcome::Passed
    } else {
        GateOutcome::Failed
    };
    (outcome, Some(median), Some(upper))
}

fn aggregate(outcomes: impl Iterator<Item = GateOutcome>) -> GateOutcome {
    let values = outcomes.collect::<Vec<_>>();
    if values.contains(&GateOutcome::Failed) {
        GateOutcome::Failed
    } else if values.contains(&GateOutcome::Unseeded) {
        GateOutcome::Unseeded
    } else if values.contains(&GateOutcome::Diagnostic) {
        GateOutcome::Diagnostic
    } else if values
        .iter()
        .all(|value| *value == GateOutcome::NotApplicable)
    {
        GateOutcome::NotApplicable
    } else {
        GateOutcome::Passed
    }
}

pub(super) fn markdown(report: &DerivedReport) -> String {
    let mut text = format!(
        "# Stab E2E Performance Report\n\n- Tier: `{}`\n- Source: `{}`\n- Architecture: `{}`\n- CPU: `{}`\n- Stim parity: `{:?}`\n- Stab self-regression: `{:?}`\n- Memory: `{:?}`\n\n| Case | Stab throughput | Stim ratio | Peak RSS | Parity | Self | Memory |\n| --- | ---: | ---: | ---: | --- | --- | --- |\n",
        report.tier,
        report.source_commit,
        report.architecture,
        report.cpu_model,
        report.parity,
        report.self_regression,
        report.memory,
    );
    for case in &report.cases {
        let ratio = case.timing.parity_ratio.as_ref().map_or_else(
            || "n/a".to_string(),
            |ratio| format!("{:.3}x", ratio.median),
        );
        text.push_str(&format!(
            "| `{}` | {:.3} | {} | {} MiB | `{:?}` | `{:?}` | `{:?}` |\n",
            case.case_id,
            case.timing.stab_throughput,
            ratio,
            case.timing.stab_peak_rss_bytes / (1 << 20),
            case.parity,
            case.self_regression,
            case.memory,
        ));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::e2e::model::{
        DataRecipe, Family, OutputContract, Policy, RunnerKind, SelfBaseline, SemanticWork,
        SizeClass, StimIdentity, TierPolicy, WorkUnit, Workload,
    };
    use crate::e2e::statistics::PairOrder;

    fn stdout_witness(bytes: u64) -> BTreeMap<OutputKey, OutputWitness> {
        BTreeMap::from([(
            OutputKey::Stdout,
            OutputWitness {
                bytes,
                sha256: "0".repeat(64),
                one_bits: None,
            },
        )])
    }

    fn derive(
        (suite, metadata, correctness, samples): &(
            Suite,
            RunMetadata,
            CorrectnessReport,
            RawSamples,
        ),
    ) -> Result<DerivedReport, String> {
        DerivedReport::derive(suite, metadata, correctness, samples)
    }

    fn suite_with_baseline(
        baseline_ratio: Option<f64>,
    ) -> (Suite, RunMetadata, CorrectnessReport, RawSamples) {
        let case = Case {
            id: "small".to_string(),
            size_class: SizeClass::Small,
            batch: 1,
            work: SemanticWork {
                amount: 100,
                unit: WorkUnit::Shots,
            },
            maximum_stab_peak_rss_bytes: 1000,
            workload: Workload::Cli {
                args: vec!["sample".to_string()],
                stdin: DataRecipe::Empty,
                files: Vec::new(),
                stdout: OutputContract::Exact { minimum_bytes: 0 },
            },
        };
        let digest = case.digest("sample").expect("digest");
        let host = HostProfile {
            architecture: "aarch64".to_string(),
            cpu_model: "test".to_string(),
            logical_cpus: 4,
            affinity_cpu: Some(2),
            kernel_release: "test".to_string(),
            thermal: Vec::new(),
            swap: super::super::host::SwapSnapshot {
                configured: Vec::new(),
                pages_in: 0,
                pages_out: 0,
            },
        };
        let metadata = RunMetadata {
            schema_version: RUN_SCHEMA_VERSION,
            suite_sha256: "s".repeat(64),
            source_commit: "c".repeat(40),
            source_clean: true,
            stim_commit: "t".repeat(40),
            stim_binary_sha256: "1".repeat(64),
            stab_binary_sha256: "2".repeat(64),
            bench_binary_sha256: "3".repeat(64),
            rustc: "rustc test".to_string(),
            target: "aarch64-test".to_string(),
            tier: Tier::Full,
            formal: true,
            host_before: host.clone(),
            host_after: host,
            selected_cases: vec![CaseId::new("sample", "small")],
            warmups: Some(0),
        };
        let mut suite = Suite {
            schema_version: 1,
            stim: StimIdentity {
                version: "v1.16.0".to_string(),
                commit: metadata.stim_commit.clone(),
            },
            policy: Policy {
                timing_boundary: TIMING_BOUNDARY.to_string(),
                parity_max_ratio: 1.25,
                self_regression_max_ratio: 1.15,
                bootstrap_seed: 7,
                bootstrap_resamples: 100,
                confidence_level: 0.95,
                maximum_temperature_millidegrees: 100_000,
                maximum_output_bytes: 1024,
                command_timeout_seconds: 60,
            },
            tiers: BTreeMap::from([(
                Tier::Full,
                TierPolicy {
                    warmups: 0,
                    samples: 3,
                },
            )]),
            expected_release_families: 1,
            expected_release_cases: 1,
            self_baselines: Vec::new(),
            families: vec![Family {
                id: "sample".to_string(),
                description: "sample".to_string(),
                runner: RunnerKind::Cli,
                comparator: Comparator::StimCli,
                prerequisites: vec!["cli.sample".to_string()],
                cases: vec![case],
            }],
        };
        if let Some(ratio) = baseline_ratio {
            suite.self_baselines.push(SelfBaseline {
                architecture: "aarch64".to_string(),
                cpu_model: "test".to_string(),
                rustc: "rustc test".to_string(),
                target: "aarch64-test".to_string(),
                timing_boundary: TIMING_BOUNDARY.to_string(),
                case_id: CaseId::new("sample", "small"),
                case_digest: digest.clone(),
                median_seconds_per_work: 0.01 / ratio,
                upper_seconds_per_work: 0.01 / ratio,
            });
        }
        let paired = (0..3)
            .map(|index| PairedTiming {
                index,
                order: PairOrder::for_index(index),
                stim_seconds: 1.0,
                stab_seconds: 1.0,
                stim_work: 100,
                stab_work: 100,
                stim_peak_rss_bytes: 100,
                stab_peak_rss_bytes: 200,
                stim_output_bytes: 10,
                stab_output_bytes: 10,
            })
            .collect();
        let samples = RawSamples {
            schema_version: SAMPLES_SCHEMA_VERSION,
            cases: vec![CaseSamples {
                case_id: CaseId::new("sample", "small"),
                case_digest: digest,
                paired,
                stab_only: Vec::new(),
            }],
        };
        let correctness = CorrectnessReport {
            schema_version: CORRECTNESS_SCHEMA_VERSION,
            cases: vec![CaseCorrectness {
                case_id: CaseId::new("sample", "small"),
                input_sha256: "0".repeat(64),
                stim_outputs: stdout_witness(10),
                stab_outputs: stdout_witness(10),
            }],
        };
        (suite, metadata, correctness, samples)
    }

    #[test]
    fn parity_and_self_regression_are_independent() {
        let report = derive(&suite_with_baseline(Some(1.149))).expect("report");
        assert_eq!(report.parity, GateOutcome::Passed);
        assert_eq!(report.self_regression, GateOutcome::Passed);

        let report = derive(&suite_with_baseline(Some(1.151))).expect("report");
        assert_eq!(report.parity, GateOutcome::Passed);
        assert_eq!(report.self_regression, GateOutcome::Failed);
    }

    #[test]
    fn missing_self_baseline_is_unseeded_not_passing() {
        let report = derive(&suite_with_baseline(None)).expect("report");
        assert_eq!(report.self_regression, GateOutcome::Unseeded);
        assert!(!report.passed);
    }

    #[test]
    fn self_regression_rejects_identity_drift_and_an_independent_upper_failure() {
        let mut fixture = suite_with_baseline(Some(1.0));
        fixture.0.self_baselines[0].target = "different-target".to_string();
        let report = derive(&fixture).expect("identity report");
        assert_eq!(report.self_regression, GateOutcome::Unseeded);

        let mut fixture = suite_with_baseline(Some(1.0));
        fixture.3.cases[0].paired[2].stab_seconds = 1.3;
        let report = derive(&fixture).expect("upper report");
        assert_eq!(report.self_regression, GateOutcome::Failed);
        assert!(
            report.cases[0]
                .self_median_ratio
                .is_some_and(|ratio| ratio <= 1.15)
        );
        assert!(
            report.cases[0]
                .self_upper_ratio
                .is_some_and(|ratio| ratio > 1.15)
        );
    }

    #[test]
    fn derivation_rejects_raw_samples_with_the_wrong_shape() {
        let valid = suite_with_baseline(None);
        derive(&valid).expect("valid shape");

        let mut short = valid.clone();
        short.3.cases[0].paired.pop();
        assert!(derive(&short).is_err_and(|error| error.contains("has 2 samples")));

        let mut work = valid.clone();
        work.3.cases[0].paired[1].stab_work = 99;
        work.3.cases[0].paired[1].stim_work = 99;
        assert!(derive(&work).is_err_and(|error| error.contains("records work 99/99")));

        let mut bytes = valid.clone();
        bytes.3.cases[0].paired[0].stab_output_bytes = 11;
        assert!(derive(&bytes).is_err_and(|error| error.contains("output bytes 10/11")));

        let mut witness = valid.clone();
        witness.2.cases[0].stab_outputs = stdout_witness(12);
        assert!(derive(&witness).is_err_and(|error| error.contains("require 10/12")));

        let mut warmups = valid.clone();
        warmups.1.warmups = Some(1);
        assert!(derive(&warmups).is_err_and(|error| error.contains("warmups")));
        let mut unrecorded = valid.clone();
        unrecorded.1.warmups = None;
        derive(&unrecorded).expect("bundles that predate the warmup field stay valid");

        let mut missing = valid;
        missing.2.cases.clear();
        assert!(derive(&missing).is_err_and(|error| error.contains("correctness report omits")));
    }
}
