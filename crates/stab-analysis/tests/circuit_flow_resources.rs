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

    let error = circuit_flow_generators(&circuit("M 0\nREPEAT 1000001 {\n    TICK\n}\n"))
        .expect_err("first flow-generator expanded-work overflow");
    let resource = error
        .resource_limit_error()
        .expect("typed expanded-work error");
    assert_eq!(resource.operation(), ResourceOperation::FlowGeneration);
    assert_eq!(resource.resource(), ResourceKind::ExpandedOperations);
    assert_eq!(resource.actual(), 1_000_002);
    assert_eq!(resource.limit(), 1_000_000);

    let error = circuit_flow_generators(&circuit("M 1000000\n"))
        .expect_err("projected flow storage overflow");
    let resource = error
        .resource_limit_error()
        .expect("typed projected-payload error");
    assert_eq!(resource.operation(), ResourceOperation::FlowGeneration);
    assert_eq!(resource.resource(), ResourceKind::ProjectedPayloadBytes);
    assert!(resource.actual() > resource.limit());
    assert_eq!(resource.limit(), 512 * 1024 * 1024);

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

    let admitted = circuit_flow_generators(&circuit("QUBIT_COORDS(0) 4095\n"))
        .expect("exact ignored-only projected-storage maximum");
    assert_eq!(admitted.len(), 8192);

    let error = circuit_flow_generators(&circuit("QUBIT_COORDS(0) 4096\n"))
        .expect_err("first ignored-only projected-storage excess");
    let resource = error
        .resource_limit_error()
        .expect("typed ignored-only projected-storage error");
    assert_eq!(resource.operation(), ResourceOperation::FlowGeneration);
    assert_eq!(resource.resource(), ResourceKind::ProjectedPayloadBytes);
    assert_eq!(resource.actual(), 4097 * 4097);
    assert_eq!(resource.limit(), 4096 * 4096);
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
