use std::fmt::{Display, Formatter, NumBuffer};
use std::io::{self, Write};
use std::str::FromStr;

use smallvec::SmallVec;

mod api;
mod coordinate_scan;
mod drop_impl;
mod equality;
mod error_mechanisms;
mod parser;
mod printing;
mod tag;
mod traversal;

pub use api::DemFlattenedInstructionIter;
pub use error_mechanisms::{
    DemErrorMechanismTraversalLimits, DemErrorMechanismView, DemErrorMechanismVisitError,
    DemErrorMechanismVisitor, DemErrorTarget, DemErrorTargetIter,
};

/// Advanced borrowed boundary for traversing compact DEM structure.
///
/// This boundary stays owned by the DEM model: analysis and execution consumers may inspect the
/// checked folded tree or implement their visitor policy, but it is not part of Stab's stable
/// public API. Consumers must not replace it with independent repeat expansion.
pub(crate) mod advanced {
    pub use super::traversal::{
        DemBlockSummary, DemRepeatSelection, DemTraversalState, FoldedDemBlock, FoldedDemItem,
        FoldedDemTraversal, FoldedDemVisitor, shifted_coordinates, shifted_detector,
        shifted_targets,
    };
}

use traversal::FoldedDemTraversal;

use crate::circuit::printing::DemFloatCache;
use crate::model_bytes::{PreparedModelText, RecoveredTags, locate_recovered_tag};
use crate::model_tag::RecoveredTag;
use crate::parse_limits::ParseAdmission;
use crate::{
    AbsoluteTolerance, DemRepeatCount, ModelDialect, ModelError, ModelFingerprint, ModelResult,
    ParseLimits, Probability, RepeatNestingLimit,
};
use parser::{parse_dem, parse_dem_admitted, parse_unsigned_dem_text_value};
use tag::DemTag;
type DemArgVec = SmallVec<[f64; 3]>;
type DemTargetVec = SmallVec<[DemTarget; 5]>;

const MAX_DEM_DETECTOR_ID: u64 = (1_u64 << 62) - 1;
pub const MAX_DEM_REPEAT_NESTING: usize = RepeatNestingLimit::HARD_MAX;

#[derive(Default)]
pub struct DetectorErrorModel {
    items: Vec<DemItem>,
}

impl DetectorErrorModel {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn from_dem_str(input: &str) -> ModelResult<Self> {
        Self::from_dem_str_with_limits(input, ParseLimits::default())
    }

    pub fn from_dem_bytes(input: &[u8]) -> ModelResult<Self> {
        Self::from_dem_bytes_with_limits(input, ParseLimits::default())
    }

    pub fn from_dem_bytes_with_limits(input: &[u8], limits: ParseLimits) -> ModelResult<Self> {
        Self::from_dem_bytes_admitted(input, limits).map(|(model, _)| model)
    }

    /// Parses like [`Self::from_dem_bytes_with_limits`] and also returns the retained model bytes
    /// the parse admitted against `limits`, excluding the `input` bytes it also charged.
    ///
    /// A caller that drops `input` after parsing holds about this many bytes for the model.
    pub fn from_dem_bytes_admitted(input: &[u8], limits: ParseLimits) -> ModelResult<(Self, u64)> {
        let prepared = PreparedModelText::new(input, ModelDialect::DetectorErrorModel, limits)?;
        let parsed = prepared.resolve(parse_dem_admitted(prepared.text(), prepared.admission()));
        let (mut model, mut admission) = parsed?;
        let Some(tags) = prepared.into_tags() else {
            return Ok((model, admission.retained_model_bytes()));
        };
        // Taking the tags freed the sanitized text copy.
        admission.release_sanitized_copy(input.len());
        model.restore_byte_tags(tags, &mut admission, input)?;
        Ok((model, admission.retained_model_bytes()))
    }

    pub fn from_dem_str_with_limits(input: &str, limits: ParseLimits) -> ModelResult<Self> {
        parse_dem(input, limits)
    }

    /// Conservative retained bytes of this model under [`ParseLimits`] item, target, argument, and
    /// tag accounting, including the display text of tags restored from non-UTF-8 bytes.
    pub fn admitted_bytes(&self) -> u64 {
        let mut total = 0_u64;
        let mut stack = vec![self.items.iter()];
        while let Some(items) = stack.last_mut() {
            let Some(item) = items.next() else {
                stack.pop();
                continue;
            };
            match item {
                DemItem::Instruction(instruction) => {
                    total = total.saturating_add(
                        crate::parse_limits::model_declaration_admitted_bytes(
                            ModelDialect::DetectorErrorModel,
                            instruction.targets().len(),
                            instruction.args().len(),
                            instruction.tag_bytes().map_or(0, <[u8]>::len),
                        ),
                    );
                    total = total.saturating_add(opaque_overhead(instruction.tag.as_ref()));
                }
                DemItem::RepeatBlock(repeat) => {
                    total = total.saturating_add(
                        crate::parse_limits::model_declaration_admitted_bytes(
                            ModelDialect::DetectorErrorModel,
                            0,
                            0,
                            repeat.tag_bytes().map_or(0, <[u8]>::len),
                        ),
                    );
                    total = total.saturating_add(opaque_overhead(repeat.tag.as_ref()));
                    stack.push(repeat.body().items().iter());
                }
            }
        }
        total
    }

    /// Returns the schema-versioned structural identity of this detector error model.
    ///
    /// The fingerprint is independent of accepted textual spelling and printer precision. It
    /// identifies this source model only, not an analysis policy or executable plan.
    pub fn fingerprint(&self) -> ModelFingerprint {
        ModelFingerprint::for_dem(self)
    }

    /// Checks structural equality while allowing corresponding numeric arguments to differ by at
    /// most `tolerance`.
    ///
    /// Instruction order, kinds, targets, tags, repeat counts, and repeat structure remain exact.
    #[must_use]
    pub fn approx_equals(&self, other: &Self, tolerance: AbsoluteTolerance) -> bool {
        equality::models_approx_equal(self, other, tolerance)
    }

    fn from_items(items: Vec<DemItem>) -> Self {
        Self { items }
    }

    pub fn items(&self) -> &[DemItem] {
        &self.items
    }

    pub fn push_instruction(&mut self, instruction: DemInstruction) {
        self.items.push(DemItem::Instruction(instruction));
    }

    pub fn push_repeat_block(&mut self, repeat: DemRepeatBlock) {
        self.items.push(DemItem::RepeatBlock(repeat));
    }

    pub(crate) fn try_reserve_items_exact(&mut self, additional: usize) -> ModelResult<()> {
        self.items.try_reserve_exact(additional).map_err(|error| {
            ModelError::invalid_detector_error_model(format!(
                "unable to reserve {additional} flattened instruction slots: {error}"
            ))
        })
    }

    /// Returns canonical DEM text as UTF-8.
    ///
    /// Opaque tag bytes are represented with the UTF-8 replacement character. Use
    /// [`Self::to_dem_bytes`] when exact metadata preservation matters.
    pub fn to_dem_string(&self) -> String {
        let mut out = String::new();
        self.write_dem(&mut out, &mut DemFloatCache::new());
        out
    }

    /// Returns canonical DEM text while preserving opaque bytes in tags.
    ///
    /// Use this method instead of [`Self::to_dem_string`] when the model came from a byte source
    /// whose tags may not be valid UTF-8.
    pub fn to_dem_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        match self.write_dem_bytes(&mut out) {
            Ok(()) => out,
            Err(error) => unreachable!("writing a DEM into Vec<u8> failed: {error}"),
        }
    }

    /// Streams the canonical DEM text of [`Self::to_dem_bytes`] into `out`.
    ///
    /// Opaque tag bytes are preserved exactly. The writer receives many small writes, so wrap
    /// unbuffered sinks such as files in [`std::io::BufWriter`].
    ///
    /// # Errors
    ///
    /// Returns the first error reported by `out`.
    pub fn write_dem_bytes<W: Write>(&self, out: &mut W) -> io::Result<()> {
        self.write_dem_io(out, &mut DemFloatCache::new())
    }

    pub fn total_detector_shift(&self) -> ModelResult<u64> {
        FoldedDemTraversal::new(self)?
            .root()
            .summary()
            .detector_shift()
    }

    pub fn count_detectors(&self) -> ModelResult<u64> {
        FoldedDemTraversal::new(self)?
            .root()
            .summary()
            .detector_count()
    }

    pub fn count_observables(&self) -> ModelResult<u64> {
        Ok(FoldedDemTraversal::new(self)?
            .root()
            .summary()
            .observable_count())
    }

    fn write_dem(&self, out: &mut String, floats: &mut DemFloatCache) {
        let walked = printing::walk_dem_text::<std::convert::Infallible>(self, |event| {
            match event {
                printing::DemTextEvent::Instruction(instruction, indent) => {
                    instruction.write_dem(out, indent, floats);
                }
                printing::DemTextEvent::RepeatOpen(repeat, indent) => {
                    repeat.write_dem_open(out, indent);
                }
                printing::DemTextEvent::RepeatClose(indent) => {
                    write_indent(out, indent);
                    out.push_str("}\n");
                }
            }
            Ok(())
        });
        if let Err(never) = walked {
            match never {}
        }
    }

    fn write_dem_io<W: Write>(&self, out: &mut W, floats: &mut DemFloatCache) -> io::Result<()> {
        printing::walk_dem_text(self, |event| match event {
            printing::DemTextEvent::Instruction(instruction, indent) => {
                instruction.write_dem_io(out, indent, floats)
            }
            printing::DemTextEvent::RepeatOpen(repeat, indent) => {
                repeat.write_dem_io_open(out, indent)
            }
            printing::DemTextEvent::RepeatClose(indent) => {
                write_indent_io(out, indent)?;
                out.write_all(b"}\n")
            }
        })
    }

    /// Replaces the sanitized tags the parser produced with the exact recovered bytes, in
    /// source order, charging what an opaque tag retains beyond its parsed text before it is
    /// built.
    fn restore_byte_tags(
        &mut self,
        tags: RecoveredTags,
        admission: &mut ParseAdmission,
        input: &[u8],
    ) -> ModelResult<()> {
        let expected = self.non_empty_tag_count();
        if expected != tags.len() {
            return Err(ModelError::invalid_domain_value(
                "DEM byte parser",
                format!(
                    "metadata scanner found {} tags but parser produced {expected}",
                    tags.len()
                ),
            ));
        }
        let mut tags = tags.into_owned_tags().enumerate();
        let mut restore = |tag: &mut Option<DemTag>| -> ModelResult<()> {
            if tag.is_none() {
                return Ok(());
            }
            let Some((index, bytes)) = tags.next() else {
                return Err(ModelError::invalid_domain_value(
                    "DEM byte parser",
                    "metadata scanner ran out of tags",
                ));
            };
            let recovered = RecoveredTag::new(bytes);
            let overhead = DemTag::opaque_overhead_bytes_of(&recovered);
            admission.admit_opaque_tag_overhead(overhead, || locate_recovered_tag(input, index))?;
            *tag = DemTag::from_recovered(recovered);
            Ok(())
        };
        let mut pending = vec![self.items.iter_mut()];
        while let Some(items) = pending.last_mut() {
            let Some(item) = items.next() else {
                pending.pop();
                continue;
            };
            match item {
                DemItem::Instruction(instruction) => restore(&mut instruction.tag)?,
                DemItem::RepeatBlock(repeat) => {
                    restore(&mut repeat.tag)?;
                    pending.push(repeat.body.items.iter_mut());
                }
            }
        }
        Ok(())
    }

    fn non_empty_tag_count(&self) -> usize {
        let mut count = 0_usize;
        let mut pending = vec![self.items.as_slice()];
        while let Some(items) = pending.pop() {
            for item in items {
                match item {
                    DemItem::Instruction(instruction) => {
                        count += usize::from(instruction.tag.is_some());
                    }
                    DemItem::RepeatBlock(repeat) => {
                        count += usize::from(repeat.tag.is_some());
                        pending.push(repeat.body.items.as_slice());
                    }
                }
            }
        }
        count
    }
}

impl Display for DetectorErrorModel {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_dem_string())
    }
}

impl Clone for DetectorErrorModel {
    fn clone(&self) -> Self {
        drop_impl::clone_model(self)
    }
}

impl std::fmt::Debug for DetectorErrorModel {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DetectorErrorModel")
            .field("top_level_items", &self.items.len())
            .finish_non_exhaustive()
    }
}

impl PartialEq for DetectorErrorModel {
    fn eq(&self, other: &Self) -> bool {
        equality::models_equal(self, other)
    }
}

impl Drop for DetectorErrorModel {
    fn drop(&mut self) {
        drop_impl::drop_items(&mut self.items);
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum DemItem {
    Instruction(DemInstruction),
    RepeatBlock(DemRepeatBlock),
}

#[derive(Clone, Debug, PartialEq)]
pub struct DemRepeatBlock {
    repeat_count: DemRepeatCount,
    body: DetectorErrorModel,
    tag: Option<DemTag>,
}

impl DemRepeatBlock {
    pub fn new(
        repeat_count: DemRepeatCount,
        body: DetectorErrorModel,
        tag: Option<String>,
    ) -> Self {
        Self::from_parts(repeat_count, body, normalize_tag(tag))
    }

    pub(crate) fn new_with_tag_bytes(
        repeat_count: DemRepeatCount,
        body: DetectorErrorModel,
        tag: Option<&[u8]>,
    ) -> Self {
        Self::from_parts(repeat_count, body, tag.and_then(DemTag::from_slice))
    }

    fn from_parts(
        repeat_count: DemRepeatCount,
        body: DetectorErrorModel,
        tag: Option<DemTag>,
    ) -> Self {
        Self {
            repeat_count,
            body,
            tag,
        }
    }

    pub fn repeat_count(&self) -> DemRepeatCount {
        self.repeat_count
    }

    pub fn body(&self) -> &DetectorErrorModel {
        &self.body
    }

    /// Returns this repeat block's tag as UTF-8 display text.
    ///
    /// Opaque bytes are represented with the UTF-8 replacement character. Use
    /// [`Self::tag_bytes`] when exact metadata preservation matters.
    pub fn tag(&self) -> Option<&str> {
        self.tag.as_ref().map(DemTag::as_str)
    }

    /// Returns the exact unescaped bytes of this repeat block's optional DEM tag.
    pub fn tag_bytes(&self) -> Option<&[u8]> {
        self.tag.as_ref().map(DemTag::as_bytes)
    }

    fn write_dem_open(&self, out: &mut String, indent: usize) {
        write_indent(out, indent);
        out.push_str("repeat");
        write_optional_tag(out, self.tag.as_ref());
        out.push(' ');
        out.push_str(self.repeat_count.get().format_into(&mut NumBuffer::new()));
        out.push_str(" {\n");
        // Pinned Stim prints one blank line inside an empty repeat body.
        if self.body.items.is_empty() {
            out.push('\n');
        }
    }

    fn write_dem_io_open<W: Write>(&self, out: &mut W, indent: usize) -> io::Result<()> {
        write_indent_io(out, indent)?;
        out.write_all(b"repeat")?;
        write_optional_tag_io(out, self.tag.as_ref())?;
        writeln!(out, " {} {{", self.repeat_count.get())?;
        if self.body.items.is_empty() {
            out.write_all(b"\n")?;
        }
        Ok(())
    }
}

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

    fn lookup_name(name: &str) -> Option<Self> {
        match name.len() {
            5 if name.eq_ignore_ascii_case("error") => Some(Self::Error),
            8 if name.eq_ignore_ascii_case("detector") => Some(Self::Detector),
            18 if name.eq_ignore_ascii_case("logical_observable") => Some(Self::LogicalObservable),
            15 if name.eq_ignore_ascii_case("shift_detectors") => Some(Self::ShiftDetectors),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DemInstruction {
    kind: DemInstructionKind,
    args: DemArgVec,
    targets: DemTargetVec,
    tag: Option<DemTag>,
}

impl DemInstruction {
    pub fn new(
        kind: DemInstructionKind,
        args: Vec<f64>,
        targets: Vec<DemTarget>,
        tag: Option<String>,
    ) -> ModelResult<Self> {
        Self::from_parts(
            kind,
            DemArgVec::from_vec(args),
            DemTargetVec::from_vec(targets),
            normalize_tag(tag),
        )
    }

    pub(crate) fn new_with_tag_bytes(
        kind: DemInstructionKind,
        args: Vec<f64>,
        targets: Vec<DemTarget>,
        tag: Option<&[u8]>,
    ) -> ModelResult<Self> {
        Self::from_parts(
            kind,
            DemArgVec::from_vec(args),
            DemTargetVec::from_vec(targets),
            tag.and_then(DemTag::from_slice),
        )
    }

    fn from_parts(
        kind: DemInstructionKind,
        args: DemArgVec,
        targets: DemTargetVec,
        tag: Option<DemTag>,
    ) -> ModelResult<Self> {
        validate_dem_instruction(kind, &args, &targets)?;
        Ok(Self::from_validated_parts(kind, args, targets, tag))
    }

    fn from_validated_parts(
        kind: DemInstructionKind,
        args: DemArgVec,
        targets: DemTargetVec,
        tag: Option<DemTag>,
    ) -> Self {
        Self {
            kind,
            args,
            targets,
            tag,
        }
    }

    pub fn error(
        probability: Probability,
        targets: Vec<DemTarget>,
        tag: Option<String>,
    ) -> ModelResult<Self> {
        let mut args = DemArgVec::new();
        args.push(probability.get());
        Self::from_parts(
            DemInstructionKind::Error,
            args,
            DemTargetVec::from_vec(targets),
            normalize_tag(tag),
        )
    }

    pub fn detector(
        coordinates: Vec<f64>,
        target: DemTarget,
        tag: Option<String>,
    ) -> ModelResult<Self> {
        let mut targets = DemTargetVec::new();
        targets.push(target);
        Self::from_parts(
            DemInstructionKind::Detector,
            DemArgVec::from_vec(coordinates),
            targets,
            normalize_tag(tag),
        )
    }

    pub fn logical_observable(target: DemTarget, tag: Option<String>) -> ModelResult<Self> {
        let mut targets = DemTargetVec::new();
        targets.push(target);
        Self::from_parts(
            DemInstructionKind::LogicalObservable,
            DemArgVec::new(),
            targets,
            normalize_tag(tag),
        )
    }

    pub fn shift_detectors(
        coordinates: Vec<f64>,
        detector_shift: u64,
        tag: Option<String>,
    ) -> ModelResult<Self> {
        let mut targets = DemTargetVec::new();
        targets.push(DemTarget::numeric(detector_shift));
        Self::from_parts(
            DemInstructionKind::ShiftDetectors,
            DemArgVec::from_vec(coordinates),
            targets,
            normalize_tag(tag),
        )
    }

    pub fn kind(&self) -> DemInstructionKind {
        self.kind
    }

    pub fn args(&self) -> &[f64] {
        &self.args
    }

    pub fn targets(&self) -> &[DemTarget] {
        &self.targets
    }

    pub fn target_groups(&self) -> Vec<&[DemTarget]> {
        self.targets
            .split(|target| matches!(target, DemTarget::Separator))
            .collect()
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

    pub(crate) fn detector_shift(&self) -> ModelResult<u64> {
        if self.kind != DemInstructionKind::ShiftDetectors {
            return Err(ModelError::invalid_detector_error_model(
                "non-shift instruction has no detector shift",
            ));
        }
        match self.targets.as_slice() {
            [DemTarget::Numeric(value)] => Ok(*value),
            _ => Err(ModelError::invalid_detector_error_model(
                "shift_detectors instruction is missing numeric target",
            )),
        }
    }

    fn write_dem(&self, out: &mut String, indent: usize, floats: &mut DemFloatCache) {
        write_indent(out, indent);
        out.push_str(self.kind.canonical_name());
        write_optional_tag(out, self.tag.as_ref());
        write_args(out, &self.args, floats);
        write_dem_targets(out, &self.targets);
        out.push('\n');
    }

    fn write_dem_io<W: Write>(
        &self,
        out: &mut W,
        indent: usize,
        floats: &mut DemFloatCache,
    ) -> io::Result<()> {
        write_indent_io(out, indent)?;
        out.write_all(self.kind.canonical_name().as_bytes())?;
        write_optional_tag_io(out, self.tag.as_ref())?;
        write_args_io(out, &self.args, floats)?;
        write_dem_targets_io(out, &self.targets)?;
        out.write_all(b"\n")
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DemDetectorId(u64);

impl DemDetectorId {
    pub fn try_new(value: u64) -> ModelResult<Self> {
        if value > MAX_DEM_DETECTOR_ID {
            return Err(ModelError::invalid_detector_error_model(format!(
                "detector id {value} exceeds {MAX_DEM_DETECTOR_ID}"
            )));
        }
        Ok(Self(value))
    }

    pub fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DemObservableId(u32);

impl DemObservableId {
    pub fn try_new(value: u64) -> ModelResult<Self> {
        let value = u32::try_from(value).map_err(|_| {
            ModelError::invalid_detector_error_model(format!(
                "observable id {value} exceeds {}",
                u32::MAX
            ))
        })?;
        Ok(Self(value))
    }

    pub fn get(self) -> u64 {
        u64::from(self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DemTarget {
    RelativeDetector(DemDetectorId),
    LogicalObservable(DemObservableId),
    Separator,
    Numeric(u64),
}

impl DemTarget {
    pub fn relative_detector(id: u64) -> ModelResult<Self> {
        Ok(Self::RelativeDetector(DemDetectorId::try_new(id)?))
    }

    pub fn logical_observable(id: u64) -> ModelResult<Self> {
        Ok(Self::LogicalObservable(DemObservableId::try_new(id)?))
    }

    pub fn separator() -> Self {
        Self::Separator
    }

    pub fn numeric(value: u64) -> Self {
        Self::Numeric(value)
    }

    /// Splits this target's DEM text into its letter prefix and its decimal digits.
    fn text_parts(self, digits: &mut NumBuffer<u64>) -> (&'static str, &str) {
        match self {
            Self::RelativeDetector(id) => ("D", id.get().format_into(digits)),
            Self::LogicalObservable(id) => ("L", id.get().format_into(digits)),
            Self::Separator => ("^", ""),
            Self::Numeric(value) => ("", value.format_into(digits)),
        }
    }
}

impl Display for DemTarget {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut digits = NumBuffer::new();
        let (prefix, number) = self.text_parts(&mut digits);
        f.write_str(prefix)?;
        f.write_str(number)
    }
}

impl FromStr for DemTarget {
    type Err = ModelError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        if raw == "^" {
            return Ok(Self::Separator);
        }
        if let Some(value) = raw.strip_prefix('D') {
            return Self::relative_detector(parse_unsigned_dem_text_value(
                value,
                "relative detector target",
            )?);
        }
        if let Some(value) = raw.strip_prefix('L') {
            return Self::logical_observable(parse_unsigned_dem_text_value(
                value,
                "logical observable target",
            )?);
        }
        Err(ModelError::invalid_detector_error_model(format!(
            "invalid DEM target {raw:?}"
        )))
    }
}

fn validate_dem_instruction(
    kind: DemInstructionKind,
    args: &[f64],
    targets: &[DemTarget],
) -> ModelResult<()> {
    match kind {
        DemInstructionKind::Error => {
            if args.len() != 1 {
                return Err(ModelError::invalid_detector_error_model(
                    "error instructions require exactly one probability argument",
                ));
            }
            let Some(probability) = args.first().copied() else {
                return Err(ModelError::invalid_detector_error_model(
                    "error instructions require exactly one probability argument",
                ));
            };
            Probability::try_new(probability)?;
            validate_error_targets(targets)
        }
        DemInstructionKind::Detector => {
            validate_finite_args("detector", args)?;
            validate_exactly_one_target("detector", targets)?;
            validate_targets("detector", targets, |target| {
                matches!(target, DemTarget::RelativeDetector(_))
            })
        }
        DemInstructionKind::LogicalObservable => {
            if !args.is_empty() {
                return Err(ModelError::invalid_detector_error_model(
                    "logical_observable instructions do not take arguments",
                ));
            }
            validate_exactly_one_target("logical_observable", targets)?;
            validate_targets("logical_observable", targets, |target| {
                matches!(target, DemTarget::LogicalObservable(_))
            })
        }
        DemInstructionKind::ShiftDetectors => {
            validate_finite_args("shift_detectors", args)?;
            match targets {
                [DemTarget::Numeric(_)] => Ok(()),
                _ => Err(ModelError::invalid_detector_error_model(
                    "shift_detectors requires exactly one numeric target",
                )),
            }
        }
    }
}

fn validate_error_targets(targets: &[DemTarget]) -> ModelResult<()> {
    let mut previous_was_separator = true;
    for target in targets {
        match target {
            DemTarget::RelativeDetector(_) | DemTarget::LogicalObservable(_) => {
                previous_was_separator = false;
            }
            DemTarget::Separator => {
                if previous_was_separator {
                    return Err(ModelError::invalid_detector_error_model(
                        "error target separators cannot be first or consecutive",
                    ));
                }
                previous_was_separator = true;
            }
            DemTarget::Numeric(_) => {
                return Err(ModelError::invalid_detector_error_model(
                    "error instructions cannot target raw numbers",
                ));
            }
        }
    }
    if previous_was_separator && !targets.is_empty() {
        return Err(ModelError::invalid_detector_error_model(
            "error target separators cannot be last",
        ));
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

fn validate_exactly_one_target(kind: &'static str, targets: &[DemTarget]) -> ModelResult<()> {
    if targets.len() != 1 {
        return Err(ModelError::invalid_detector_error_model(format!(
            "{kind} requires exactly one target"
        )));
    }
    Ok(())
}

fn validate_targets(
    kind: &'static str,
    targets: &[DemTarget],
    predicate: impl Fn(&DemTarget) -> bool,
) -> ModelResult<()> {
    for target in targets {
        if !predicate(target) {
            return Err(ModelError::invalid_detector_error_model(format!(
                "{kind} received invalid target {target}"
            )));
        }
    }
    Ok(())
}

fn write_indent(out: &mut String, indent: usize) {
    out.extend(std::iter::repeat_n(' ', indent));
}

fn write_optional_tag(out: &mut String, tag: Option<&DemTag>) {
    let Some(tag) = tag else {
        return;
    };
    out.push('[');
    tag.write_escaped_text(out);
    out.push(']');
}

fn write_optional_tag_io<W: Write>(out: &mut W, tag: Option<&DemTag>) -> io::Result<()> {
    let Some(tag) = tag else {
        return Ok(());
    };
    out.write_all(b"[")?;
    tag.write_escaped_bytes(out)?;
    out.write_all(b"]")
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

fn write_dem_targets(out: &mut String, targets: &[DemTarget]) {
    let mut digits = NumBuffer::new();
    for target in targets {
        let (prefix, number) = target.text_parts(&mut digits);
        out.push(' ');
        out.push_str(prefix);
        out.push_str(number);
    }
}

fn write_dem_targets_io<W: Write>(out: &mut W, targets: &[DemTarget]) -> io::Result<()> {
    let mut digits = NumBuffer::new();
    for target in targets {
        let (prefix, number) = target.text_parts(&mut digits);
        out.write_all(b" ")?;
        out.write_all(prefix.as_bytes())?;
        out.write_all(number.as_bytes())?;
    }
    Ok(())
}

fn write_indent_io<W: Write>(out: &mut W, indent: usize) -> io::Result<()> {
    for _ in 0..indent {
        out.write_all(b" ")?;
    }
    Ok(())
}

/// Bytes an item's opaque tag retains beyond its tag charge.
fn opaque_overhead(tag: Option<&DemTag>) -> u64 {
    tag.map_or(0, |tag| tag.opaque_overhead_bytes() as u64)
}

fn normalize_tag(tag: Option<String>) -> Option<DemTag> {
    tag.and_then(DemTag::from_string)
}
