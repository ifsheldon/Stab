use crate::{Gate, MeasureRecordOffset, QubitId, Target};

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
    if gate.canonical_name() == "CZ"
        && let (Some(first), Some(second)) = (first_control, second_control)
    {
        return ControlledPauliTargetPair::ClassicalNoop { first, second };
    }

    let classical = match gate.canonical_name() {
        "CX" | "CY" if second.qubit_id().is_some() => first_control.zip(second.qubit_id()),
        "CZ" if second.qubit_id().is_some() => first_control.zip(second.qubit_id()),
        "CZ" if first.qubit_id().is_some() => second_control.zip(first.qubit_id()),
        "XCZ" | "YCZ" if first.qubit_id().is_some() => second_control.zip(first.qubit_id()),
        _ => None,
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

        assert!(matches!(
            classify_controlled_pauli_target_pair(cx, &[rec(-1), q(0)]),
            ControlledPauliTargetPair::Classical {
                control: ClassicalControl::Record(_),
                target,
            } if target.get() == 0
        ));
        assert!(matches!(
            classify_controlled_pauli_target_pair(xcz, &[q(0), Target::sweep_bit(3)]),
            ControlledPauliTargetPair::Classical {
                control: ClassicalControl::Sweep(3),
                target,
            } if target.get() == 0
        ));
        assert!(matches!(
            classify_controlled_pauli_target_pair(cz, &[rec(-1), Target::sweep_bit(2)]),
            ControlledPauliTargetPair::ClassicalNoop {
                first: ClassicalControl::Record(_),
                second: ClassicalControl::Sweep(2),
            }
        ));
        assert_eq!(
            classify_controlled_pauli_target_pair(cx, &[q(0), Target::sweep_bit(0)]),
            ControlledPauliTargetPair::Unsupported
        );
    }
}
