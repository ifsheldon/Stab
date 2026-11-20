use std::fmt::{Display, Formatter, NumBuffer};
use std::io::{self, Write};

use smallvec::SmallVec;

mod api;
mod coordinate_scan;
mod error_mechanisms;
mod instruction;
mod parser;
mod printing;
mod tag;
mod target;
mod traversal;
mod tree;

pub use api::DemFlattenedInstructionIter;
pub use error_mechanisms::{
    DemErrorMechanismTraversalLimits, DemErrorMechanismView, DemErrorMechanismVisitError,
    DemErrorMechanismVisitor, DemErrorTarget, DemErrorTargetIter,
};
pub use instruction::{DemInstruction, DemInstructionKind, DemInstructionView};
pub use target::{DemDetectorId, DemObservableId, DemTarget, DemTargetKind};

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
use crate::model_bytes::PreparedModelText;
use crate::model_tree;
use crate::{
    AbsoluteTolerance, DemRepeatCount, ModelDialect, ModelError, ModelFingerprint, ModelResult,
    ParseLimits, RepeatNestingLimit,
};
use parser::{parse_dem, parse_dem_admitted};
use tag::DemTag;
type DemArgVec = SmallVec<[f64; 3]>;
type DemTargetVec = SmallVec<[DemTarget; 5]>;

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
        model_tree::restore_byte_tags::<Self>(&mut model.items, tags, &mut admission, input)?;
        Ok((model, admission.retained_model_bytes()))
    }

    pub fn from_dem_str_with_limits(input: &str, limits: ParseLimits) -> ModelResult<Self> {
        parse_dem(input, limits)
    }

    /// Conservative retained bytes of this model under [`ParseLimits`] item, target, argument, and
    /// tag accounting, including the display text of tags restored from non-UTF-8 bytes.
    pub fn admitted_bytes(&self) -> u64 {
        model_tree::admitted_bytes::<Self>(&self.items)
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
        tree::models_approx_equal(self, other, tolerance)
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
            ModelError::dem_allocation_failed(format!(
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
}

impl Display for DetectorErrorModel {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_dem_string())
    }
}

impl Clone for DetectorErrorModel {
    fn clone(&self) -> Self {
        Self {
            items: model_tree::clone_items::<Self>(&self.items),
        }
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
        tree::models_equal(self, other)
    }
}

impl Drop for DetectorErrorModel {
    fn drop(&mut self) {
        model_tree::drop_items::<Self>(&mut self.items);
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

fn write_indent_io<W: Write>(out: &mut W, indent: usize) -> io::Result<()> {
    for _ in 0..indent {
        out.write_all(b" ")?;
    }
    Ok(())
}

fn normalize_tag(tag: Option<String>) -> Option<DemTag> {
    tag.and_then(DemTag::from_string)
}
