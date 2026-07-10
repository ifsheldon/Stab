use crate::{CircuitInstruction, GateCategory, PauliBasis};

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

    match instruction.gate().canonical_name() {
        "M" => Transition::Measurement(PauliBasis::Z),
        "MX" => Transition::Measurement(PauliBasis::X),
        "MY" => Transition::Measurement(PauliBasis::Y),
        "R" => Transition::Reset(PauliBasis::Z),
        "RX" => Transition::Reset(PauliBasis::X),
        "RY" => Transition::Reset(PauliBasis::Y),
        "MR" => Transition::MeasureReset(PauliBasis::Z),
        "MRX" => Transition::MeasureReset(PauliBasis::X),
        "MRY" => Transition::MeasureReset(PauliBasis::Y),
        "MXX" => Transition::PairMeasurement(PauliBasis::X),
        "MYY" => Transition::PairMeasurement(PauliBasis::Y),
        "MZZ" => Transition::PairMeasurement(PauliBasis::Z),
        "MPP" => Transition::PauliProductMeasurement,
        "MPAD" => Transition::MeasurementPad,
        "HERALDED_ERASE" | "HERALDED_PAULI_CHANNEL_1" => Transition::HeraldedMeasurement,
        "SPP" | "SPP_DAG" => Transition::PauliProductUnitary,
        "DETECTOR" => Transition::Detector,
        "OBSERVABLE_INCLUDE" => Transition::Observable,
        "CX" | "XCZ" if sweep_controlled_pauli_is_sign_only_noop(instruction) => {
            Transition::SweepControlledPauliNoop
        }
        "CY" | "YCZ" if sweep_controlled_pauli_is_sign_only_noop(instruction) => {
            Transition::SweepControlledPauliNoop
        }
        "CZ" if sweep_controlled_pauli_is_sign_only_noop(instruction) => {
            Transition::SweepControlledPauliNoop
        }
        "CX" | "XCZ" => Transition::ControlledPauli(PauliBasis::X),
        "CY" | "YCZ" => Transition::ControlledPauli(PauliBasis::Y),
        "CZ" => Transition::ControlledPauli(PauliBasis::Z),
        name if crate::circuit_tableau::gate_has_tableau(name) => Transition::Tableau,
        _ if matches!(
            instruction.gate().category(),
            GateCategory::Annotation | GateCategory::Noise
        ) =>
        {
            Transition::Ignored
        }
        _ => Transition::Unsupported,
    }
}

pub(crate) fn sweep_controlled_pauli_is_sign_only_noop(instruction: &CircuitInstruction) -> bool {
    if !matches!(
        instruction.gate().canonical_name(),
        "CX" | "CY" | "CZ" | "XCZ" | "YCZ"
    ) {
        return false;
    }
    let groups = instruction.target_groups();
    !groups.is_empty()
        && groups.iter().all(|group| {
            let [left, right] = *group else {
                return false;
            };
            let left_is_sweep = left.is_sweep_bit_target();
            let right_is_sweep = right.is_sweep_bit_target();
            (left_is_sweep ^ right_is_sweep)
                && left.qubit_id().is_some() != right.qubit_id().is_some()
        })
}
