#![allow(
    clippy::expect_used,
    reason = "sampling-estimate fixtures use fixed valid circuits"
)]

use stab_engine::estimate_sampling_request;
use stab_model::{Circuit, Estimate};
use stab_records::RecordFormat;

#[test]
fn sampling_request_estimate_is_folded_bounded_and_allocation_free() {
    let circuit = Circuit::from_stim_str("R 0\nREPEAT 5 {\n    X_ERROR(0.25) 0\n    M 0\n}\n")
        .expect("valid sampling request");

    let dense = estimate_sampling_request(&circuit, 64, RecordFormat::ZeroOne);
    assert_eq!(dense.input_items(), Estimate::Exact(4));
    assert_eq!(dense.folded_traversal(), Estimate::Exact(4));
    assert_eq!(dense.expanded_operations(), Estimate::Exact(11));
    assert_eq!(dense.output_bytes(), Estimate::Exact(384));
    assert_eq!(dense.scratch_bytes(), Estimate::Unknown);
    assert_eq!(dense.resident_bytes(), Estimate::Unknown);
    assert_eq!(dense.work_units(), Estimate::Unknown);

    assert_eq!(
        estimate_sampling_request(&circuit, 64, RecordFormat::B8).output_bytes(),
        Estimate::Exact(64)
    );
    assert_eq!(
        estimate_sampling_request(&circuit, 64, RecordFormat::Ptb64).output_bytes(),
        Estimate::Exact(40)
    );
    for format in [RecordFormat::R8, RecordFormat::Hits, RecordFormat::Dets] {
        assert_eq!(
            estimate_sampling_request(&circuit, 64, format).output_bytes(),
            Estimate::Unknown
        );
    }

    let expanded_overflow = Circuit::from_stim_str(
        "REPEAT 4294967296 {\n    REPEAT 4294967296 {\n        M 0\n    }\n}\n",
    )
    .expect("valid compact circuit");
    let estimate = estimate_sampling_request(&expanded_overflow, 1, RecordFormat::ZeroOne);
    assert_eq!(estimate.input_items(), Estimate::Exact(3));
    assert_eq!(estimate.expanded_operations(), Estimate::Unknown);
    assert_eq!(estimate.output_bytes(), Estimate::Unknown);

    let single_measurement = Circuit::from_stim_str("M 0\n").expect("valid circuit");
    assert_eq!(
        estimate_sampling_request(&single_measurement, usize::MAX, RecordFormat::ZeroOne)
            .output_bytes(),
        Estimate::Unknown
    );

    let allocations = allocation_counter::measure(|| {
        std::hint::black_box(estimate_sampling_request(
            &single_measurement,
            64,
            RecordFormat::ZeroOne,
        ));
    });
    assert_eq!(allocations.count_total, 0, "{allocations:?}");
}
