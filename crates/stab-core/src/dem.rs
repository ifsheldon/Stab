use std::fmt::{Display, Formatter};
use std::str::FromStr;

mod analyze;
mod graphlike;
mod hyper;

pub use analyze::{
    DisjointPauliProbabilities, ErrorAnalyzerOptions, IndependentPauliProbabilities,
    circuit_to_detector_error_model, independent_to_disjoint_xyz_errors,
    try_disjoint_to_independent_xyz_errors,
};

use crate::{CircuitError, CircuitResult, Probability, RepeatCount};

const MAX_DEM_DETECTOR_ID: u64 = (1_u64 << 62) - 1;
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
        self.total_detector_shift_inner()
    }

    pub fn count_detectors(&self) -> CircuitResult<u64> {
        self.count_detectors_from(0)
    }

    pub fn count_observables(&self) -> CircuitResult<u64> {
        let mut max_observable = None;
        self.visit_observables(&mut max_observable)?;
        Ok(max_observable.map_or(0, |id| id.saturating_add(1)))
    }

    fn total_detector_shift_inner(&self) -> CircuitResult<u64> {
        let mut shift = 0_u64;
        for item in &self.items {
            match item {
                DemItem::Instruction(instruction) => {
                    if instruction.kind == DemInstructionKind::ShiftDetectors {
                        shift = shift
                            .checked_add(instruction.detector_shift()?)
                            .ok_or_else(|| {
                                CircuitError::invalid_detector_error_model(
                                    "detector shift overflowed",
                                )
                            })?;
                    }
                }
                DemItem::RepeatBlock(repeat) => {
                    let body_shift = repeat.body.total_detector_shift_inner()?;
                    let repeated = body_shift
                        .checked_mul(repeat.repeat_count.get())
                        .ok_or_else(|| {
                            CircuitError::invalid_detector_error_model("repeat shift overflowed")
                        })?;
                    shift = shift.checked_add(repeated).ok_or_else(|| {
                        CircuitError::invalid_detector_error_model("detector shift overflowed")
                    })?;
                }
            }
        }
        Ok(shift)
    }

    fn count_detectors_from(&self, mut detector_offset: u64) -> CircuitResult<u64> {
        let mut count = detector_offset;
        for item in &self.items {
            match item {
                DemItem::Instruction(instruction) => match instruction.kind {
                    DemInstructionKind::Error | DemInstructionKind::Detector => {
                        for target in instruction.targets() {
                            if let DemTarget::RelativeDetector(id) = target {
                                count = count.max(
                                    detector_offset.checked_add(id.get()).ok_or_else(|| {
                                        CircuitError::invalid_detector_error_model(
                                            "detector id overflowed",
                                        )
                                    })? + 1,
                                );
                            }
                        }
                    }
                    DemInstructionKind::ShiftDetectors => {
                        detector_offset = detector_offset
                            .checked_add(instruction.detector_shift()?)
                            .ok_or_else(|| {
                                CircuitError::invalid_detector_error_model(
                                    "detector shift overflowed",
                                )
                            })?;
                    }
                    DemInstructionKind::LogicalObservable => {}
                },
                DemItem::RepeatBlock(repeat) => {
                    let body_shift = repeat.body.total_detector_shift_inner()?;
                    for _ in 0..repeat.repeat_count.get() {
                        count = count.max(repeat.body.count_detectors_from(detector_offset)?);
                        detector_offset =
                            detector_offset.checked_add(body_shift).ok_or_else(|| {
                                CircuitError::invalid_detector_error_model(
                                    "repeat detector shift overflowed",
                                )
                            })?;
                    }
                }
            }
        }
        Ok(count)
    }

    fn visit_observables(&self, max_observable: &mut Option<u64>) -> CircuitResult<()> {
        for item in &self.items {
            match item {
                DemItem::Instruction(instruction) => {
                    for target in instruction.targets() {
                        if let DemTarget::LogicalObservable(id) = target {
                            *max_observable = Some(
                                max_observable.map_or(id.get(), |current| current.max(id.get())),
                            );
                        }
                    }
                }
                DemItem::RepeatBlock(repeat) => repeat.body.visit_observables(max_observable)?,
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
    tag: Option<String>,
}

impl DemRepeatBlock {
    pub fn new(repeat_count: RepeatCount, body: DetectorErrorModel, tag: Option<String>) -> Self {
        Self {
            repeat_count,
            body,
            tag: normalize_tag(tag),
        }
    }

    pub fn repeat_count(&self) -> RepeatCount {
        self.repeat_count
    }

    pub fn body(&self) -> &DetectorErrorModel {
        &self.body
    }

    pub fn tag(&self) -> Option<&str> {
        self.tag.as_deref()
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
        match name.to_ascii_lowercase().as_str() {
            "error" => Ok(Self::Error),
            "detector" => Ok(Self::Detector),
            "logical_observable" => Ok(Self::LogicalObservable),
            "shift_detectors" => Ok(Self::ShiftDetectors),
            _ => Err(CircuitError::invalid_detector_error_model(format!(
                "unknown DEM instruction {name}"
            ))),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DemInstruction {
    kind: DemInstructionKind,
    args: Vec<f64>,
    targets: Vec<DemTarget>,
    tag: Option<String>,
}

impl DemInstruction {
    pub fn new(
        kind: DemInstructionKind,
        args: Vec<f64>,
        targets: Vec<DemTarget>,
        tag: Option<String>,
    ) -> CircuitResult<Self> {
        validate_dem_instruction(kind, &args, &targets)?;
        Ok(Self {
            kind,
            args,
            targets,
            tag: normalize_tag(tag),
        })
    }

    pub fn error(
        probability: Probability,
        targets: Vec<DemTarget>,
        tag: Option<String>,
    ) -> CircuitResult<Self> {
        Self::new(
            DemInstructionKind::Error,
            vec![probability.get()],
            targets,
            tag,
        )
    }

    pub fn detector(
        coordinates: Vec<f64>,
        target: DemTarget,
        tag: Option<String>,
    ) -> CircuitResult<Self> {
        Self::new(DemInstructionKind::Detector, coordinates, vec![target], tag)
    }

    pub fn logical_observable(target: DemTarget, tag: Option<String>) -> CircuitResult<Self> {
        Self::new(
            DemInstructionKind::LogicalObservable,
            Vec::new(),
            vec![target],
            tag,
        )
    }

    pub fn shift_detectors(
        coordinates: Vec<f64>,
        detector_shift: u64,
        tag: Option<String>,
    ) -> CircuitResult<Self> {
        Self::new(
            DemInstructionKind::ShiftDetectors,
            coordinates,
            vec![DemTarget::numeric(detector_shift)],
            tag,
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
        self.tag.as_deref()
    }

    fn detector_shift(&self) -> CircuitResult<u64> {
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
        if let Some(value) = raw.strip_prefix('D') {
            return Self::relative_detector(parse_unsigned_dem_value(
                value,
                "relative detector target",
            )?);
        }
        if let Some(value) = raw.strip_prefix('L') {
            return Self::logical_observable(parse_unsigned_dem_value(
                value,
                "logical observable target",
            )?);
        }
        Ok(Self::numeric(parse_unsigned_dem_value(
            raw,
            "numeric DEM target",
        )?))
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
            validate_non_empty_targets("detector", targets)?;
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
            validate_non_empty_targets("logical_observable", targets)?;
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

fn validate_non_empty_targets(kind: &'static str, targets: &[DemTarget]) -> CircuitResult<()> {
    if targets.is_empty() {
        return Err(CircuitError::invalid_detector_error_model(format!(
            "{kind} requires at least one target"
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
    lines: Vec<&'a str>,
    index: usize,
}

impl<'a> DemParser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            lines: input.lines().collect(),
            index: 0,
        }
    }

    fn parse(mut self) -> CircuitResult<DetectorErrorModel> {
        let model = self.parse_block(false)?;
        if self.index < self.lines.len() {
            return Err(CircuitError::UnexpectedRepeatTerminator);
        }
        Ok(model)
    }

    fn parse_block(&mut self, stop_on_terminator: bool) -> CircuitResult<DetectorErrorModel> {
        let mut model = DetectorErrorModel::new();
        while let Some(raw_line) = self.lines.get(self.index).copied() {
            let line_number = self.index + 1;
            self.index += 1;
            let Some(line) = strip_comment(raw_line) else {
                continue;
            };
            let line = line.trim();
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
                model.push_repeat_block(self.parse_repeat(line_number, prefix.trim_end())?);
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

    fn parse_repeat(&mut self, line_number: usize, line: &str) -> CircuitResult<DemRepeatBlock> {
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
        let body = self.parse_block(true)?;
        Ok(DemRepeatBlock::new(RepeatCount::try_new(count)?, body, tag))
    }
}

fn parse_dem_instruction(line_number: usize, line: &str) -> CircuitResult<DemInstruction> {
    let (name, rest) = parse_name(line_number, line)?;
    let kind =
        DemInstructionKind::from_name(name).map_err(|error| wrap_line(line_number, error))?;
    let (tag, rest) = parse_optional_tag(line_number, rest)?;
    let (args, rest) = parse_optional_args(line_number, rest)?;
    let targets = parse_dem_targets(rest).map_err(|error| wrap_line(line_number, error))?;
    DemInstruction::new(kind, args, targets, tag).map_err(|error| wrap_line(line_number, error))
}

fn parse_name(line_number: usize, line: &str) -> CircuitResult<(&str, &str)> {
    let end = line
        .char_indices()
        .take_while(|(index, ch)| {
            if *index == 0 {
                ch.is_ascii_alphabetic()
            } else {
                ch.is_ascii_alphanumeric() || *ch == '_'
            }
        })
        .last()
        .map(|(index, ch)| index + ch.len_utf8())
        .ok_or_else(|| CircuitError::parse_line(line_number, "missing DEM instruction name"))?;
    Ok(line.split_at(end))
}

fn parse_optional_tag(line_number: usize, rest: &str) -> CircuitResult<(Option<String>, &str)> {
    let rest = rest.trim_start();
    let Some(mut body) = rest.strip_prefix('[') else {
        return Ok((None, rest));
    };
    let mut tag = String::new();
    loop {
        let Some((ch, after_ch)) = split_first_char(body) else {
            return Err(CircuitError::parse_line(line_number, "unterminated tag"));
        };
        body = after_ch;
        match ch {
            ']' => return Ok((Some(tag), body)),
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

fn parse_optional_args(line_number: usize, rest: &str) -> CircuitResult<(Vec<f64>, &str)> {
    let rest = rest.trim_start();
    let Some(body) = rest.strip_prefix('(') else {
        return Ok((Vec::new(), rest));
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
    let mut args = Vec::new();
    if !raw_args.trim().is_empty() {
        for arg in raw_args.split(',') {
            let arg = arg.trim();
            args.push(arg.parse::<f64>().map_err(|_| {
                CircuitError::parse_line(line_number, format!("invalid argument {arg}"))
            })?);
        }
    }
    Ok((args, tail))
}

fn parse_dem_targets(rest: &str) -> CircuitResult<Vec<DemTarget>> {
    rest.split_whitespace().map(str::parse).collect()
}

fn parse_unsigned_dem_value(text: &str, kind: &'static str) -> CircuitResult<u64> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(CircuitError::invalid_detector_error_model(format!(
            "invalid {kind} {text:?}"
        )));
    }
    text.parse::<u64>()
        .map_err(|_| CircuitError::invalid_detector_error_model(format!("invalid {kind} {text:?}")))
}

fn strip_comment(line: &str) -> Option<&str> {
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

fn normalize_tag(tag: Option<String>) -> Option<String> {
    tag.filter(|tag| !tag.is_empty())
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

#[cfg(test)]
mod tests;
