use crate::{Gate, GateKind, MeasureRecordOffset, QubitId, Target};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClassicalControl {
    Record(MeasureRecordOffset),
    Sweep(u32),
}

impl ClassicalControl {
    fn from_target(target: &Target) -> Option<Self> {
        target
            .measurement_record_offset()
            .map(Self::Record)
            .or_else(|| target.sweep_bit_id().map(Self::Sweep))
    }

    pub const fn is_record(self) -> bool {
        matches!(self, Self::Record(_))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlledPauliTargetPair {
    Quantum {
        first: QubitId,
        second: QubitId,
    },
    Classical {
        control: ClassicalControl,
        target: QubitId,
    },
    ClassicalNoop {
        first: ClassicalControl,
        second: ClassicalControl,
    },
    Unsupported,
}

pub fn classify_controlled_pauli_target_pair(
    gate: Gate,
    targets: &[Target],
) -> ControlledPauliTargetPair {
    let [first, second] = targets else {
        return ControlledPauliTargetPair::Unsupported;
    };
    if let (Some(first), Some(second)) = (first.qubit_id(), second.qubit_id()) {
        return ControlledPauliTargetPair::Quantum { first, second };
    }

    let first_control = ClassicalControl::from_target(first);
    let second_control = ClassicalControl::from_target(second);
    if gate.kind() == GateKind::CZ
        && let (Some(first), Some(second)) = (first_control, second_control)
    {
        return ControlledPauliTargetPair::ClassicalNoop { first, second };
    }

    let classical = match gate.kind() {
        GateKind::CX | GateKind::CY => first_control.zip(second.qubit_id()),
        GateKind::CZ => first_control
            .zip(second.qubit_id())
            .or_else(|| second_control.zip(first.qubit_id())),
        GateKind::XCZ | GateKind::YCZ => second_control.zip(first.qubit_id()),
        GateKind::DETECTOR
        | GateKind::OBSERVABLE_INCLUDE
        | GateKind::TICK
        | GateKind::QUBIT_COORDS
        | GateKind::SHIFT_COORDS
        | GateKind::REPEAT
        | GateKind::MPAD
        | GateKind::MX
        | GateKind::MY
        | GateKind::M
        | GateKind::MRX
        | GateKind::MRY
        | GateKind::MR
        | GateKind::RX
        | GateKind::RY
        | GateKind::R
        | GateKind::XCX
        | GateKind::XCY
        | GateKind::YCX
        | GateKind::YCY
        | GateKind::H
        | GateKind::H_XY
        | GateKind::H_YZ
        | GateKind::H_NXY
        | GateKind::H_NXZ
        | GateKind::H_NYZ
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
        | GateKind::ELSE_CORRELATED_ERROR
        | GateKind::HERALDED_ERASE
        | GateKind::HERALDED_PAULI_CHANNEL_1
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
        | GateKind::MPP
        | GateKind::SPP
        | GateKind::SPP_DAG
        | GateKind::SWAP
        | GateKind::ISWAP
        | GateKind::CXSWAP
        | GateKind::SWAPCX
        | GateKind::CZSWAP
        | GateKind::ISWAP_DAG
        | GateKind::MXX
        | GateKind::MYY
        | GateKind::MZZ => None,
    };
    classical.map_or(
        ControlledPauliTargetPair::Unsupported,
        |(control, target)| ControlledPauliTargetPair::Classical { control, target },
    )
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        reason = "fixed gate and target fixtures must be valid before classification"
    )]

    use std::assert_matches;

    use super::*;

    fn q(id: u32) -> Target {
        Target::qubit(QubitId::new(id).expect("small qubit"), false)
    }

    fn rec(offset: i32) -> Target {
        Target::measurement_record(MeasureRecordOffset::try_new(offset).expect("negative offset"))
    }

    #[test]
    fn classifier_owns_control_orientation_and_symmetric_noops() {
        let cx = Gate::from_name("CX").expect("CX");
        let cz = Gate::from_name("CZ").expect("CZ");
        let xcz = Gate::from_name("XCZ").expect("XCZ");

        assert_matches!(
            classify_controlled_pauli_target_pair(cx, &[rec(-1), q(0)]),
            ControlledPauliTargetPair::Classical {
                control: ClassicalControl::Record(_),
                target,
            } if target.get() == 0
        );
        assert_matches!(
            classify_controlled_pauli_target_pair(xcz, &[q(0), Target::sweep_bit(3)]),
            ControlledPauliTargetPair::Classical {
                control: ClassicalControl::Sweep(3),
                target,
            } if target.get() == 0
        );
        assert_matches!(
            classify_controlled_pauli_target_pair(cz, &[rec(-1), Target::sweep_bit(2)]),
            ControlledPauliTargetPair::ClassicalNoop {
                first: ClassicalControl::Record(_),
                second: ClassicalControl::Sweep(2),
            }
        );
        assert_eq!(
            classify_controlled_pauli_target_pair(cx, &[q(0), Target::sweep_bit(0)]),
            ControlledPauliTargetPair::Unsupported
        );
    }
}
