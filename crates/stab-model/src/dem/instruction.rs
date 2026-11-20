use std::fmt::NumBuffer;
use std::io::{self, Write};

use super::tag::DemTag;
use super::target::{DetectorTarget, ObservableTarget};
use super::traversal::shifted_coordinates;
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

/// Borrowed, exhaustive view of one [`DemInstruction`], returned by [`DemInstruction::view`].
///
/// Each variant is one instruction kind and carries exactly the data valid for that kind, so a
/// `match` needs no fallible follow-up. The tag is shared by every kind and stays on
/// [`DemInstruction::tag`] and [`DemInstruction::tag_bytes`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DemInstructionView<'a> {
    /// `error(p) targets`: the probability and the target list, which may hold `^` separators
    /// between the components of a decomposed error.
    Error {
        probability: Probability,
        targets: &'a [DemTarget],
    },
    /// `detector(coordinates) D<detector>`, with the detector relative to the enclosing shifts.
    Detector {
        coordinates: &'a [f64],
        detector: DemDetectorId,
    },
    /// `logical_observable L<observable>`.
    LogicalObservable { observable: DemObservableId },
    /// `shift_detectors(coordinate_shift) detector_shift`.
    ShiftDetectors {
        coordinate_shift: &'a [f64],
        detector_shift: u64,
    },
}

impl DemInstructionView<'_> {
    pub const fn kind(&self) -> DemInstructionKind {
        match self {
            Self::Error { .. } => DemInstructionKind::Error,
            Self::Detector { .. } => DemInstructionKind::Detector,
            Self::LogicalObservable { .. } => DemInstructionKind::LogicalObservable,
            Self::ShiftDetectors { .. } => DemInstructionKind::ShiftDetectors,
        }
    }
}

/// The validated data of one instruction, stored per kind.
///
/// Each variant holds exactly what its kind declares: an `error` keeps its checked probability
/// beside its targets, `detector` and `shift_detectors` keep their finite coordinates, and
/// `Detector` and `LogicalObservable` hold targets that are a detector and an observable by
/// construction. No variant carries fields another kind uses, so reading one needs no fallible
/// follow-up.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum DemPayload {
    Error {
        probability: Probability,
        targets: DemTargetVec,
    },
    Detector {
        coordinates: DemArgVec,
        detector: DetectorTarget,
    },
    LogicalObservable {
        observable: ObservableTarget,
    },
    ShiftDetectors {
        coordinates: DemArgVec,
        shift: u64,
    },
}

impl DemPayload {
    pub(super) fn kind(&self) -> DemInstructionKind {
        match self {
            Self::Error { .. } => DemInstructionKind::Error,
            Self::Detector { .. } => DemInstructionKind::Detector,
            Self::LogicalObservable { .. } => DemInstructionKind::LogicalObservable,
            Self::ShiftDetectors { .. } => DemInstructionKind::ShiftDetectors,
        }
    }

    pub(super) fn detector(coordinates: DemArgVec, detector: DemDetectorId) -> Self {
        Self::Detector {
            coordinates,
            detector: DetectorTarget::new(detector),
        }
    }

    pub(super) fn logical_observable(observable: DemObservableId) -> Self {
        Self::LogicalObservable {
            observable: ObservableTarget::new(observable),
        }
    }

    fn args(&self) -> &[f64] {
        match self {
            Self::Error { probability, .. } => std::slice::from_ref(probability.as_f64_ref()),
            Self::Detector { coordinates, .. } | Self::ShiftDetectors { coordinates, .. } => {
                coordinates
            }
            Self::LogicalObservable { .. } => &[],
        }
    }

    fn targets(&self) -> &[DemTarget] {
        match self {
            Self::Error { targets, .. } => targets,
            Self::Detector { detector, .. } => std::slice::from_ref(detector.as_target()),
            Self::LogicalObservable { observable } => std::slice::from_ref(observable.as_target()),
            Self::ShiftDetectors { .. } => &[],
        }
    }

    /// Compares kind, targets, and detector shift exactly, ignoring every argument.
    fn same_except_args(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Error { targets: left, .. }, Self::Error { targets: right, .. }) => {
                left == right
            }
            (
                Self::Detector { detector: left, .. },
                Self::Detector {
                    detector: right, ..
                },
            ) => left == right,
            (
                Self::LogicalObservable { observable: left },
                Self::LogicalObservable { observable: right },
            ) => left == right,
            (
                Self::ShiftDetectors { shift: left, .. },
                Self::ShiftDetectors { shift: right, .. },
            ) => left == right,
            _ => false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DemInstruction {
    payload: DemPayload,
    pub(super) tag: Option<DemTag>,
}

impl DemInstruction {
    /// Assembles an instruction whose payload the caller already validated.
    pub(super) fn from_validated_parts(payload: DemPayload, tag: Option<DemTag>) -> Self {
        Self { payload, tag }
    }

    fn error_with_tag(
        probability: Probability,
        targets: DemTargetVec,
        tag: Option<DemTag>,
    ) -> ModelResult<Self> {
        validate_error_targets(&targets)?;
        Ok(Self::from_validated_parts(
            DemPayload::Error {
                probability,
                targets,
            },
            tag,
        ))
    }

    fn detector_with_tag(
        coordinates: Vec<f64>,
        detector: DemDetectorId,
        tag: Option<DemTag>,
    ) -> ModelResult<Self> {
        validate_coordinates(DemInstructionKind::Detector, &coordinates)?;
        Ok(Self::from_validated_parts(
            DemPayload::detector(DemArgVec::from_vec(coordinates), detector),
            tag,
        ))
    }

    fn shift_detectors_with_tag(
        coordinates: Vec<f64>,
        shift: u64,
        tag: Option<DemTag>,
    ) -> ModelResult<Self> {
        validate_coordinates(DemInstructionKind::ShiftDetectors, &coordinates)?;
        Ok(Self::from_validated_parts(
            DemPayload::ShiftDetectors {
                coordinates: DemArgVec::from_vec(coordinates),
                shift,
            },
            tag,
        ))
    }

    pub fn error(
        probability: Probability,
        targets: Vec<DemTarget>,
        tag: Option<String>,
    ) -> ModelResult<Self> {
        Self::error_with_tag(
            probability,
            DemTargetVec::from_vec(targets),
            normalize_tag(tag),
        )
    }

    pub fn detector(
        coordinates: Vec<f64>,
        detector: DemDetectorId,
        tag: Option<String>,
    ) -> ModelResult<Self> {
        Self::detector_with_tag(coordinates, detector, normalize_tag(tag))
    }

    pub fn logical_observable(observable: DemObservableId, tag: Option<String>) -> Self {
        Self::from_validated_parts(
            DemPayload::logical_observable(observable),
            normalize_tag(tag),
        )
    }

    pub fn shift_detectors(
        coordinates: Vec<f64>,
        detector_shift: u64,
        tag: Option<String>,
    ) -> ModelResult<Self> {
        Self::shift_detectors_with_tag(coordinates, detector_shift, normalize_tag(tag))
    }

    /// Builds a validated error instruction from a borrowed target slice.
    pub(crate) fn error_from_targets(
        probability: Probability,
        targets: &[DemTarget],
        tag: Option<&[u8]>,
    ) -> ModelResult<Self> {
        Self::error_with_tag(
            probability,
            DemTargetVec::from_slice(targets),
            tag.and_then(DemTag::from_slice),
        )
    }

    pub(crate) fn detector_with_tag_bytes(
        coordinates: Vec<f64>,
        detector: DemDetectorId,
        tag: Option<&[u8]>,
    ) -> ModelResult<Self> {
        Self::detector_with_tag(coordinates, detector, tag.and_then(DemTag::from_slice))
    }

    pub(crate) fn logical_observable_with_tag_bytes(
        observable: DemObservableId,
        tag: Option<&[u8]>,
    ) -> Self {
        Self::from_validated_parts(
            DemPayload::logical_observable(observable),
            tag.and_then(DemTag::from_slice),
        )
    }

    pub(crate) fn shift_detectors_with_tag_bytes(
        coordinates: Vec<f64>,
        detector_shift: u64,
        tag: Option<&[u8]>,
    ) -> ModelResult<Self> {
        Self::shift_detectors_with_tag(
            coordinates,
            detector_shift,
            tag.and_then(DemTag::from_slice),
        )
    }

    /// Returns a copy with every relative detector raised by `detector_offset` and, for a
    /// `detector`, its coordinates shifted by `coordinate_shift`; the tag is kept.
    pub(super) fn relocated(
        &self,
        detector_offset: u64,
        coordinate_shift: &[f64],
    ) -> ModelResult<Self> {
        let payload = match &self.payload {
            DemPayload::Error {
                probability,
                targets,
            } => {
                let mut shifted = DemTargetVec::new();
                shifted.try_reserve_exact(targets.len()).map_err(|_| {
                    ModelError::invalid_detector_error_model("target allocation failed")
                })?;
                for target in targets {
                    shifted.push(target.shifted(detector_offset)?);
                }
                DemPayload::Error {
                    probability: *probability,
                    targets: shifted,
                }
            }
            DemPayload::Detector {
                coordinates,
                detector,
            } => {
                let coordinates = shifted_coordinates(coordinates, coordinate_shift)?;
                validate_coordinates(DemInstructionKind::Detector, &coordinates)?;
                DemPayload::Detector {
                    coordinates: DemArgVec::from_vec(coordinates),
                    detector: detector.shifted(detector_offset)?,
                }
            }
            DemPayload::LogicalObservable { observable } => DemPayload::LogicalObservable {
                observable: *observable,
            },
            DemPayload::ShiftDetectors { coordinates, shift } => {
                let mut copied = DemArgVec::new();
                copied.try_reserve_exact(coordinates.len()).map_err(|_| {
                    ModelError::invalid_detector_error_model(
                        "flattened instruction argument allocation failed",
                    )
                })?;
                copied.extend_from_slice(coordinates);
                DemPayload::ShiftDetectors {
                    coordinates: copied,
                    shift: *shift,
                }
            }
        };
        Ok(Self::from_validated_parts(payload, self.tag.clone()))
    }

    pub fn kind(&self) -> DemInstructionKind {
        self.payload.kind()
    }

    /// Returns the matchable view of this instruction: its kind together with exactly the data
    /// that kind holds.
    pub fn view(&self) -> DemInstructionView<'_> {
        match &self.payload {
            DemPayload::Error {
                probability,
                targets,
            } => DemInstructionView::Error {
                probability: *probability,
                targets,
            },
            DemPayload::Detector {
                coordinates,
                detector,
            } => DemInstructionView::Detector {
                coordinates,
                detector: detector.id(),
            },
            DemPayload::LogicalObservable { observable } => DemInstructionView::LogicalObservable {
                observable: observable.id(),
            },
            DemPayload::ShiftDetectors { coordinates, shift } => {
                DemInstructionView::ShiftDetectors {
                    coordinate_shift: coordinates,
                    detector_shift: *shift,
                }
            }
        }
    }

    /// Returns the parenthesized arguments: the one probability of an `error`, the coordinates of
    /// a `detector`, the coordinate shift of a `shift_detectors`, and nothing for a
    /// `logical_observable`.
    pub fn args(&self) -> &[f64] {
        self.payload.args()
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
            DemPayload::ShiftDetectors { shift, .. } => Some(shift),
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
        self.payload.same_except_args(&other.payload) && self.tag == other.tag
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
        match &mut self.payload {
            DemPayload::Error { targets, .. } => {
                for target in targets.iter_mut() {
                    if let Some(lowered) = target.detector().and_then(|id| id.checked_sub(base)) {
                        *target = DemTarget::from_detector(lowered);
                    }
                }
            }
            DemPayload::Detector { detector, .. } => {
                if let Some(lowered) = detector.id().checked_sub(base) {
                    *detector = DetectorTarget::new(lowered);
                }
            }
            DemPayload::LogicalObservable { .. } | DemPayload::ShiftDetectors { .. } => {}
        }
        Ok(())
    }

    pub(super) fn write_dem(&self, out: &mut String, indent: usize, floats: &mut DemFloatCache) {
        write_indent(out, indent);
        out.push_str(self.kind().canonical_name());
        write_optional_tag(out, self.tag.as_ref());
        write_args(out, self.args(), floats);
        let mut digits = NumBuffer::new();
        if let DemPayload::ShiftDetectors { shift, .. } = self.payload {
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
        write_args_io(out, self.args(), floats)?;
        let mut digits = NumBuffer::new();
        if let DemPayload::ShiftDetectors { shift, .. } = self.payload {
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

/// Checks the arguments of an `error`: exactly one probability.
pub(super) fn error_probability(args: &[f64]) -> ModelResult<Probability> {
    let [probability] = args else {
        return Err(ModelError::invalid_detector_error_model(
            "error instructions require exactly one probability argument",
        ));
    };
    Probability::try_new(*probability)
}

/// Checks the coordinates of a `detector` or `shift_detectors`: every one is finite.
pub(super) fn validate_coordinates(kind: DemInstructionKind, args: &[f64]) -> ModelResult<()> {
    for arg in args {
        if !arg.is_finite() {
            return Err(ModelError::invalid_detector_error_model(format!(
                "{} argument {arg} is not finite",
                kind.canonical_name()
            )));
        }
    }
    Ok(())
}

/// Checks the arguments of a `logical_observable`: there are none.
pub(super) fn validate_no_args(args: &[f64]) -> ModelResult<()> {
    if args.is_empty() {
        Ok(())
    } else {
        Err(ModelError::invalid_detector_error_model(
            "logical_observable instructions do not take arguments",
        ))
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
