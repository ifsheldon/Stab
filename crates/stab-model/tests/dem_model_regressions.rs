#![allow(
    clippy::expect_used,
    reason = "compatibility tests use direct fixture assertions for compact diagnostics"
)]

use stab_model::{DemInstruction, DemRepeatBlock, DemRepeatCount, DemTarget, DetectorErrorModel};

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

#[test]
fn deeply_programmatic_dem_drops_on_a_small_stack() {
    std::thread::Builder::new()
        .stack_size(64 * 1024)
        .spawn(|| {
            let mut body = DetectorErrorModel::new();
            for _ in 0..4_096 {
                let mut outer = DetectorErrorModel::new();
                outer.push_repeat_block(DemRepeatBlock::new(DemRepeatCount::new(1), body, None));
                body = outer;
            }
            drop(body);
        })
        .expect("spawn small-stack drop worker")
        .join()
        .expect("deep DEM drop remains stack safe");
}
