#![allow(
    clippy::expect_used,
    reason = "resource-boundary tests use compact fixture construction and exact diagnostics"
)]

use stab_model::{
    DemInstruction, DemRepeatBlock, DemRepeatCount, DemTarget, DetectorErrorModel, Probability,
};

fn leaf_model() -> DetectorErrorModel {
    let mut model = DetectorErrorModel::new();
    model.push_instruction(
        DemInstruction::error(
            Probability::try_new(0.125).expect("valid probability"),
            vec![
                DemTarget::relative_detector(0).expect("D0"),
                DemTarget::logical_observable(0).expect("L0"),
            ],
            Some("leaf".to_string()),
        )
        .expect("valid DEM error instruction"),
    );
    model
}

fn nested_repeat_model(depth: usize) -> DetectorErrorModel {
    let mut model = leaf_model();
    for _ in 0..depth {
        let mut outer = DetectorErrorModel::new();
        outer.push_repeat_block(DemRepeatBlock::new(
            DemRepeatCount::new(1),
            model,
            Some("repeat".to_string()),
        ));
        model = outer;
    }
    model
}

#[test]
fn programmatic_folded_traversal_accepts_exact_repeat_depth_boundary() {
    let model = nested_repeat_model(256);

    assert_eq!(model.total_detector_shift().expect("shift summary"), 0);
    assert_eq!(model.count_detectors().expect("detector summary"), 1);
    assert_eq!(model.count_observables().expect("observable summary"), 1);
}

#[test]
fn programmatic_folded_traversal_preserves_depth_257_compatibility() {
    let model = nested_repeat_model(257);

    assert_eq!(model.count_detectors().expect("detector summary"), 1);
    assert_eq!(model.count_observables().expect("observable summary"), 1);
}

#[test]
fn programmatic_folded_clone_debug_and_drop_use_bounded_stack() {
    let model = nested_repeat_model(10_000);
    let handle = std::thread::Builder::new()
        .stack_size(64 * 1024)
        .spawn(move || {
            let cloned = model.clone();
            let result = (
                model.count_detectors().map_err(|error| error.to_string()),
                model.count_observables().map_err(|error| error.to_string()),
                model == cloned,
                format!("{model:?}"),
            );
            drop(cloned);
            drop(model);
            Ok::<_, String>(result)
        })
        .expect("spawn constrained-stack regression");

    let (detectors, observables, equal, debug) = handle
        .join()
        .expect("constrained-stack query should not panic")
        .expect("deep transforms should succeed");
    assert_eq!(detectors, Ok(1));
    assert_eq!(observables, Ok(1));
    assert!(equal);
    assert!(debug.starts_with("DetectorErrorModel { top_level_items: 1"));
}
