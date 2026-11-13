#![allow(
    clippy::expect_used,
    reason = "focused analyzer regressions use direct failure diagnostics"
)]

use stab_model::{Circuit, MemoryBudget};

use crate::ErrorAnalyzerOptions;

use super::try_analyze;

fn no_recurrence_loop(iterations: u64) -> Circuit {
    Circuit::from_stim_str(&format!(
        "R 0\nM 0\nDETECTOR rec[-1]\nREPEAT {iterations} {{\n    X_ERROR(0.125) 0\n    M 0\n    DETECTOR rec[-1] rec[-2]\n}}\nM 0\nDETECTOR rec[-1]\n"
    ))
    .expect("valid no-recurrence circuit")
}

#[test]
fn analyzer_output_is_admitted_against_the_memory_budget_for_folded_and_unrolled_loops() {
    for fold_loops in [false, true] {
        let options = |bytes| ErrorAnalyzerOptions {
            fold_loops,
            memory_budget: MemoryBudget::from_bytes(bytes),
            ..ErrorAnalyzerOptions::default()
        };
        let circuit = no_recurrence_loop(200);
        // Find the smallest budget that admits the produced model.
        let (mut low, mut high) = (0_u64, 1 << 20);
        assert!(try_analyze(&circuit, options(high)).is_ok());
        while low < high {
            let middle = low + (high - low) / 2;
            if try_analyze(&circuit, options(middle)).is_ok() {
                high = middle;
            } else {
                low = middle + 1;
            }
        }
        let error = try_analyze(&circuit, options(low - 1))
            .expect_err("one byte below the exact budget is refused");
        let resource = error.resource_limit_error().expect("typed budget refusal");
        assert_eq!(
            resource.operation(),
            crate::ResourceOperation::CircuitToDetectorErrorModel
        );
        assert_eq!(resource.resource(), crate::ResourceKind::MaterializedBytes);
        assert_eq!(resource.limit(), low - 1);
        assert!(error.to_string().contains("memory budget"), "{error}");
    }

    // Unrolled analysis above the retired one-million expanded-instruction cap completes.
    let dem = try_analyze(
        &no_recurrence_loop(400_000),
        ErrorAnalyzerOptions::default(),
    )
    .expect("expanded work is not capped");
    assert_eq!(dem.count_detectors(), Ok(400_002));
}
