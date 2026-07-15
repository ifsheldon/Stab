use std::fmt::{Display, Formatter};
use std::str::FromStr;

use crate::ids::STIM_TARGET_VALUE_LIMIT;
use crate::{CircuitError, CircuitResult, MeasureRecordOffset, QubitId};
use smallvec::SmallVec;

pub(crate) type TargetVec = SmallVec<[Target; 4]>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Pauli {
    X,
    Y,
    Z,
}

impl Pauli {
    fn parse_prefixed_target(text: &str) -> Option<(Self, &str)> {
        let mut chars = text.chars();
        let pauli = match chars.next()? {
            'X' | 'x' => Self::X,
            'Y' | 'y' => Self::Y,
            'Z' | 'z' => Self::Z,
            _ => return None,
        };
        Some((pauli, chars.as_str()))
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

    /// Returns the qubit id carried by a qubit or Pauli target.
    pub fn qubit_id(&self) -> Option<QubitId> {
        match self {
            Self::Qubit { id, .. } | Self::Pauli { id, .. } => Some(*id),
            Self::MeasurementRecord { .. } | Self::SweepBit { .. } | Self::Combiner => None,
        }
    }

    /// Returns the measurement-record offset carried by this target.
    pub fn measurement_record_offset(&self) -> Option<MeasureRecordOffset> {
        match self {
            Self::MeasurementRecord { offset } => Some(*offset),
            Self::Qubit { .. } | Self::SweepBit { .. } | Self::Pauli { .. } | Self::Combiner => {
                None
            }
        }
    }

    /// Returns the sweep-bit id carried by this target.
    pub fn sweep_bit_id(&self) -> Option<u32> {
        match self {
            Self::SweepBit { id } => Some(*id),
            Self::Qubit { .. }
            | Self::MeasurementRecord { .. }
            | Self::Pauli { .. }
            | Self::Combiner => None,
        }
    }

    /// Returns this target with its inversion flag toggled.
    pub fn try_inverted(&self) -> CircuitResult<Self> {
        match self {
            Self::Qubit { id, inverted } => Ok(Self::qubit(*id, !*inverted)),
            Self::Pauli {
                pauli,
                id,
                inverted,
            } => Ok(Self::pauli(*pauli, *id, !*inverted)),
            Self::MeasurementRecord { .. } | Self::SweepBit { .. } | Self::Combiner => Err(
                CircuitError::invalid_domain_value("invertible target", self),
            ),
        }
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
            let offset = parse_u24(offset, "measurement record target")?;
            let offset = i32::try_from(offset).map_err(|_| {
                CircuitError::invalid_domain_value("measurement record target", raw)
            })?;
            return Ok(Self::measurement_record(
                MeasureRecordOffset::from_stim_text(-offset)?,
            ));
        }
        if let Some(index) = raw.strip_prefix("sweep[").and_then(|v| v.strip_suffix(']')) {
            let id = parse_u24(index, "sweep target")?;
            return Ok(Self::sweep_bit(id));
        }

        let (inverted, body) = raw
            .strip_prefix('!')
            .map_or((false, raw), |body| (true, body));
        if let Some((pauli, id_text)) = Pauli::parse_prefixed_target(body) {
            let id = parse_u24(id_text, "pauli target")?;
            return Ok(Self::pauli(pauli, QubitId::new(id)?, inverted));
        }
        let id = parse_u24(body, "qubit target")?;
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
            Self::MeasurementRecord { offset } if offset.get() == 0 => f.write_str("rec[-0]"),
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

pub(crate) fn parse_target_token_into(token: &str, targets: &mut TargetVec) -> CircuitResult<()> {
    if token == "*" {
        targets.push(Target::combiner());
        return Ok(());
    }
    if !token.contains('*') {
        if token
            .as_bytes()
            .first()
            .is_some_and(|byte| byte.is_ascii_digit())
        {
            let id = parse_u24(token, "qubit target")?;
            targets.push(Target::qubit(QubitId::new(id)?, false));
        } else {
            targets.push(Target::from_str(token)?);
        }
        return Ok(());
    }
    for (index, part) in token.split('*').enumerate() {
        if part.is_empty() {
            return Err(CircuitError::invalid_domain_value("target combiner", token));
        }
        if index > 0 {
            targets.push(Target::combiner());
        }
        targets.push(Target::from_str(part)?);
    }
    Ok(())
}

pub(crate) fn parse_plain_qubit_target_text(text: &str) -> CircuitResult<Option<TargetVec>> {
    let mut targets = TargetVec::new();
    let mut value = None;
    for byte in text.bytes() {
        if byte.is_ascii_digit() {
            let digit = u32::from(byte - b'0');
            let next = value
                .unwrap_or(0u32)
                .checked_mul(10)
                .and_then(|value| value.checked_add(digit))
                .ok_or_else(|| CircuitError::invalid_domain_value("qubit target", text))?;
            if next >= STIM_TARGET_VALUE_LIMIT {
                return Err(CircuitError::invalid_domain_value("qubit target", text));
            }
            value = Some(next);
        } else if byte.is_ascii_whitespace() {
            if let Some(id) = value.take() {
                targets.push(Target::qubit(QubitId::new(id)?, false));
            }
        } else {
            return Ok(None);
        }
    }
    if let Some(id) = value {
        targets.push(Target::qubit(QubitId::new(id)?, false));
    }
    Ok(Some(targets))
}

fn parse_u24(text: &str, kind: &'static str) -> CircuitResult<u32> {
    if text.is_empty() {
        return Err(CircuitError::invalid_domain_value(kind, text));
    }
    let mut value = 0u32;
    for byte in text.bytes() {
        if !byte.is_ascii_digit() {
            return Err(CircuitError::invalid_domain_value(kind, text));
        }
        let digit = u32::from(byte - b'0');
        value = value
            .checked_mul(10)
            .and_then(|value| value.checked_add(digit))
            .ok_or_else(|| CircuitError::invalid_domain_value(kind, text))?;
        if value >= STIM_TARGET_VALUE_LIMIT {
            return Err(CircuitError::invalid_domain_value(kind, text));
        }
    }
    Ok(value)
}
