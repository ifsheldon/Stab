use std::fmt::{Display, Formatter};
use std::str::FromStr;
use std::str::Lines;

use smallvec::SmallVec;

mod analyze;
mod api;
mod arena_index;
mod coordinate_scan;
mod error_traversal;
#[cfg(test)]
mod generated_qec_tests;
mod graphlike;
mod hyper;
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
pub use sat::{likeliest_error_sat_problem, shortest_error_sat_problem};
pub(crate) use traversal::{
    DemRepeatSelection, DemTraversalState, FoldedDemBlock, FoldedDemItem, FoldedDemTraversal,
    FoldedDemVisitor,
};

use crate::{CircuitError, CircuitResult, Probability, RepeatCount};
use tag::DemTag;
type DemArgVec = SmallVec<[f64; 2]>;
type DemTargetVec = SmallVec<[DemTarget; 1]>;

const MAX_DEM_DETECTOR_ID: u64 = (1_u64 << 62) - 1;
const MAX_DEM_PARSE_LINES: usize = 1_000_000;
const MAX_DEM_PREALLOCATED_ITEMS: usize = 131_072;
const DEM_PREALLOCATION_SAMPLE_BYTES: usize = 256;
pub(crate) const MAX_DEM_REPEAT_NESTING: usize = 256;
pub(crate) const MAX_DEM_FLATTEN_REPEAT_UNROLL: u64 = 100_000;
pub(crate) const MAX_DEM_FLATTEN_EXPANDED_INSTRUCTIONS: u64 = 1_000_000;
pub(crate) const MAX_DEM_FLATTEN_REPEAT_ITERATIONS: u64 = 1_000_000;
const DEM_FLOAT_PRECISION: i32 = 34;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DetectorErrorModel {
    items: Vec<DemItem>,
}

impl DetectorErrorModel {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn from_dem_str(input: &str) -> CircuitResult<Self> {
        DemParser::new(input).parse()
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

    pub fn to_dem_string(&self) -> String {
        let mut out = String::new();
        self.write_dem(&mut out, 0);
        out
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

    pub(crate) fn validate_flattening_budget(&self, context: &'static str) -> CircuitResult<()> {
        let mut budget = DemFlatteningBudget::default();
        self.validate_flattening_budget_items(1, 0, context, &mut budget)
    }

    fn validate_flattening_budget_items(
        &self,
        multiplier: u64,
        depth: usize,
        context: &'static str,
        budget: &mut DemFlatteningBudget,
    ) -> CircuitResult<()> {
        if depth > MAX_DEM_REPEAT_NESTING {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "DEM {context} repeat nesting exceeds current limit {MAX_DEM_REPEAT_NESTING}"
            )));
        }
        for item in &self.items {
            match item {
                DemItem::Instruction(_) => {
                    budget.add_expanded_instructions(multiplier, context)?;
                }
                DemItem::RepeatBlock(repeat) => {
                    let repeat_count = repeat.repeat_count.get();
                    if repeat_count > MAX_DEM_FLATTEN_REPEAT_UNROLL {
                        return Err(CircuitError::invalid_detector_error_model(format!(
                            "DEM {context} currently supports repeat counts up to {MAX_DEM_FLATTEN_REPEAT_UNROLL}, got {repeat_count}"
                        )));
                    }
                    let repeated_multiplier =
                        multiplier.checked_mul(repeat_count).ok_or_else(|| {
                            CircuitError::invalid_detector_error_model(format!(
                                "DEM {context} repeat expansion count overflowed"
                            ))
                        })?;
                    budget.add_repeat_iterations(repeated_multiplier, context)?;
                    repeat.body.validate_flattening_budget_items(
                        repeated_multiplier,
                        depth + 1,
                        context,
                        budget,
                    )?;
                }
            }
        }
        Ok(())
    }

    fn write_dem(&self, out: &mut String, indent: usize) {
        for item in &self.items {
            item.write_dem(out, indent);
        }
    }
}

#[derive(Clone, Debug, Default)]
struct DemFlatteningBudget {
    expanded_instructions: u64,
    repeat_iterations: u64,
}

impl DemFlatteningBudget {
    fn add_expanded_instructions(
        &mut self,
        count: u64,
        context: &'static str,
    ) -> CircuitResult<()> {
        self.expanded_instructions =
            self.expanded_instructions
                .checked_add(count)
                .ok_or_else(|| {
                    CircuitError::invalid_detector_error_model(format!(
                        "DEM {context} expanded instruction count overflowed"
                    ))
                })?;
        if self.expanded_instructions > MAX_DEM_FLATTEN_EXPANDED_INSTRUCTIONS {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "DEM {context} currently supports at most {MAX_DEM_FLATTEN_EXPANDED_INSTRUCTIONS} expanded instructions, got at least {}",
                self.expanded_instructions
            )));
        }
        Ok(())
    }

    fn add_repeat_iterations(&mut self, count: u64, context: &'static str) -> CircuitResult<()> {
        self.repeat_iterations = self.repeat_iterations.checked_add(count).ok_or_else(|| {
            CircuitError::invalid_detector_error_model(format!(
                "DEM {context} repeat iteration count overflowed"
            ))
        })?;
        if self.repeat_iterations > MAX_DEM_FLATTEN_REPEAT_ITERATIONS {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "DEM {context} currently supports at most {MAX_DEM_FLATTEN_REPEAT_ITERATIONS} expanded repeat iterations, got at least {}",
                self.repeat_iterations
            )));
        }
        Ok(())
    }
}

impl Display for DetectorErrorModel {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_dem_string())
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
}

#[derive(Clone, Debug, PartialEq)]
pub struct DemRepeatBlock {
    repeat_count: RepeatCount,
    body: DetectorErrorModel,
    tag: Option<DemTag>,
}

impl DemRepeatBlock {
    pub fn new(repeat_count: RepeatCount, body: DetectorErrorModel, tag: Option<String>) -> Self {
        Self::from_parts(repeat_count, body, normalize_tag(tag))
    }

    fn from_parts(
        repeat_count: RepeatCount,
        body: DetectorErrorModel,
        tag: Option<DemTag>,
    ) -> Self {
        Self {
            repeat_count,
            body,
            tag,
        }
    }

    pub fn repeat_count(&self) -> RepeatCount {
        self.repeat_count
    }

    pub fn body(&self) -> &DetectorErrorModel {
        &self.body
    }

    pub fn tag(&self) -> Option<&str> {
        self.tag.as_ref().map(DemTag::as_str)
    }

    fn write_dem(&self, out: &mut String, indent: usize) {
        write_indent(out, indent);
        out.push_str("repeat");
        write_optional_tag(out, self.tag());
        out.push(' ');
        out.push_str(&self.repeat_count.get().to_string());
        out.push_str(" {\n");
        self.body.write_dem(out, indent + 4);
        write_indent(out, indent);
        out.push_str("}\n");
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

    fn from_name(name: &str) -> CircuitResult<Self> {
        match name.len() {
            5 if name.eq_ignore_ascii_case("error") => Ok(Self::Error),
            8 if name.eq_ignore_ascii_case("detector") => Ok(Self::Detector),
            18 if name.eq_ignore_ascii_case("logical_observable") => Ok(Self::LogicalObservable),
            15 if name.eq_ignore_ascii_case("shift_detectors") => Ok(Self::ShiftDetectors),
            _ => Err(CircuitError::invalid_detector_error_model(format!(
                "unknown DEM instruction {name}"
            ))),
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

    fn from_parts(
        kind: DemInstructionKind,
        args: DemArgVec,
        targets: DemTargetVec,
        tag: Option<DemTag>,
    ) -> CircuitResult<Self> {
        validate_dem_instruction(kind, &args, &targets)?;
        Ok(Self {
            kind,
            args,
            targets,
            tag,
        })
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

    pub fn tag(&self) -> Option<&str> {
        self.tag.as_ref().map(DemTag::as_str)
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
        write_optional_tag(out, self.tag());
        write_args(out, &self.args);
        write_dem_targets(out, &self.targets);
        out.push('\n');
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
        if let Some(value) = raw.strip_prefix('D').or_else(|| raw.strip_prefix('d')) {
            return Self::relative_detector(parse_unsigned_dem_value(
                value,
                "relative detector target",
            )?);
        }
        if let Some(value) = raw.strip_prefix('L').or_else(|| raw.strip_prefix('l')) {
            return Self::logical_observable(parse_unsigned_dem_value(
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

struct DemParser<'a> {
    lines: Lines<'a>,
    line_number: usize,
    top_level_capacity: usize,
}

impl<'a> DemParser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            lines: input.lines(),
            line_number: 0,
            top_level_capacity: top_level_item_capacity(input),
        }
    }

    fn parse(mut self) -> CircuitResult<DetectorErrorModel> {
        self.parse_block(false, 0)
    }

    fn parse_block(
        &mut self,
        stop_on_terminator: bool,
        depth: usize,
    ) -> CircuitResult<DetectorErrorModel> {
        if depth > MAX_DEM_REPEAT_NESTING {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "DEM repeat nesting exceeds current limit {MAX_DEM_REPEAT_NESTING}"
            )));
        }
        let mut model = if stop_on_terminator {
            DetectorErrorModel::new()
        } else {
            DetectorErrorModel::with_capacity(self.top_level_capacity)
        };
        while let Some((line_number, raw_line)) = self.next_line()? {
            let Some(line) = strip_comment(raw_line) else {
                continue;
            };
            let line = trim_dem(line);
            if line.is_empty() {
                continue;
            }
            if line == "}" {
                if stop_on_terminator {
                    return Ok(model);
                }
                return Err(CircuitError::UnexpectedRepeatTerminator);
            }
            if let Some(prefix) = line.strip_suffix('{') {
                model.push_repeat_block(self.parse_repeat(
                    line_number,
                    trim_dem_end(prefix),
                    depth,
                )?);
            } else {
                model.push_instruction(parse_dem_instruction(line_number, line)?);
            }
        }
        if stop_on_terminator {
            Err(CircuitError::UnterminatedRepeatBlock)
        } else {
            Ok(model)
        }
    }

    fn next_line(&mut self) -> CircuitResult<Option<(usize, &'a str)>> {
        let Some(raw_line) = self.lines.next() else {
            return Ok(None);
        };
        self.line_number = self.line_number.checked_add(1).ok_or_else(|| {
            CircuitError::invalid_detector_error_model("DEM line count overflowed")
        })?;
        if self.line_number > MAX_DEM_PARSE_LINES {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "DEM input has more than {MAX_DEM_PARSE_LINES} lines"
            )));
        }
        Ok(Some((self.line_number, raw_line)))
    }

    fn parse_repeat(
        &mut self,
        line_number: usize,
        line: &str,
        parent_depth: usize,
    ) -> CircuitResult<DemRepeatBlock> {
        let (name, rest) = parse_name(line_number, line)?;
        if !name.eq_ignore_ascii_case("repeat") {
            return Err(CircuitError::parse_line(
                line_number,
                "repeat blocks must be written as repeat <count> {",
            ));
        }
        let (tag, rest) = parse_optional_tag(line_number, rest)?;
        let mut parts = rest.split_whitespace();
        let count = parts
            .next()
            .ok_or_else(|| CircuitError::parse_line(line_number, "missing repeat count"))?;
        if parts.next().is_some() {
            return Err(CircuitError::parse_line(
                line_number,
                "repeat blocks must be written as repeat <count> {",
            ));
        }
        let count = count
            .parse::<u64>()
            .map_err(|_| CircuitError::parse_line(line_number, "invalid repeat count"))?;
        let body = self.parse_block(true, parent_depth + 1)?;
        Ok(DemRepeatBlock::from_parts(
            RepeatCount::try_new(count)?,
            body,
            tag,
        ))
    }
}

fn top_level_item_capacity(input: &str) -> usize {
    if input.is_empty() {
        return 0;
    }
    let sample_len = input.len().min(DEM_PREALLOCATION_SAMPLE_BYTES);
    let newline_count = input
        .as_bytes()
        .iter()
        .take(sample_len)
        .filter(|byte| **byte == b'\n')
        .count();
    if newline_count == 0 {
        return 1;
    }
    input
        .len()
        .saturating_mul(newline_count)
        .div_ceil(sample_len)
        .saturating_add(1)
        .min(MAX_DEM_PREALLOCATED_ITEMS)
}

fn parse_dem_instruction(line_number: usize, line: &str) -> CircuitResult<DemInstruction> {
    let (name, rest) = parse_name(line_number, line)?;
    let kind =
        DemInstructionKind::from_name(name).map_err(|error| wrap_line(line_number, error))?;
    let (tag, rest) = parse_optional_tag(line_number, rest)?;
    let (args, rest) = parse_optional_args(line_number, rest)?;
    let targets = parse_dem_targets(rest).map_err(|error| wrap_line(line_number, error))?;
    DemInstruction::from_parts(kind, args, targets, tag)
        .map_err(|error| wrap_line(line_number, error))
}

fn parse_name(line_number: usize, line: &str) -> CircuitResult<(&str, &str)> {
    let mut end = None;
    for (index, byte) in line.bytes().enumerate() {
        let valid = if index == 0 {
            byte.is_ascii_alphabetic()
        } else {
            byte.is_ascii_alphanumeric() || byte == b'_'
        };
        if !valid {
            break;
        }
        end = Some(index + 1);
    }
    let end =
        end.ok_or_else(|| CircuitError::parse_line(line_number, "missing DEM instruction name"))?;
    Ok(line.split_at(end))
}

fn parse_optional_tag(line_number: usize, rest: &str) -> CircuitResult<(Option<DemTag>, &str)> {
    let rest = trim_dem_start(rest);
    let Some(mut body) = rest.strip_prefix('[') else {
        return Ok((None, rest));
    };
    if let Some(end) = body.as_bytes().iter().position(|byte| *byte == b']') {
        let (raw_tag, tail_with_terminator) = body.split_at(end);
        if !raw_tag
            .as_bytes()
            .iter()
            .any(|byte| matches!(byte, b'\\' | b'\r' | b'\n'))
        {
            let tail = tail_with_terminator
                .strip_prefix(']')
                .ok_or_else(|| CircuitError::parse_line(line_number, "unterminated tag"))?;
            return Ok((DemTag::from_text(raw_tag), tail));
        }
    }
    let mut tag = String::new();
    loop {
        let Some((ch, after_ch)) = split_first_char(body) else {
            return Err(CircuitError::parse_line(line_number, "unterminated tag"));
        };
        body = after_ch;
        match ch {
            ']' => return Ok((DemTag::from_string(tag), body)),
            '\\' => {
                let Some((escaped, after_escaped)) = split_first_char(body) else {
                    return Err(CircuitError::parse_line(
                        line_number,
                        "unterminated tag escape",
                    ));
                };
                body = after_escaped;
                tag.push(match escaped {
                    'C' => ']',
                    'r' => '\r',
                    'n' => '\n',
                    'B' => '\\',
                    _ => {
                        return Err(CircuitError::parse_line(
                            line_number,
                            format!("invalid tag escape \\{escaped}"),
                        ));
                    }
                });
            }
            '\r' | '\n' => {
                return Err(CircuitError::parse_line(line_number, "invalid tag newline"));
            }
            _ => tag.push(ch),
        }
    }
}

fn parse_optional_args(line_number: usize, rest: &str) -> CircuitResult<(DemArgVec, &str)> {
    let rest = trim_dem_start(rest);
    let Some(body) = rest.strip_prefix('(') else {
        return Ok((DemArgVec::new(), rest));
    };
    let Some(end) = body.find(')') else {
        return Err(CircuitError::parse_line(
            line_number,
            "unterminated argument list",
        ));
    };
    let (raw_args, tail_with_paren) = body.split_at(end);
    let tail = tail_with_paren
        .strip_prefix(')')
        .ok_or_else(|| CircuitError::parse_line(line_number, "unterminated argument list"))?;
    let mut args = DemArgVec::new();
    let raw_args = trim_dem(raw_args);
    if !raw_args.is_empty() {
        for arg in raw_args.split(',') {
            let arg = trim_dem(arg);
            args.push(arg.parse::<f64>().map_err(|_| {
                CircuitError::parse_line(line_number, format!("invalid argument {arg}"))
            })?);
        }
    }
    Ok((args, tail))
}

fn parse_dem_targets(rest: &str) -> CircuitResult<DemTargetVec> {
    let mut targets = DemTargetVec::new();
    for raw in rest.split_whitespace() {
        if targets.len() == 1 {
            targets.reserve(4);
        }
        targets.push(parse_dem_target_token(raw)?);
    }
    Ok(targets)
}

fn parse_dem_target_token(raw: &str) -> CircuitResult<DemTarget> {
    let Some((&prefix, _)) = raw.as_bytes().split_first() else {
        return Err(CircuitError::invalid_detector_error_model(
            "invalid DEM target \"\"",
        ));
    };
    let value = raw.get(1..).ok_or_else(|| {
        CircuitError::invalid_detector_error_model(format!("invalid DEM target {raw:?}"))
    })?;
    match prefix {
        b'D' | b'd' => DemTarget::relative_detector(parse_unsigned_dem_value(
            value,
            "relative detector target",
        )?),
        b'L' | b'l' => DemTarget::logical_observable(parse_unsigned_dem_value(
            value,
            "logical observable target",
        )?),
        b'^' if value.is_empty() => Ok(DemTarget::separator()),
        b'0'..=b'9' => Ok(DemTarget::numeric(parse_unsigned_dem_value(
            raw,
            "numeric DEM target",
        )?)),
        _ => Err(CircuitError::invalid_detector_error_model(format!(
            "invalid DEM target {raw:?}"
        ))),
    }
}

fn parse_unsigned_dem_value(text: &str, kind: &'static str) -> CircuitResult<u64> {
    if text.is_empty() {
        return Err(CircuitError::invalid_detector_error_model(format!(
            "invalid {kind} {text:?}"
        )));
    }
    let mut value = 0_u64;
    for byte in text.bytes() {
        if !byte.is_ascii_digit() {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "invalid {kind} {text:?}"
            )));
        }
        let digit = u64::from(byte - b'0');
        if value > (u64::MAX - digit) / 10 {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "invalid {kind} {text:?}"
            )));
        }
        value = value * 10 + digit;
    }
    Ok(value)
}

fn strip_comment(line: &str) -> Option<&str> {
    if !line.as_bytes().contains(&b'#') {
        return Some(line);
    }

    let mut in_tag = false;
    let mut escaped = false;
    for (index, ch) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' if in_tag => escaped = true,
            '[' if !in_tag => in_tag = true,
            ']' if in_tag => in_tag = false,
            '#' if !in_tag => return Some(line.split_at(index).0),
            _ => {}
        }
    }
    Some(line)
}

fn split_first_char(text: &str) -> Option<(char, &str)> {
    let ch = text.chars().next()?;
    Some((ch, text.split_at(ch.len_utf8()).1))
}

fn trim_dem_start(text: &str) -> &str {
    let trimmed = text.trim_ascii_start();
    if trimmed
        .as_bytes()
        .first()
        .is_some_and(|byte| !byte.is_ascii())
    {
        trimmed.trim_start()
    } else {
        trimmed
    }
}

fn trim_dem_end(text: &str) -> &str {
    let trimmed = text.trim_ascii_end();
    if trimmed
        .as_bytes()
        .last()
        .is_some_and(|byte| !byte.is_ascii())
    {
        trimmed.trim_end()
    } else {
        trimmed
    }
}

fn trim_dem(text: &str) -> &str {
    trim_dem_end(trim_dem_start(text))
}

fn write_indent(out: &mut String, indent: usize) {
    out.extend(std::iter::repeat_n(' ', indent));
}

fn write_optional_tag(out: &mut String, tag: Option<&str>) {
    let Some(tag) = tag else {
        return;
    };
    out.push('[');
    write_escaped_tag(out, tag);
    out.push(']');
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

fn write_dem_targets(out: &mut String, targets: &[DemTarget]) {
    for target in targets {
        out.push(' ');
        out.push_str(&target.to_string());
    }
}

fn write_escaped_tag(out: &mut String, tag: &str) {
    for ch in tag.chars() {
        match ch {
            ']' => out.push_str("\\C"),
            '\r' => out.push_str("\\r"),
            '\n' => out.push_str("\\n"),
            '\\' => out.push_str("\\B"),
            _ => out.push(ch),
        }
    }
}

fn normalize_tag(tag: Option<String>) -> Option<DemTag> {
    tag.and_then(DemTag::from_string)
}

fn wrap_line(line: usize, error: CircuitError) -> CircuitError {
    CircuitError::parse_line(line, error.to_string())
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

#[cfg(test)]
mod tests;
