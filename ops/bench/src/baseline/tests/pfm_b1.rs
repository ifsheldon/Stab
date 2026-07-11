use super::measurement_work;

#[test]
fn transform_benchmark_work_contracts_are_exact() {
    for (name, operations) in [
        (
            "stab_circuit_time_reversed_for_flows_generated_surface_d3_r2",
            66.0,
        ),
        (
            "stab_circuit_time_reversed_for_flows_generated_surface_d5_r2",
            130.0,
        ),
        (
            "stab_circuit_time_reversed_for_flows_generated_surface_d7_r2",
            226.0,
        ),
    ] {
        assert_eq!(
            measurement_work("pfm-b1-time-reverse-generated-surface", name),
            Some((operations, "source-instructions/s"))
        );
    }

    assert_eq!(
        measurement_work(
            "pfm-b1-time-reverse-mpad-matrix",
            "stab_circuit_time_reversed_for_flows_mpad_matrix"
        ),
        Some((7.0, "flows/s"))
    );
    for size in [1, 8, 64] {
        assert_eq!(
            measurement_work(
                "pfm-b1-time-reverse-mpad-matrix",
                &format!("stab_circuit_time_reversed_for_flows_mpad_scale_{size}")
            ),
            Some((size as f64, "flows/s"))
        );
    }

    for name in [
        "stab_circuit_time_reversed_for_flows_unitary_repeat_count_1",
        "stab_circuit_time_reversed_for_flows_unitary_repeat_count_1024",
        "stab_circuit_time_reversed_for_flows_unitary_repeat_count_1b",
        "stab_circuit_time_reversed_for_flows_unitary_repeat_wide_body_1b",
    ] {
        assert_eq!(
            measurement_work("pfm-b1-time-reverse-large-unitary-repeat", name),
            Some((1.0, "transforms/s"))
        );
    }

    for name in [
        "stab_circuit_time_reversed_for_flows_sparse_qubit_0",
        "stab_circuit_time_reversed_for_flows_sparse_qubit_1000000",
    ] {
        assert_eq!(
            measurement_work("pfm-b1-time-reverse-sparse-high-qubit", name),
            Some((1.0, "transforms/s"))
        );
    }
}

#[cfg(feature = "count-allocations")]
mod allocations {
    use std::hint::black_box;
    use std::str::FromStr;

    use stab_core::{Circuit, Flow};

    fn reverse(circuit: &Circuit, flows: &[Flow]) {
        let reversed = circuit
            .time_reversed_for_flows(flows)
            .expect("PFM-B1 allocation fixture must reverse");
        black_box(reversed);
    }

    fn circuit(text: &str) -> Circuit {
        Circuit::from_stim_str(text).expect("PFM-B1 allocation circuit")
    }

    fn flow(text: &str) -> Flow {
        Flow::from_str(text).expect("PFM-B1 allocation flow")
    }

    fn mpad_case(size: usize) -> (Circuit, Vec<Flow>) {
        let mut text = String::from("MPAD");
        for _ in 0..size {
            text.push_str(" 1");
        }
        text.push('\n');
        let flows = (0..size)
            .map(|index| flow(&format!("1 -> rec[{index}]")))
            .collect();
        (circuit(&text), flows)
    }

    fn unitary_repeat_case(qubits: usize, repeat_count: u64) -> (Circuit, Flow) {
        let mut text = format!("REPEAT {repeat_count} {{\n");
        for qubit in 0..qubits {
            for gate in ["X", "H", "Z", "Y", "Y", "Z", "H", "X"] {
                text.push_str(&format!("{gate} {qubit}\n"));
            }
        }
        text.push_str("}\n");
        let pauli = "X".repeat(qubits);
        (circuit(&text), flow(&format!("{pauli} -> {pauli}")))
    }

    fn incremental_slope_is_bounded(
        sizes: [u64; 3],
        values: [u64; 3],
        max_ratio_numerator: u64,
        max_ratio_denominator: u64,
    ) -> bool {
        let [small_size, medium_size, large_size] = sizes;
        let [small, medium, large] = values;
        let Some(first_size_delta) = medium_size.checked_sub(small_size) else {
            return false;
        };
        let Some(second_size_delta) = large_size.checked_sub(medium_size) else {
            return false;
        };
        let Some(first_value_delta) = medium.checked_sub(small) else {
            return false;
        };
        let Some(second_value_delta) = large.checked_sub(medium) else {
            return false;
        };
        if first_size_delta == 0
            || second_size_delta == 0
            || max_ratio_denominator == 0
            || first_value_delta == 0
        {
            return false;
        }
        let left = u128::from(second_value_delta)
            .checked_mul(u128::from(first_size_delta))
            .and_then(|value| value.checked_mul(u128::from(max_ratio_denominator)));
        let right = u128::from(first_value_delta)
            .checked_mul(u128::from(second_size_delta))
            .and_then(|value| value.checked_mul(u128::from(max_ratio_numerator)));
        matches!((left, right), (Some(left), Some(right)) if left <= right)
    }

    #[test]
    fn incremental_slope_gate_rejects_quadratic_growth() {
        let sizes = [8, 64, 1024];
        let linear = sizes.map(|size| 30_000 + 18_000 * size);
        let linear_plus_dense_matrix = sizes.map(|size| {
            30_000_u64
                .saturating_add(18_000 * size)
                .saturating_add(8 * size.saturating_mul(size))
        });

        assert!(incremental_slope_is_bounded(sizes, linear, 7, 5));
        assert!(
            !incremental_slope_is_bounded(sizes, linear_plus_dense_matrix, 7, 5),
            "the allocation acceptance gate must reject a retained dense u64 matrix"
        );
        assert!(
            !incremental_slope_is_bounded(
                [0, u64::MAX / 2, u64::MAX],
                [0, u64::MAX / 2, u64::MAX],
                u64::MAX,
                u64::MAX,
            ),
            "overflowing acceptance arithmetic must fail closed"
        );
    }

    #[test]
    fn sparse_high_qubit_allocation_is_index_magnitude_independent() {
        let low = circuit("H 0\n");
        let high = circuit("H 1000000\n");
        let flows = [flow("Z1 -> Z1")];
        reverse(&low, &flows);
        reverse(&high, &flows);

        let low_allocations = allocation_counter::measure(|| reverse(&low, &flows));
        let high_allocations = allocation_counter::measure(|| reverse(&high, &flows));
        const ALLOWED_COUNT_DELTA: u64 = 8;
        const ALLOWED_BYTE_DELTA: u64 = 4_096;
        assert!(
            high_allocations.count_total
                <= low_allocations
                    .count_total
                    .saturating_add(ALLOWED_COUNT_DELTA),
            "maximum qubit id increased allocation count: low={low_allocations:?}, high={high_allocations:?}"
        );
        assert!(
            high_allocations.bytes_total
                <= low_allocations
                    .bytes_total
                    .saturating_add(ALLOWED_BYTE_DELTA),
            "maximum qubit id increased total allocated bytes: low={low_allocations:?}, high={high_allocations:?}"
        );
        assert!(
            high_allocations.bytes_max
                <= low_allocations.bytes_max.saturating_add(ALLOWED_BYTE_DELTA),
            "maximum qubit id increased peak live bytes: low={low_allocations:?}, high={high_allocations:?}"
        );
    }

    #[test]
    fn unitary_repeat_allocation_is_repeat_count_bounded() {
        let (one, flow_one) = unitary_repeat_case(1, 1);
        let (billion, flow_billion) = unitary_repeat_case(1, 1_000_000_000);
        let (medium, flow_medium) = unitary_repeat_case(4, 1_000_000_000);
        let (wide, flow_wide) = unitary_repeat_case(16, 1_000_000_000);
        reverse(&one, std::slice::from_ref(&flow_one));
        reverse(&billion, std::slice::from_ref(&flow_billion));
        reverse(&medium, std::slice::from_ref(&flow_medium));
        reverse(&wide, std::slice::from_ref(&flow_wide));

        let one_allocations = allocation_counter::measure(|| {
            reverse(&one, std::slice::from_ref(&flow_one));
        });
        let billion_allocations = allocation_counter::measure(|| {
            reverse(&billion, std::slice::from_ref(&flow_billion));
        });
        let medium_allocations = allocation_counter::measure(|| {
            reverse(&medium, std::slice::from_ref(&flow_medium));
        });
        let wide_allocations = allocation_counter::measure(|| {
            reverse(&wide, std::slice::from_ref(&flow_wide));
        });
        const ALLOWED_COUNT_DELTA: u64 = 384;
        const ALLOWED_TOTAL_BYTE_DELTA: u64 = 32 << 10;
        const ALLOWED_PEAK_BYTE_DELTA: u64 = 1_024;
        assert!(
            billion_allocations.count_total
                <= one_allocations
                    .count_total
                    .saturating_add(ALLOWED_COUNT_DELTA),
            "repeat count caused unbounded allocation calls: one={one_allocations:?}, billion={billion_allocations:?}"
        );
        assert!(
            billion_allocations.bytes_total
                <= one_allocations
                    .bytes_total
                    .saturating_add(ALLOWED_TOTAL_BYTE_DELTA),
            "repeat count caused unbounded total allocation: one={one_allocations:?}, billion={billion_allocations:?}"
        );
        assert!(
            billion_allocations.bytes_max
                <= one_allocations
                    .bytes_max
                    .saturating_add(ALLOWED_PEAK_BYTE_DELTA),
            "repeat count caused unbounded peak allocation: one={one_allocations:?}, billion={billion_allocations:?}"
        );
        let sizes = [1, 4, 16];
        for (label, values, ratio) in [
            (
                "allocation calls",
                [
                    billion_allocations.count_total,
                    medium_allocations.count_total,
                    wide_allocations.count_total,
                ],
                (2, 1),
            ),
            (
                "total allocated bytes",
                [
                    billion_allocations.bytes_total,
                    medium_allocations.bytes_total,
                    wide_allocations.bytes_total,
                ],
                (2, 1),
            ),
            (
                "peak live bytes",
                [
                    billion_allocations.bytes_max,
                    medium_allocations.bytes_max,
                    wide_allocations.bytes_max,
                ],
                (3, 1),
            ),
        ] {
            assert!(
                incremental_slope_is_bounded(sizes, values, ratio.0, ratio.1),
                "unitary repeat {label} grew faster than compact body/state width: small={billion_allocations:?}, medium={medium_allocations:?}, wide={wide_allocations:?}"
            );
        }
    }

    #[test]
    fn mpad_flow_allocation_scales_linearly() {
        let (small_circuit, small_flows) = mpad_case(8);
        let (medium_circuit, medium_flows) = mpad_case(64);
        let (large_circuit, large_flows) = mpad_case(1024);
        reverse(&small_circuit, &small_flows);
        reverse(&medium_circuit, &medium_flows);
        reverse(&large_circuit, &large_flows);

        let small = allocation_counter::measure(|| reverse(&small_circuit, &small_flows));
        let medium = allocation_counter::measure(|| reverse(&medium_circuit, &medium_flows));
        let large = allocation_counter::measure(|| reverse(&large_circuit, &large_flows));
        let sizes = [8, 64, 1024];
        for (label, values) in [
            (
                "allocation calls",
                [small.count_total, medium.count_total, large.count_total],
            ),
            (
                "total allocated bytes",
                [small.bytes_total, medium.bytes_total, large.bytes_total],
            ),
            (
                "peak live bytes",
                [small.bytes_max, medium.bytes_max, large.bytes_max],
            ),
        ] {
            assert!(
                incremental_slope_is_bounded(sizes, values, 7, 5),
                "MPAD {label} has a superlinear incremental slope: small={small:?}, medium={medium:?}, large={large:?}"
            );
        }
    }
}
