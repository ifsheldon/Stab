#![allow(
    clippy::panic,
    reason = "typed resource assertions report the unexpected analysis error variant"
)]

use super::*;

pub(super) fn assert_resource_contract() {
    repeat_skips_unrequested_compact_tick_spans();
    repeat_folds_nontrivial_zero_tick_body();
    large_identity_snapshot_stays_sparse();
    large_active_snapshot_is_admitted_in_bytes();
    huge_repeat_reports_semantic_errors_promptly();
    repeat_nesting_boundary_is_exact();
    budget_boundaries_are_typed_and_fail_closed();
    caller_budget_governs_region_requests();
    live_tracker_sensitivity_is_metered_without_reset_round_overcounting();
}

fn assert_resource_error(
    error: AnalysisError,
    expected_resource: ResourceKind,
    expected_actual: u64,
    expected_limit: u64,
) {
    let AnalysisError::ResourceLimit(resource) = error else {
        panic!("expected typed detecting-region resource error, got {error}");
    };
    assert_eq!(resource.operation(), AnalysisOperation::DetectingRegions);
    assert_eq!(resource.resource(), expected_resource);
    assert_eq!(resource.actual(), expected_actual);
    assert_eq!(resource.limit(), expected_limit);
}

fn repeat_skips_unrequested_compact_tick_spans() {
    let circuit = Circuit::from_stim_str(
        "RX 0 1\nREPEAT 1000000000000 {\n    TICK\n}\nMXX 0 1\nDETECTOR rec[-1]\n",
    )
    .unwrap();
    let actual = circuit_detecting_regions(
        &circuit,
        DetectingRegionOptions {
            detectors: vec![detector(0)],
            ticks: ticks(&[0, 500_000_000_000, 999_999_999_999]),
            ignore_anticommutation_errors: false,
            memory_budget: stab_model::MemoryBudget::DEFAULT,
            cancellation: stab_model::Cancellation::never(),
        },
    )
    .unwrap();

    for selected_tick in [0, 500_000_000_000, 999_999_999_999] {
        assert_eq!(
            actual[&detector(0)][&tick(selected_tick)].to_string(),
            "+XX"
        );
    }
}

fn repeat_folds_nontrivial_zero_tick_body() {
    let empty = Circuit::from_stim_str("REPEAT 1000000000000 {\n}\n").unwrap();
    assert!(
        circuit_detecting_regions_for_targets(
            &empty,
            DetectingRegionTargetOptions {
                targets: vec![],
                ticks: vec![],
                ignore_anticommutation_errors: false,
                memory_budget: stab_model::MemoryBudget::DEFAULT,
                cancellation: stab_model::Cancellation::never(),
            },
        )
        .unwrap()
        .is_empty()
    );
    let huge = regions(
        "RX 0\nTICK\nREPEAT 1000000000001 {\n    H 0\n}\nM 0\nDETECTOR rec[-1]\n",
        vec![detector(0)],
        vec![0],
    );
    let single = regions(
        "RX 0\nTICK\nH 0\nM 0\nDETECTOR rec[-1]\n",
        vec![detector(0)],
        vec![0],
    );
    assert_eq!(huge, single);
    assert_eq!(huge[&detector(0)][&tick(0)].to_string(), "+X");
}

fn large_identity_snapshot_stays_sparse() {
    let circuit = Circuit::from_stim_str(
        "QUBIT_COORDS 16000000\nREPEAT 1000000000000 {\n    TICK\n}\nM 0\nDETECTOR rec[-1]\nTICK\n",
    )
    .unwrap();
    let actual = circuit_detecting_regions(
        &circuit,
        DetectingRegionOptions {
            detectors: vec![detector(0)],
            ticks: ticks(&[1_000_000_000_000]),
            ignore_anticommutation_errors: false,
            memory_budget: stab_model::MemoryBudget::DEFAULT,
            cancellation: stab_model::Cancellation::never(),
        },
    )
    .unwrap();
    assert!(!actual.contains_key(&detector(0)));
}

fn large_active_snapshot_is_admitted_in_bytes() {
    // One past the former 1,048,576-qubit count cap: the dense output region is charged to the
    // memory budget instead, which a 16,000,001-qubit region fits.
    let circuit = Circuit::from_stim_str(
        "QUBIT_COORDS 16000000\nREPEAT 1000000000000 {\n    TICK\n}\nM 16000000\nDETECTOR rec[-1]\n",
    )
    .unwrap();
    let actual = circuit_detecting_regions(
        &circuit,
        DetectingRegionOptions {
            detectors: vec![detector(0)],
            ticks: ticks(&[999_999_999_999]),
            ignore_anticommutation_errors: false,
            memory_budget: stab_model::MemoryBudget::DEFAULT,
            cancellation: stab_model::Cancellation::never(),
        },
    )
    .unwrap();
    let Some(region) = actual
        .get(&detector(0))
        .and_then(|regions| regions.get(&tick(999_999_999_999)))
    else {
        panic!("expected the active region at the requested tick");
    };
    assert_eq!(region.len(), 16_000_001);
    assert_eq!(region.value().weight(), 1);
    assert_eq!(region.get(16_000_000), Some(PauliBasis::Z));
}

fn huge_repeat_reports_semantic_errors_promptly() {
    let circuit = Circuit::from_stim_str("REPEAT 1000000000000 {\n    MPP X0*Z0\n}\n").unwrap();
    let error = circuit_detecting_regions_for_targets(
        &circuit,
        DetectingRegionTargetOptions {
            targets: vec![],
            ticks: vec![],
            ignore_anticommutation_errors: false,
            memory_budget: stab_model::MemoryBudget::DEFAULT,
            cancellation: stab_model::Cancellation::never(),
        },
    )
    .unwrap_err();
    assert!(error.to_string().contains("anti-Hermitian"));
}

fn repeat_nesting_boundary_is_exact() {
    fn nested(depth: usize, inner_stim: &str) -> Circuit {
        let mut body = Circuit::from_stim_str(inner_stim).unwrap();
        for _ in 0..depth {
            let mut outer = Circuit::new();
            outer.append_repeat_block(RepeatBlock::new(
                RepeatCount::try_new(1).unwrap(),
                body,
                None,
            ));
            body = outer;
        }
        body
    }

    // Run on an explicit 2 MiB stack in debug builds, exercising both the skipped-span
    // tracker walk and a tick-selected snapshot descent across all 256 repeat levels.
    std::thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(|| {
            let max_depth = RepeatNestingLimit::HARD_MAX;
            let empty_options = || DetectingRegionTargetOptions {
                targets: vec![],
                ticks: vec![],
                ignore_anticommutation_errors: false,
                memory_budget: stab_model::MemoryBudget::DEFAULT,
                cancellation: stab_model::Cancellation::never(),
            };
            circuit_detecting_regions_for_targets(&nested(max_depth, "TICK\n"), empty_options())
                .unwrap();

            let active = nested(max_depth, "R 0\nTICK\nX 0\nM 0\nDETECTOR rec[-1]\n");
            let regions = circuit_detecting_regions_for_targets(
                &active,
                DetectingRegionTargetOptions {
                    targets: vec![DemTarget::relative_detector(0).unwrap()],
                    ticks: vec![CircuitTick::new(0)],
                    ignore_anticommutation_errors: false,
                    memory_budget: stab_model::MemoryBudget::DEFAULT,
                    cancellation: stab_model::Cancellation::never(),
                },
            )
            .unwrap();
            assert_eq!(regions.len(), 1);

            // Nested unitary repeats above one iteration fold one body undo per level on the
            // tracker's own stack; unrolling these would take 2^256 body undos.
            let mut unitary = Circuit::from_stim_str("H 0\nH 0\n").unwrap();
            let suffix = Circuit::from_stim_str("H 0\nS 0\nS 0\nH 0\n").unwrap();
            for _ in 1..max_depth {
                let mut outer = Circuit::new();
                outer.append_repeat_block(RepeatBlock::new(
                    RepeatCount::try_new(2).unwrap(),
                    unitary,
                    None,
                ));
                for item in suffix.items() {
                    if let CircuitItem::Instruction(instruction) = item {
                        outer.append_instruction(instruction.clone());
                    }
                }
                unitary = outer;
            }
            let mut folded = Circuit::from_stim_str("R 0\nTICK\n").unwrap();
            folded.append_repeat_block(RepeatBlock::new(
                RepeatCount::try_new(3).unwrap(),
                unitary,
                None,
            ));
            for item in Circuit::from_stim_str("M 0\nDETECTOR rec[-1]\n")
                .unwrap()
                .items()
            {
                if let CircuitItem::Instruction(instruction) = item {
                    folded.append_instruction(instruction.clone());
                }
            }
            let regions = circuit_detecting_regions_for_targets(
                &folded,
                DetectingRegionTargetOptions {
                    targets: vec![DemTarget::relative_detector(0).unwrap()],
                    ticks: vec![CircuitTick::new(0)],
                    ignore_anticommutation_errors: false,
                    memory_budget: stab_model::MemoryBudget::DEFAULT,
                    cancellation: stab_model::Cancellation::never(),
                },
            )
            .unwrap();
            assert_eq!(regions.len(), 1);

            assert_resource_error(
                circuit_detecting_regions_for_targets(
                    &nested(max_depth + 1, "TICK\n"),
                    empty_options(),
                )
                .unwrap_err(),
                ResourceKind::RepeatNesting,
                (max_depth + 1) as u64,
                max_depth as u64,
            );
        })
        .unwrap()
        .join()
        .unwrap();
}

fn budget_boundaries_are_typed_and_fail_closed() {
    let budget_bytes = MemoryBudget::DEFAULT.bytes();
    let max_units = budget_bytes / DETECTING_REGION_LIVE_STATE_UNIT_BYTES;
    let mut live = DetectingRegionBudget::for_request(MemoryBudget::DEFAULT, 0, 0).unwrap();
    live.reserve_live_state(max_units).unwrap();
    assert_resource_error(
        live.reserve_live_state(1).unwrap_err(),
        ResourceKind::LiveStateUnits,
        (max_units + 1) * DETECTING_REGION_LIVE_STATE_UNIT_BYTES,
        budget_bytes,
    );

    let mut bytes = DetectingRegionBudget::for_request(MemoryBudget::DEFAULT, 0, 0).unwrap();
    bytes.commit_output_region(budget_bytes, 0).unwrap();
    assert_resource_error(
        bytes.commit_output_region(1, 0).unwrap_err(),
        ResourceKind::OutputBytes,
        budget_bytes + 1,
        budget_bytes,
    );
}

fn live_tracker_sensitivity_is_metered_without_reset_round_overcounting() {
    // Unfolded reset-bounded rounds clear qubit sensitivity every round, so live tracker state
    // stays O(1) instead of growing quadratically with detector count.
    let mut rounds = String::new();
    for _ in 0..400 {
        rounds.push_str("R 0\nTICK\nM 0\nDETECTOR rec[-1]\n");
    }
    let reset_bounded = Circuit::from_stim_str(&rounds).unwrap();
    let tight = MemoryBudget::from_bytes(4096);
    let extracted = circuit_detecting_regions(
        &reset_bounded,
        DetectingRegionOptions {
            detectors: vec![detector(0)],
            ticks: ticks(&[0]),
            ignore_anticommutation_errors: false,
            memory_budget: tight,
            cancellation: stab_model::Cancellation::never(),
        },
    )
    .unwrap();
    assert_eq!(extracted[&detector(0)][&tick(0)].to_string(), "+Z");

    // Conversely, a detector whose live sensitivity fans out across 128 qubits simultaneously
    // exceeds a budget that admits the 128 represented qubits alone.
    let mut fanout = String::from("R 0\nTICK\n");
    for target in 1..128 {
        fanout.push_str(&format!("CX 0 {target}\n"));
    }
    fanout.push_str("H 0\n");
    for target in 1..128 {
        fanout.push_str(&format!("CX 0 {target}\n"));
    }
    fanout.push_str("H 0\nM 0\nDETECTOR rec[-1]\n");
    let wide_live = Circuit::from_stim_str(&fanout).unwrap();
    let represented_only = MemoryBudget::from_bytes(132 * DETECTING_REGION_LIVE_STATE_UNIT_BYTES);
    let error = circuit_detecting_regions(
        &wide_live,
        DetectingRegionOptions {
            detectors: vec![detector(0)],
            ticks: ticks(&[0]),
            ignore_anticommutation_errors: false,
            memory_budget: represented_only,
            cancellation: stab_model::Cancellation::never(),
        },
    )
    .unwrap_err();
    let AnalysisError::ResourceLimit(resource) = error else {
        panic!("expected live sensitivity budget refusal, got {error}");
    };
    assert_eq!(resource.operation(), AnalysisOperation::DetectingRegions);
    assert_eq!(resource.resource(), ResourceKind::LiveStateUnits);
    assert_eq!(resource.limit(), represented_only.bytes());
    assert!(resource.actual() > resource.limit());
}

fn caller_budget_governs_region_requests() {
    let circuit = Circuit::from_stim_str("R 0\nTICK\nM 0\nDETECTOR rec[-1]\n").unwrap();
    let request = |memory_budget| {
        circuit_detecting_regions(
            &circuit,
            DetectingRegionOptions {
                detectors: vec![detector(0)],
                ticks: ticks(&[0]),
                ignore_anticommutation_errors: false,
                memory_budget,
                cancellation: stab_model::Cancellation::never(),
            },
        )
    };
    // The requested detector and tick are each one live-state unit before traversal.
    let required = 2 * DETECTING_REGION_LIVE_STATE_UNIT_BYTES;
    let short = MemoryBudget::from_bytes(required - 1);
    assert_resource_error(
        request(short).unwrap_err(),
        ResourceKind::LiveStateUnits,
        required,
        short.bytes(),
    );
    let regions = request(MemoryBudget::DEFAULT).unwrap();
    assert_eq!(regions[&detector(0)][&tick(0)].to_string(), "+Z");
}
