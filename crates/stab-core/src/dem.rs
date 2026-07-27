use std::fmt::{Display, Formatter};
use std::io::{self, Write};
use std::str::FromStr;

use smallvec::SmallVec;

mod analyze;
mod api;
mod arena_index;
mod coordinate_scan;
mod drop_impl;
mod error_traversal;
mod flatten;
#[cfg(test)]
mod generated_qec_tests;
mod graphlike;
mod hyper;
mod parser;
mod sat;
mod search_budget;
mod tag;
mod traversal;

#[cfg(feature = "ops-contracts")]
#[doc(hidden)]
pub use analyze::{__circuit_to_detector_error_model_with_diagnostics, ErrorAnalyzerDiagnostics};
pub use analyze::{
    DisjointPauliProbabilities, ErrorAnalyzerOptions, IndependentPauliProbabilities,
    circuit_to_detector_error_model, independent_to_disjoint_xyz_errors,
    try_disjoint_to_independent_xyz_errors,
};
pub use api::DemFlattenedInstructionIter;
pub use flatten::DemFlattenLimits;
pub use sat::{
    SatMaterializationLimits, likeliest_error_sat_problem, likeliest_error_sat_problem_with_limits,
    shortest_error_sat_problem, shortest_error_sat_problem_with_limits,
};
pub use search_budget::LogicalErrorSearchLimits;
/// Crate-internal advanced boundary for traversing compact DEM structure.
///
/// This boundary stays owned by the DEM model: analysis and execution consumers may inspect the
/// checked folded tree or implement its visitor policy, but it is not part of Stab's stable public
/// API. Consumers must not replace it with independent repeat expansion.
pub(crate) use traversal::{
    DemRepeatSelection, DemTraversalState, FoldedDemBlock, FoldedDemItem, FoldedDemTraversal,
    FoldedDemVisitor,
};

use crate::model_bytes::PreparedModelText;
use crate::{
    CircuitError, CircuitResult, DemRepeatCount, ModelDialect, ModelFingerprint, ParseLimits,
    Probability, RepeatNestingLimit,
};
use parser::{parse_dem, parse_unsigned_dem_text_value};
use tag::DemTag;
type DemArgVec = SmallVec<[f64; 3]>;
type DemTargetVec = SmallVec<[DemTarget; 5]>;

const MAX_DEM_DETECTOR_ID: u64 = (1_u64 << 62) - 1;
pub(crate) const MAX_DEM_REPEAT_NESTING: usize = RepeatNestingLimit::HARD_MAX;
pub(crate) const MAX_DEM_FLATTEN_REPEAT_UNROLL: u64 = 100_000;
pub(crate) const MAX_DEM_FLATTEN_EXPANDED_INSTRUCTIONS: u64 = 1_000_000;
pub(crate) const MAX_DEM_FLATTEN_REPEAT_ITERATIONS: u64 = 1_000_000;
pub(crate) const MAX_DEM_FLATTEN_TARGET_OCCURRENCES: u64 = 32_000_000;
pub(crate) const MAX_DEM_FLATTEN_ARGUMENT_VALUES: u64 = 16_000_000;
pub(crate) const MAX_DEM_FLATTEN_MATERIALIZED_BYTES: u64 = 512 * 1024 * 1024;
const DEM_FLOAT_PRECISION: i32 = 34;

#[derive(Default)]
pub struct DetectorErrorModel {
    items: Vec<DemItem>,
}

impl DetectorErrorModel {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn from_dem_str(input: &str) -> CircuitResult<Self> {
        Self::from_dem_str_with_limits(input, ParseLimits::default())
    }

    pub fn from_dem_bytes(input: &[u8]) -> CircuitResult<Self> {
        Self::from_dem_bytes_with_limits(input, ParseLimits::default())
    }

    pub fn from_dem_bytes_with_limits(input: &[u8], limits: ParseLimits) -> CircuitResult<Self> {
        let prepared = PreparedModelText::new(input, ModelDialect::DetectorErrorModel, limits)?;
        let parsed = prepared.resolve(Self::from_dem_str_with_limits(prepared.text(), limits));
        let mut model = parsed?;
        if let Some(tags) = prepared.into_tags() {
            model.restore_byte_tags(tags)?;
        }
        Ok(model)
    }

    pub fn from_dem_str_with_limits(input: &str, limits: ParseLimits) -> CircuitResult<Self> {
        parse_dem(input, limits)
    }

    /// Returns the schema-versioned structural identity of this detector error model.
    ///
    /// The fingerprint is independent of accepted textual spelling and printer precision. It
    /// identifies this source model only, not an analysis policy or executable plan.
    pub fn fingerprint(&self) -> ModelFingerprint {
        ModelFingerprint::for_dem(self)
    }

    fn with_capacity(capacity: usize) -> Self {
        Self {
            items: Vec::with_capacity(capacity),
        }
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

    pub(crate) fn try_reserve_items_exact(&mut self, additional: usize) -> CircuitResult<()> {
        self.items.try_reserve_exact(additional).map_err(|error| {
            CircuitError::invalid_detector_error_model(format!(
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
        self.write_dem(&mut out, 0);
        out
    }

    /// Returns canonical DEM text while preserving opaque bytes in tags.
    ///
    /// Use this method instead of [`Self::to_dem_string`] when the model came from a byte source
    /// whose tags may not be valid UTF-8.
    pub fn to_dem_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        match self.write_dem_io(&mut out, 0) {
            Ok(()) => out,
            Err(error) => unreachable!("writing a DEM into Vec<u8> failed: {error}"),
        }
    }

    pub fn total_detector_shift(&self) -> CircuitResult<u64> {
        FoldedDemTraversal::new(self)?
            .root()
            .summary()
            .detector_shift()
    }

    pub fn count_detectors(&self) -> CircuitResult<u64> {
        FoldedDemTraversal::new(self)?
            .root()
            .summary()
            .detector_count()
    }

    pub fn count_observables(&self) -> CircuitResult<u64> {
        Ok(FoldedDemTraversal::new(self)?
            .root()
            .summary()
            .observable_count())
    }

    fn write_dem(&self, out: &mut String, indent: usize) {
        for item in &self.items {
            item.write_dem(out, indent);
        }
    }

    fn write_dem_io(&self, out: &mut impl Write, indent: usize) -> io::Result<()> {
        for item in &self.items {
            item.write_dem_io(out, indent)?;
        }
        Ok(())
    }

    fn restore_byte_tags(&mut self, tags: Vec<Vec<u8>>) -> CircuitResult<()> {
        let expected = self.non_empty_tag_count();
        if expected != tags.len() {
            return Err(CircuitError::invalid_domain_value(
                "DEM byte parser",
                format!(
                    "metadata scanner found {} tags but parser produced {expected}",
                    tags.len()
                ),
            ));
        }
        let mut tags = tags.into_iter();
        self.restore_byte_tags_from(&mut tags);
        let exhausted = tags.next().is_none();
        debug_assert!(exhausted);
        Ok(())
    }

    fn restore_byte_tags_from(&mut self, tags: &mut impl Iterator<Item = Vec<u8>>) {
        for item in &mut self.items {
            match item {
                DemItem::Instruction(instruction) => {
                    if instruction.tag.is_some() {
                        instruction.tag = tags.next().and_then(DemTag::from_bytes);
                    }
                }
                DemItem::RepeatBlock(repeat) => {
                    if repeat.tag.is_some() {
                        repeat.tag = tags.next().and_then(DemTag::from_bytes);
                    }
                    repeat.body.restore_byte_tags_from(tags);
                }
            }
        }
    }

    fn non_empty_tag_count(&self) -> usize {
        self.items
            .iter()
            .map(|item| match item {
                DemItem::Instruction(instruction) => usize::from(instruction.tag.is_some()),
                DemItem::RepeatBlock(repeat) => {
                    usize::from(repeat.tag.is_some()) + repeat.body.non_empty_tag_count()
                }
            })
            .sum()
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
        drop_impl::models_equal(self, other)
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

impl DemItem {
    fn write_dem(&self, out: &mut String, indent: usize) {
        match self {
            Self::Instruction(instruction) => instruction.write_dem(out, indent),
            Self::RepeatBlock(repeat) => repeat.write_dem(out, indent),
        }
    }

    fn write_dem_io(&self, out: &mut impl Write, indent: usize) -> io::Result<()> {
        match self {
            Self::Instruction(instruction) => instruction.write_dem_io(out, indent),
            Self::RepeatBlock(repeat) => repeat.write_dem_io(out, indent),
        }
    }
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

    fn write_dem(&self, out: &mut String, indent: usize) {
        write_indent(out, indent);
        out.push_str("repeat");
        write_optional_tag(out, self.tag.as_ref());
        out.push(' ');
        out.push_str(&self.repeat_count.get().to_string());
        out.push_str(" {\n");
        self.body.write_dem(out, indent + 4);
        write_indent(out, indent);
        out.push_str("}\n");
    }

    fn write_dem_io(&self, out: &mut impl Write, indent: usize) -> io::Result<()> {
        write_indent_io(out, indent)?;
        out.write_all(b"repeat")?;
        write_optional_tag_io(out, self.tag.as_ref())?;
        writeln!(out, " {} {{", self.repeat_count.get())?;
        self.body.write_dem_io(out, indent + 4)?;
        write_indent_io(out, indent)?;
        out.write_all(b"}\n")
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
    ) -> CircuitResult<Self> {
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
    ) -> CircuitResult<Self> {
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
    ) -> CircuitResult<Self> {
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
    ) -> CircuitResult<Self> {
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
    ) -> CircuitResult<Self> {
        let mut targets = DemTargetVec::new();
        targets.push(target);
        Self::from_parts(
            DemInstructionKind::Detector,
            DemArgVec::from_vec(coordinates),
            targets,
            normalize_tag(tag),
        )
    }

    pub fn logical_observable(target: DemTarget, tag: Option<String>) -> CircuitResult<Self> {
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
    ) -> CircuitResult<Self> {
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

    pub(crate) fn detector_shift(&self) -> CircuitResult<u64> {
        if self.kind != DemInstructionKind::ShiftDetectors {
            return Err(CircuitError::invalid_detector_error_model(
                "non-shift instruction has no detector shift",
            ));
        }
        match self.targets.as_slice() {
            [DemTarget::Numeric(value)] => Ok(*value),
            _ => Err(CircuitError::invalid_detector_error_model(
                "shift_detectors instruction is missing numeric target",
            )),
        }
    }

    fn write_dem(&self, out: &mut String, indent: usize) {
        write_indent(out, indent);
        out.push_str(self.kind.canonical_name());
        write_optional_tag(out, self.tag.as_ref());
        write_args(out, &self.args);
        write_dem_targets(out, &self.targets);
        out.push('\n');
    }

    fn write_dem_io(&self, out: &mut impl Write, indent: usize) -> io::Result<()> {
        write_indent_io(out, indent)?;
        out.write_all(self.kind.canonical_name().as_bytes())?;
        write_optional_tag_io(out, self.tag.as_ref())?;
        write_args_io(out, &self.args)?;
        write_dem_targets_io(out, &self.targets)?;
        out.write_all(b"\n")
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DemDetectorId(u64);

impl DemDetectorId {
    pub fn try_new(value: u64) -> CircuitResult<Self> {
        if value > MAX_DEM_DETECTOR_ID {
            return Err(CircuitError::invalid_detector_error_model(format!(
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
    pub fn try_new(value: u64) -> CircuitResult<Self> {
        let value = u32::try_from(value).map_err(|_| {
            CircuitError::invalid_detector_error_model(format!(
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
    pub fn relative_detector(id: u64) -> CircuitResult<Self> {
        Ok(Self::RelativeDetector(DemDetectorId::try_new(id)?))
    }

    pub fn logical_observable(id: u64) -> CircuitResult<Self> {
        Ok(Self::LogicalObservable(DemObservableId::try_new(id)?))
    }

    pub fn separator() -> Self {
        Self::Separator
    }

    pub fn numeric(value: u64) -> Self {
        Self::Numeric(value)
    }
}

impl Display for DemTarget {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RelativeDetector(id) => write!(f, "D{}", id.get()),
            Self::LogicalObservable(id) => write!(f, "L{}", id.get()),
            Self::Separator => f.write_str("^"),
            Self::Numeric(value) => write!(f, "{value}"),
        }
    }
}

impl FromStr for DemTarget {
    type Err = CircuitError;

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
        Err(CircuitError::invalid_detector_error_model(format!(
            "invalid DEM target {raw:?}"
        )))
    }
}

fn validate_dem_instruction(
    kind: DemInstructionKind,
    args: &[f64],
    targets: &[DemTarget],
) -> CircuitResult<()> {
    match kind {
        DemInstructionKind::Error => {
            if args.len() != 1 {
                return Err(CircuitError::invalid_detector_error_model(
                    "error instructions require exactly one probability argument",
                ));
            }
            let Some(probability) = args.first().copied() else {
                return Err(CircuitError::invalid_detector_error_model(
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
                return Err(CircuitError::invalid_detector_error_model(
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
                _ => Err(CircuitError::invalid_detector_error_model(
                    "shift_detectors requires exactly one numeric target",
                )),
            }
        }
    }
}

fn validate_error_targets(targets: &[DemTarget]) -> CircuitResult<()> {
    let mut previous_was_separator = true;
    for target in targets {
        match target {
            DemTarget::RelativeDetector(_) | DemTarget::LogicalObservable(_) => {
                previous_was_separator = false;
            }
            DemTarget::Separator => {
                if previous_was_separator {
                    return Err(CircuitError::invalid_detector_error_model(
                        "error target separators cannot be first or consecutive",
                    ));
                }
                previous_was_separator = true;
            }
            DemTarget::Numeric(_) => {
                return Err(CircuitError::invalid_detector_error_model(
                    "error instructions cannot target raw numbers",
                ));
            }
        }
    }
    if previous_was_separator && !targets.is_empty() {
        return Err(CircuitError::invalid_detector_error_model(
            "error target separators cannot be last",
        ));
    }
    Ok(())
}

fn validate_finite_args(kind: &'static str, args: &[f64]) -> CircuitResult<()> {
    for arg in args {
        if !arg.is_finite() {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "{kind} argument {arg} is not finite"
            )));
        }
    }
    Ok(())
}

fn validate_exactly_one_target(kind: &'static str, targets: &[DemTarget]) -> CircuitResult<()> {
    if targets.len() != 1 {
        return Err(CircuitError::invalid_detector_error_model(format!(
            "{kind} requires exactly one target"
        )));
    }
    Ok(())
}

fn validate_targets(
    kind: &'static str,
    targets: &[DemTarget],
    predicate: impl Fn(&DemTarget) -> bool,
) -> CircuitResult<()> {
    for target in targets {
        if !predicate(target) {
            return Err(CircuitError::invalid_detector_error_model(format!(
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

fn write_optional_tag_io(out: &mut impl Write, tag: Option<&DemTag>) -> io::Result<()> {
    let Some(tag) = tag else {
        return Ok(());
    };
    out.write_all(b"[")?;
    tag.write_escaped_bytes(out)?;
    out.write_all(b"]")
}

fn write_args(out: &mut String, args: &[f64]) {
    if args.is_empty() {
        return;
    }
    out.push('(');
    for (index, arg) in args.iter().enumerate() {
        if index > 0 {
            out.push_str(", ");
        }
        out.push_str(&format_float(*arg));
    }
    out.push(')');
}

fn write_args_io(out: &mut impl Write, args: &[f64]) -> io::Result<()> {
    if args.is_empty() {
        return Ok(());
    }
    out.write_all(b"(")?;
    for (index, arg) in args.iter().enumerate() {
        if index > 0 {
            out.write_all(b", ")?;
        }
        out.write_all(format_float(*arg).as_bytes())?;
    }
    out.write_all(b")")
}

fn write_dem_targets(out: &mut String, targets: &[DemTarget]) {
    for target in targets {
        out.push(' ');
        out.push_str(&target.to_string());
    }
}

fn write_dem_targets_io(out: &mut impl Write, targets: &[DemTarget]) -> io::Result<()> {
    for target in targets {
        write!(out, " {target}")?;
    }
    Ok(())
}

fn write_indent_io(out: &mut impl Write, indent: usize) -> io::Result<()> {
    for _ in 0..indent {
        out.write_all(b" ")?;
    }
    Ok(())
}

fn normalize_tag(tag: Option<String>) -> Option<DemTag> {
    tag.and_then(DemTag::from_string)
}

fn format_float(value: f64) -> String {
    if let Some(integer) = stim_integer_like_i64(value) {
        return integer.to_string();
    }

    #[allow(
        clippy::cast_possible_truncation,
        reason = "finite f64 base-10 exponents fit i32 and are only used for formatting"
    )]
    let exponent = value.abs().log10().floor() as i32;
    if (-4..DEM_FLOAT_PRECISION).contains(&exponent) {
        let decimal_places = usize::try_from(DEM_FLOAT_PRECISION - 1 - exponent).unwrap_or(0);
        trim_decimal_float(format!("{value:.decimal_places$}"))
    } else {
        let digits_after_decimal = usize::try_from(DEM_FLOAT_PRECISION - 1).unwrap_or(0);
        let scientific = format!("{value:.digits_after_decimal$e}");
        let Some((mantissa, exponent)) = scientific.split_once('e') else {
            return value.to_string();
        };
        let Ok(exponent) = exponent.parse::<i32>() else {
            return value.to_string();
        };
        format!(
            "{}e{}",
            trim_decimal_float(mantissa.to_string()),
            format_scientific_exponent(exponent)
        )
    }
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "Stim's C++ printer casts integral doubles to int64_t before printing"
)]
fn stim_integer_like_i64(value: f64) -> Option<i64> {
    if value > i64::MIN as f64 && value < i64::MAX as f64 {
        let integer = value as i64;
        if integer as f64 == value {
            return Some(integer);
        }
    }
    None
}

fn trim_decimal_float(mut text: String) -> String {
    if text.contains('.') {
        text = text.trim_end_matches('0').trim_end_matches('.').to_string();
    }
    text
}

fn format_scientific_exponent(exponent: i32) -> String {
    if exponent < 0 {
        format!("-{:02}", exponent.abs())
    } else {
        format!("+{exponent:02}")
    }
}

pub fn shortest_graphlike_undetectable_logical_error(
    model: &DetectorErrorModel,
    ignore_ungraphlike_errors: bool,
) -> CircuitResult<DetectorErrorModel> {
    graphlike::shortest_graphlike_undetectable_logical_error(model, ignore_ungraphlike_errors)
}

pub fn shortest_graphlike_undetectable_logical_error_with_limits(
    model: &DetectorErrorModel,
    ignore_ungraphlike_errors: bool,
    limits: LogicalErrorSearchLimits,
) -> CircuitResult<DetectorErrorModel> {
    graphlike::shortest_graphlike_undetectable_logical_error_with_limits(
        model,
        ignore_ungraphlike_errors,
        limits,
    )
}

pub fn find_undetectable_logical_error(
    model: &DetectorErrorModel,
    dont_explore_detection_event_sets_with_size_above: usize,
    dont_explore_edges_with_degree_above: usize,
    dont_explore_edges_increasing_symptom_degree: bool,
) -> CircuitResult<DetectorErrorModel> {
    hyper::find_undetectable_logical_error(
        model,
        dont_explore_detection_event_sets_with_size_above,
        dont_explore_edges_with_degree_above,
        dont_explore_edges_increasing_symptom_degree,
    )
}

pub fn find_undetectable_logical_error_with_limits(
    model: &DetectorErrorModel,
    dont_explore_detection_event_sets_with_size_above: usize,
    dont_explore_edges_with_degree_above: usize,
    dont_explore_edges_increasing_symptom_degree: bool,
    limits: LogicalErrorSearchLimits,
) -> CircuitResult<DetectorErrorModel> {
    hyper::find_undetectable_logical_error_with_limits(
        model,
        dont_explore_detection_event_sets_with_size_above,
        dont_explore_edges_with_degree_above,
        dont_explore_edges_increasing_symptom_degree,
        limits,
    )
}

#[cfg(test)]
mod tests;
