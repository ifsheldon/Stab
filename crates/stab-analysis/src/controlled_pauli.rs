//! The classical-control slot of the controlled-Pauli gates that accept classical controls.

use stab_model::{Gate, GateKind, Pauli};

/// Where a controlled-Pauli gate accepts a classical control (vendor
/// `sparse_rev_frame_tracker.cc`, `undo_ZC{X,Y,Z}_single`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ControlSlot {
    /// `CX` and `CY` carry the classical-capable control first.
    First,
    /// `XCZ` and `YCZ` carry the Pauli side first and the control second.
    Second,
    /// `CZ` accepts a classical bit in either slot.
    Either,
}

impl ControlSlot {
    /// Reports whether a classical control first or second in a pair is in this slot.
    pub(crate) fn accepts(self, control_is_first: bool) -> bool {
        match self {
            Self::First => control_is_first,
            Self::Second => !control_is_first,
            Self::Either => true,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ControlledPauliForm {
    /// The Pauli applied to the target when the control is set.
    pub(crate) pauli: Pauli,
    pub(crate) slot: ControlSlot,
}

/// Returns the applied Pauli and control slot of `CX`, `CY`, `CZ`, `XCZ`, or `YCZ`.
pub(crate) fn controlled_pauli_form(gate: Gate) -> Option<ControlledPauliForm> {
    let slot = match gate.kind() {
        GateKind::CX | GateKind::CY => ControlSlot::First,
        GateKind::CZ => ControlSlot::Either,
        GateKind::XCZ | GateKind::YCZ => ControlSlot::Second,
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
        | GateKind::MZZ => return None,
    };
    Some(ControlledPauliForm {
        pauli: gate.controlled_pauli()?,
        slot,
    })
}
