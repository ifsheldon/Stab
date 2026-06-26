use std::fmt::{Display, Formatter};
use std::str::FromStr;

use crate::{CircuitError, CircuitResult, MeasureRecordOffset, QubitId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Pauli {
    X,
    Y,
    Z,
}

impl Pauli {
    fn parse_prefixed_target(text: &str) -> Option<(Self, &str)> {
        if let Some(rest) = text.strip_prefix('X') {
            Some((Self::X, rest))
        } else if let Some(rest) = text.strip_prefix('Y') {
            Some((Self::Y, rest))
        } else {
            text.strip_prefix('Z').map(|rest| (Self::Z, rest))
        }
    }
}

impl Display for Pauli {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::X => f.write_str("X"),
            Self::Y => f.write_str("Y"),
            Self::Z => f.write_str("Z"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Target {
    Qubit {
        id: QubitId,
        inverted: bool,
    },
    MeasurementRecord {
        offset: MeasureRecordOffset,
    },
    SweepBit {
        id: u32,
    },
    Pauli {
        pauli: Pauli,
        id: QubitId,
        inverted: bool,
    },
    Combiner,
}

impl Target {
    /// Creates a qubit target.
    pub fn qubit(id: QubitId, inverted: bool) -> Self {
        Self::Qubit { id, inverted }
    }

    /// Creates a measurement-record target such as `rec[-1]`.
    pub fn measurement_record(offset: MeasureRecordOffset) -> Self {
        Self::MeasurementRecord { offset }
    }

    /// Creates a sweep-bit target such as `sweep[0]`.
    pub fn sweep_bit(id: u32) -> Self {
        Self::SweepBit { id }
    }

    /// Creates a Pauli product target such as `X0` or `!Z1`.
    pub fn pauli(pauli: Pauli, id: QubitId, inverted: bool) -> Self {
        Self::Pauli {
            pauli,
            id,
            inverted,
        }
    }

    /// Creates the `*` combiner used inside Pauli product targets.
    pub fn combiner() -> Self {
        Self::Combiner
    }

    /// Returns true when this target is a plain qubit target.
    pub fn is_qubit_target(&self) -> bool {
        matches!(self, Self::Qubit { .. })
    }

    /// Returns true when this target is inverted with `!`.
    pub fn is_inverted_result_target(&self) -> bool {
        matches!(
            self,
            Self::Qubit { inverted: true, .. } | Self::Pauli { inverted: true, .. }
        )
    }

    /// Returns true when this target is a measurement-record target.
    pub fn is_measurement_record_target(&self) -> bool {
        matches!(self, Self::MeasurementRecord { .. })
    }

    /// Returns true when this target is a sweep-bit target.
    pub fn is_sweep_bit_target(&self) -> bool {
        matches!(self, Self::SweepBit { .. })
    }

    /// Returns true when this target is one of `X`, `Y`, or `Z`.
    pub fn is_pauli_target(&self) -> bool {
        matches!(self, Self::Pauli { .. })
    }

    /// Returns true when this target is an `X` Pauli target.
    pub fn is_x_target(&self) -> bool {
        matches!(
            self,
            Self::Pauli {
                pauli: Pauli::X,
                ..
            }
        )
    }

    /// Returns true when this target is a `Y` Pauli target.
    pub fn is_y_target(&self) -> bool {
        matches!(
            self,
            Self::Pauli {
                pauli: Pauli::Y,
                ..
            }
        )
    }

    /// Returns true when this target is a `Z` Pauli target.
    pub fn is_z_target(&self) -> bool {
        matches!(
            self,
            Self::Pauli {
                pauli: Pauli::Z,
                ..
            }
        )
    }

    /// Returns true when this target refers to a classical bit source.
    pub fn is_classical_bit_target(&self) -> bool {
        matches!(self, Self::SweepBit { .. } | Self::MeasurementRecord { .. })
    }

    /// Returns true when this target is the `*` Pauli-product combiner.
    pub fn is_combiner(&self) -> bool {
        matches!(self, Self::Combiner)
    }

    /// Returns the Pauli type for Pauli targets.
    pub fn pauli_type(&self) -> Option<Pauli> {
        match self {
            Self::Pauli { pauli, .. } => Some(*pauli),
            Self::Qubit { .. }
            | Self::MeasurementRecord { .. }
            | Self::SweepBit { .. }
            | Self::Combiner => None,
        }
    }

    pub(crate) fn is_qubit_like(&self) -> bool {
        self.is_qubit_target()
    }

    pub(crate) fn is_classical_or_qubit(&self) -> bool {
        self.is_qubit_target() || self.is_classical_bit_target()
    }

    pub(crate) fn is_measurement_record(&self) -> bool {
        self.is_measurement_record_target()
    }

    pub(crate) fn is_pauli_product_part(&self) -> bool {
        self.is_pauli_target() || self.is_combiner()
    }
}

impl FromStr for Target {
    type Err = CircuitError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        if raw.is_empty() {
            return Err(CircuitError::invalid_domain_value("target", raw));
        }
        if raw == "*" {
            return Ok(Self::Combiner);
        }
        if let Some(offset) = raw.strip_prefix("rec[-").and_then(|v| v.strip_suffix(']')) {
            let offset = offset.parse::<i32>().map_err(|_| {
                CircuitError::invalid_domain_value("measurement record target", raw)
            })?;
            return Ok(Self::measurement_record(MeasureRecordOffset::try_new(
                -offset,
            )?));
        }
        if let Some(index) = raw.strip_prefix("sweep[").and_then(|v| v.strip_suffix(']')) {
            let id = parse_u32(index, "sweep target")?;
            return Ok(Self::sweep_bit(id));
        }

        let (inverted, body) = raw
            .strip_prefix('!')
            .map_or((false, raw), |body| (true, body));
        if let Some((pauli, id_text)) = Pauli::parse_prefixed_target(body) {
            let id = parse_u32(id_text, "pauli target")?;
            return Ok(Self::pauli(pauli, QubitId::new(id)?, inverted));
        }
        let id = parse_u32(body, "qubit target")?;
        Ok(Self::qubit(QubitId::new(id)?, inverted))
    }
}

impl Display for Target {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Qubit { id, inverted } => {
                if *inverted {
                    write!(f, "!{}", id.get())
                } else {
                    write!(f, "{}", id.get())
                }
            }
            Self::MeasurementRecord { offset } => write!(f, "rec[{}]", offset.get()),
            Self::SweepBit { id } => write!(f, "sweep[{id}]"),
            Self::Pauli {
                pauli,
                id,
                inverted,
            } => {
                if *inverted {
                    write!(f, "!{pauli}{}", id.get())
                } else {
                    write!(f, "{pauli}{}", id.get())
                }
            }
            Self::Combiner => f.write_str("*"),
        }
    }
}

pub(crate) fn parse_target_token(token: &str) -> CircuitResult<Vec<Target>> {
    if token == "*" {
        return Ok(vec![Target::combiner()]);
    }
    if !token.contains('*') {
        return Ok(vec![Target::from_str(token)?]);
    }
    let mut targets = Vec::new();
    for (index, part) in token.split('*').enumerate() {
        if part.is_empty() {
            return Err(CircuitError::invalid_domain_value("target combiner", token));
        }
        if index > 0 {
            targets.push(Target::combiner());
        }
        targets.push(Target::from_str(part)?);
    }
    Ok(targets)
}

fn parse_u32(text: &str, kind: &'static str) -> CircuitResult<u32> {
    text.parse::<u32>()
        .map_err(|_| CircuitError::invalid_domain_value(kind, text))
}
