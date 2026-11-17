use std::fmt::{Display, Formatter};
use std::str::FromStr;

use crate::ids::STIM_TARGET_VALUE_LIMIT;
use crate::{MeasureRecordOffset, ModelError, ModelResult, QubitId};
use smallvec::SmallVec;

/// Targets stored inline in an instruction before it spills to the heap.
const INLINE_TARGETS: usize = 4;

pub(crate) type TargetVec = SmallVec<[Target; INLINE_TARGETS]>;

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

/// One Stim instruction target, packed into four bytes as Stim packs its `GateTarget`.
///
/// A target is a plain `Copy` value. Construct it with [`Target::qubit`],
/// [`Target::measurement_record`], [`Target::sweep_bit`], [`Target::pauli`], or
/// [`Target::combiner`], or parse it with [`FromStr`]; read it through [`Target::kind`], which
/// returns the typed [`TargetKind`] view for exhaustive matches, or through the accessors below.
/// The packed encoding is private.
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct Target(u32);

/// The typed view of one [`Target`], returned by [`Target::kind`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TargetKind {
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

/// Bits of the packed target value: a qubit id, sweep-bit id, or record lookback distance.
const VALUE_MASK: u32 = STIM_TARGET_VALUE_LIMIT - 1;
/// The packed target class sits above the 24 value bits.
const CLASS_SHIFT: u32 = 24;
const CLASS_MASK: u32 = 0b111;
const CLASS_QUBIT: u32 = 0;
const CLASS_RECORD: u32 = 1;
const CLASS_SWEEP: u32 = 2;
const CLASS_PAULI_X: u32 = 3;
const CLASS_PAULI_Y: u32 = 4;
const CLASS_PAULI_Z: u32 = 5;
const CLASS_COMBINER: u32 = 6;
/// The `!` inversion of a qubit or Pauli target.
const INVERTED_BIT: u32 = 1 << 27;

impl Target {
    #[inline(always)]
    const fn pack(class: u32, value: u32, inverted: bool) -> Self {
        let inverted = if inverted { INVERTED_BIT } else { 0 };
        Self((class << CLASS_SHIFT) | (value & VALUE_MASK) | inverted)
    }

    #[inline(always)]
    const fn class(self) -> u32 {
        (self.0 >> CLASS_SHIFT) & CLASS_MASK
    }

    #[inline(always)]
    const fn value(self) -> u32 {
        self.0 & VALUE_MASK
    }

    #[inline(always)]
    const fn inverted_flag(self) -> bool {
        self.0 & INVERTED_BIT != 0
    }

    /// Creates a qubit target.
    #[inline]
    pub fn qubit(id: QubitId, inverted: bool) -> Self {
        Self::pack(CLASS_QUBIT, id.get(), inverted)
    }

    /// Creates a measurement-record target such as `rec[-1]`.
    #[inline]
    pub fn measurement_record(offset: MeasureRecordOffset) -> Self {
        // A lookback is nonzero, so distance zero is free to encode Stim's distinct `rec[-0]`.
        Self::pack(CLASS_RECORD, offset.get().unsigned_abs(), false)
    }

    /// Creates a sweep-bit target such as `sweep[0]`, refusing ids Stim cannot encode
    /// (`2^24` and above).
    pub fn sweep_bit(id: u32) -> ModelResult<Self> {
        if id >= STIM_TARGET_VALUE_LIMIT {
            return Err(ModelError::invalid_domain_value("sweep target", id));
        }
        Ok(Self::pack(CLASS_SWEEP, id, false))
    }

    /// Creates a Pauli product target such as `X0` or `!Z1`.
    #[inline]
    pub fn pauli(pauli: Pauli, id: QubitId, inverted: bool) -> Self {
        let class = match pauli {
            Pauli::X => CLASS_PAULI_X,
            Pauli::Y => CLASS_PAULI_Y,
            Pauli::Z => CLASS_PAULI_Z,
        };
        Self::pack(class, id.get(), inverted)
    }

    /// Creates the `*` combiner used inside Pauli product targets.
    #[inline]
    pub fn combiner() -> Self {
        Self::pack(CLASS_COMBINER, 0, false)
    }

    /// Returns the typed view of this target.
    #[inline]
    pub fn kind(&self) -> TargetKind {
        let id = QubitId::from_packed(self.value());
        match self.class() {
            CLASS_QUBIT => TargetKind::Qubit {
                id,
                inverted: self.inverted_flag(),
            },
            CLASS_RECORD => TargetKind::MeasurementRecord {
                offset: MeasureRecordOffset::from_packed_distance(self.value()),
            },
            CLASS_SWEEP => TargetKind::SweepBit { id: self.value() },
            CLASS_PAULI_X => self.pauli_kind(Pauli::X, id),
            CLASS_PAULI_Y => self.pauli_kind(Pauli::Y, id),
            CLASS_PAULI_Z => self.pauli_kind(Pauli::Z, id),
            // Constructors produce no other class, so the remaining one is the combiner.
            _ => TargetKind::Combiner,
        }
    }

    #[inline(always)]
    const fn pauli_kind(self, pauli: Pauli, id: QubitId) -> TargetKind {
        TargetKind::Pauli {
            pauli,
            id,
            inverted: self.inverted_flag(),
        }
    }

    #[inline(always)]
    fn pauli_class(self) -> Option<Pauli> {
        match self.class() {
            CLASS_PAULI_X => Some(Pauli::X),
            CLASS_PAULI_Y => Some(Pauli::Y),
            CLASS_PAULI_Z => Some(Pauli::Z),
            _ => None,
        }
    }

    /// Returns the qubit id carried by a qubit or Pauli target.
    #[inline]
    pub fn qubit_id(&self) -> Option<QubitId> {
        matches!(
            self.class(),
            CLASS_QUBIT | CLASS_PAULI_X | CLASS_PAULI_Y | CLASS_PAULI_Z
        )
        .then(|| QubitId::from_packed(self.value()))
    }

    /// Returns the measurement-record offset carried by this target.
    #[inline]
    pub fn measurement_record_offset(&self) -> Option<MeasureRecordOffset> {
        (self.class() == CLASS_RECORD)
            .then(|| MeasureRecordOffset::from_packed_distance(self.value()))
    }

    /// Returns the sweep-bit id carried by this target.
    #[inline]
    pub fn sweep_bit_id(&self) -> Option<u32> {
        (self.class() == CLASS_SWEEP).then_some(self.value())
    }

    /// Returns this target with its inversion flag toggled.
    pub fn try_inverted(&self) -> ModelResult<Self> {
        if self.qubit_id().is_some() {
            Ok(Self(self.0 ^ INVERTED_BIT))
        } else {
            Err(ModelError::invalid_domain_value("invertible target", self))
        }
    }

    /// Returns true when this target is a plain qubit target.
    #[inline]
    pub fn is_qubit_target(&self) -> bool {
        self.class() == CLASS_QUBIT
    }

    /// Returns true when this target is inverted with `!`.
    #[inline]
    pub fn is_inverted_result_target(&self) -> bool {
        self.inverted_flag()
    }

    /// Returns true when this target is a measurement-record target.
    #[inline]
    pub fn is_measurement_record_target(&self) -> bool {
        self.class() == CLASS_RECORD
    }

    /// Returns true when this target is a sweep-bit target.
    #[inline]
    pub fn is_sweep_bit_target(&self) -> bool {
        self.class() == CLASS_SWEEP
    }

    /// Returns true when this target is one of `X`, `Y`, or `Z`.
    #[inline]
    pub fn is_pauli_target(&self) -> bool {
        self.pauli_class().is_some()
    }

    /// Returns true when this target is an `X` Pauli target.
    #[inline]
    pub fn is_x_target(&self) -> bool {
        self.class() == CLASS_PAULI_X
    }

    /// Returns true when this target is a `Y` Pauli target.
    #[inline]
    pub fn is_y_target(&self) -> bool {
        self.class() == CLASS_PAULI_Y
    }

    /// Returns true when this target is a `Z` Pauli target.
    #[inline]
    pub fn is_z_target(&self) -> bool {
        self.class() == CLASS_PAULI_Z
    }

    /// Returns true when this target refers to a classical bit source.
    #[inline]
    pub fn is_classical_bit_target(&self) -> bool {
        matches!(self.class(), CLASS_RECORD | CLASS_SWEEP)
    }

    /// Returns true when this target is the `*` Pauli-product combiner.
    #[inline]
    pub fn is_combiner(&self) -> bool {
        self.class() == CLASS_COMBINER
    }

    /// Returns the Pauli type for Pauli targets.
    #[inline]
    pub fn pauli_type(&self) -> Option<Pauli> {
        self.pauli_class()
    }
}

impl std::fmt::Debug for Target {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.kind(), f)
    }
}

impl FromStr for Target {
    type Err = ModelError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        if raw.is_empty() {
            return Err(ModelError::invalid_domain_value("target", raw));
        }
        if raw == "*" {
            return Ok(Self::combiner());
        }
        if let Some(offset) = raw.strip_circumfix("rec[-", ']') {
            let offset = parse_u24(offset, "measurement record target")?;
            let offset = i32::try_from(offset)
                .map_err(|_| ModelError::invalid_domain_value("measurement record target", raw))?;
            return Ok(Self::measurement_record(
                MeasureRecordOffset::from_stim_text(-offset)?,
            ));
        }
        if let Some(index) = raw.strip_circumfix("sweep[", ']') {
            let id = parse_u24(index, "sweep target")?;
            return Self::sweep_bit(id);
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
        match self.kind() {
            TargetKind::Qubit { id, inverted } => {
                if inverted {
                    f.write_str("!")?;
                }
                write!(f, "{}", id.get())
            }
            TargetKind::MeasurementRecord { offset } => write!(f, "rec[{}]", offset.stim_text()),
            TargetKind::SweepBit { id } => write!(f, "sweep[{id}]"),
            TargetKind::Pauli {
                pauli,
                id,
                inverted,
            } => {
                if inverted {
                    f.write_str("!")?;
                }
                write!(f, "{pauli}{}", id.get())
            }
            TargetKind::Combiner => f.write_str("*"),
        }
    }
}

/// Parses one whitespace-free target token, handing each target to `push` in order.
///
/// `push` admits and retains a target; a token such as `X0*Y1` yields several.
pub(crate) fn parse_target_token_into(
    token: &str,
    push: &mut impl FnMut(Target) -> ModelResult<()>,
) -> ModelResult<()> {
    if token == "*" {
        return push(Target::combiner());
    }
    if !token.contains('*') {
        if token
            .as_bytes()
            .first()
            .is_some_and(|byte| byte.is_ascii_digit())
        {
            let id = parse_u24(token, "qubit target")?;
            return push(Target::qubit(QubitId::new(id)?, false));
        }
        return push(Target::from_str(token)?);
    }
    // Stim tokenizes '*' as a combiner wherever it appears, attached or spaced, and rejects
    // misplaced combiners at gate validation ("combiners that aren't between other targets"),
    // which validate_combiners mirrors; empty split parts therefore emit only the combiner.
    for (index, part) in token.split('*').enumerate() {
        if index > 0 {
            push(Target::combiner())?;
        }
        if !part.is_empty() {
            push(Target::from_str(part)?)?;
        }
    }
    Ok(())
}

/// Hands the targets of a plain qubit list such as `0 1 2` to `push`, returning `false` when
/// the text holds anything other than ASCII digits and target spacing (space, tab, or carriage
/// return).
pub(crate) fn parse_plain_qubit_targets_into(
    text: &str,
    push: &mut impl FnMut(Target) -> ModelResult<()>,
) -> ModelResult<bool> {
    let mut value = None;
    for byte in text.bytes() {
        if byte.is_ascii_digit() {
            let digit = u32::from(byte - b'0');
            let next = value
                .unwrap_or(0u32)
                .checked_mul(10)
                .and_then(|value| value.checked_add(digit))
                .ok_or_else(|| ModelError::invalid_domain_value("qubit target", text))?;
            if next >= STIM_TARGET_VALUE_LIMIT {
                return Err(ModelError::invalid_domain_value("qubit target", text));
            }
            value = Some(next);
        } else if matches!(byte, b' ' | b'\t' | b'\r') {
            if let Some(id) = value.take() {
                push(Target::qubit(QubitId::new(id)?, false))?;
            }
        } else {
            return Ok(false);
        }
    }
    if let Some(id) = value {
        push(Target::qubit(QubitId::new(id)?, false))?;
    }
    Ok(true)
}

/// Scratch values above this count are handed over instead of copied (see [`take_exact`]).
const SCRATCH_HANDOFF_VALUES: usize = 16 * 1024;

/// Moves the values parsed into a reusable `scratch` buffer into exact-size instruction storage.
///
/// Ordinary lines are copied with one exact-size allocation (none when empty), so the scratch
/// keeps its capacity for the next line. A very long line hands its scratch buffer over and
/// shrinks it instead, so its values are never resident twice; at that size the buffer is its
/// own mapping and shrinking it does not copy.
pub(crate) fn take_exact<T: Copy>(scratch: &mut Vec<T>) -> Vec<T> {
    if scratch.len() > SCRATCH_HANDOFF_VALUES {
        let mut values = std::mem::take(scratch);
        values.shrink_to_fit();
        values
    } else {
        scratch.as_slice().to_vec()
    }
}

/// Moves parsed targets into instruction storage: inline when they fit, otherwise exact-size
/// heap storage as described by [`take_exact`].
pub(crate) fn take_exact_targets(scratch: &mut Vec<Target>) -> TargetVec {
    if scratch.len() <= INLINE_TARGETS {
        return TargetVec::from_slice(scratch);
    }
    TargetVec::from_vec(take_exact(scratch))
}

fn parse_u24(text: &str, kind: &'static str) -> ModelResult<u32> {
    if text.is_empty() {
        return Err(ModelError::invalid_domain_value(kind, text));
    }
    let mut value = 0u32;
    for byte in text.bytes() {
        if !byte.is_ascii_digit() {
            return Err(ModelError::invalid_domain_value(kind, text));
        }
        let digit = u32::from(byte - b'0');
        value = value
            .checked_mul(10)
            .and_then(|value| value.checked_add(digit))
            .ok_or_else(|| ModelError::invalid_domain_value(kind, text))?;
        if value >= STIM_TARGET_VALUE_LIMIT {
            return Err(ModelError::invalid_domain_value(kind, text));
        }
    }
    Ok(value)
}
