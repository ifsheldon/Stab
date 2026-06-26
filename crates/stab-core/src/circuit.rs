use std::fmt::{Display, Formatter};

use crate::gate::{ArgRule, TargetGroupKind};
use crate::target::parse_target_token;
use crate::{CircuitError, CircuitResult, Gate, ObservableId, Probability, RepeatCount, Target};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Circuit {
    items: Vec<CircuitItem>,
}

impl Circuit {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn from_stim_str(input: &str) -> CircuitResult<Self> {
        Parser::new(input).parse()
    }

    pub fn items(&self) -> &[CircuitItem] {
        &self.items
    }

    pub fn count_qubits(&self) -> usize {
        self.items
            .iter()
            .map(CircuitItem::count_qubits)
            .max()
            .unwrap_or(0)
    }

    pub fn to_tableau(
        &self,
        ignore_noise: bool,
        ignore_measurement: bool,
        ignore_reset: bool,
    ) -> CircuitResult<crate::Tableau> {
        crate::circuit_to_tableau(self, ignore_noise, ignore_measurement, ignore_reset)
    }

    pub fn inverse_unitary(&self) -> CircuitResult<Self> {
        crate::circuit_inverse_unitary(self)
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
}

#[derive(Clone, Debug, PartialEq)]
pub enum CircuitItem {
    Instruction(CircuitInstruction),
    RepeatBlock(RepeatBlock),
}

impl CircuitItem {
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
}

#[derive(Clone, Debug, PartialEq)]
pub struct CircuitInstruction {
    gate: Gate,
    args: Vec<f64>,
    targets: Vec<Target>,
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
        gate.validate(&args, &targets)?;
        Ok(Self {
            gate,
            args,
            targets,
            tag: normalize_tag(tag),
        })
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
        if !matches!(self.gate.arg_rule(), ArgRule::ProbabilityList(_)) {
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
            TargetGroupKind::None => Vec::new(),
            TargetGroupKind::Singles => self.targets.chunks(1).collect(),
            TargetGroupKind::Pairs => self.targets.chunks(2).collect(),
            TargetGroupKind::PauliProducts => pauli_product_target_groups(&self.targets),
            TargetGroupKind::AllTargets => {
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
        self.targets.extend_from_slice(&other.targets);
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
            targets,
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
}

impl Display for Circuit {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_stim_string())
    }
}

struct Parser<'a> {
    lines: Vec<&'a str>,
    index: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            lines: input.lines().collect(),
            index: 0,
        }
    }

    fn parse(mut self) -> CircuitResult<Circuit> {
        let circuit = self.parse_block(false)?;
        if self.index < self.lines.len() {
            return Err(CircuitError::UnexpectedRepeatTerminator);
        }
        Ok(circuit)
    }

    fn parse_block(&mut self, stop_on_terminator: bool) -> CircuitResult<Circuit> {
        let mut circuit = Circuit::new();
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
                    return Ok(circuit);
                }
                return Err(CircuitError::UnexpectedRepeatTerminator);
            }
            if let Some(prefix) = line.strip_suffix('{') {
                circuit.push(CircuitItem::RepeatBlock(
                    self.parse_repeat(line_number, prefix.trim_end())?,
                ));
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

    fn parse_repeat(&mut self, line_number: usize, line: &str) -> CircuitResult<RepeatBlock> {
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
        let body = self.parse_block(true)?;
        Ok(RepeatBlock::new(RepeatCount::try_new(count)?, body, tag))
    }
}

fn parse_instruction(line_number: usize, line: &str) -> CircuitResult<CircuitInstruction> {
    let (name, rest) = parse_name(line_number, line)?;
    let gate = Gate::from_name(name).map_err(|error| wrap_line(line_number, error))?;
    let (tag, rest) = parse_optional_tag(line_number, rest)?;
    let (args, rest) = parse_optional_args(line_number, rest)?;
    let targets = parse_targets(rest).map_err(|error| wrap_line(line_number, error))?;
    CircuitInstruction::new(gate, args, targets, tag).map_err(|error| wrap_line(line_number, error))
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
        .ok_or_else(|| CircuitError::parse_line(line_number, "missing instruction name"))?;
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

fn parse_targets(rest: &str) -> CircuitResult<Vec<Target>> {
    let mut targets = Vec::new();
    for token in rest.split_whitespace() {
        targets.extend(parse_target_token(token)?);
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
