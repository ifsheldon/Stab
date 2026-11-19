use std::fmt::NumBuffer;
use std::io::{self, Write};

use super::tag::DemTag;
use super::{
    DemArgVec, DemDetectorId, DemObservableId, DemTarget, DemTargetVec, normalize_tag,
    write_indent, write_indent_io, write_optional_tag, write_optional_tag_io,
};
use crate::circuit::printing::DemFloatCache;
use crate::{ModelError, ModelResult, Probability};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DemInstructionKind {
    Error,
    Detector,
    LogicalObservable,
    ShiftDetectors,
}

impl DemInstructionKind {
    fn canonical_name(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Detector => "detector",
            Self::LogicalObservable => "logical_observable",
            Self::ShiftDetectors => "shift_detectors",
        }
    }

    pub(super) fn lookup_name(name: &str) -> Option<Self> {
        match name.len() {
            5 if name.eq_ignore_ascii_case("error") => Some(Self::Error),
            8 if name.eq_ignore_ascii_case("detector") => Some(Self::Detector),
            18 if name.eq_ignore_ascii_case("logical_observable") => Some(Self::LogicalObservable),
            15 if name.eq_ignore_ascii_case("shift_detectors") => Some(Self::ShiftDetectors),
            _ => None,
        }
    }
}

/// What an instruction applies to: the targets of an `error`, the single detector or observable
/// it declares, or the detector offset of a `shift_detectors`.
///
/// `Detector` always holds a relative detector target and `LogicalObservable` always holds a
/// logical observable target; every constructor builds them from typed ids.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum DemPayload {
    Error(DemTargetVec),
    Detector(DemTarget),
    LogicalObservable(DemTarget),
    ShiftDetectors(u64),
}

impl DemPayload {
    pub(super) fn kind(&self) -> DemInstructionKind {
        match self {
            Self::Error(_) => DemInstructionKind::Error,
            Self::Detector(_) => DemInstructionKind::Detector,
            Self::LogicalObservable(_) => DemInstructionKind::LogicalObservable,
            Self::ShiftDetectors(_) => DemInstructionKind::ShiftDetectors,
        }
    }

    pub(super) fn detector(detector: DemDetectorId) -> Self {
        Self::Detector(DemTarget::from_detector(detector))
    }

    pub(super) fn logical_observable(observable: DemObservableId) -> Self {
        Self::LogicalObservable(DemTarget::from_observable(observable))
    }

    fn targets(&self) -> &[DemTarget] {
        match self {
            Self::Error(targets) => targets,
            Self::Detector(target) | Self::LogicalObservable(target) => {
                std::slice::from_ref(target)
            }
            Self::ShiftDetectors(_) => &[],
        }
    }

    fn targets_mut(&mut self) -> &mut [DemTarget] {
        match self {
            Self::Error(targets) => targets,
            Self::Detector(target) | Self::LogicalObservable(target) => {
                std::slice::from_mut(target)
            }
            Self::ShiftDetectors(_) => &mut [],
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DemInstruction {
    args: DemArgVec,
    payload: DemPayload,
    pub(super) tag: Option<DemTag>,
}

impl DemInstruction {
    fn from_parts(args: DemArgVec, payload: DemPayload, tag: Option<DemTag>) -> ModelResult<Self> {
        validate_dem_args(payload.kind(), &args)?;
        if let DemPayload::Error(targets) = &payload {
            validate_error_targets(targets)?;
        }
        Ok(Self::from_validated_parts(args, payload, tag))
    }

    /// Assembles an instruction whose arguments and payload the caller already validated.
    pub(super) fn from_validated_parts(
        args: DemArgVec,
        payload: DemPayload,
        tag: Option<DemTag>,
    ) -> Self {
        Self { args, payload, tag }
    }

    pub fn error(
        probability: Probability,
        targets: Vec<DemTarget>,
        tag: Option<String>,
    ) -> ModelResult<Self> {
        Self::from_parts(
            probability_args(probability),
            DemPayload::Error(DemTargetVec::from_vec(targets)),
            normalize_tag(tag),
        )
    }

    pub fn detector(
        coordinates: Vec<f64>,
        detector: DemDetectorId,
        tag: Option<String>,
    ) -> ModelResult<Self> {
        Self::from_parts(
            DemArgVec::from_vec(coordinates),
            DemPayload::detector(detector),
            normalize_tag(tag),
        )
    }

    pub fn logical_observable(observable: DemObservableId, tag: Option<String>) -> Self {
        Self::from_validated_parts(
            DemArgVec::new(),
            DemPayload::logical_observable(observable),
            normalize_tag(tag),
        )
    }

    pub fn shift_detectors(
        coordinates: Vec<f64>,
        detector_shift: u64,
        tag: Option<String>,
    ) -> ModelResult<Self> {
        Self::from_parts(
            DemArgVec::from_vec(coordinates),
            DemPayload::ShiftDetectors(detector_shift),
            normalize_tag(tag),
        )
    }

    /// Builds a validated error instruction from a borrowed target slice.
    pub(crate) fn error_from_targets(
        probability: Probability,
        targets: &[DemTarget],
        tag: Option<&[u8]>,
    ) -> ModelResult<Self> {
        Self::from_parts(
            probability_args(probability),
            DemPayload::Error(DemTargetVec::from_slice(targets)),
            tag.and_then(DemTag::from_slice),
        )
    }

    pub(crate) fn detector_with_tag_bytes(
        coordinates: Vec<f64>,
        detector: DemDetectorId,
        tag: Option<&[u8]>,
    ) -> ModelResult<Self> {
        Self::from_parts(
            DemArgVec::from_vec(coordinates),
            DemPayload::detector(detector),
            tag.and_then(DemTag::from_slice),
        )
    }

    pub(crate) fn logical_observable_with_tag_bytes(
        observable: DemObservableId,
        tag: Option<&[u8]>,
    ) -> Self {
        Self::from_validated_parts(
            DemArgVec::new(),
            DemPayload::logical_observable(observable),
            tag.and_then(DemTag::from_slice),
        )
    }

    pub(crate) fn shift_detectors_with_tag_bytes(
        coordinates: Vec<f64>,
        detector_shift: u64,
        tag: Option<&[u8]>,
    ) -> ModelResult<Self> {
        Self::from_parts(
            DemArgVec::from_vec(coordinates),
            DemPayload::ShiftDetectors(detector_shift),
            tag.and_then(DemTag::from_slice),
        )
    }

    /// Returns a copy with every relative detector raised by `detector_offset` and, for a
    /// `detector`, its coordinates replaced by `args`; the tag is kept.
    pub(super) fn relocated(&self, args: Vec<f64>, detector_offset: u64) -> ModelResult<Self> {
        let payload = match &self.payload {
            DemPayload::Error(targets) => {
                let mut shifted = DemTargetVec::new();
                shifted.try_reserve_exact(targets.len()).map_err(|_| {
                    ModelError::invalid_detector_error_model("target allocation failed")
                })?;
                for target in targets {
                    shifted.push(target.shifted(detector_offset)?);
                }
                DemPayload::Error(shifted)
            }
            DemPayload::Detector(target) => DemPayload::Detector(target.shifted(detector_offset)?),
            DemPayload::LogicalObservable(_) | DemPayload::ShiftDetectors(_) => {
                self.payload.clone()
            }
        };
        Self::from_parts(DemArgVec::from_vec(args), payload, self.tag.clone())
    }

    pub fn kind(&self) -> DemInstructionKind {
        self.payload.kind()
    }

    pub fn args(&self) -> &[f64] {
        &self.args
    }

    /// Returns the targets of an `error`, or the single target a `detector` or
    /// `logical_observable` declares. A `shift_detectors` has no targets; its offset is
    /// [`Self::detector_shift`].
    pub fn targets(&self) -> &[DemTarget] {
        self.payload.targets()
    }

    pub fn target_groups(&self) -> Vec<&[DemTarget]> {
        self.targets()
            .split(|target| target.is_separator())
            .collect()
    }

    /// Returns the detector offset of a `shift_detectors` instruction, and `None` for every other
    /// instruction kind.
    pub fn detector_shift(&self) -> Option<u64> {
        match self.payload {
            DemPayload::ShiftDetectors(shift) => Some(shift),
            _ => None,
        }
    }

    /// Returns this instruction's tag as UTF-8 display text.
    ///
    /// Opaque bytes are represented with the UTF-8 replacement character. Use
    /// [`Self::tag_bytes`] when exact metadata preservation matters.
    pub fn tag(&self) -> Option<&str> {
        self.tag.as_ref().map(DemTag::as_str)
    }

    /// Returns the exact unescaped bytes of this instruction's optional DEM tag.
    pub fn tag_bytes(&self) -> Option<&[u8]> {
        self.tag.as_ref().map(DemTag::as_bytes)
    }

    /// Compares kind, targets, detector shift, and tag exactly, leaving arguments to the caller.
    pub(super) fn same_except_args(&self, other: &Self) -> bool {
        self.payload == other.payload && self.tag == other.tag
    }

    /// Removes this instruction's tag without changing its validated semantic fields.
    ///
    /// This is a crate-private model primitive used by analysis transforms. Recursive tag
    /// stripping remains owned by the analysis layer.
    pub(crate) fn clear_tag(&mut self) {
        self.tag = None;
    }

    /// Subtracts `base` from every relative detector target in place.
    ///
    /// Fails with the first detector id below `base` and leaves the instruction unchanged.
    /// Lowering detector ids keeps every validated instruction shape valid.
    pub(crate) fn rebase_detectors(&mut self, base: u64) -> Result<(), u64> {
        if let Some(detector) = self
            .targets()
            .iter()
            .filter_map(|target| target.detector())
            .find(|detector| detector.get() < base)
        {
            return Err(detector.get());
        }
        for target in self.payload.targets_mut() {
            if let Some(lowered) = target.detector().and_then(|id| id.checked_sub(base)) {
                *target = DemTarget::from_detector(lowered);
            }
        }
        Ok(())
    }

    pub(super) fn write_dem(&self, out: &mut String, indent: usize, floats: &mut DemFloatCache) {
        write_indent(out, indent);
        out.push_str(self.kind().canonical_name());
        write_optional_tag(out, self.tag.as_ref());
        write_args(out, &self.args, floats);
        let mut digits = NumBuffer::new();
        if let DemPayload::ShiftDetectors(shift) = self.payload {
            out.push(' ');
            out.push_str(shift.format_into(&mut digits));
        }
        for target in self.targets() {
            let (prefix, number) = target.text_parts(&mut digits);
            out.push(' ');
            out.push_str(prefix);
            out.push_str(number);
        }
        out.push('\n');
    }

    pub(super) fn write_dem_io<W: Write>(
        &self,
        out: &mut W,
        indent: usize,
        floats: &mut DemFloatCache,
    ) -> io::Result<()> {
        write_indent_io(out, indent)?;
        out.write_all(self.kind().canonical_name().as_bytes())?;
        write_optional_tag_io(out, self.tag.as_ref())?;
        write_args_io(out, &self.args, floats)?;
        let mut digits = NumBuffer::new();
        if let DemPayload::ShiftDetectors(shift) = self.payload {
            out.write_all(b" ")?;
            out.write_all(shift.format_into(&mut digits).as_bytes())?;
        }
        for target in self.targets() {
            let (prefix, number) = target.text_parts(&mut digits);
            out.write_all(b" ")?;
            out.write_all(prefix.as_bytes())?;
            out.write_all(number.as_bytes())?;
        }
        out.write_all(b"\n")
    }
}

fn probability_args(probability: Probability) -> DemArgVec {
    let mut args = DemArgVec::new();
    args.push(probability.get());
    args
}

/// Checks the arguments an instruction of `kind` takes.
pub(super) fn validate_dem_args(kind: DemInstructionKind, args: &[f64]) -> ModelResult<()> {
    match kind {
        DemInstructionKind::Error => {
            let [probability] = args else {
                return Err(ModelError::invalid_detector_error_model(
                    "error instructions require exactly one probability argument",
                ));
            };
            Probability::try_new(*probability).map(|_| ())
        }
        DemInstructionKind::Detector => validate_finite_args("detector", args),
        DemInstructionKind::LogicalObservable => {
            if args.is_empty() {
                Ok(())
            } else {
                Err(ModelError::invalid_detector_error_model(
                    "logical_observable instructions do not take arguments",
                ))
            }
        }
        DemInstructionKind::ShiftDetectors => validate_finite_args("shift_detectors", args),
    }
}

/// Checks the separator placement of a complete `error` target list.
pub(super) fn validate_error_targets(targets: &[DemTarget]) -> ModelResult<()> {
    validate_error_separator_prefix(targets)?;
    if targets.last().is_some_and(|target| target.is_separator()) {
        return Err(ModelError::invalid_detector_error_model(
            "error target separators cannot be last",
        ));
    }
    Ok(())
}

/// Checks that no separator in `targets` comes first or right after another separator.
pub(super) fn validate_error_separator_prefix(targets: &[DemTarget]) -> ModelResult<()> {
    let mut previous_was_separator = true;
    for target in targets {
        if target.is_separator() {
            if previous_was_separator {
                return Err(ModelError::invalid_detector_error_model(
                    "error target separators cannot be first or consecutive",
                ));
            }
            previous_was_separator = true;
        } else {
            previous_was_separator = false;
        }
    }
    Ok(())
}

fn validate_finite_args(kind: &'static str, args: &[f64]) -> ModelResult<()> {
    for arg in args {
        if !arg.is_finite() {
            return Err(ModelError::invalid_detector_error_model(format!(
                "{kind} argument {arg} is not finite"
            )));
        }
    }
    Ok(())
}

fn write_args(out: &mut String, args: &[f64], floats: &mut DemFloatCache) {
    if args.is_empty() {
        return;
    }
    out.push('(');
    for (index, arg) in args.iter().enumerate() {
        if index > 0 {
            out.push_str(", ");
        }
        // The cache returns the ASCII text of `format_dem_float`.
        out.extend(floats.format(*arg).iter().copied().map(char::from));
    }
    out.push(')');
}

fn write_args_io<W: Write>(
    out: &mut W,
    args: &[f64],
    floats: &mut DemFloatCache,
) -> io::Result<()> {
    if args.is_empty() {
        return Ok(());
    }
    out.write_all(b"(")?;
    for (index, arg) in args.iter().enumerate() {
        if index > 0 {
            out.write_all(b", ")?;
        }
        out.write_all(floats.format(*arg))?;
    }
    out.write_all(b")")
}
