use stab_algebra::PauliBasis;
use stab_model::advanced::{ClassicalControl, ControlledPauliTargetPair};
use stab_model::{CircuitInstruction, GateKind, Pauli};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReverseFlowTransition {
    Measurement(PauliBasis),
    Reset(PauliBasis),
    MeasureReset(PauliBasis),
    PairMeasurement(PauliBasis),
    PauliProductMeasurement,
    MeasurementPad,
    HeraldedMeasurement,
    PauliProductUnitary,
    ControlledPauli(PauliBasis),
    SweepControlledPauliNoop,
    Detector,
    Observable,
    Tableau,
    Ignored,
    Unsupported,
}

impl ReverseFlowTransition {
    pub(crate) fn is_measurement_rich(self) -> bool {
        matches!(
            self,
            Self::Measurement(_)
                | Self::Reset(_)
                | Self::MeasureReset(_)
                | Self::PairMeasurement(_)
                | Self::PauliProductMeasurement
                | Self::MeasurementPad
                | Self::HeraldedMeasurement
                | Self::SweepControlledPauliNoop
        )
    }
}

pub(crate) fn reverse_flow_transition(instruction: &CircuitInstruction) -> ReverseFlowTransition {
    use ReverseFlowTransition as Transition;

    match instruction.gate().kind() {
        GateKind::M => Transition::Measurement(PauliBasis::Z),
        GateKind::MX => Transition::Measurement(PauliBasis::X),
        GateKind::MY => Transition::Measurement(PauliBasis::Y),
        GateKind::R => Transition::Reset(PauliBasis::Z),
        GateKind::RX => Transition::Reset(PauliBasis::X),
        GateKind::RY => Transition::Reset(PauliBasis::Y),
        GateKind::MR => Transition::MeasureReset(PauliBasis::Z),
        GateKind::MRX => Transition::MeasureReset(PauliBasis::X),
        GateKind::MRY => Transition::MeasureReset(PauliBasis::Y),
        GateKind::MXX => Transition::PairMeasurement(PauliBasis::X),
        GateKind::MYY => Transition::PairMeasurement(PauliBasis::Y),
        GateKind::MZZ => Transition::PairMeasurement(PauliBasis::Z),
        GateKind::MPP => Transition::PauliProductMeasurement,
        GateKind::MPAD => Transition::MeasurementPad,
        GateKind::HERALDED_ERASE | GateKind::HERALDED_PAULI_CHANNEL_1 => {
            Transition::HeraldedMeasurement
        }
        GateKind::SPP | GateKind::SPP_DAG => Transition::PauliProductUnitary,
        GateKind::DETECTOR => Transition::Detector,
        GateKind::OBSERVABLE_INCLUDE => Transition::Observable,
        GateKind::CX | GateKind::XCZ | GateKind::CY | GateKind::YCZ | GateKind::CZ => {
            if sweep_controlled_pauli_is_sign_only_noop(instruction) {
                Transition::SweepControlledPauliNoop
            } else {
                match instruction.gate().controlled_pauli() {
                    Some(Pauli::X) => Transition::ControlledPauli(PauliBasis::X),
                    Some(Pauli::Y) => Transition::ControlledPauli(PauliBasis::Y),
                    Some(Pauli::Z) => Transition::ControlledPauli(PauliBasis::Z),
                    None => Transition::Unsupported,
                }
            }
        }
        GateKind::XCX
        | GateKind::XCY
        | GateKind::YCX
        | GateKind::YCY
        | GateKind::H
        | GateKind::H_XY
        | GateKind::H_YZ
        | GateKind::H_NXY
        | GateKind::H_NXZ
        | GateKind::H_NYZ
        | GateKind::I
        | GateKind::X
        | GateKind::Y
        | GateKind::Z
        | GateKind::C_XYZ
        | GateKind::C_ZYX
        | GateKind::C_NXYZ
        | GateKind::C_XNYZ
        | GateKind::C_XYNZ
        | GateKind::C_NZYX
        | GateKind::C_ZNYX
        | GateKind::C_ZYNX
        | GateKind::SQRT_X
        | GateKind::SQRT_X_DAG
        | GateKind::SQRT_Y
        | GateKind::SQRT_Y_DAG
        | GateKind::S
        | GateKind::S_DAG
        | GateKind::II
        | GateKind::SQRT_XX
        | GateKind::SQRT_XX_DAG
        | GateKind::SQRT_YY
        | GateKind::SQRT_YY_DAG
        | GateKind::SQRT_ZZ
        | GateKind::SQRT_ZZ_DAG
        | GateKind::SWAP
        | GateKind::ISWAP
        | GateKind::CXSWAP
        | GateKind::SWAPCX
        | GateKind::CZSWAP
        | GateKind::ISWAP_DAG => Transition::Tableau,
        GateKind::TICK
        | GateKind::QUBIT_COORDS
        | GateKind::SHIFT_COORDS
        | GateKind::DEPOLARIZE1
        | GateKind::DEPOLARIZE2
        | GateKind::X_ERROR
        | GateKind::Y_ERROR
        | GateKind::Z_ERROR
        | GateKind::I_ERROR
        | GateKind::II_ERROR
        | GateKind::PAULI_CHANNEL_1
        | GateKind::PAULI_CHANNEL_2
        | GateKind::E
        | GateKind::ELSE_CORRELATED_ERROR => Transition::Ignored,
        GateKind::REPEAT => Transition::Unsupported,
    }
}

/// Reports whether every target pair of a controlled-Pauli instruction is a sweep bit in the
/// gate's classical-control slot paired with a qubit, which only flips a sign.
///
/// A sweep bit is a sign-only no-op only in the classical-control slot (vendor
/// sparse_rev_frame_tracker.cc undo_ZC{X,Y,Z}_single): CX/CY take the control first, XCZ/YCZ
/// take the Pauli side first, and CZ accepts a classical bit in either slot. The model's
/// controlled-Pauli classifier owns that orientation, so a sweep bit in the Pauli slot falls
/// through to the tracker's not-a-qubit rejection instead of classifying as a no-op.
pub(crate) fn sweep_controlled_pauli_is_sign_only_noop(instruction: &CircuitInstruction) -> bool {
    if instruction.gate().controlled_pauli().is_none() {
        return false;
    }
    let groups = instruction.target_groups();
    !groups.is_empty()
        && groups.iter().all(|group| {
            matches!(
                ControlledPauliTargetPair::classify(instruction.gate(), group),
                ControlledPauliTargetPair::Classical {
                    control: ClassicalControl::Sweep(_),
                    ..
                }
            )
        })
}
