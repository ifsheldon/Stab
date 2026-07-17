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
        "m6-pauli-string" => [
            (
                "PauliString_multiplication_10K",
                "stab_pauli_string_multiplication_10K",
                "small",
            ),
            (
                "PauliString_multiplication_100K",
                "stab_pauli_string_multiplication_100K",
                "medium",
            ),
            (
                "PauliString_multiplication_1M",
                "stab_pauli_string_multiplication_1M",
                "large",
            ),
        ]
        .into_iter()
        .map(|(stim, stab, scale)| {
            replacement(
                stim,
                stab,
                "PERFQ-M6-PAULI-STRING",
                "right-multiply-in-place",
                Some(scale),
            )
        })
        .collect(),
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
    fn pauli_replacements_bind_each_legacy_pair_to_one_exact_scale() {
        let replacements = contracts(&row("m6-pauli-string"));
        assert_eq!(replacements.len(), 3);
        assert_eq!(
            replacements
                .iter()
                .map(|replacement| replacement.runtime_scale_id.as_deref())
                .collect::<Vec<_>>(),
            [Some("small"), Some("medium"), Some("large")]
        );
        assert!(replacements.iter().all(|replacement| {
            replacement.runtime_group_id == "PERFQ-M6-PAULI-STRING"
                && replacement.runtime_measurement_id == "right-multiply-in-place"
        }));
    }

    #[test]
    fn scale_family_replacement_can_still_omit_an_exact_scale() {
        let replacements = contracts(&row("m5-simd-bits"));
        assert_eq!(replacements.len(), 1);
        let replacement = replacements.first().expect("one replacement");
        assert_eq!(replacement.runtime_scale_id, None);
    }
}
