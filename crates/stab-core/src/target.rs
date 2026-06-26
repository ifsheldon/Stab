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
    pub fn qubit(id: QubitId, inverted: bool) -> Self {
        Self::Qubit { id, inverted }
    }

    pub fn measurement_record(offset: MeasureRecordOffset) -> Self {
        Self::MeasurementRecord { offset }
    }

    pub fn sweep_bit(id: u32) -> Self {
        Self::SweepBit { id }
    }

    pub fn pauli(pauli: Pauli, id: QubitId, inverted: bool) -> Self {
        Self::Pauli {
            pauli,
            id,
            inverted,
        }
    }

    pub fn combiner() -> Self {
        Self::Combiner
    }

    pub(crate) fn is_qubit_like(&self) -> bool {
        matches!(self, Self::Qubit { .. })
    }

    pub(crate) fn is_classical_or_qubit(&self) -> bool {
        matches!(
            self,
            Self::Qubit { .. } | Self::SweepBit { .. } | Self::MeasurementRecord { .. }
        )
    }

    pub(crate) fn is_measurement_record(&self) -> bool {
        matches!(self, Self::MeasurementRecord { .. })
    }

    pub(crate) fn is_pauli_product_part(&self) -> bool {
        matches!(self, Self::Pauli { .. } | Self::Combiner)
    }

    pub(crate) fn is_combiner(&self) -> bool {
        matches!(self, Self::Combiner)
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
