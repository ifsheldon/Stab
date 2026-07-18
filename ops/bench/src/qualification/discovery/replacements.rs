use crate::manifest::BenchmarkRow;
use crate::qualification::model::ReplacementContract;

pub(super) fn contracts(row: &BenchmarkRow) -> Vec<ReplacementContract> {
    match row.id.as_str() {
        "m5-simd-bits" => vec![replacement(
            "simd_bits_xor_10K",
            "stab_simd_bits_xor_10K",
            "PERFQ-M5-SIMD-BITS",
            "xor-complete-vector",
            None,
        )],
        "m6-clifford-string" => vec![replacement(
            "CliffordString_multiplication_10K",
            "stab_clifford_string_multiplication_10K",
            "PERFQ-M6-CLIFFORD-STRING",
            "right-multiply-identity",
            Some("small"),
        )],
        _ => Vec::new(),
    }
}

fn replacement(
    legacy_stim_name: &str,
    legacy_stab_name: &str,
    runtime_group_id: &str,
    runtime_measurement_id: &str,
    runtime_scale_id: Option<&str>,
) -> ReplacementContract {
    ReplacementContract {
        legacy_stim_name: legacy_stim_name.to_string(),
        legacy_stab_name: legacy_stab_name.to_string(),
        runtime_group_id: runtime_group_id.to_string(),
        runtime_measurement_id: runtime_measurement_id.to_string(),
        runtime_scale_id: runtime_scale_id.map(str::to_string),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::comparability::ComparabilityClass;
    use crate::manifest::{Milestone, Runner, ThresholdClass};

    fn row(id: &str) -> BenchmarkRow {
        BenchmarkRow {
            id: id.to_string(),
            milestone: Milestone::M6,
            threshold_class: ThresholdClass::ReportOnly,
            runner: Runner::StimPerf,
            upstream_source: "src/stim/stabilizers/pauli_string.perf.cc".to_string(),
            stim_perf_filter: "PauliString_*".to_string(),
            argv: String::new(),
            stdin_path: String::new(),
            phase: "throughput".to_string(),
            measurement: "pauli-string".to_string(),
            description: "Pauli string multiplication workloads".to_string(),
            comparability: ComparabilityClass::DirectMatch,
        }
    }

    #[test]
    fn scale_family_replacement_can_still_omit_an_exact_scale() {
        let replacements = contracts(&row("m5-simd-bits"));
        assert_eq!(replacements.len(), 1);
        let replacement = replacements.first().expect("one replacement");
        assert_eq!(replacement.runtime_scale_id, None);
    }

    #[test]
    fn clifford_replacement_names_only_the_exact_identity_small_contract() {
        let replacements = contracts(&row("m6-clifford-string"));
        assert_eq!(replacements.len(), 1);
        let replacement = replacements.first().expect("one Clifford replacement");
        assert_eq!(
            replacement.legacy_stim_name,
            "CliffordString_multiplication_10K"
        );
        assert_eq!(
            replacement.legacy_stab_name,
            "stab_clifford_string_multiplication_10K"
        );
        assert_eq!(replacement.runtime_group_id, "PERFQ-M6-CLIFFORD-STRING");
        assert_eq!(
            replacement.runtime_measurement_id,
            "right-multiply-identity"
        );
        assert_eq!(replacement.runtime_scale_id.as_deref(), Some("small"));
    }
}
