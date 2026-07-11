//! Validation for evidence that supports an evidence-close blocker as a whole.

use std::collections::{BTreeMap, BTreeSet};

use super::oracle::{
    oracle_class_matches_runner, oracle_signature_matches, validate_oracle_signature,
};
use super::{
    BenchmarkClass, BenchmarkId, BenchmarkManifestRow, BenchmarkRunner, BenchmarkThresholdClass,
    BlockerRecord, FixtureId, OracleEvidenceClass, OracleManifestRow, OracleManifestStatus,
    benchmark_class_matches_row, validate_identifier,
};

const PFM3_ANALYZER_ORACLES: &[&str] = &["pf3-analyze-errors-sweep-cli"];
const PFM5_DETECTING_ORACLES: &[&str] = &[
    "pf5-detecting-regions-repeat-rust",
    "pf5-detecting-regions-targets-rust",
    "pf5-detecting-regions-target-shapes-rust",
    "pf5-detecting-regions-noise-tags-rust",
    "pf5-detecting-regions-clifford-rust",
    "pf5-detecting-regions-gauge-rust",
    "pf5-detecting-regions-generated-repetition-rust",
    "pf5-detecting-regions-generated-surface-rust",
    "pf5-detecting-regions-generated-unrotated-surface-rust",
    "pf5-detecting-regions-generated-surface-memory-x-rust",
];
const PFM5_MISSING_ORACLES: &[&str] = &[
    "pf5-missing-detectors-clifford-rust",
    "pf5-missing-detectors-spp-rust",
    "pf5-missing-detectors-mpad-rust",
    "pf5-missing-detectors-repeat-rust",
    "pf5-missing-detectors-nested-final-repeat-rust",
    "pf5-missing-detectors-observable-neutral-final-repeat-rust",
];
const PFM5_FLOW_ORACLES: &[&str] = &[
    "pf5-has-flow-record-observable-rust",
    "pf5-has-all-flows-rust",
    "pf5-has-flow-diagnostics-rust",
    "pf5-signed-sampled-flows-rust",
];
const PFM6_ANALYZER_SEARCH_ORACLES: &[&str] = &[
    "pfm-b5-analyzer-nested-loop",
    "pfm-b5-analyzer-coordinate-loop",
    "pfm-b5-analyzer-gauge-loop",
    "pfm-b5-analyzer-nested-probe-budget-rust",
    "pfm-b5-analyzer-unitary-nested-probe-budget-rust",
    "pfm-b5-analyzer-local-decomposition-17-rust",
    "pfm-b5-analyzer-repetition-code-loop-rust",
    "pfm-b5-graphlike-finite-corpus-rust",
    "pfm-b5-graphlike-state-payload-rust",
    "pfm-b5-search-aggregate-target-work-rust",
    "pfm-b5-graphlike-construction-budget-rust",
    "pfm-b5-hypergraph-finite-corpus-rust",
    "pfm-b5-hypergraph-state-payload-rust",
    "pfm-b5-hypergraph-construction-budget-rust",
    "pfm-b5-wcnf-finite-corpus-rust",
];
#[derive(Clone, Copy)]
struct ExpectedBenchmarkSignature {
    id: &'static str,
    runner: BenchmarkRunner,
    threshold_class: BenchmarkThresholdClass,
    comparability: BenchmarkClass,
}

const fn pf5_report_only_benchmark(id: &'static str) -> ExpectedBenchmarkSignature {
    ExpectedBenchmarkSignature {
        id,
        runner: BenchmarkRunner::ContractOnly,
        threshold_class: BenchmarkThresholdClass::NonPrimaryReportOnly,
        comparability: BenchmarkClass::ReportOnly,
    }
}

const fn pfm2_contract_benchmark(id: &'static str) -> ExpectedBenchmarkSignature {
    ExpectedBenchmarkSignature {
        id,
        runner: BenchmarkRunner::ContractOnly,
        threshold_class: BenchmarkThresholdClass::NonPrimaryReportOnly,
        comparability: BenchmarkClass::ContractOnly,
    }
}

const fn pfm6_report_only_benchmark(id: &'static str) -> ExpectedBenchmarkSignature {
    ExpectedBenchmarkSignature {
        id,
        runner: BenchmarkRunner::ContractOnly,
        threshold_class: BenchmarkThresholdClass::NonPrimaryReportOnly,
        comparability: BenchmarkClass::ReportOnly,
    }
}

const fn pfm6_direct_match_benchmark(id: &'static str) -> ExpectedBenchmarkSignature {
    ExpectedBenchmarkSignature {
        id,
        runner: BenchmarkRunner::StimPerf,
        threshold_class: BenchmarkThresholdClass::NonPrimaryReportOnly,
        comparability: BenchmarkClass::DirectMatch,
    }
}

const PFM2_QEC_BENCHMARKS: &[ExpectedBenchmarkSignature] = &[
    pfm2_contract_benchmark("pfm-b1-time-reverse-generated-surface"),
    pfm2_contract_benchmark("pfm-b1-time-reverse-mpad-matrix"),
    pfm2_contract_benchmark("pfm-b1-time-reverse-large-unitary-repeat"),
    pfm2_contract_benchmark("pfm-b1-time-reverse-sparse-high-qubit"),
];

const PFM5_DETECTING_BENCHMARKS: &[ExpectedBenchmarkSignature] = &[
    pf5_report_only_benchmark("pf5-detecting-regions-repeat"),
    pf5_report_only_benchmark("pf5-detecting-regions-clifford"),
    pf5_report_only_benchmark("pf5-detecting-regions-generated-repetition"),
    pf5_report_only_benchmark("pf5-detecting-regions-generated-surface"),
];
const PFM5_MISSING_BENCHMARKS: &[ExpectedBenchmarkSignature] =
    &[pf5_report_only_benchmark("pf5-missing-detectors-mpad")];
const PFM5_FLOW_BENCHMARKS: &[ExpectedBenchmarkSignature] =
    &[pf5_report_only_benchmark("pf5-has-all-flows-batch")];
const PFM6_ANALYZER_SEARCH_BENCHMARKS: &[ExpectedBenchmarkSignature] = &[
    pfm6_report_only_benchmark("pfm-b5-analyzer-cycle-folding"),
    pfm6_report_only_benchmark("pfm-b5-analyzer-generated-qec"),
    pfm6_report_only_benchmark("pfm-b5-graphlike-search-direct-dem"),
    pfm6_direct_match_benchmark("pfm-b5-graphlike-generated-d25"),
    pfm6_direct_match_benchmark("pfm-b5-graphlike-generated-d11-r1000"),
    pfm6_report_only_benchmark("pfm-b5-hypergraph-search-direct-dem"),
    pfm6_report_only_benchmark("pfm-b5-hypergraph-search-generated-qec"),
    pfm6_report_only_benchmark("pfm-b5-wcnf-direct-dem"),
    pfm6_report_only_benchmark("pfm-b5-wcnf-generated-qec"),
    pfm6_report_only_benchmark("pf6-error-decomp-loop-folded"),
    pfm6_report_only_benchmark("pf6-sparse-rev-frame-loop"),
];

pub(super) fn validate_supporting_oracles(
    blocker: &BlockerRecord,
    oracle_rows: &BTreeMap<FixtureId, OracleManifestRow>,
    violations: &mut Vec<String>,
) {
    let expected_values = match blocker.id.as_str() {
        "pfm3-analyzer-sweep" => PFM3_ANALYZER_ORACLES,
        "pfm5-detecting-regions" => PFM5_DETECTING_ORACLES,
        "pfm5-missing-detectors" => PFM5_MISSING_ORACLES,
        "pfm5-flow-engine" => PFM5_FLOW_ORACLES,
        "pfm6-analyzer-search" => PFM6_ANALYZER_SEARCH_ORACLES,
        _ => &[],
    };
    let expected = expected_values.iter().copied().collect::<BTreeSet<_>>();
    let mut actual = BTreeSet::new();
    for reference in &blocker.supporting_oracles {
        validate_identifier("supporting oracle", reference.value.as_str(), violations);
        validate_oracle_signature(&reference.signature, violations);
        if !actual.insert(reference.value.as_str()) {
            violations.push(format!(
                "blocker {:?} repeats supporting oracle {:?}",
                blocker.id,
                reference.value.as_str()
            ));
        }
        match oracle_rows.get(&reference.value) {
            Some(row) if row.status != OracleManifestStatus::Implemented => {
                violations.push(format!(
                    "blocker {:?} supporting oracle {:?} is not implemented",
                    blocker.id,
                    reference.value.as_str()
                ))
            }
            Some(_) if reference.classification == OracleEvidenceClass::Planned => {
                violations.push(format!(
                    "blocker {:?} supporting oracle {:?} cannot be planned",
                    blocker.id,
                    reference.value.as_str()
                ));
            }
            Some(row)
                if !oracle_class_matches_runner(reference.classification, row.command.runner)
                    || !oracle_signature_matches(&reference.signature, row) =>
            {
                violations.push(format!(
                    "blocker {:?} supporting oracle {:?} classification {:?} or frozen signature is incompatible with runner {:?}, argv {:?}, and upstream {:?}",
                    blocker.id,
                    reference.value.as_str(),
                    reference.classification,
                    row.command.runner,
                    row.command.argv,
                    row.upstream_source.0
                ));
            }
            Some(_) => {}
            None => violations.push(format!(
                "blocker {:?} references missing supporting oracle {:?}",
                blocker.id,
                reference.value.as_str()
            )),
        }
    }
    if actual != expected {
        let missing = expected.difference(&actual).copied().collect::<Vec<_>>();
        let unexpected = actual.difference(&expected).copied().collect::<Vec<_>>();
        violations.push(format!(
            "blocker {:?} supporting oracle set differs from the frozen contract; missing={missing:?} unexpected={unexpected:?}",
            blocker.id
        ));
    }
}

pub(super) fn validate_supporting_benchmarks(
    blocker: &BlockerRecord,
    benchmark_rows: &BTreeMap<BenchmarkId, BenchmarkManifestRow>,
    violations: &mut Vec<String>,
) {
    let expected_values = match blocker.id.as_str() {
        "pfm2-qec-transforms" => PFM2_QEC_BENCHMARKS,
        "pfm5-detecting-regions" => PFM5_DETECTING_BENCHMARKS,
        "pfm5-missing-detectors" => PFM5_MISSING_BENCHMARKS,
        "pfm5-flow-engine" => PFM5_FLOW_BENCHMARKS,
        "pfm6-analyzer-search" => PFM6_ANALYZER_SEARCH_BENCHMARKS,
        _ => &[],
    };
    let expected = expected_values
        .iter()
        .map(|signature| signature.id)
        .collect::<BTreeSet<_>>();
    let mut actual = BTreeSet::new();
    for reference in &blocker.supporting_benchmarks {
        validate_identifier("supporting benchmark", reference.value.as_str(), violations);
        if !actual.insert(reference.value.as_str()) {
            violations.push(format!(
                "blocker {:?} repeats supporting benchmark {:?}",
                blocker.id,
                reference.value.as_str()
            ));
        }
        let expected_signature = expected_values
            .iter()
            .find(|signature| signature.id == reference.value.as_str());
        match (benchmark_rows.get(&reference.value), expected_signature) {
            (Some(row), Some(signature))
                if row.runner == signature.runner
                    && row.threshold_class == signature.threshold_class
                    && row.comparability == signature.comparability
                    && benchmark_class_matches_row(reference.classification, row) => {}
            (Some(row), Some(signature)) => violations.push(format!(
                "blocker {:?} supporting benchmark classification {:?} is incompatible with row {:?} signature ({:?}/{:?}/{:?}); expected ({:?}/{:?}/{:?})",
                blocker.id,
                reference.classification,
                row.id.as_str(),
                row.runner,
                row.threshold_class,
                row.comparability,
                signature.runner,
                signature.threshold_class,
                signature.comparability
            )),
            (Some(_), None) => violations.push(format!(
                "blocker {:?} has no frozen supporting benchmark signature for {:?}",
                blocker.id,
                reference.value.as_str()
            )),
            (None, _) => violations.push(format!(
                "blocker {:?} references missing supporting benchmark {:?}",
                blocker.id,
                reference.value.as_str()
            )),
        }
    }
    if actual != expected {
        let missing = expected.difference(&actual).copied().collect::<Vec<_>>();
        let unexpected = actual.difference(&expected).copied().collect::<Vec<_>>();
        violations.push(format!(
            "blocker {:?} supporting benchmark set differs from the frozen contract; missing={missing:?} unexpected={unexpected:?}",
            blocker.id
        ));
    }
}
