#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "program tests use direct structural fixture assertions"
)]

use std::assert_matches;

use super::*;

#[test]
fn structural_validation_does_not_execute_repeat_iterations() {
    let entries = [
        ProgramEntry::Repeat {
            count: u64::MAX,
            body_end: 2,
        },
        ProgramEntry::Instruction(ProgramInstruction::Reset {
            basis: PauliBasis::Z,
            targets: vec![0],
            fusion: NoiselessFusion::Starts,
        }),
        ProgramEntry::EndRepeat,
    ];

    validate_program(&entries).expect("validate compact repeat tape structurally");
}

#[test]
fn spp_lowering_stops_at_the_first_compiled_byte_rejection() {
    let circuit = Circuit::from_stim_str("SPP X0 X1*Z1\n").expect("parse SPP fixture");
    let error = CircuitProgram::admit(&circuit, ProgramOptions::DETECTION, 0, 0)
        .expect_err("the first lowered instruction exceeds a zero-byte budget");
    assert_matches!(
        error,
        DetectionError::ResourceLimit(ref resource)
            if resource.kind() == crate::detection::DetectionResourceKind::CompiledBytes
    );
}

/// Every unitary gate reaches the inverse-tableau reference simulator through its exact pinned
/// `H`/`S`/`CX` decomposition, which costs a few row operations, instead of the generic
/// local-tableau rewrite.
#[test]
fn every_unitary_gate_resolves_a_decomposed_reference_action() {
    use stab_model::{Gate, GateTargetGroupKind};
    for gate in Gate::all().filter(|gate| {
        gate.is_unitary() && gate.target_group_kind() != GateTargetGroupKind::PauliProducts
    }) {
        let targets = match gate.target_group_kind() {
            GateTargetGroupKind::Pairs => "0 1",
            _ => "0",
        };
        let circuit = Circuit::from_stim_str(&format!("{} {targets}\n", gate.canonical_name()))
            .expect("parse gate fixture");
        let program =
            CircuitProgram::compile(&circuit, ProgramOptions::SAMPLING).expect("compile gate");
        let Some(ProgramEntry::Instruction(ProgramInstruction::Clifford { reference, .. })) =
            program.entries().first()
        else {
            panic!(
                "{} did not lower to a Clifford entry",
                gate.canonical_name()
            );
        };
        let clifford = program
            .reference_clifford(*reference)
            .expect("resolved reference action");
        assert!(
            !matches!(clifford, ReferenceClifford::Tableau { steps: None, .. }),
            "{} has no decomposed reference action",
            gate.canonical_name()
        );
    }
}
