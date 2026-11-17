#![allow(
    clippy::expect_used,
    reason = "fixed resource fixtures use direct assertions for compact diagnostics"
)]

use stab_algebra::Flow;
use stab_analysis::{ResourceKind, ResourceOperation, circuit_flow_generators};
use stab_model::{Circuit, RepeatBlock, RepeatCount};

#[test]
fn circuit_flow_generators_admit_owned_rows_and_report_typed_limits() {
    let flows = circuit_flow_generators(&circuit("REPEAT 4097 {\n    M 0\n}\n"))
        .expect("flow generator beyond the former arbitrary row cap");
    assert_eq!(flows.len(), 4098);
    let owned_records = flows
        .iter()
        .flat_map(Flow::measurements)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(owned_records.len(), 4097);
    assert!(owned_records.contains(&0));
    assert!(owned_records.contains(&4096));

    // Expanded solver work above the retired one-million cap completes.
    assert_eq!(
        circuit_flow_generators(&circuit("M 0\nREPEAT 1000001 {\n    TICK\n}\n"))
            .expect("uncapped flow-generator work"),
        circuit_flow_generators(&circuit("M 0\n")).expect("single measurement flows")
    );

    let error = circuit_flow_generators(&circuit("M 1000000\n"))
        .expect_err("projected flow storage overflow");
    let resource = error
        .resource_limit_error()
        .expect("typed projected-payload error");
    assert_eq!(resource.operation(), ResourceOperation::FlowGeneration);
    assert_eq!(resource.resource(), ResourceKind::ProjectedPayloadBytes);
    assert!(resource.actual() > resource.limit());
    assert_eq!(resource.limit(), 1 << 30);
    assert!(error.to_string().contains("memory budget"), "{error}");

    let mut nested = circuit("M 0\n");
    for _ in 0..257 {
        let mut outer = Circuit::new();
        outer.append_repeat_block(RepeatBlock::new(
            RepeatCount::try_new(1).expect("one repetition"),
            nested,
            None,
        ));
        nested = outer;
    }
    let error = circuit_flow_generators(&nested).expect_err("flow-generator nesting boundary");
    let resource = error
        .resource_limit_error()
        .expect("typed repeat-nesting error");
    assert_eq!(resource.operation(), ResourceOperation::FlowGeneration);
    assert_eq!(resource.resource(), ResourceKind::RepeatNesting);
    assert_eq!(resource.actual(), 257);
    assert_eq!(resource.limit(), 256);

    // Ignored-only and unitary flow rows are admitted in bytes like measurement-rich rows, so the
    // former 4096-qubit dense bound is gone and a projection above the budget is refused.
    let admitted = circuit_flow_generators(&circuit("QUBIT_COORDS(0) 4096\n"))
        .expect("ignored-only flows beyond the former dense bound");
    assert_eq!(admitted.len(), 8194);
    let unitary = circuit_flow_generators(&circuit("H 4096\n")).expect("wide unitary flows");
    assert_eq!(unitary.len(), 8194);

    for text in ["QUBIT_COORDS(0) 100000\n", "H 100000\n"] {
        let error = circuit_flow_generators(&circuit(text))
            .expect_err("measurement-free flow storage above the budget");
        let resource = error
            .resource_limit_error()
            .expect("typed measurement-free projected-storage error");
        assert_eq!(resource.operation(), ResourceOperation::FlowGeneration);
        assert_eq!(resource.resource(), ResourceKind::ProjectedPayloadBytes);
        let qubits = 100_001_u64;
        let row_bytes = std::mem::size_of::<Flow>() as u64 + 2 * qubits;
        assert_eq!(resource.actual(), 2 * qubits * row_bytes);
        assert_eq!(resource.limit(), 1 << 30);
    }
}

#[test]
fn flow_generation_skips_instructionless_repeats_without_expanding_their_counts() {
    let compact = circuit("M 0\nREPEAT 1000000000000 {\n    REPEAT 1000000000000 {\n    }\n}\n");
    let expected = circuit_flow_generators(&circuit("M 0\n")).expect("single measurement flows");
    assert_eq!(
        circuit_flow_generators(&compact).expect("compact empty-repeat flows"),
        expected
    );
}

#[test]
fn rejected_mpp_storage_is_admitted_before_product_allocation() {
    fn rejected_mpp(group_count: usize) -> Circuit {
        let mut text = String::from("MPP X1000000");
        for index in 1..group_count {
            use std::fmt::Write as _;
            write!(text, " X{}", index % 16).expect("write MPP target");
        }
        text.push('\n');
        circuit(&text)
    }

    fn rejected_allocations(circuit: &Circuit) -> allocation_counter::AllocationInfo {
        allocation_counter::measure(|| {
            let error = circuit_flow_generators(circuit).expect_err("projected MPP storage");
            let resource = error
                .resource_limit_error()
                .expect("typed projected MPP storage error");
            assert_eq!(resource.operation(), ResourceOperation::FlowGeneration);
            assert_eq!(resource.resource(), ResourceKind::ProjectedPayloadBytes);
        })
    }

    let one = rejected_allocations(&rejected_mpp(1));
    let many = rejected_allocations(&rejected_mpp(128));
    assert!(
        many.count_total <= one.count_total + 8,
        "rejected MPP allocation count scaled with product count: one={one:?}, many={many:?}"
    );
    assert!(
        many.bytes_total <= one.bytes_total + 64 * 1024,
        "rejected MPP bytes scaled with product width: one={one:?}, many={many:?}"
    );
}

fn circuit(text: &str) -> Circuit {
    Circuit::from_stim_str(text).expect("parse circuit")
}

/// Nests `depth` repeat blocks of `count`, each body being the inner block followed by `suffix`.
fn nested_repeats(depth: usize, count: u64, innermost: &str, suffix: &str) -> Circuit {
    let suffix = circuit(suffix);
    let mut body = circuit(innermost);
    for _ in 0..depth {
        let mut outer = Circuit::new();
        outer.append_repeat_block(RepeatBlock::new(
            RepeatCount::try_new(count).expect("repeat count"),
            body,
            None,
        ));
        for item in suffix.items() {
            let stab_model::CircuitItem::Instruction(instruction) = item else {
                continue;
            };
            outer.append_instruction(instruction.clone());
        }
        body = outer;
    }
    body
}

/// The sparse checker folds nested unitary repeats one body undo per level and without native
/// recursion, so the full 256-level envelope of `REPEAT 2` blocks is checked on a small stack;
/// unrolling it would take 2^256 body undos.
#[test]
fn sparse_flow_check_folds_the_full_nesting_envelope_of_unitary_repeats() {
    std::thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(|| {
            // Each level applies H S S H = X, which keeps Z and X up to sign.
            let nested = nested_repeats(
                stab_model::RepeatNestingLimit::HARD_MAX,
                2,
                "H 0\nH 0\nCX 0 1\nCX 0 1\n",
                "H 0\nS 0\nS 0\nH 0\n",
            );
            let flows = ["Z0 -> Z0", "X0 -> X0", "X0 -> Z0", "Z1 -> Z1"]
                .map(|flow| flow.parse::<Flow>().expect("flow"));
            assert_eq!(
                stab_analysis::check_if_circuit_has_unsigned_stabilizer_flows_with_options(
                    &nested,
                    &flows,
                    &stab_analysis::UnsignedFlowCheckOptions::default(),
                ),
                Ok(vec![true, true, false, true])
            );
        })
        .expect("spawn")
        .join()
        .expect("deep unitary flow check");
}
