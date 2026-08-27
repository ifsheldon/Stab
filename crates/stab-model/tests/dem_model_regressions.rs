#![allow(
    clippy::expect_used,
    reason = "compatibility tests use direct fixture assertions for compact diagnostics"
)]

use stab_model::{DemInstruction, DemRepeatBlock, DemRepeatCount, DemTarget, DetectorErrorModel};

#[test]
fn dem_instruction_targets_parse_stim_limits() {
    assert_eq!(
        "D1152921504606846975"
            .parse::<DemTarget>()
            .expect("detector target"),
        DemTarget::relative_detector(1_152_921_504_606_846_975).expect("detector target")
    );
    assert!(DemTarget::relative_detector(4_611_686_018_427_387_903).is_ok());
    assert_eq!(
        "L4294967295"
            .parse::<DemTarget>()
            .expect("observable target"),
        DemTarget::logical_observable(4_294_967_295).expect("observable target")
    );
    assert_eq!(
        "^".parse::<DemTarget>().expect("separator target"),
        DemTarget::separator()
    );
    assert!("10".parse::<DemTarget>().is_err());
    assert_eq!(
        DetectorErrorModel::from_dem_str("shift_detectors 10\n")
            .expect("valid detector shift")
            .to_dem_string(),
        "shift_detectors 10\n"
    );

    for invalid in ["D1152921504606846976", "L4294967296", "D-1", "Da", ""] {
        assert!(
            invalid.parse::<DemTarget>().is_err(),
            "unexpectedly accepted {invalid:?}"
        );
    }
}

#[test]
fn dem_parse_print_round_trip_includes_repeats_shifts_coordinates_and_tags() {
    let text = "error(0.125) D0\nrepeat[test\\Ctag] 100 {\n    error(0.25) D0 D1 L0 ^ D2\n    shift_detectors(1.5, 3) 10\n    detector(0.5) D0\n    logical_observable L0\n}\n";

    let model = DetectorErrorModel::from_dem_str(text).expect("valid DEM");
    let canonical = model.to_dem_string();

    assert_eq!(canonical, text);
    assert_eq!(
        DetectorErrorModel::from_dem_str(&canonical).expect("canonical DEM reparses"),
        model
    );
}

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
