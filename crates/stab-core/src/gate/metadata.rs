use super::{ArgRule, Gate, GateCategory, GateUnitaryMatrix, TargetRule};
use crate::{
    CircuitError, CircuitResult, Flow, GateDecomposition, PauliBasis, PauliSign, PauliString,
    StabilizerError,
};

/// Public argument validation shape for a Stim gate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GateArgumentRule {
    /// The gate takes exactly this many parenthesized arguments.
    Exact(usize),
    /// The gate accepts any finite coordinate-like argument list.
    Any,
    /// The gate accepts zero or one probability argument.
    OptionalProbability,
    /// The gate takes exactly this many disjoint probability arguments.
    ProbabilityList(usize),
    /// The gate accepts any number of disjoint probability arguments.
    AnyProbabilityList,
    /// The gate takes exactly one unsigned integer argument.
    UnsignedInteger,
}

/// Public target validation shape for a Stim gate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GateTargetRule {
    None,
    AnySingleQubit,
    MeasurementQubits,
    MeasurementPads,
    PlainPairs,
    ClassicalControlPairs,
    MeasurementPairs,
    RecOnly,
    RecOrPauli,
    QubitCoords,
    PauliProducts,
    PauliList,
}

/// How a circuit instruction's flat target list is grouped by this gate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GateTargetGroupKind {
    None,
    Singles,
    Pairs,
    PauliProducts,
    AllTargets,
}

impl Gate {
    /// Returns all accepted names for this gate, in Stim v1.16.0 alias order.
    pub fn aliases(self) -> &'static [&'static str] {
        match self.info.name {
            "DETECTOR" => &["DETECTOR"],
            "OBSERVABLE_INCLUDE" => &["OBSERVABLE_INCLUDE"],
            "TICK" => &["TICK"],
            "QUBIT_COORDS" => &["QUBIT_COORDS"],
            "SHIFT_COORDS" => &["SHIFT_COORDS"],
            "REPEAT" => &["REPEAT"],
            "MPAD" => &["MPAD"],
            "MX" => &["MX"],
            "MY" => &["MY"],
            "M" => &["M", "MZ"],
            "MRX" => &["MRX"],
            "MRY" => &["MRY"],
            "MR" => &["MR", "MRZ"],
            "RX" => &["RX"],
            "RY" => &["RY"],
            "R" => &["R", "RZ"],
            "XCX" => &["XCX"],
            "XCY" => &["XCY"],
            "XCZ" => &["XCZ"],
            "YCX" => &["YCX"],
            "YCY" => &["YCY"],
            "YCZ" => &["YCZ"],
            "CX" => &["CNOT", "CX", "ZCX"],
            "CY" => &["CY", "ZCY"],
            "CZ" => &["CZ", "ZCZ"],
            "H" => &["H", "H_XZ"],
            "H_XY" => &["H_XY"],
            "H_YZ" => &["H_YZ"],
            "H_NXY" => &["H_NXY"],
            "H_NXZ" => &["H_NXZ"],
            "H_NYZ" => &["H_NYZ"],
            "DEPOLARIZE1" => &["DEPOLARIZE1"],
            "DEPOLARIZE2" => &["DEPOLARIZE2"],
            "X_ERROR" => &["X_ERROR"],
            "Y_ERROR" => &["Y_ERROR"],
            "Z_ERROR" => &["Z_ERROR"],
            "I_ERROR" => &["I_ERROR"],
            "II_ERROR" => &["II_ERROR"],
            "PAULI_CHANNEL_1" => &["PAULI_CHANNEL_1"],
            "PAULI_CHANNEL_2" => &["PAULI_CHANNEL_2"],
            "E" => &["CORRELATED_ERROR", "E"],
            "ELSE_CORRELATED_ERROR" => &["ELSE_CORRELATED_ERROR"],
            "HERALDED_ERASE" => &["HERALDED_ERASE"],
            "HERALDED_PAULI_CHANNEL_1" => &["HERALDED_PAULI_CHANNEL_1"],
            "I" => &["I"],
            "X" => &["X"],
            "Y" => &["Y"],
            "Z" => &["Z"],
            "C_XYZ" => &["C_XYZ"],
            "C_ZYX" => &["C_ZYX"],
            "C_NXYZ" => &["C_NXYZ"],
            "C_XNYZ" => &["C_XNYZ"],
            "C_XYNZ" => &["C_XYNZ"],
            "C_NZYX" => &["C_NZYX"],
            "C_ZNYX" => &["C_ZNYX"],
            "C_ZYNX" => &["C_ZYNX"],
            "SQRT_X" => &["SQRT_X"],
            "SQRT_X_DAG" => &["SQRT_X_DAG"],
            "SQRT_Y" => &["SQRT_Y"],
            "SQRT_Y_DAG" => &["SQRT_Y_DAG"],
            "S" => &["S", "SQRT_Z"],
            "S_DAG" => &["S_DAG", "SQRT_Z_DAG"],
            "II" => &["II"],
            "SQRT_XX" => &["SQRT_XX"],
            "SQRT_XX_DAG" => &["SQRT_XX_DAG"],
            "SQRT_YY" => &["SQRT_YY"],
            "SQRT_YY_DAG" => &["SQRT_YY_DAG"],
            "SQRT_ZZ" => &["SQRT_ZZ"],
            "SQRT_ZZ_DAG" => &["SQRT_ZZ_DAG"],
            "MPP" => &["MPP"],
            "SPP" => &["SPP"],
            "SPP_DAG" => &["SPP_DAG"],
            "SWAP" => &["SWAP"],
            "ISWAP" => &["ISWAP"],
            "CXSWAP" => &["CXSWAP"],
            "SWAPCX" => &["SWAPCX"],
            "CZSWAP" => &["CZSWAP", "SWAPCZ"],
            "ISWAP_DAG" => &["ISWAP_DAG"],
            "MXX" => &["MXX"],
            "MYY" => &["MYY"],
            "MZZ" => &["MZZ"],
            _ => &[],
        }
    }

    pub fn argument_rule(self) -> GateArgumentRule {
        self.info.arg_rule.into()
    }

    pub fn target_rule(self) -> GateTargetRule {
        self.info.target_rule.into()
    }

    pub fn target_group_kind(self) -> GateTargetGroupKind {
        self.info.target_rule.target_group_kind()
    }

    /// Returns true when Stim has a unitary/tableau inverse for this gate.
    pub fn is_unitary(self) -> bool {
        matches!(
            self.info.category,
            GateCategory::Controlled
                | GateCategory::HadamardLike
                | GateCategory::Pauli
                | GateCategory::Period3
                | GateCategory::Period4
                | GateCategory::ParityPhasing
                | GateCategory::Swap
        ) || matches!(self.info.name, "SPP" | "SPP_DAG")
    }

    /// Returns true for reset or measure-reset gates.
    pub fn is_reset(self) -> bool {
        matches!(self.info.name, "RX" | "RY" | "R" | "MRX" | "MRY" | "MR")
    }

    /// Returns Stim v1.16.0's `GateData.is_noisy_gate` flag.
    ///
    /// This intentionally excludes `MPAD`, which can take a probability argument but is not flagged as noisy by Stim.
    pub fn is_noisy(self) -> bool {
        matches!(
            self.info.category,
            GateCategory::Noise | GateCategory::HeraldedNoise | GateCategory::PairMeasurement
        ) || matches!(
            self.info.name,
            "MX" | "MY" | "M" | "MRX" | "MRY" | "MR" | "MPP"
        )
    }

    pub fn produces_measurements(self) -> bool {
        matches!(
            self.info.name,
            "MPAD"
                | "MX"
                | "MY"
                | "M"
                | "MRX"
                | "MRY"
                | "MR"
                | "MPP"
                | "HERALDED_ERASE"
                | "HERALDED_PAULI_CHANNEL_1"
                | "MXX"
                | "MYY"
                | "MZZ"
        )
    }

    pub fn is_single_qubit_gate(self) -> bool {
        matches!(
            self.info.target_rule,
            TargetRule::AnySingleQubit | TargetRule::MeasurementQubits
        )
    }

    pub fn is_two_qubit_gate(self) -> bool {
        matches!(
            self.info.target_rule,
            TargetRule::PlainPairs
                | TargetRule::ClassicalControlPairs
                | TargetRule::MeasurementPairs
        )
    }

    pub fn takes_measurement_record_targets(self) -> bool {
        matches!(
            self.info.target_rule,
            TargetRule::ClassicalControlPairs | TargetRule::RecOnly | TargetRule::RecOrPauli
        )
    }

    pub fn takes_pauli_targets(self) -> bool {
        matches!(
            self.info.target_rule,
            TargetRule::RecOrPauli | TargetRule::PauliProducts | TargetRule::PauliList
        )
    }

    pub fn is_symmetric_gate(self) -> bool {
        if matches!(
            self.info.target_rule,
            TargetRule::AnySingleQubit | TargetRule::MeasurementQubits
        ) {
            return true;
        }
        matches!(
            self.info.name,
            "DEPOLARIZE2"
                | "II_ERROR"
                | "XCX"
                | "YCY"
                | "CZ"
                | "II"
                | "SQRT_XX"
                | "SQRT_XX_DAG"
                | "SQRT_YY"
                | "SQRT_YY_DAG"
                | "SQRT_ZZ"
                | "SQRT_ZZ_DAG"
                | "SWAP"
                | "ISWAP"
                | "ISWAP_DAG"
                | "CZSWAP"
                | "MXX"
                | "MYY"
                | "MZZ"
        )
    }

    /// Returns the true unitary inverse, or `None` for non-unitary gates.
    pub fn inverse(self) -> Option<Self> {
        self.is_unitary()
            .then(|| Self::from_name(self.info.inverse_name).ok())
            .flatten()
    }

    /// Returns Stim's best candidate inverse, including non-unitary generalized inverses.
    pub fn generalized_inverse(self) -> crate::CircuitResult<Self> {
        self.best_candidate_inverse()
    }

    /// Returns the local Clifford tableau metadata for gates with known tableau data.
    pub fn tableau(self) -> crate::CircuitResult<crate::Tableau> {
        crate::circuit_tableau::gate_tableau(self.info.name)
    }

    /// Returns true when `tableau` can produce local Clifford tableau metadata for this gate.
    pub fn has_tableau(self) -> bool {
        crate::circuit_tableau::gate_has_tableau(self.info.name)
    }

    /// Returns tableau-backed stabilizer flow generators for fixed-shape local Clifford gates.
    ///
    /// Measurement-rich and variable-target flow metadata, such as `MXX` and `MPP`, is owned by later flow milestones.
    pub fn flows(self) -> CircuitResult<Vec<Flow>> {
        if !self.has_flows() {
            return Err(CircuitError::invalid_tableau_conversion(format!(
                "gate {} does not have tableau-backed flow data",
                self.info.name
            )));
        }
        let tableau = self.tableau()?;
        let mut flows = Vec::with_capacity(tableau.len() * 2);
        for index in 0..tableau.len() {
            flows.push(Flow::new(
                single_pauli(tableau.len(), index, PauliBasis::X),
                tableau
                    .x_output(index)
                    .map_err(stabilizer_to_circuit_error)?
                    .clone(),
                [],
                [],
            ));
            flows.push(Flow::new(
                single_pauli(tableau.len(), index, PauliBasis::Z),
                tableau
                    .z_output(index)
                    .map_err(stabilizer_to_circuit_error)?
                    .clone(),
                [],
                [],
            ));
        }
        Ok(flows)
    }

    /// Returns true when `flows` can produce tableau-backed stabilizer flow metadata.
    pub fn has_flows(self) -> bool {
        self.has_tableau()
    }

    /// Returns Stim v1.16.0's fixed-shape one- or two-qubit unitary matrix metadata.
    ///
    /// Variable-target unitary gate families, such as `SPP` and `SPP_DAG`, do not have fixed
    /// matrix metadata in Stim's gate table and are rejected here.
    pub fn unitary_matrix(self) -> CircuitResult<GateUnitaryMatrix> {
        crate::gate::unitary::gate_unitary_matrix(self.info.name).ok_or_else(|| {
            CircuitError::invalid_tableau_conversion(format!(
                "gate {} does not have fixed-shape unitary matrix data",
                self.info.name
            ))
        })
    }

    /// Returns true when `unitary_matrix` can produce fixed-shape unitary metadata.
    pub fn has_unitary_matrix(self) -> bool {
        crate::gate::unitary::gate_has_unitary_matrix(self.info.name)
    }

    /// Returns Stim v1.16.0's H/S/CX/M/R decomposition metadata for this gate.
    ///
    /// This exposes the static gate-table metadata only. Full circuit decomposition is owned by
    /// the circuit transform milestones and is not implied by this accessor.
    pub fn h_s_cx_m_r_decomposition(self) -> CircuitResult<GateDecomposition> {
        crate::gate::decomposition::gate_decomposition(self.info.name).ok_or_else(|| {
            CircuitError::invalid_tableau_conversion(format!(
                "gate {} does not have H/S/CX/M/R decomposition data",
                self.info.name
            ))
        })
    }

    /// Returns true when `h_s_cx_m_r_decomposition` can produce gate-table metadata.
    pub fn has_h_s_cx_m_r_decomposition(self) -> bool {
        crate::gate::decomposition::gate_has_decomposition(self.info.name)
    }

    pub fn can_fuse(self) -> bool {
        self.info.can_fuse
    }
}

fn single_pauli(len: usize, index: usize, basis: PauliBasis) -> PauliString {
    PauliString::from_bases(
        PauliSign::Plus,
        (0..len).map(|candidate| {
            if candidate == index {
                basis
            } else {
                PauliBasis::I
            }
        }),
    )
}

fn stabilizer_to_circuit_error(error: StabilizerError) -> CircuitError {
    CircuitError::invalid_tableau_conversion(error.to_string())
}

impl From<ArgRule> for GateArgumentRule {
    fn from(value: ArgRule) -> Self {
        match value {
            ArgRule::Exact(count) => Self::Exact(count),
            ArgRule::Any => Self::Any,
            ArgRule::ZeroOrOneProbability => Self::OptionalProbability,
            ArgRule::ProbabilityList(count) => Self::ProbabilityList(count),
            ArgRule::AnyProbabilityList => Self::AnyProbabilityList,
            ArgRule::UnsignedInteger => Self::UnsignedInteger,
        }
    }
}

impl From<TargetRule> for GateTargetRule {
    fn from(value: TargetRule) -> Self {
        match value {
            TargetRule::None => Self::None,
            TargetRule::AnySingleQubit => Self::AnySingleQubit,
            TargetRule::MeasurementQubits => Self::MeasurementQubits,
            TargetRule::MeasurementPads => Self::MeasurementPads,
            TargetRule::PlainPairs => Self::PlainPairs,
            TargetRule::ClassicalControlPairs => Self::ClassicalControlPairs,
            TargetRule::MeasurementPairs => Self::MeasurementPairs,
            TargetRule::RecOnly => Self::RecOnly,
            TargetRule::RecOrPauli => Self::RecOrPauli,
            TargetRule::QubitCoords => Self::QubitCoords,
            TargetRule::PauliProducts => Self::PauliProducts,
            TargetRule::PauliList => Self::PauliList,
        }
    }
}
