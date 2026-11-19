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

#[test]
fn error_accumulator_admits_live_classes_before_flushing_to_dem() {
    // An anti-Hermitian MPP sits at the start of the circuit (visited last in reverse). Under the
    // default budget, reverse traversal reaches MPP and reports the semantic error; under a tight
    // budget, accumulating distinct tagged error classes fails closed mid-circuit before flush.
    let mut stim = String::from("MPP X0*Z0\nR 0\n");
    for tag_id in 0..256 {
        stim.push_str(&format!("X_ERROR[t{tag_id}](0.01) 0\n"));
    }
    stim.push_str("M 0\nDETECTOR rec[-1]\n");
    let circuit = Circuit::from_stim_str(&stim).expect("valid circuit syntax");

    let semantic = try_analyze(&circuit, ErrorAnalyzerOptions::default())
        .expect_err("default budget reaches start-of-circuit anti-Hermitian MPP");
    assert!(
        semantic.to_string().contains("anti-Hermitian"),
        "{semantic}"
    );

    let tight = MemoryBudget::from_bytes(4096);
    let refused = try_analyze(
        &circuit,
        ErrorAnalyzerOptions {
            memory_budget: tight,
            ..ErrorAnalyzerOptions::default()
        },
    )
    .expect_err("tight budget refuses live ErrorAccumulator growth before MPP or flush");
    let resource = refused
        .resource_limit_error()
        .expect("typed accumulator budget refusal");
    assert_eq!(
        resource.operation(),
        crate::ResourceOperation::CircuitToDetectorErrorModel
    );
    assert_eq!(resource.resource(), crate::ResourceKind::MaterializedBytes);
    assert_eq!(resource.limit(), tight.bytes());
    assert!(resource.actual() > resource.limit());
}

/// Folded and unrolled analysis both recurse once per repeat level, so both admit exactly the
/// shared nesting envelope and refuse one level more with the typed nesting error.
#[test]
fn analyzer_repeat_nesting_boundary_is_exact_with_and_without_folding() {
    fn nested(depth: usize) -> Circuit {
        let mut body = Circuit::from_stim_str("R 0\nX_ERROR(0.125) 0\nM 0\nDETECTOR rec[-1]\n")
            .expect("innermost body");
        for _ in 0..depth {
            let mut outer = Circuit::new();
            outer.append_repeat_block(stab_model::RepeatBlock::new(
                stab_model::RepeatCount::try_new(1).expect("one repetition"),
                body,
                None,
            ));
            body = outer;
        }
        body
    }
    std::thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(|| {
            let max_depth = stab_model::RepeatNestingLimit::HARD_MAX;
            for fold_loops in [false, true] {
                let options = ErrorAnalyzerOptions {
                    fold_loops,
                    ..ErrorAnalyzerOptions::default()
                };
                let model = try_analyze(&nested(max_depth), options.clone())
                    .expect("the full nesting envelope is analyzed");
                assert_eq!(model.count_errors(), Ok(1), "fold_loops={fold_loops}");
                let error = try_analyze(&nested(max_depth + 1), options)
                    .expect_err("one level beyond the envelope");
                let resource = error
                    .resource_limit_error()
                    .expect("typed repeat-nesting error");
                assert_eq!(resource.resource(), crate::ResourceKind::RepeatNesting);
                assert_eq!(resource.actual(), (max_depth + 1) as u64);
                assert_eq!(resource.limit(), max_depth as u64);
            }
        })
        .expect("spawn")
        .join()
        .expect("nesting boundary thread");
}

/// A wide reset, a CX fan-out onto qubit 0, one measurement, and `detectors` coordinate-heavy
/// detectors on it: the retained output is large before the reverse fan-out copies qubit 0's
/// sensitivity set onto every other qubit.
fn fan_out_after_heavy_output(qubits: u32, detectors: usize) -> Circuit {
    let targets = (0..qubits).map(|q| q.to_string()).collect::<Vec<_>>();
    let fan_out = (1..qubits).map(|q| format!("{q} 0")).collect::<Vec<_>>();
    let coordinates = vec!["1"; 32].join(",");
    let mut text = format!(
        "R {}\nX_ERROR(0.125) {}\nCX {}\nM 0\n",
        targets.join(" "),
        targets.join(" "),
        fan_out.join(" ")
    );
    for _ in 0..detectors {
        text.push_str(&format!("DETECTOR({coordinates}) rec[-1]\n"));
    }
    Circuit::from_stim_str(&text).expect("valid fan-out circuit")
}

/// The reverse tracker grows against the budget less the output and error classes already held,
/// so a refusal inside an instruction reports a need within one set growth of the budget instead
/// of discovering the overrun, by up to the whole held output, at the end of the instruction.
#[test]
fn tracker_growth_is_refused_against_the_bytes_the_analyzer_already_holds() {
    const DETECTORS: usize = 2_000;
    let circuit = fan_out_after_heavy_output(40, DETECTORS);
    let options = |bytes| ErrorAnalyzerOptions {
        memory_budget: MemoryBudget::from_bytes(bytes),
        ..ErrorAnalyzerOptions::default()
    };
    let one_set_growth = 2 * DETECTORS as u64 * size_of::<stab_model::DemTarget>() as u64;
    for budget in [1_000_000, 1_200_000, 1_400_000] {
        let error = try_analyze(&circuit, options(budget)).expect_err("the fan-out does not fit");
        let resource = error.resource_limit_error().expect("typed budget refusal");
        assert_eq!(resource.limit(), budget);
        assert!(
            resource.actual() <= budget + one_set_growth,
            "refusal needs {} bytes against a {budget}-byte budget",
            resource.actual()
        );
    }
}

/// When the fold search's two tracker clones do not fit, `--fold_loops` falls back to unrolling
/// instead of refusing a circuit that the unfolded run completes under the same budget.
#[test]
fn fold_search_that_does_not_fit_falls_back_to_unrolling() {
    let qubits = (0..40).map(|q| q.to_string()).collect::<Vec<_>>().join(" ");
    let fan_out = (1..40)
        .map(|q| format!("{q} 0"))
        .collect::<Vec<_>>()
        .join(" ");
    let mut text =
        format!("R {qubits}\nREPEAT 10 {{\n    X_ERROR(0.001) 100\n}}\nCX {fan_out}\nM 0\n");
    for _ in 0..2_000 {
        text.push_str("DETECTOR rec[-1]\n");
    }
    let circuit = Circuit::from_stim_str(&text).expect("valid fold-budget circuit");
    let options = |fold_loops, bytes| ErrorAnalyzerOptions {
        fold_loops,
        memory_budget: MemoryBudget::from_bytes(bytes),
        ..ErrorAnalyzerOptions::default()
    };
    // The smallest budget the unfolded run admits.
    let (mut low, mut high) = (0_u64, 1 << 24);
    let unfolded = try_analyze(&circuit, options(false, high)).expect("unfolded analysis");
    while low < high {
        let middle = low + (high - low) / 2;
        if try_analyze(&circuit, options(false, middle)).is_ok() {
            high = middle;
        } else {
            low = middle + 1;
        }
    }

    let folded = try_analyze(&circuit, options(true, low))
        .expect("the fold search gives up and unrolls within the unfolded budget");
    assert_eq!(folded, unfolded);
    let error = try_analyze(&circuit, options(true, low - 1))
        .expect_err("a genuine shortage still resurfaces from the unrolled run");
    assert_eq!(
        error.resource_limit_error().map(|error| error.limit()),
        Some(low - 1)
    );
}

/// Pinned Stim saturates its tick count, so more than `2^64` ticks is not an error. Expected
/// output is the pinned Stim v1.16.0 `analyze_errors --fold_loops` capture.
#[test]
fn folded_analysis_accepts_more_ticks_than_fit_in_u64() {
    let circuit = Circuit::from_stim_str(
        "REPEAT 1000000000000 {\n    REPEAT 1000000000000 {\n        TICK\n    }\n}\nR 0\nX_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]\n",
    )
    .expect("valid tick circuit");
    let model = try_analyze(
        &circuit,
        ErrorAnalyzerOptions {
            fold_loops: true,
            ..ErrorAnalyzerOptions::default()
        },
    )
    .expect("tick count does not limit analysis");
    assert_eq!(
        model.to_dem_string(),
        "repeat 1000000000000 {\n    repeat 1000000000000 {\n\n    }\n}\nerror(0.1000000000000000055511151231257827) D0\n"
    );
}
