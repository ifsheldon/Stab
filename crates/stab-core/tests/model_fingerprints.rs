#![allow(
    clippy::expect_used,
    reason = "fingerprint contract tests use direct fixture assertions"
)]

use stab_core::{
    Circuit, DemRepeatBlock, DemRepeatCount, DetectorErrorModel, ModelDialect, ModelFingerprint,
    RepeatBlock, RepeatCount, RepeatNestingLimit,
};

const RICH_CIRCUIT: &str = "X_ERROR[π](0.12345641) 0\n\
M !1\n\
CX sweep[7] 2\n\
MPP !X0*Y1*!Z2\n\
DETECTOR[coord](-0, 1.25) rec[-0] rec[-1]\n\
REPEAT[loop] 3 {\n\
    H 3\n\
}\n";

const RICH_DEM: &str = "error[ε](0.12345641) D0 L1 ^ D2\n\
detector[coord](-0, 2.5) D3\n\
logical_observable[obs] L4\n\
shift_detectors[shift](-0, 7.25) 9\n\
repeat[loop] 3 {\n\
    error(0.25) D5\n\
}\n";

#[test]
fn circuit_fingerprint_is_canonical_and_deterministic() {
    let decorated = Circuit::from_stim_str(
        "# ignored\r\nx_error[π](0.12345641) 0\r\nM !1\r\nCNOT sweep[7] 2\r\n\
         MPP !X0*Y1*!Z2 # ignored too\r\nDETECTOR[coord](-0, 1.25) rec[-0] rec[-1]\r\n\
         REPEAT[loop] 3 {\r\nH 3\r\n}\r\n",
    )
    .expect("decorated circuit");
    let canonical = Circuit::from_stim_str(RICH_CIRCUIT).expect("canonical circuit");

    let fingerprint = decorated.fingerprint();
    assert_eq!(fingerprint, decorated.fingerprint());
    assert_eq!(fingerprint, canonical.fingerprint());
    assert_eq!(fingerprint.schema_version(), 1);
    assert_eq!(fingerprint.dialect(), ModelDialect::StimCircuit);
    assert_eq!(fingerprint.digest_hex().len(), 64);

    let allocations = allocation_counter::measure(|| {
        std::hint::black_box(canonical.fingerprint());
    });
    assert_eq!(allocations.count_total, 0, "{allocations:?}");
    assert_eq!(allocations.bytes_total, 0, "{allocations:?}");
}

#[test]
fn dem_fingerprint_is_canonical_and_deterministic() {
    let decorated = DetectorErrorModel::from_dem_str(
        "# ignored\r\nERROR[ε](0.12345641) D0 L1 ^ D2\r\n\
         detector[coord](-0, 2.5) D3\r\nlogical_observable[obs] L4\r\n\
         SHIFT_DETECTORS[shift](-0, 7.25) 9\r\nrepeat[loop] 3 {\r\nerror(0.25) D5\r\n}\r\n",
    )
    .expect("decorated DEM");
    let canonical = DetectorErrorModel::from_dem_str(RICH_DEM).expect("canonical DEM");

    let fingerprint = decorated.fingerprint();
    assert_eq!(fingerprint, decorated.fingerprint());
    assert_eq!(fingerprint, canonical.fingerprint());
    assert_eq!(fingerprint.schema_version(), 1);
    assert_eq!(fingerprint.dialect(), ModelDialect::DetectorErrorModel);

    let allocations = allocation_counter::measure(|| {
        std::hint::black_box(canonical.fingerprint());
    });
    assert_eq!(allocations.count_total, 0, "{allocations:?}");
    assert_eq!(allocations.bytes_total, 0, "{allocations:?}");
}

#[test]
fn model_fingerprint_value_contract_is_domain_separated_and_frozen() {
    let empty_circuit = Circuit::new().fingerprint();
    let empty_dem = DetectorErrorModel::new().fingerprint();
    assert_ne!(empty_circuit, empty_dem);
    assert_ne!(empty_circuit.digest(), empty_dem.digest());
    assert_eq!(ModelDialect::StimCircuit.as_str(), "stim-circuit");
    assert_eq!(
        ModelDialect::DetectorErrorModel.as_str(),
        "detector-error-model"
    );

    let first = Circuit::from_stim_str("X_ERROR(0.12345641) 0\n").expect("first circuit");
    let second = Circuit::from_stim_str("X_ERROR(0.12345649) 0\n").expect("second circuit");
    assert_eq!(
        first.to_stim_string(),
        second.to_stim_string(),
        "the Stim-compatible printer intentionally rounds both values alike"
    );
    assert_ne!(
        first.fingerprint(),
        second.fingerprint(),
        "model identity must retain semantic precision beyond printer output"
    );

    let positive_zero =
        Circuit::from_stim_str("DETECTOR(0, 1) rec[-1]\n").expect("positive zero circuit");
    let negative_zero =
        Circuit::from_stim_str("DETECTOR(-0, 1) rec[-1]\n").expect("negative zero circuit");
    assert_eq!(positive_zero.fingerprint(), negative_zero.fingerprint());

    assert_eq!(ModelFingerprint::ALGORITHM, "sha256");
    assert_eq!(
        Circuit::from_stim_str(RICH_CIRCUIT)
            .expect("frozen circuit")
            .fingerprint()
            .digest_hex(),
        "78361913886a45606681a49071b1689ad37758308655e69f28ed68675046f3dd"
    );
    assert_eq!(
        DetectorErrorModel::from_dem_str(RICH_DEM)
            .expect("frozen DEM")
            .fingerprint()
            .digest_hex(),
        "a9da2cfcc5bbb92bdf4f50a9da5a5669f7f50909baaf7700a9aece756d554c65"
    );

    assert_repeat_depth_resource_contract();
}

fn assert_repeat_depth_resource_contract() {
    let admitted_circuit = nested_circuit(RepeatNestingLimit::HARD_MAX);
    let admitted_dem = nested_dem(RepeatNestingLimit::HARD_MAX);
    for allocations in [
        allocation_counter::measure(|| {
            std::hint::black_box(admitted_circuit.fingerprint());
        }),
        allocation_counter::measure(|| {
            std::hint::black_box(admitted_dem.fingerprint());
        }),
    ] {
        assert_eq!(allocations.count_total, 0, "{allocations:?}");
        assert_eq!(allocations.bytes_total, 0, "{allocations:?}");
    }

    let programmatic_depth = RepeatNestingLimit::HARD_MAX + 128;
    let deep_circuit = nested_circuit(programmatic_depth);
    let deep_dem = nested_dem(programmatic_depth);
    for allocations in [
        allocation_counter::measure(|| {
            std::hint::black_box(deep_circuit.fingerprint());
        }),
        allocation_counter::measure(|| {
            std::hint::black_box(deep_dem.fingerprint());
        }),
    ] {
        assert!(
            allocations.bytes_max <= 16 * 1024,
            "depth-only traversal scratch exceeded its bound: {allocations:?}"
        );
    }
}

fn nested_circuit(depth: usize) -> Circuit {
    let repeat_count = RepeatCount::try_new(1).expect("nonzero circuit repeat count");
    let mut body = Circuit::new();
    for _ in 0..depth {
        let mut outer = Circuit::new();
        outer.append_repeat_block(RepeatBlock::new(repeat_count, body, None));
        body = outer;
    }
    body
}

fn nested_dem(depth: usize) -> DetectorErrorModel {
    let mut body = DetectorErrorModel::new();
    for _ in 0..depth {
        let mut outer = DetectorErrorModel::new();
        outer.push_repeat_block(DemRepeatBlock::new(DemRepeatCount::new(1), body, None));
        body = outer;
    }
    body
}
