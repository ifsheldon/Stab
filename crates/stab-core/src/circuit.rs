use std::fmt::{Display, Formatter};
use std::io::{self, Write};
use std::ops::RangeBounds;
use std::str::Lines;

use crate::gate::{ArgRule, GateTargetGroupKind};
use crate::target::{TargetVec, parse_plain_qubit_target_text, parse_target_token_into};
use crate::{CircuitError, CircuitResult, Gate, ObservableId, Probability, RepeatCount, Target};

const MAX_CIRCUIT_PARSE_LINES: usize = 1_000_000;
const MAX_CIRCUIT_REPEAT_NESTING: usize = 256;

mod api;
mod iter;

pub use iter::{CircuitFlattenedInstructionIter, CircuitFlattenedInstructionRevIter};

use self::iter::{checked_item_range, circuit_item_range_error};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Circuit {
    items: Vec<CircuitItem>,
}

impl Circuit {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    fn with_capacity(capacity: usize) -> Self {
        Self {
            items: Vec::with_capacity(capacity),
        }
    }

    pub(crate) fn from_unfused_items(items: Vec<CircuitItem>) -> Self {
        Self { items }
    }

    pub fn from_stim_str(input: &str) -> CircuitResult<Self> {
        Parser::new(input).parse()
    }

    pub fn items(&self) -> &[CircuitItem] {
        &self.items
    }

    pub fn iter_items(&self) -> impl DoubleEndedIterator<Item = &CircuitItem> + ExactSizeIterator {
        self.items.iter()
    }

    pub fn item_range(
        &self,
        range: impl RangeBounds<usize>,
    ) -> CircuitResult<impl DoubleEndedIterator<Item = &CircuitItem> + ExactSizeIterator> {
        Ok(self.item_slice(range)?.iter())
    }

    pub fn instruction_range(
        &self,
        range: impl RangeBounds<usize>,
    ) -> CircuitResult<impl DoubleEndedIterator<Item = &CircuitInstruction>> {
        let range = checked_item_range(range, self.items.len())?;
        let items = self.item_slice(range.clone())?;
        for (offset, item) in items.iter().enumerate() {
            if matches!(item, CircuitItem::RepeatBlock(_)) {
                return Err(CircuitError::invalid_domain_value(
                    "circuit instruction range",
                    format!(
                        "repeat block at top-level item index {}",
                        range.start + offset
                    ),
                ));
            }
        }
        Ok(items.iter().filter_map(CircuitItem::as_instruction))
    }

    pub fn iter_flattened_instructions(&self) -> CircuitFlattenedInstructionIter<'_> {
        CircuitFlattenedInstructionIter::new(self)
    }

    pub fn iter_flattened_instructions_reverse(&self) -> CircuitFlattenedInstructionRevIter<'_> {
        CircuitFlattenedInstructionRevIter::new(self)
    }

    pub fn count_qubits(&self) -> usize {
        self.items
            .iter()
            .map(CircuitItem::count_qubits)
            .max()
            .unwrap_or(0)
    }

    /// Converts the currently supported Clifford circuit subset into a tableau.
    ///
    /// M6 supports unitary Clifford operations, plus explicit ignore flags for noise,
    /// measurements, and resets. Measurement feedback, detector semantics, and
    /// simulator-backed tableau extraction are outside this helper's current contract.
    pub fn to_tableau(
        &self,
        ignore_noise: bool,
        ignore_measurement: bool,
        ignore_reset: bool,
    ) -> CircuitResult<crate::Tableau> {
        crate::circuit_to_tableau(self, ignore_noise, ignore_measurement, ignore_reset)
    }

    /// Returns the inverse of a unitary Clifford circuit.
    ///
    /// Non-unitary operations such as measurements, resets, detectors, and noise
    /// return an error instead of being rewritten.
    pub fn inverse_unitary(&self) -> CircuitResult<Self> {
        crate::circuit_inverse_unitary(self)
    }

    /// Returns the currently supported QEC inverse subset.
    ///
    /// This includes `inverse_unitary` plus selected reset-measure-detector,
    /// two-to-one detector-flow, selected `m_det`, selected MPP
    /// identity-parity detector-flow, selected noisy MZZ detector-flow, noisy
    /// measurement-only, noisy measure-reset-only, exact noisy measure-reset
    /// detector-flow, and measure-reset pass-through rewrites for one detector.
    /// Stim's broader measurement, detector, feedback, and noise rewrites remain
    /// active follow-up work.
    pub fn inverse_qec(&self) -> CircuitResult<Self> {
        crate::circuit_inverse_qec(self)
    }

    /// Returns the currently supported unitary time-reversal subset for flows.
    ///
    /// The scoped Rust API accepts unsigned flows with Pauli input and output
    /// terms only. Measurement-record and observable flow rewrites are reserved
    /// for the richer QEC inverse milestone.
    pub fn time_reversed_for_flows(
        &self,
        flows: &[crate::Flow],
    ) -> CircuitResult<(Self, Vec<crate::Flow>)> {
        crate::circuit_time_reversed_for_flows(self, flows)
    }

    /// Returns a circuit rewritten into the current base-gate simplification subset.
    ///
    /// M6 decomposes supported single-qubit Clifford gates and selected two-qubit
    /// Clifford gates. Unsupported gates are preserved verbatim.
    pub fn simplified(&self) -> CircuitResult<Self> {
        crate::simplified_circuit(self)
    }

    /// Appends an instruction, fusing it into the previous instruction when Stim formatting allows it.
    pub fn append_instruction(&mut self, instruction: CircuitInstruction) {
        self.push_instruction(instruction);
    }

    /// Appends a repeat block without modifying its body.
    pub fn append_repeat_block(&mut self, repeat: RepeatBlock) {
        self.push(CircuitItem::RepeatBlock(repeat));
    }

    pub fn to_stim_string(&self) -> String {
        let mut out = String::new();
        self.write_stim(&mut out, 0);
        out
    }

    /// Returns a copy of this circuit with all instruction and repeat-block tags removed.
    pub fn without_tags(&self) -> Self {
        Self {
            items: self.items.iter().map(CircuitItem::without_tags).collect(),
        }
    }

    fn push(&mut self, item: CircuitItem) {
        self.items.push(item);
    }

    fn push_instruction(&mut self, instruction: CircuitInstruction) {
        if let Some(CircuitItem::Instruction(previous)) = self.items.last_mut()
            && previous.try_fuse(&instruction)
        {
            return;
        }
        self.items.push(CircuitItem::Instruction(instruction));
    }

    fn write_stim(&self, out: &mut String, indent: usize) {
        for item in &self.items {
            item.write_stim(out, indent);
        }
    }

    pub(crate) fn write_stim_io(&self, out: &mut impl Write) -> io::Result<()> {
        self.write_stim_io_indented(out, 0)
    }

    fn write_stim_io_indented(&self, out: &mut impl Write, indent: usize) -> io::Result<()> {
        for item in &self.items {
            item.write_stim_io(out, indent)?;
        }
        Ok(())
    }

    fn item_slice(&self, range: impl RangeBounds<usize>) -> CircuitResult<&[CircuitItem]> {
        let range = checked_item_range(range, self.items.len())?;
        self.items
            .get(range)
            .ok_or_else(|| circuit_item_range_error("computed range was outside item list"))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum CircuitItem {
    Instruction(CircuitInstruction),
    RepeatBlock(RepeatBlock),
}

impl CircuitItem {
    pub fn as_instruction(&self) -> Option<&CircuitInstruction> {
        match self {
            Self::Instruction(instruction) => Some(instruction),
            Self::RepeatBlock(_) => None,
        }
    }

    pub fn as_repeat_block(&self) -> Option<&RepeatBlock> {
        match self {
            Self::Instruction(_) => None,
            Self::RepeatBlock(repeat) => Some(repeat),
        }
    }

    fn count_qubits(&self) -> usize {
        match self {
            Self::Instruction(instruction) => instruction.count_qubits(),
            Self::RepeatBlock(repeat) => repeat.body().count_qubits(),
        }
    }

    fn without_tags(&self) -> Self {
        match self {
            Self::Instruction(instruction) => Self::Instruction(instruction.without_tag()),
            Self::RepeatBlock(repeat) => Self::RepeatBlock(repeat.without_tags()),
        }
    }

    fn write_stim(&self, out: &mut String, indent: usize) {
        match self {
            Self::Instruction(instruction) => instruction.write_stim(out, indent),
            Self::RepeatBlock(repeat) => repeat.write_stim(out, indent),
        }
    }

    fn write_stim_io(&self, out: &mut impl Write, indent: usize) -> io::Result<()> {
        match self {
            Self::Instruction(instruction) => instruction.write_stim_io(out, indent),
            Self::RepeatBlock(repeat) => repeat.write_stim_io(out, indent),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CircuitInstruction {
    gate: Gate,
    args: Vec<f64>,
    targets: TargetVec,
    tag: Option<String>,
}

impl CircuitInstruction {
    /// Creates a Stim circuit instruction, normalizing empty tags to no tag.
    pub fn new(
        gate: Gate,
        args: Vec<f64>,
        targets: Vec<Target>,
        tag: Option<String>,
    ) -> CircuitResult<Self> {
        let targets = TargetVec::from_vec(targets);
        gate.validate(&args, &targets)?;
        Ok(Self::from_validated_parts(gate, args, targets, tag))
    }

    fn from_validated_parts(
        gate: Gate,
        args: Vec<f64>,
        targets: TargetVec,
        tag: Option<String>,
    ) -> Self {
        Self {
            gate,
            args,
            targets,
            tag: normalize_tag(tag),
        }
    }

    pub fn gate(&self) -> Gate {
        self.gate
    }

    pub fn args(&self) -> &[f64] {
        &self.args
    }

    /// Returns this instruction's optional probability argument when the gate has one.
    pub fn probability_argument(&self) -> CircuitResult<Option<Probability>> {
        if !matches!(
            self.gate.arg_rule(),
            ArgRule::ZeroOrOneProbability | ArgRule::ProbabilityList(1)
        ) {
            return Ok(None);
        }
        self.args
            .first()
            .copied()
            .map(|arg| probability_from_validated_arg(self.gate.canonical_name(), arg))
            .transpose()
    }

    /// Returns this instruction's disjoint probability-list arguments when the gate has them.
    pub fn probability_arguments(&self) -> CircuitResult<Option<Vec<Probability>>> {
        if !matches!(
            self.gate.arg_rule(),
            ArgRule::ProbabilityList(_) | ArgRule::AnyProbabilityList
        ) {
            return Ok(None);
        }
        self.args
            .iter()
            .copied()
            .map(|arg| probability_from_validated_arg(self.gate.canonical_name(), arg))
            .collect::<CircuitResult<Vec<_>>>()
            .map(Some)
    }

    /// Returns this instruction's observable id argument when the gate has one.
    pub fn observable_id_argument(&self) -> CircuitResult<Option<ObservableId>> {
        if self.gate.arg_rule() != ArgRule::UnsignedInteger {
            return Ok(None);
        }
        self.args
            .first()
            .copied()
            .map(|arg| observable_id_from_validated_arg(self.gate.canonical_name(), arg))
            .transpose()
    }

    /// Returns coordinate-like arguments for gates whose argument list is arbitrary floats.
    pub fn coordinate_arguments(&self) -> Option<&[f64]> {
        (self.gate.arg_rule() == ArgRule::Any).then_some(&self.args)
    }

    pub fn targets(&self) -> &[Target] {
        &self.targets
    }

    fn count_qubits(&self) -> usize {
        self.targets
            .iter()
            .filter_map(Target::qubit_id)
            .map(|qubit| qubit.get() as usize + 1)
            .max()
            .unwrap_or(0)
    }

    /// Returns the non-empty Stim tag attached to this instruction.
    pub fn tag(&self) -> Option<&str> {
        self.tag.as_deref()
    }

    pub fn target_groups(&self) -> Vec<&[Target]> {
        match self.gate.target_group_kind() {
            GateTargetGroupKind::None => Vec::new(),
            GateTargetGroupKind::Singles => self.targets.chunks(1).collect(),
            GateTargetGroupKind::Pairs => self.targets.chunks(2).collect(),
            GateTargetGroupKind::PauliProducts => pauli_product_target_groups(&self.targets),
            GateTargetGroupKind::AllTargets => {
                if self.targets.is_empty() {
                    Vec::new()
                } else {
                    vec![self.targets.as_slice()]
                }
            }
        }
    }

    /// Splits this instruction into maximal segments whose target groups touch disjoint qubits.
    pub fn disjoint_target_segments(&self) -> Vec<Self> {
        let mut segments = Vec::new();
        let mut current_targets = Vec::new();
        let mut current_qubits = Vec::new();

        for group in self.target_groups() {
            let group_qubits = group
                .iter()
                .filter_map(Target::qubit_id)
                .collect::<Vec<_>>();
            if group_qubits
                .iter()
                .any(|qubit| current_qubits.contains(qubit))
                && !current_targets.is_empty()
            {
                segments.push(self.with_targets(current_targets));
                current_targets = Vec::new();
                current_qubits = Vec::new();
            }
            for qubit in group_qubits {
                if !current_qubits.contains(&qubit) {
                    current_qubits.push(qubit);
                }
            }
            current_targets.extend_from_slice(group);
        }

        if !current_targets.is_empty() {
            segments.push(self.with_targets(current_targets));
        }

        segments
    }

    /// Splits this instruction from the end into maximal segments whose target groups touch disjoint qubits.
    pub fn disjoint_target_segments_reversed(&self) -> Vec<Self> {
        let mut segments = Vec::new();
        let mut current_targets = Vec::new();
        let mut current_qubits = Vec::new();

        for group in self.target_groups().into_iter().rev() {
            let group_qubits = group
                .iter()
                .filter_map(Target::qubit_id)
                .collect::<Vec<_>>();
            if group_qubits
                .iter()
                .any(|qubit| current_qubits.contains(qubit))
                && !current_targets.is_empty()
            {
                segments.push(self.with_targets(current_targets));
                current_targets = Vec::new();
                current_qubits = Vec::new();
            }
            for qubit in group_qubits {
                if !current_qubits.contains(&qubit) {
                    current_qubits.push(qubit);
                }
            }
            let mut next_targets = group.to_vec();
            next_targets.extend(current_targets);
            current_targets = next_targets;
        }

        if !current_targets.is_empty() {
            segments.push(self.with_targets(current_targets));
        }

        segments
    }

    fn without_tag(&self) -> Self {
        Self {
            gate: self.gate,
            args: self.args.clone(),
            targets: self.targets.clone(),
            tag: None,
        }
    }

    fn try_fuse(&mut self, other: &Self) -> bool {
        if !self.can_fuse(other) {
            return false;
        }
        self.targets.extend(other.targets.iter().cloned());
        true
    }

    fn can_fuse(&self, other: &Self) -> bool {
        self.gate == other.gate
            && self.args == other.args
            && self.tag == other.tag
            && self.gate.can_fuse()
    }

    fn with_targets(&self, targets: Vec<Target>) -> Self {
        Self {
            gate: self.gate,
            args: self.args.clone(),
            targets: TargetVec::from_vec(targets),
            tag: self.tag.clone(),
        }
    }

    fn write_stim(&self, out: &mut String, indent: usize) {
        write_indent(out, indent);
        out.push_str(self.gate.canonical_name());
        if let Some(tag) = &self.tag {
            out.push('[');
            write_escaped_tag(out, tag);
            out.push(']');
        }
        if !self.args.is_empty() {
            out.push('(');
            for (index, arg) in self.args.iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                out.push_str(&format_float(*arg));
            }
            out.push(')');
        }
        write_targets(out, &self.targets);
        out.push('\n');
    }

    fn write_stim_io(&self, out: &mut impl Write, indent: usize) -> io::Result<()> {
        write_indent_io(out, indent)?;
        out.write_all(self.gate.canonical_name().as_bytes())?;
        if let Some(tag) = &self.tag {
            out.write_all(b"[")?;
            write_escaped_tag_io(out, tag)?;
            out.write_all(b"]")?;
        }
        if !self.args.is_empty() {
            out.write_all(b"(")?;
            for (index, arg) in self.args.iter().enumerate() {
                if index > 0 {
                    out.write_all(b", ")?;
                }
                out.write_all(format_float(*arg).as_bytes())?;
            }
            out.write_all(b")")?;
        }
        write_targets_io(out, &self.targets)?;
        out.write_all(b"\n")
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RepeatBlock {
    repeat_count: RepeatCount,
    body: Circuit,
    tag: Option<String>,
}

impl RepeatBlock {
    /// Creates a `REPEAT` block, normalizing empty tags to no tag.
    pub fn new(repeat_count: RepeatCount, body: Circuit, tag: Option<String>) -> Self {
        Self {
            repeat_count,
            body,
            tag: normalize_tag(tag),
        }
    }

    /// Returns how many times the block body repeats.
    pub fn repeat_count(&self) -> RepeatCount {
        self.repeat_count
    }

    /// Returns the repeated body circuit.
    pub fn body(&self) -> &Circuit {
        &self.body
    }

    /// Returns the non-empty tag attached to this `REPEAT` block.
    pub fn tag(&self) -> Option<&str> {
        self.tag.as_deref()
    }

    fn without_tags(&self) -> Self {
        Self {
            repeat_count: self.repeat_count,
            body: self.body.without_tags(),
            tag: None,
        }
    }

    fn write_stim(&self, out: &mut String, indent: usize) {
        write_indent(out, indent);
        out.push_str("REPEAT");
        if let Some(tag) = &self.tag {
            out.push('[');
            write_escaped_tag(out, tag);
            out.push(']');
        }
        out.push(' ');
        out.push_str(&self.repeat_count.get().to_string());
        out.push_str(" {\n");
        self.body.write_stim(out, indent + 4);
        write_indent(out, indent);
        out.push_str("}\n");
    }

    fn write_stim_io(&self, out: &mut impl Write, indent: usize) -> io::Result<()> {
        write_indent_io(out, indent)?;
        out.write_all(b"REPEAT")?;
        if let Some(tag) = &self.tag {
            out.write_all(b"[")?;
            write_escaped_tag_io(out, tag)?;
            out.write_all(b"]")?;
        }
        writeln!(out, " {} {{", self.repeat_count.get())?;
        self.body.write_stim_io_indented(out, indent + 4)?;
        write_indent_io(out, indent)?;
        out.write_all(b"}\n")
    }
}

impl Display for Circuit {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_stim_string())
    }
}

struct Parser<'a> {
    lines: Lines<'a>,
    line_number: usize,
    top_level_capacity: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            lines: input.lines(),
            line_number: 0,
            top_level_capacity: top_level_item_capacity(input),
        }
    }

    fn parse(mut self) -> CircuitResult<Circuit> {
        self.parse_block(false, 0)
    }

    fn parse_block(&mut self, stop_on_terminator: bool, depth: usize) -> CircuitResult<Circuit> {
        let mut circuit = if stop_on_terminator {
            Circuit::new()
        } else {
            Circuit::with_capacity(self.top_level_capacity)
        };
        while let Some(raw_line) = self.lines.next() {
            self.line_number += 1;
            let line_number = self.line_number;
            if line_number > MAX_CIRCUIT_PARSE_LINES {
                return Err(CircuitError::parse_line(
                    line_number,
                    format!("circuit input has more than {MAX_CIRCUIT_PARSE_LINES} lines"),
                ));
            }
            let Some(line) = strip_comment(raw_line) else {
                continue;
            };
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if line == "}" {
                if stop_on_terminator {
                    return Ok(circuit);
                }
                return Err(CircuitError::UnexpectedRepeatTerminator);
            }
            if let Some(prefix) = line.strip_suffix('{') {
                circuit.push(CircuitItem::RepeatBlock(self.parse_repeat(
                    line_number,
                    prefix.trim_end(),
                    depth,
                )?));
            } else {
                circuit.push_instruction(parse_instruction(line_number, line)?);
            }
        }
        if stop_on_terminator {
            Err(CircuitError::UnterminatedRepeatBlock)
        } else {
            Ok(circuit)
        }
    }

    fn parse_repeat(
        &mut self,
        line_number: usize,
        line: &str,
        depth: usize,
    ) -> CircuitResult<RepeatBlock> {
        if depth >= MAX_CIRCUIT_REPEAT_NESTING {
            return Err(CircuitError::parse_line(
                line_number,
                format!("repeat nesting exceeds current limit {MAX_CIRCUIT_REPEAT_NESTING}"),
            ));
        }
        let (name, rest) = parse_name(line_number, line)?;
        if !name.eq_ignore_ascii_case("REPEAT") {
            return Err(CircuitError::parse_line(
                line_number,
                "repeat blocks must be written as REPEAT <count> {",
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
                "repeat blocks must be written as REPEAT <count> {",
            ));
        }
        let count = count
            .parse::<u64>()
            .map_err(|_| CircuitError::parse_line(line_number, "invalid repeat count"))?;
        let body = self.parse_block(true, depth + 1)?;
        Ok(RepeatBlock::new(RepeatCount::try_new(count)?, body, tag))
    }
}

fn top_level_item_capacity(input: &str) -> usize {
    let newline_count = input.bytes().filter(|byte| *byte == b'\n').count();
    newline_count + usize::from(!input.is_empty() && !input.ends_with('\n'))
}

fn parse_instruction(line_number: usize, line: &str) -> CircuitResult<CircuitInstruction> {
    if let Some(instruction) = parse_common_plain_instruction(line_number, line) {
        return instruction;
    }
    let (name, rest) = parse_name(line_number, line)?;
    if let Some(instruction) = parse_simple_plain_instruction(line_number, name, rest) {
        return instruction;
    }
    let gate = Gate::from_name(name).map_err(|error| wrap_line(line_number, error))?;
    let (tag, rest) = parse_optional_tag(line_number, rest)?;
    let (args, rest) = parse_optional_args(line_number, rest)?;
    let targets = parse_targets(rest).map_err(|error| wrap_line(line_number, error))?;
    gate.validate(&args, &targets)
        .map_err(|error| wrap_line(line_number, error))?;
    Ok(CircuitInstruction::from_validated_parts(
        gate, args, targets, tag,
    ))
}

fn parse_common_plain_instruction(
    line_number: usize,
    line: &str,
) -> Option<CircuitResult<CircuitInstruction>> {
    if let Some(rest) = line.strip_prefix("H ") {
        return parse_common_single_qubit_instruction(line_number, Gate::plain_h(), rest);
    }
    if let Some(rest) = line.strip_prefix("M ").or_else(|| line.strip_prefix("MZ ")) {
        return parse_common_single_qubit_instruction(line_number, Gate::plain_m(), rest);
    }
    if let Some(rest) = line
        .strip_prefix("CX ")
        .or_else(|| line.strip_prefix("CNOT "))
    {
        return parse_common_pair_instruction(line_number, Gate::plain_cx(), rest);
    }
    None
}

fn parse_common_single_qubit_instruction(
    line_number: usize,
    gate: Gate,
    rest: &str,
) -> Option<CircuitResult<CircuitInstruction>> {
    let target = match parse_common_qubit_id(rest) {
        Ok(Some(target)) => target,
        Ok(None) => return None,
        Err(error) => return Some(Err(wrap_line(line_number, error))),
    };
    let mut targets = TargetVec::new();
    targets.push(Target::qubit(target, false));
    Some(Ok(CircuitInstruction::from_validated_parts(
        gate,
        Vec::new(),
        targets,
        None,
    )))
}

fn parse_common_pair_instruction(
    line_number: usize,
    gate: Gate,
    rest: &str,
) -> Option<CircuitResult<CircuitInstruction>> {
    let (left, right) = rest.split_once(' ')?;
    let left = match parse_common_qubit_id(left) {
        Ok(Some(target)) => target,
        Ok(None) => return None,
        Err(error) => return Some(Err(wrap_line(line_number, error))),
    };
    let right = match parse_common_qubit_id(right) {
        Ok(Some(target)) => target,
        Ok(None) => return None,
        Err(error) => return Some(Err(wrap_line(line_number, error))),
    };
    if left == right {
        return Some(Err(wrap_line(
            line_number,
            CircuitError::InvalidTarget {
                gate: gate.canonical_name(),
                target: left.get().to_string(),
            },
        )));
    }
    let mut targets = TargetVec::new();
    targets.push(Target::qubit(left, false));
    targets.push(Target::qubit(right, false));
    Some(Ok(CircuitInstruction::from_validated_parts(
        gate,
        Vec::new(),
        targets,
        None,
    )))
}

fn parse_common_qubit_id(text: &str) -> CircuitResult<Option<crate::QubitId>> {
    if text.is_empty() || !text.as_bytes().iter().all(u8::is_ascii_digit) {
        return Ok(None);
    }
    let mut value = 0u32;
    for byte in text.bytes() {
        let digit = u32::from(byte - b'0');
        value = value
            .checked_mul(10)
            .and_then(|value| value.checked_add(digit))
            .ok_or_else(|| CircuitError::invalid_domain_value("qubit target", text))?;
        if value >= crate::ids::STIM_TARGET_VALUE_LIMIT {
            return Err(CircuitError::invalid_domain_value("qubit target", text));
        }
    }
    crate::QubitId::new(value).map(Some)
}

fn parse_simple_plain_instruction(
    line_number: usize,
    name: &str,
    rest: &str,
) -> Option<CircuitResult<CircuitInstruction>> {
    let gate = Gate::from_simple_plain_name(name)?;
    let rest = rest.trim_start();
    if rest.starts_with('[') || rest.starts_with('(') {
        return None;
    }
    let targets = match parse_plain_qubit_target_text(rest) {
        Ok(Some(targets)) => targets,
        Ok(None) => return None,
        Err(error) => return Some(Err(wrap_line(line_number, error))),
    };
    let gate_name = gate.canonical_name();
    if gate_name == "CX"
        && let Err(error) = validate_simple_plain_pairs(gate_name, &targets)
    {
        return Some(Err(wrap_line(line_number, error)));
    }
    Some(Ok(CircuitInstruction::from_validated_parts(
        gate,
        Vec::new(),
        targets,
        None,
    )))
}

fn validate_simple_plain_pairs(gate: &'static str, targets: &[Target]) -> CircuitResult<()> {
    if !targets.len().is_multiple_of(2) {
        return Err(CircuitError::InvalidTargetCount {
            gate,
            count: targets.len(),
        });
    }
    for pair in targets.chunks_exact(2) {
        if let [left, right] = pair
            && left == right
        {
            return Err(CircuitError::InvalidTarget {
                gate,
                target: left.to_string(),
            });
        }
    }
    Ok(())
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
        end.ok_or_else(|| CircuitError::parse_line(line_number, "missing instruction name"))?;
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

fn parse_targets(rest: &str) -> CircuitResult<TargetVec> {
    if let Some(targets) = parse_plain_qubit_target_text(rest)? {
        return Ok(targets);
    }

    let mut targets = TargetVec::new();
    for token in rest.split_whitespace() {
        parse_target_token_into(token, &mut targets)?;
    }
    Ok(targets)
}

fn pauli_product_target_groups(targets: &[Target]) -> Vec<&[Target]> {
    let mut groups = Vec::new();
    let mut start = 0;
    while start < targets.len() {
        let mut end = start + 1;
        while matches!(targets.get(end), Some(target) if target.is_combiner()) {
            end = (end + 2).min(targets.len());
        }
        if let Some(group) = targets.get(start..end) {
            groups.push(group);
        }
        start = end;
    }
    groups
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

fn write_indent(out: &mut String, indent: usize) {
    out.extend(std::iter::repeat_n(' ', indent));
}

fn write_indent_io(out: &mut impl Write, indent: usize) -> io::Result<()> {
    for _ in 0..indent {
        out.write_all(b" ")?;
    }
    Ok(())
}

fn write_targets(out: &mut String, targets: &[Target]) {
    let mut pending_combiner = false;
    for target in targets {
        if target.is_combiner() {
            pending_combiner = true;
            continue;
        }
        if pending_combiner {
            out.push('*');
            pending_combiner = false;
        } else {
            out.push(' ');
        }
        out.push_str(&target.to_string());
    }
}

fn write_targets_io(out: &mut impl Write, targets: &[Target]) -> io::Result<()> {
    let mut pending_combiner = false;
    for target in targets {
        if target.is_combiner() {
            pending_combiner = true;
            continue;
        }
        if pending_combiner {
            out.write_all(b"*")?;
            pending_combiner = false;
        } else {
            out.write_all(b" ")?;
        }
        out.write_all(target.to_string().as_bytes())?;
    }
    Ok(())
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

fn write_escaped_tag_io(out: &mut impl Write, tag: &str) -> io::Result<()> {
    for ch in tag.chars() {
        match ch {
            ']' => out.write_all(b"\\C")?,
            '\r' => out.write_all(b"\\r")?,
            '\n' => out.write_all(b"\\n")?,
            '\\' => out.write_all(b"\\B")?,
            _ => {
                let mut buffer = [0; 4];
                out.write_all(ch.encode_utf8(&mut buffer).as_bytes())?;
            }
        }
    }
    Ok(())
}

fn normalize_tag(tag: Option<String>) -> Option<String> {
    tag.filter(|tag| !tag.is_empty())
}

fn probability_from_validated_arg(gate: &'static str, arg: f64) -> CircuitResult<Probability> {
    Probability::try_new(arg).map_err(|_| CircuitError::InvalidArgument {
        gate,
        argument: arg.to_string(),
    })
}

fn observable_id_from_validated_arg(gate: &'static str, arg: f64) -> CircuitResult<ObservableId> {
    if !arg.is_finite() || arg < 0.0 || arg.fract() != 0.0 {
        return Err(CircuitError::InvalidArgument {
            gate,
            argument: arg.to_string(),
        });
    }
    let value = format!("{arg:.0}")
        .parse::<u64>()
        .map_err(|_| CircuitError::InvalidArgument {
            gate,
            argument: arg.to_string(),
        })?;
    Ok(ObservableId::new(value))
}

fn format_float(value: f64) -> String {
    if let Some(integer) = stim_integer_like_i64(value) {
        return integer.to_string();
    }

    let scientific = format!("{value:.5e}");
    let Some((mantissa, exponent)) = scientific.split_once('e') else {
        return value.to_string();
    };
    let Ok(exponent) = exponent.parse::<i32>() else {
        return value.to_string();
    };

    if (-4..6).contains(&exponent) {
        let decimal_places = usize::try_from(5 - exponent).unwrap_or(0);
        trim_decimal_float(format!("{value:.decimal_places$}"))
    } else {
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

fn wrap_line(line: usize, error: CircuitError) -> CircuitError {
    CircuitError::parse_line(line, error.to_string())
}
