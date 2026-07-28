use super::*;
use crate::{
    allocations::{AllocationTrackingGuard, allocation_tracking_test_lock},
    baseline::{
        batch_sinks::OutputWitness, cli_process, m9, m11,
        measure_stab_iterations_with_memory_operation,
    },
};
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn process_timing_and_product_memory_operations_are_separate() {
    let _test_lock = allocation_tracking_test_lock();
    let _guard = AllocationTrackingGuard::set(cfg!(feature = "count-allocations"))
        .expect("select available allocation mode");
    let timed_calls = AtomicUsize::new(0);
    let memory_calls = AtomicUsize::new(0);

    let measurement = measure_stab_iterations_with_memory_operation(
        "process-timing-product-memory",
        2,
        || {
            timed_calls.fetch_add(1, Ordering::Relaxed);
            Ok(())
        },
        || {
            memory_calls.fetch_add(1, Ordering::Relaxed);
            Ok(())
        },
    )
    .expect("measure split operations");

    assert_eq!(timed_calls.load(Ordering::Relaxed), 2);
    assert_eq!(
        memory_calls.load(Ordering::Relaxed),
        usize::from(cfg!(feature = "count-allocations"))
    );
    assert_eq!(measurement.iterations, Some(2));
    assert_eq!(
        measurement.allocation.is_some(),
        cfg!(feature = "count-allocations")
    );
}

#[cfg(feature = "count-allocations")]
#[test]
fn a5_phase_rows_separate_timed_witnesses_from_memory_observation() {
    let _test_lock = allocation_tracking_test_lock();
    let _guard = AllocationTrackingGuard::set(true).expect("enable allocation tracking");
    let root = RepoRoot::resolve(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("repository root"),
    )
    .expect("resolve repository root");
    let manifest = BenchmarkManifest::read(&root).expect("read benchmark manifest");

    for row_id in [
        "m9-detection-batch-phases",
        "m9-m2d-batch-phases",
        "m11-dem-batch-phases",
    ] {
        let row = manifest
            .rows
            .iter()
            .find(|row| row.id == row_id)
            .expect("A5 phase row");
        let measurements = if row_id.starts_with("m11-") {
            m11::run_dem_sampling_compare_row(&root, "release", row)
        } else {
            m9::run_detection_compare_row(&root, "release", row)
        }
        .expect("run allocation-enabled A5 phase row")
        .expect("A5 phase row has a Stab runner");

        assert!(!measurements.is_empty(), "{row_id}");
        assert!(
            measurements
                .iter()
                .all(|measurement| measurement.allocation.is_some()),
            "{row_id} must use an independent tracked-memory operation"
        );
    }
}

#[test]
fn affected_detection_and_dem_cli_rows_retain_one_process_ratio() {
    let root = RepoRoot::resolve(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("repository root"),
    )
    .expect("resolve repository root");
    let manifest = BenchmarkManifest::read(&root).expect("read benchmark manifest");
    for (row_id, stab_name) in [
        ("m9-detect-text-cli", "stab_detect_1024_dets"),
        ("m9-detect-bitpacked-cli", "stab_detect_1024_b8"),
        (
            "m9-detect-primary-matrix-contract",
            "stab_detect_primary_repetition_d3_r3_b8",
        ),
        ("m9-m2d-text-cli", "stab_m2d_dets"),
        ("m9-m2d-bitpacked-contract", "stab_m2d_b8"),
        (
            "m9-m2d-primary-matrix-contract",
            "stab_m2d_primary_repetition_d3_r3_b8",
        ),
        ("m11-sample-dem-cli", "stab_sample_dem_cli_1024_zero_one"),
        (
            "m11-sample-dem-sparse-contract",
            "stab_sample_dem_sparse_b8",
        ),
        ("m11-sample-dem-dense-contract", "stab_sample_dem_dense_b8"),
        (
            "m11-sample-dem-repeated-contract",
            "stab_sample_dem_repeated_b8",
        ),
        (
            "m11-sample-dem-high-detector-contract",
            "stab_sample_dem_high_detector_b8",
        ),
    ] {
        let dispatched_name = m9::process_cli_measurement_name(row_id)
            .or_else(|| m11::process_cli_measurement_name(row_id));
        assert_eq!(
            dispatched_name,
            Some(stab_name),
            "{row_id} must use the process-equivalent dispatcher"
        );
        let row = manifest
            .rows
            .iter()
            .find(|row| row.id == row_id)
            .expect("affected CLI manifest row");
        let measurement = |name: &str| Measurement {
            name: name.to_string(),
            seconds: 1.0,
            variance_seconds: Some(0.0),
            allocation: None,
            resident_bytes: None,
            resident_delta_bytes: None,
            observations: Vec::new(),
            iterations: Some(1),
        };
        let result = build_compare_row_result(CompareRowBuild {
            row,
            status: "measured",
            baseline_summary: "one Stim process measurement",
            stab_summary: "one Stab process measurement",
            note: compare_note(row_id).map(str::to_owned),
            stim_measurements: vec![measurement(row_id)],
            stab_measurements: vec![measurement(stab_name)],
            baseline_status: BaselineCompareStatus::Comparable,
        });

        assert_eq!(
            result.relative_ratio,
            Some(1.0),
            "{row_id} must retain a comparable single-process ratio"
        );
        assert_eq!(result.stim_median_seconds, Some(1.0), "{row_id}");
        assert_eq!(result.stab_median_seconds, Some(1.0), "{row_id}");
    }
}

#[test]
fn affected_detection_and_dem_cli_rows_validate_pinned_stim_output() {
    let root = RepoRoot::resolve(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("repository root"),
    )
    .expect("resolve repository root");
    let manifest = BenchmarkManifest::read(&root).expect("read benchmark manifest");
    for row_id in [
        "m9-detect-text-cli",
        "m9-detect-bitpacked-cli",
        "m9-detect-primary-matrix-contract",
        "m9-m2d-text-cli",
        "m9-m2d-bitpacked-contract",
        "m9-m2d-primary-matrix-contract",
        "m11-sample-dem-cli",
        "m11-sample-dem-sparse-contract",
        "m11-sample-dem-dense-contract",
        "m11-sample-dem-repeated-contract",
        "m11-sample-dem-high-detector-contract",
    ] {
        let row = manifest
            .rows
            .iter()
            .find(|row| row.id == row_id)
            .expect("affected CLI manifest row");
        let expected =
            cli_process::stim_cli_expected_witness(row_id).expect("pinned Stim output witness");
        cli_process::ensure_stim_cli_witness(row, expected).expect("matching witness");

        let changed = OutputWitness::new(expected.bytes.saturating_add(1), expected.digest);
        let error =
            cli_process::ensure_stim_cli_witness(row, changed).expect_err("changed witness");
        assert!(
            error.to_string().contains("pinned Stim CLI output changed"),
            "{row_id}: {error}"
        );
    }
}
