#![allow(
    clippy::panic,
    reason = "typed resource assertions report the unexpected analysis error variant"
)]

use super::*;

pub(super) fn assert_resource_contract() {
    repeat_skips_unrequested_compact_tick_spans();
    repeat_folds_nontrivial_zero_tick_body();
    large_identity_snapshot_stays_sparse();
    large_active_snapshot_fails_before_dense_allocation();
    huge_repeat_reports_semantic_errors_promptly();
    repeat_nesting_boundary_is_exact();
    budget_boundaries_are_typed_and_fail_closed();
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
    assert_eq!(resource.operation(), ResourceOperation::DetectingRegions);
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
        },
    )
    .unwrap();
    assert!(!actual.contains_key(&detector(0)));
}

fn large_active_snapshot_fails_before_dense_allocation() {
    let circuit = Circuit::from_stim_str(
        "QUBIT_COORDS 16000000\nREPEAT 1000000000000 {\n    TICK\n}\nM 16000000\nDETECTOR rec[-1]\n",
    )
    .unwrap();
    let error = circuit_detecting_regions(
        &circuit,
        DetectingRegionOptions {
            detectors: vec![detector(0)],
            ticks: ticks(&[999_999_999_999]),
            ignore_anticommutation_errors: false,
        },
    )
    .unwrap_err();
    let AnalysisError::ResourceLimit(resource) = error else {
        panic!("expected typed output materialization rejection, got {error}");
    };
    assert_eq!(resource.operation(), ResourceOperation::DetectingRegions);
    assert_eq!(resource.resource(), ResourceKind::MaterializedUnits);
    assert!(resource.actual() > resource.limit());
    assert_eq!(
        resource.limit(),
        stab_algebra::StabilizerResource::PauliQubits.limit() as u64
    );
}

fn huge_repeat_reports_semantic_errors_promptly() {
    let circuit = Circuit::from_stim_str("REPEAT 1000000000000 {\n    MPP X0*Z0\n}\n").unwrap();
    let error = circuit_detecting_regions_for_targets(
        &circuit,
        DetectingRegionTargetOptions {
            targets: vec![],
            ticks: vec![],
            ignore_anticommutation_errors: false,
        },
    )
    .unwrap_err();
    assert!(error.to_string().contains("anti-Hermitian"));
}

fn repeat_nesting_boundary_is_exact() {
    fn nested(depth: usize) -> Circuit {
        let mut body = Circuit::from_stim_str("TICK\n").unwrap();
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
    let options = || DetectingRegionTargetOptions {
        targets: vec![],
        ticks: vec![],
        ignore_anticommutation_errors: false,
    };
    circuit_detecting_regions_for_targets(&nested(MAX_DETECTING_REGION_REPEAT_NESTING), options())
        .unwrap();
    assert_resource_error(
        circuit_detecting_regions_for_targets(
            &nested(MAX_DETECTING_REGION_REPEAT_NESTING + 1),
            options(),
        )
        .unwrap_err(),
        ResourceKind::RepeatNesting,
        (MAX_DETECTING_REGION_REPEAT_NESTING + 1) as u64,
        MAX_DETECTING_REGION_REPEAT_NESTING as u64,
    );
}

fn budget_boundaries_are_typed_and_fail_closed() {
    let mut represented = DetectingRegionBudget::for_request(0, 0).unwrap();
    represented
        .add_represented_work(MAX_DETECTING_REGION_REPRESENTED_WORK)
        .unwrap();
    assert_resource_error(
        represented.add_represented_work(1).unwrap_err(),
        ResourceKind::RepresentedItems,
        MAX_DETECTING_REGION_REPRESENTED_WORK + 1,
        MAX_DETECTING_REGION_REPRESENTED_WORK,
    );

    let mut traversal = DetectingRegionBudget::for_request(0, 0).unwrap();
    traversal
        .consume_traversal_work(MAX_DETECTING_REGION_TRAVERSAL_WORK)
        .unwrap();
    assert_resource_error(
        traversal.consume_traversal_work(1).unwrap_err(),
        ResourceKind::TraversalWork,
        MAX_DETECTING_REGION_TRAVERSAL_WORK + 1,
        MAX_DETECTING_REGION_TRAVERSAL_WORK,
    );

    let mut live = DetectingRegionBudget::for_request(0, 0).unwrap();
    live.reserve_live_state(MAX_DETECTING_REGION_LIVE_STATE_UNITS)
        .unwrap();
    assert_resource_error(
        live.reserve_live_state(1).unwrap_err(),
        ResourceKind::LiveStateUnits,
        MAX_DETECTING_REGION_LIVE_STATE_UNITS + 1,
        MAX_DETECTING_REGION_LIVE_STATE_UNITS,
    );

    let mut clone_boundary = DetectingRegionBudget::for_request(0, 0).unwrap();
    clone_boundary
        .reserve_tracker_state(MAX_DETECTING_REGION_LIVE_STATE_UNITS / 3)
        .unwrap();
    clone_boundary.admit_recurrence_probe().unwrap();
    let mut clone_excess = DetectingRegionBudget::for_request(0, 0).unwrap();
    clone_excess
        .reserve_tracker_state(MAX_DETECTING_REGION_LIVE_STATE_UNITS / 3 + 1)
        .unwrap();
    let error = clone_excess.admit_recurrence_probe().unwrap_err();
    let AnalysisError::ResourceLimit(resource) = error else {
        panic!("expected typed live-state rejection, got {error}");
    };
    assert_eq!(resource.operation(), ResourceOperation::DetectingRegions);
    assert_eq!(resource.resource(), ResourceKind::LiveStateUnits);
    assert!(resource.actual() > resource.limit());

    let mut regions = DetectingRegionBudget::for_request(0, 0).unwrap();
    regions.output_regions = MAX_DETECTING_REGION_OUTPUT_REGIONS;
    assert_resource_error(
        regions.commit_output_region(0).unwrap_err(),
        ResourceKind::OutputRecords,
        MAX_DETECTING_REGION_OUTPUT_REGIONS + 1,
        MAX_DETECTING_REGION_OUTPUT_REGIONS,
    );

    let mut bytes = DetectingRegionBudget::for_request(0, 0).unwrap();
    bytes
        .commit_output_region(MAX_DETECTING_REGION_OUTPUT_BYTES)
        .unwrap();
    assert_resource_error(
        bytes.commit_output_region(1).unwrap_err(),
        ResourceKind::OutputBytes,
        MAX_DETECTING_REGION_OUTPUT_BYTES + 1,
        MAX_DETECTING_REGION_OUTPUT_BYTES,
    );
}
