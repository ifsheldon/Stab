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
            targets: ArenaSpan::EMPTY,
            fusion: NoiselessFusion::Starts,
        }),
        ProgramEntry::EndRepeat,
    ];

    validate_program(&entries, &ProgramArena::default())
        .expect("validate compact repeat tape structurally")
        .expect("measure window needs");
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

/// Admission charges exactly what materialization retains: one fixed-size entry per retained
/// instruction or repeat marker plus every arena element, including the slots that `MPP`
/// normalization and classically controlled no-op pairs leave unused.
#[test]
fn admission_charges_exactly_the_materialized_program() {
    let circuit = Circuit::from_stim_str(
        "R 0 1 2\nH 0\nCX 0 1 sweep[0] 2\nMPAD 0 1\nDEPOLARIZE2(0.1) 0 1\n\
         HERALDED_PAULI_CHANNEL_1(0.1, 0.1, 0.1, 0.1) 2\nE(0.1) X0 Z1\n\
         REPEAT 3 {\n  MPP X0*X0*Z1 !Y2\n  MXX !0 1\n  CX rec[-1] 2\n}\n\
         M !0 1\nOBSERVABLE_INCLUDE(0) X2 rec[-1]\n",
    )
    .expect("parse admission fixture");
    for options in [ProgramOptions::SAMPLING, ProgramOptions::DETECTION] {
        let admission =
            CircuitProgram::admit(&circuit, options, 0, u64::MAX).expect("admit fixture");
        let program = CircuitProgram::materialize(&circuit, &admission).expect("materialize");
        assert_eq!(program.retained_bytes(), admission.retained_bytes());
        let refused = CircuitProgram::admit(&circuit, options, 1, admission.retained_bytes());
        assert_matches!(refused, Err(DetectionError::ResourceLimit(_)));
    }
}

/// A one-target instruction costs one 32-byte entry and four bytes of target storage, with no
/// allocation of its own.
#[test]
fn short_instructions_share_one_target_arena() {
    let circuit = Circuit::from_stim_str(&"H 0\nS 1\n".repeat(1000)).expect("parse fixture");
    let program =
        CircuitProgram::compile(&circuit, ProgramOptions::SAMPLING).expect("compile fixture");
    assert_eq!(program.entry_count(), 2000);
    assert_eq!(program.retained_bytes(), 2000 * (32 + 4));
}
