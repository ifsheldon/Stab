use crate::{CircuitError, CircuitResult, Probability, Target};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GateCategory {
    Annotation,
    ControlFlow,
    Collapsing,
    Controlled,
    HadamardLike,
    Noise,
    HeraldedNoise,
    Pauli,
    Period3,
    Period4,
    ParityPhasing,
    PauliProduct,
    Swap,
    PairMeasurement,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gate {
    info: &'static GateInfo,
}

impl Gate {
    pub fn from_name(name: &str) -> CircuitResult<Self> {
        let canonical =
            canonical_gate_name(name).ok_or_else(|| CircuitError::UnknownGate(name.to_string()))?;
        let info = GATES
            .iter()
            .find(|gate| gate.name == canonical)
            .ok_or_else(|| CircuitError::UnknownGate(name.to_string()))?;
        Ok(Self { info })
    }

    pub fn canonical_name(self) -> &'static str {
        self.info.name
    }

    pub fn category(self) -> GateCategory {
        self.info.category
    }

    pub fn best_candidate_inverse(self) -> CircuitResult<Self> {
        Self::from_name(self.info.inverse_name)
    }

    pub(crate) fn validate(self, args: &[f64], targets: &[Target]) -> CircuitResult<()> {
        self.info.arg_rule.validate(self.info.name, args)?;
        self.info.target_rule.validate(self.info.name, targets)
    }

    pub(crate) fn target_group_kind(self) -> TargetGroupKind {
        self.info.target_rule.target_group_kind()
    }

    pub(crate) fn can_fuse(self) -> bool {
        self.info.can_fuse
    }
}

#[derive(Debug, Eq, PartialEq)]
struct GateInfo {
    name: &'static str,
    inverse_name: &'static str,
    category: GateCategory,
    arg_rule: ArgRule,
    target_rule: TargetRule,
    can_fuse: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ArgRule {
    Exact(usize),
    Any,
    ZeroOrOneProbability,
    ProbabilityList(usize),
    UnsignedInteger,
}

impl ArgRule {
    fn validate(self, gate: &'static str, args: &[f64]) -> CircuitResult<()> {
        match self {
            Self::Exact(expected) if args.len() != expected => {
                Err(CircuitError::InvalidArgumentCount {
                    gate,
                    expected: match expected {
                        0 => "0",
                        1 => "1",
                        2 => "2",
                        _ => "fixed",
                    },
                    actual: args.len(),
                })
            }
            Self::Exact(_) => Ok(()),
            Self::Any => {
                for arg in args {
                    validate_finite_arg(gate, *arg)?;
                }
                Ok(())
            }
            Self::ZeroOrOneProbability => {
                if args.len() > 1 {
                    return Err(CircuitError::InvalidArgumentCount {
                        gate,
                        expected: "0 or 1",
                        actual: args.len(),
                    });
                }
                if let Some(arg) = args.first().copied() {
                    Probability::try_new(arg).map_err(|_| CircuitError::InvalidArgument {
                        gate,
                        argument: arg.to_string(),
                    })?;
                }
                Ok(())
            }
            Self::ProbabilityList(expected) => {
                if args.len() != expected {
                    return Err(CircuitError::InvalidArgumentCount {
                        gate,
                        expected: "probability list",
                        actual: args.len(),
                    });
                }
                for arg in args {
                    Probability::try_new(*arg).map_err(|_| CircuitError::InvalidArgument {
                        gate,
                        argument: arg.to_string(),
                    })?;
                }
                Ok(())
            }
            Self::UnsignedInteger => {
                if args.len() != 1 {
                    return Err(CircuitError::InvalidArgumentCount {
                        gate,
                        expected: "1",
                        actual: args.len(),
                    });
                }
                let Some(arg) = args.first().copied() else {
                    return Err(CircuitError::InvalidArgumentCount {
                        gate,
                        expected: "1",
                        actual: args.len(),
                    });
                };
                if !arg.is_finite() || arg < 0.0 || arg.fract() != 0.0 || arg > f64::from(u32::MAX)
                {
                    return Err(CircuitError::InvalidArgument {
                        gate,
                        argument: arg.to_string(),
                    });
                }
                Ok(())
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TargetRule {
    None,
    AnySingleQubit,
    MeasurementQubits,
    MeasurementPads,
    Pairs,
    RecOnly,
    RecOrPauli,
    QubitCoords,
    PauliProducts,
    PauliList,
}

impl TargetRule {
    fn validate(self, gate: &'static str, targets: &[Target]) -> CircuitResult<()> {
        match self {
            Self::None => {
                if targets.is_empty() {
                    Ok(())
                } else {
                    Err(CircuitError::InvalidTargetCount {
                        gate,
                        count: targets.len(),
                    })
                }
            }
            Self::AnySingleQubit => validate_targets(gate, targets, is_plain_qubit_target),
            Self::MeasurementQubits => validate_targets(gate, targets, Target::is_qubit_like),
            Self::MeasurementPads => validate_targets(gate, targets, is_measurement_pad_target),
            Self::Pairs => {
                if !targets.len().is_multiple_of(2) {
                    return Err(CircuitError::InvalidTargetCount {
                        gate,
                        count: targets.len(),
                    });
                }
                validate_targets(gate, targets, Target::is_classical_or_qubit)
            }
            Self::RecOnly => validate_targets(gate, targets, Target::is_measurement_record),
            Self::RecOrPauli => validate_targets(gate, targets, |target| {
                target.is_measurement_record_target() || target.is_pauli_target()
            }),
            Self::QubitCoords => validate_targets(gate, targets, Target::is_qubit_like),
            Self::PauliProducts => {
                validate_targets(gate, targets, Target::is_pauli_product_part)?;
                validate_combiners(gate, targets)
            }
            Self::PauliList => validate_targets(gate, targets, |target| {
                matches!(target, Target::Pauli { .. })
            }),
        }
    }

    fn target_group_kind(self) -> TargetGroupKind {
        match self {
            Self::None => TargetGroupKind::None,
            Self::AnySingleQubit
            | Self::MeasurementQubits
            | Self::MeasurementPads
            | Self::RecOnly
            | Self::RecOrPauli
            | Self::QubitCoords => TargetGroupKind::Singles,
            Self::Pairs => TargetGroupKind::Pairs,
            Self::PauliProducts => TargetGroupKind::PauliProducts,
            Self::PauliList => TargetGroupKind::AllTargets,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TargetGroupKind {
    None,
    Singles,
    Pairs,
    PauliProducts,
    AllTargets,
}

fn is_plain_qubit_target(target: &Target) -> bool {
    matches!(
        target,
        Target::Qubit {
            inverted: false,
            ..
        }
    )
}

fn is_measurement_pad_target(target: &Target) -> bool {
    matches!(target, Target::Qubit { id, inverted: false } if id.get() <= 1)
}

fn validate_targets(
    gate: &'static str,
    targets: &[Target],
    predicate: impl Fn(&Target) -> bool,
) -> CircuitResult<()> {
    for target in targets {
        if !predicate(target) {
            return Err(CircuitError::InvalidTarget {
                gate,
                target: target.to_string(),
            });
        }
    }
    Ok(())
}

fn validate_combiners(gate: &'static str, targets: &[Target]) -> CircuitResult<()> {
    let mut previous_was_combiner = true;
    for target in targets {
        if target.is_combiner() {
            if previous_was_combiner {
                return Err(CircuitError::InvalidTarget {
                    gate,
                    target: target.to_string(),
                });
            }
            previous_was_combiner = true;
        } else {
            previous_was_combiner = false;
        }
    }
    if previous_was_combiner && !targets.is_empty() {
        return Err(CircuitError::InvalidTarget {
            gate,
            target: "*".to_string(),
        });
    }
    Ok(())
}

fn validate_finite_arg(gate: &'static str, arg: f64) -> CircuitResult<()> {
    if arg.is_finite() {
        Ok(())
    } else {
        Err(CircuitError::InvalidArgument {
            gate,
            argument: arg.to_string(),
        })
    }
}

fn canonical_gate_name(name: &str) -> Option<&'static str> {
    let name = name.to_ascii_uppercase();
    for (alias, canonical) in GATE_ALIASES {
        if name == *alias {
            return Some(canonical);
        }
    }
    GATES
        .iter()
        .find(|gate| gate.name == name)
        .map(|gate| gate.name)
}

const GATE_ALIASES: &[(&str, &str)] = &[
    ("MZ", "M"),
    ("MRZ", "MR"),
    ("RZ", "R"),
    ("CNOT", "CX"),
    ("ZCX", "CX"),
    ("ZCY", "CY"),
    ("ZCZ", "CZ"),
    ("H_XZ", "H"),
    ("SQRT_Z", "S"),
    ("SQRT_Z_DAG", "S_DAG"),
    ("CORRELATED_ERROR", "E"),
];

const GATES: &[GateInfo] = &[
    not_fusable_gate(
        "DETECTOR",
        GateCategory::Annotation,
        ArgRule::Any,
        TargetRule::RecOnly,
    ),
    not_fusable_gate(
        "OBSERVABLE_INCLUDE",
        GateCategory::Annotation,
        ArgRule::UnsignedInteger,
        TargetRule::RecOrPauli,
    ),
    not_fusable_gate(
        "TICK",
        GateCategory::Annotation,
        ArgRule::Exact(0),
        TargetRule::None,
    ),
    not_fusable_gate(
        "QUBIT_COORDS",
        GateCategory::Annotation,
        ArgRule::Any,
        TargetRule::QubitCoords,
    ),
    not_fusable_gate(
        "SHIFT_COORDS",
        GateCategory::Annotation,
        ArgRule::Any,
        TargetRule::None,
    ),
    not_fusable_gate(
        "REPEAT",
        GateCategory::ControlFlow,
        ArgRule::Exact(0),
        TargetRule::None,
    ),
    gate(
        "MPAD",
        GateCategory::Annotation,
        ArgRule::ZeroOrOneProbability,
        TargetRule::MeasurementPads,
    ),
    gate(
        "MX",
        GateCategory::Collapsing,
        ArgRule::ZeroOrOneProbability,
        TargetRule::MeasurementQubits,
    ),
    gate(
        "MY",
        GateCategory::Collapsing,
        ArgRule::ZeroOrOneProbability,
        TargetRule::MeasurementQubits,
    ),
    gate(
        "M",
        GateCategory::Collapsing,
        ArgRule::ZeroOrOneProbability,
        TargetRule::MeasurementQubits,
    ),
    gate(
        "MRX",
        GateCategory::Collapsing,
        ArgRule::ZeroOrOneProbability,
        TargetRule::MeasurementQubits,
    ),
    gate(
        "MRY",
        GateCategory::Collapsing,
        ArgRule::ZeroOrOneProbability,
        TargetRule::MeasurementQubits,
    ),
    gate(
        "MR",
        GateCategory::Collapsing,
        ArgRule::ZeroOrOneProbability,
        TargetRule::MeasurementQubits,
    ),
    gate_with_inverse(
        "RX",
        "MX",
        GateCategory::Collapsing,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate_with_inverse(
        "RY",
        "MY",
        GateCategory::Collapsing,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate_with_inverse(
        "R",
        "M",
        GateCategory::Collapsing,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate(
        "XCX",
        GateCategory::Controlled,
        ArgRule::Exact(0),
        TargetRule::Pairs,
    ),
    gate(
        "XCY",
        GateCategory::Controlled,
        ArgRule::Exact(0),
        TargetRule::Pairs,
    ),
    gate(
        "XCZ",
        GateCategory::Controlled,
        ArgRule::Exact(0),
        TargetRule::Pairs,
    ),
    gate(
        "YCX",
        GateCategory::Controlled,
        ArgRule::Exact(0),
        TargetRule::Pairs,
    ),
    gate(
        "YCY",
        GateCategory::Controlled,
        ArgRule::Exact(0),
        TargetRule::Pairs,
    ),
    gate(
        "YCZ",
        GateCategory::Controlled,
        ArgRule::Exact(0),
        TargetRule::Pairs,
    ),
    gate(
        "CX",
        GateCategory::Controlled,
        ArgRule::Exact(0),
        TargetRule::Pairs,
    ),
    gate(
        "CY",
        GateCategory::Controlled,
        ArgRule::Exact(0),
        TargetRule::Pairs,
    ),
    gate(
        "CZ",
        GateCategory::Controlled,
        ArgRule::Exact(0),
        TargetRule::Pairs,
    ),
    gate(
        "H",
        GateCategory::HadamardLike,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate(
        "H_XY",
        GateCategory::HadamardLike,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate(
        "H_YZ",
        GateCategory::HadamardLike,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate(
        "H_NXY",
        GateCategory::HadamardLike,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate(
        "H_NXZ",
        GateCategory::HadamardLike,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate(
        "H_NYZ",
        GateCategory::HadamardLike,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate(
        "DEPOLARIZE1",
        GateCategory::Noise,
        ArgRule::ProbabilityList(1),
        TargetRule::AnySingleQubit,
    ),
    gate(
        "DEPOLARIZE2",
        GateCategory::Noise,
        ArgRule::ProbabilityList(1),
        TargetRule::Pairs,
    ),
    gate(
        "X_ERROR",
        GateCategory::Noise,
        ArgRule::ProbabilityList(1),
        TargetRule::AnySingleQubit,
    ),
    gate(
        "Y_ERROR",
        GateCategory::Noise,
        ArgRule::ProbabilityList(1),
        TargetRule::AnySingleQubit,
    ),
    gate(
        "Z_ERROR",
        GateCategory::Noise,
        ArgRule::ProbabilityList(1),
        TargetRule::AnySingleQubit,
    ),
    gate(
        "I_ERROR",
        GateCategory::Noise,
        ArgRule::ProbabilityList(1),
        TargetRule::AnySingleQubit,
    ),
    gate(
        "II_ERROR",
        GateCategory::Noise,
        ArgRule::ProbabilityList(1),
        TargetRule::Pairs,
    ),
    gate(
        "PAULI_CHANNEL_1",
        GateCategory::Noise,
        ArgRule::ProbabilityList(3),
        TargetRule::AnySingleQubit,
    ),
    gate(
        "PAULI_CHANNEL_2",
        GateCategory::Noise,
        ArgRule::ProbabilityList(15),
        TargetRule::Pairs,
    ),
    not_fusable_gate(
        "E",
        GateCategory::Noise,
        ArgRule::ProbabilityList(1),
        TargetRule::PauliList,
    ),
    not_fusable_gate(
        "ELSE_CORRELATED_ERROR",
        GateCategory::Noise,
        ArgRule::ProbabilityList(1),
        TargetRule::PauliList,
    ),
    gate(
        "HERALDED_ERASE",
        GateCategory::HeraldedNoise,
        ArgRule::ProbabilityList(1),
        TargetRule::AnySingleQubit,
    ),
    gate(
        "HERALDED_PAULI_CHANNEL_1",
        GateCategory::HeraldedNoise,
        ArgRule::ProbabilityList(4),
        TargetRule::AnySingleQubit,
    ),
    gate(
        "I",
        GateCategory::Pauli,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate(
        "X",
        GateCategory::Pauli,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate(
        "Y",
        GateCategory::Pauli,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate(
        "Z",
        GateCategory::Pauli,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate_with_inverse(
        "C_XYZ",
        "C_ZYX",
        GateCategory::Period3,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate_with_inverse(
        "C_ZYX",
        "C_XYZ",
        GateCategory::Period3,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate_with_inverse(
        "C_NXYZ",
        "C_ZYNX",
        GateCategory::Period3,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate_with_inverse(
        "C_XNYZ",
        "C_ZNYX",
        GateCategory::Period3,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate_with_inverse(
        "C_XYNZ",
        "C_NZYX",
        GateCategory::Period3,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate_with_inverse(
        "C_NZYX",
        "C_XYNZ",
        GateCategory::Period3,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate_with_inverse(
        "C_ZNYX",
        "C_XNYZ",
        GateCategory::Period3,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate_with_inverse(
        "C_ZYNX",
        "C_NXYZ",
        GateCategory::Period3,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate_with_inverse(
        "SQRT_X",
        "SQRT_X_DAG",
        GateCategory::Period4,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate_with_inverse(
        "SQRT_X_DAG",
        "SQRT_X",
        GateCategory::Period4,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate_with_inverse(
        "SQRT_Y",
        "SQRT_Y_DAG",
        GateCategory::Period4,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate_with_inverse(
        "SQRT_Y_DAG",
        "SQRT_Y",
        GateCategory::Period4,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate_with_inverse(
        "S",
        "S_DAG",
        GateCategory::Period4,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate_with_inverse(
        "S_DAG",
        "S",
        GateCategory::Period4,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate(
        "II",
        GateCategory::ParityPhasing,
        ArgRule::Exact(0),
        TargetRule::Pairs,
    ),
    gate_with_inverse(
        "SQRT_XX",
        "SQRT_XX_DAG",
        GateCategory::ParityPhasing,
        ArgRule::Exact(0),
        TargetRule::Pairs,
    ),
    gate_with_inverse(
        "SQRT_XX_DAG",
        "SQRT_XX",
        GateCategory::ParityPhasing,
        ArgRule::Exact(0),
        TargetRule::Pairs,
    ),
    gate_with_inverse(
        "SQRT_YY",
        "SQRT_YY_DAG",
        GateCategory::ParityPhasing,
        ArgRule::Exact(0),
        TargetRule::Pairs,
    ),
    gate_with_inverse(
        "SQRT_YY_DAG",
        "SQRT_YY",
        GateCategory::ParityPhasing,
        ArgRule::Exact(0),
        TargetRule::Pairs,
    ),
    gate_with_inverse(
        "SQRT_ZZ",
        "SQRT_ZZ_DAG",
        GateCategory::ParityPhasing,
        ArgRule::Exact(0),
        TargetRule::Pairs,
    ),
    gate_with_inverse(
        "SQRT_ZZ_DAG",
        "SQRT_ZZ",
        GateCategory::ParityPhasing,
        ArgRule::Exact(0),
        TargetRule::Pairs,
    ),
    gate(
        "MPP",
        GateCategory::PauliProduct,
        ArgRule::ZeroOrOneProbability,
        TargetRule::PauliProducts,
    ),
    gate_with_inverse(
        "SPP",
        "SPP_DAG",
        GateCategory::PauliProduct,
        ArgRule::Exact(0),
        TargetRule::PauliProducts,
    ),
    gate_with_inverse(
        "SPP_DAG",
        "SPP",
        GateCategory::PauliProduct,
        ArgRule::Exact(0),
        TargetRule::PauliProducts,
    ),
    gate(
        "SWAP",
        GateCategory::Swap,
        ArgRule::Exact(0),
        TargetRule::Pairs,
    ),
    gate_with_inverse(
        "ISWAP",
        "ISWAP_DAG",
        GateCategory::Swap,
        ArgRule::Exact(0),
        TargetRule::Pairs,
    ),
    gate_with_inverse(
        "CXSWAP",
        "SWAPCX",
        GateCategory::Swap,
        ArgRule::Exact(0),
        TargetRule::Pairs,
    ),
    gate_with_inverse(
        "SWAPCX",
        "CXSWAP",
        GateCategory::Swap,
        ArgRule::Exact(0),
        TargetRule::Pairs,
    ),
    gate(
        "CZSWAP",
        GateCategory::Swap,
        ArgRule::Exact(0),
        TargetRule::Pairs,
    ),
    gate_with_inverse(
        "ISWAP_DAG",
        "ISWAP",
        GateCategory::Swap,
        ArgRule::Exact(0),
        TargetRule::Pairs,
    ),
    gate(
        "MXX",
        GateCategory::PairMeasurement,
        ArgRule::ZeroOrOneProbability,
        TargetRule::Pairs,
    ),
    gate(
        "MYY",
        GateCategory::PairMeasurement,
        ArgRule::ZeroOrOneProbability,
        TargetRule::Pairs,
    ),
    gate(
        "MZZ",
        GateCategory::PairMeasurement,
        ArgRule::ZeroOrOneProbability,
        TargetRule::Pairs,
    ),
];

const fn gate(
    name: &'static str,
    category: GateCategory,
    arg_rule: ArgRule,
    target_rule: TargetRule,
) -> GateInfo {
    gate_with_inverse(name, name, category, arg_rule, target_rule)
}

const fn gate_with_inverse(
    name: &'static str,
    inverse_name: &'static str,
    category: GateCategory,
    arg_rule: ArgRule,
    target_rule: TargetRule,
) -> GateInfo {
    GateInfo {
        name,
        inverse_name,
        category,
        arg_rule,
        target_rule,
        can_fuse: true,
    }
}

const fn not_fusable_gate(
    name: &'static str,
    category: GateCategory,
    arg_rule: ArgRule,
    target_rule: TargetRule,
) -> GateInfo {
    GateInfo {
        name,
        inverse_name: name,
        category,
        arg_rule,
        target_rule,
        can_fuse: false,
    }
}
