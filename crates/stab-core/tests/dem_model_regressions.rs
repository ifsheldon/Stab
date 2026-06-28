#![allow(
    clippy::expect_used,
    reason = "compatibility tests use direct fixture assertions for compact diagnostics"
)]

use stab_core::{DemInstruction, DemTarget, DetectorErrorModel};

#[test]
fn dem_count_detectors_rejects_shifted_detector_count_overflow() {
    let mut model = DetectorErrorModel::new();
    model.push_instruction(
        DemInstruction::shift_detectors(Vec::new(), u64::MAX, None).expect("detector shift"),
    );
    model.push_instruction(
        DemInstruction::detector(
            Vec::new(),
            DemTarget::relative_detector(0).expect("detector target"),
            None,
        )
        .expect("detector instruction"),
    );

    let error = model.count_detectors().expect_err("reject overflow");

    assert!(error.to_string().contains("detector count overflowed"));
}
