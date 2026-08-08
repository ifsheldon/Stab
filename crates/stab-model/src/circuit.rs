use std::fmt::{Display, Formatter};
use std::io::{self, Write};
use std::ops::RangeBounds;

use crate::gate::GateTargetGroupKind;
use crate::model_bytes::PreparedModelText;
use crate::model_tag::ModelTag;
use crate::target::TargetVec;
use crate::{
    Gate, GateArgumentRule, ModelDialect, ModelError, ModelFingerprint, ModelResult, ObservableId,
    ParseLimits, Probability, RepeatCount, Target, ValidationError,
};

mod api;
mod counts;
mod iter;

pub use iter::{CircuitFlattenedInstructionIter, CircuitFlattenedInstructionRevIter};

use self::iter::{checked_item_range, circuit_item_range_error};

#[derive(Default)]
pub struct Circuit {
    items: Vec<CircuitItem>,
}

impl std::fmt::Debug for Circuit {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        // Nested bodies are elided so debug-formatting a deeply nested
        // API-built circuit cannot recurse off the stack.
        formatter
            .debug_struct("Circuit")
            .field("top_level_items", &self.items.len())
            .finish_non_exhaustive()
    }
}

impl Clone for Circuit {
    fn clone(&self) -> Self {
        drop_impl::clone_circuit(self)
    }
}

impl PartialEq for Circuit {
    fn eq(&self, other: &Self) -> bool {
        drop_impl::circuits_equal(self, other)
    }
}

impl Drop for Circuit {
    fn drop(&mut self) {
        drop_impl::drop_items(&mut self.items);
    }
}

#[derive(Debug)]
pub(crate) struct CircuitAssembler {
    circuit: Circuit,
}

impl CircuitAssembler {
    pub(crate) fn new() -> Self {
        Self {
            circuit: Circuit::new(),
        }
    }

    pub(crate) fn from_unfused_items(items: Vec<CircuitItem>) -> Self {
        Self {
            circuit: Circuit { items },
        }
    }

    pub(crate) fn try_reserve_exact(&mut self, additional: usize) -> ModelResult<()> {
        self.circuit.try_reserve_items_exact(additional)
    }

    pub(crate) fn try_append_instruction(
        &mut self,
        instruction: CircuitInstruction,
    ) -> ModelResult<()> {
        self.circuit.try_append_instruction(instruction)
    }

    pub(crate) fn try_append_repeat_block(&mut self, repeat: RepeatBlock) -> ModelResult<()> {
        self.circuit.try_append_repeat_block(repeat)
    }

    pub(crate) fn finish(self) -> Circuit {
        self.circuit
    }
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

    pub fn from_stim_str(input: &str) -> ModelResult<Self> {
        Self::from_stim_str_with_limits(input, ParseLimits::default())
    }

    pub fn from_stim_bytes(input: &[u8]) -> ModelResult<Self> {
        Self::from_stim_bytes_with_limits(input, ParseLimits::default())
    }

    pub fn from_stim_bytes_with_limits(input: &[u8], limits: ParseLimits) -> ModelResult<Self> {
        let prepared = PreparedModelText::new(input, ModelDialect::StimCircuit, limits)?;
        let parsed = if prepared.requires_tag_restore() {
            parser::parse_circuit_unfused(prepared.text(), limits)
        } else {
            parser::parse_circuit(prepared.text(), limits)
        };
        let parsed = prepared.resolve(parsed);
        let mut circuit = parsed?;
        if let Some(tags) = prepared.into_tags() {
            circuit.restore_byte_tags(tags)?;
            circuit.fuse_adjacent_instructions();
        }
        Ok(circuit)
    }

    pub fn from_stim_str_with_limits(input: &str, limits: ParseLimits) -> ModelResult<Self> {
        parser::parse_circuit(input, limits)
    }

    /// Returns the schema-versioned structural identity of this circuit.
    ///
    /// The fingerprint is independent of accepted textual spelling and printer precision. It
    /// identifies this source model only, not compiler options, a backend, or an executable plan.
    pub fn fingerprint(&self) -> ModelFingerprint {
        ModelFingerprint::for_circuit(self)
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
    ) -> ModelResult<impl DoubleEndedIterator<Item = &CircuitItem> + ExactSizeIterator> {
        Ok(self.item_slice(range)?.iter())
    }

    pub fn instruction_range(
        &self,
        range: impl RangeBounds<usize>,
    ) -> ModelResult<impl DoubleEndedIterator<Item = &CircuitInstruction>> {
        let range = checked_item_range(range, self.items.len())?;
        let items = self.item_slice(range.clone())?;
        for (offset, item) in items.iter().enumerate() {
            if matches!(item, CircuitItem::RepeatBlock(_)) {
                return Err(ModelError::invalid_domain_value(
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

    /// Appends an instruction, fusing it into the previous instruction when Stim formatting allows it.
    pub fn append_instruction(&mut self, instruction: CircuitInstruction) {
        self.push_instruction(instruction);
    }

    pub(crate) fn try_reserve_items_exact(&mut self, additional: usize) -> ModelResult<()> {
        self.items.try_reserve_exact(additional).map_err(|error| {
            ModelError::invalid_domain_value(
                "circuit allocation",
                format!("unable to reserve {additional} item slots: {error}"),
            )
        })
    }

    pub(crate) fn try_append_instruction(
        &mut self,
        instruction: CircuitInstruction,
    ) -> ModelResult<()> {
        if let Some(CircuitItem::Instruction(previous)) = self.items.last_mut()
            && previous.can_fuse(&instruction)
        {
            let additional = instruction.targets.len();
            previous
                .targets
                .try_reserve_exact(additional)
                .map_err(|error| {
                    ModelError::invalid_domain_value(
                        "circuit target allocation",
                        format!("unable to reserve {additional} target slots: {error}"),
                    )
                })?;
            previous.targets.extend(instruction.targets);
            return Ok(());
        }

        self.items.try_reserve(1).map_err(|error| {
            ModelError::invalid_domain_value(
                "circuit allocation",
                format!("unable to reserve an instruction slot: {error}"),
            )
        })?;
        self.items.push(CircuitItem::Instruction(instruction));
        Ok(())
    }

    /// Appends a repeat block without modifying its body.
    pub fn append_repeat_block(&mut self, repeat: RepeatBlock) {
        self.push(CircuitItem::RepeatBlock(repeat));
    }

    pub(crate) fn try_append_repeat_block(&mut self, repeat: RepeatBlock) -> ModelResult<()> {
        self.items.try_reserve(1).map_err(|error| {
            ModelError::invalid_domain_value(
                "circuit allocation",
                format!("unable to reserve a repeat-block slot: {error}"),
            )
        })?;
        self.items.push(CircuitItem::RepeatBlock(repeat));
        Ok(())
    }

    /// Returns canonical Stim text as UTF-8.
    ///
    /// Opaque tag bytes are represented with the UTF-8 replacement character. Use
    /// [`Self::to_stim_bytes`] when exact metadata preservation matters.
    pub fn to_stim_string(&self) -> String {
        let mut out = String::with_capacity(printing::stim_text_capacity(self, 0));
        self.write_stim(&mut out, 0);
        out
    }

    /// Returns canonical Stim text while preserving opaque bytes in tags.
    ///
    /// Use this method instead of [`Self::to_stim_string`] when the circuit came from a byte
    /// source whose tags may not be valid UTF-8.
    pub fn to_stim_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(printing::stim_text_capacity(self, 0));
        match self.write_stim_io(&mut out) {
            Ok(()) => out,
            Err(error) => unreachable!("writing a circuit into Vec<u8> failed: {error}"),
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

    /// Writes canonical Stim bytes to an existing output stream.
    ///
    /// This operation does not open, create, or truncate filesystem paths.
    pub fn write_stim_io(&self, out: &mut impl Write) -> io::Result<()> {
        self.write_stim_io_indented(out, 0)
    }

    fn write_stim_io_indented(&self, out: &mut impl Write, indent: usize) -> io::Result<()> {
        for item in &self.items {
            item.write_stim_io(out, indent)?;
        }
        Ok(())
    }

    fn restore_byte_tags(&mut self, tags: Vec<Vec<u8>>) -> ModelResult<()> {
        let expected = self.non_empty_tag_count();
        if expected != tags.len() {
            return Err(ModelError::invalid_domain_value(
                "Stim byte parser",
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
                CircuitItem::Instruction(instruction) => {
                    if instruction.tag.is_some() {
                        instruction.tag = tags.next().and_then(ModelTag::from_bytes);
                    }
                }
                CircuitItem::RepeatBlock(repeat) => {
                    if repeat.tag.is_some() {
                        repeat.tag = tags.next().and_then(ModelTag::from_bytes);
                    }
                    repeat.body.restore_byte_tags_from(tags);
                }
            }
        }
    }

    fn fuse_adjacent_instructions(&mut self) {
        let items = std::mem::take(&mut self.items);
        for item in items {
            match item {
                CircuitItem::Instruction(instruction) => self.push_instruction(instruction),
                CircuitItem::RepeatBlock(mut repeat) => {
                    repeat.body.fuse_adjacent_instructions();
                    self.push(CircuitItem::RepeatBlock(repeat));
                }
            }
        }
    }

    fn non_empty_tag_count(&self) -> usize {
        self.items
            .iter()
            .map(|item| match item {
                CircuitItem::Instruction(instruction) => usize::from(instruction.tag.is_some()),
                CircuitItem::RepeatBlock(repeat) => {
                    usize::from(repeat.tag.is_some()) + repeat.body.non_empty_tag_count()
                }
            })
            .sum()
    }

    fn item_slice(&self, range: impl RangeBounds<usize>) -> ModelResult<&[CircuitItem]> {
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
    tag: Option<ModelTag>,
}

impl CircuitInstruction {
    /// Creates a Stim circuit instruction, normalizing empty tags to no tag.
    pub fn new(
        gate: Gate,
        args: Vec<f64>,
        targets: Vec<Target>,
        tag: Option<String>,
    ) -> ModelResult<Self> {
        let targets = TargetVec::from_vec(targets);
        gate.validate(&args, &targets)?;
        Ok(Self::from_validated_parts(gate, args, targets, tag))
    }

    pub(crate) fn new_with_tag_bytes(
        gate: Gate,
        args: Vec<f64>,
        targets: Vec<Target>,
        tag: Option<&[u8]>,
    ) -> ModelResult<Self> {
        let targets = TargetVec::from_vec(targets);
        gate.validate(&args, &targets)?;
        Ok(Self {
            gate,
            args,
            targets,
            tag: tag.and_then(ModelTag::from_slice),
        })
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
            tag: tag.and_then(ModelTag::from_string),
        }
    }

    pub fn gate(&self) -> Gate {
        self.gate
    }

    pub fn args(&self) -> &[f64] {
        &self.args
    }

    /// Returns this instruction's optional probability argument when the gate has one.
    pub fn probability_argument(&self) -> ModelResult<Option<Probability>> {
        if !matches!(
            self.gate.argument_rule(),
            GateArgumentRule::OptionalProbability | GateArgumentRule::ProbabilityList(1)
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
    pub fn probability_arguments(&self) -> ModelResult<Option<Vec<Probability>>> {
        if !matches!(
            self.gate.argument_rule(),
            GateArgumentRule::ProbabilityList(_) | GateArgumentRule::AnyProbabilityList
        ) {
            return Ok(None);
        }
        self.args
            .iter()
            .copied()
            .map(|arg| probability_from_validated_arg(self.gate.canonical_name(), arg))
            .collect::<ModelResult<Vec<_>>>()
            .map(Some)
    }

    /// Returns this instruction's observable id argument when the gate has one.
    pub fn observable_id_argument(&self) -> ModelResult<Option<ObservableId>> {
        if self.gate.argument_rule() != GateArgumentRule::UnsignedInteger {
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
        (self.gate.argument_rule() == GateArgumentRule::Any).then_some(&self.args)
    }

    pub fn targets(&self) -> &[Target] {
        &self.targets
    }

    /// Returns the non-empty Stim tag attached to this instruction as UTF-8 display text.
    ///
    /// Opaque bytes are represented with the UTF-8 replacement character. Use
    /// [`Self::tag_bytes`] when exact metadata preservation matters.
    pub fn tag(&self) -> Option<&str> {
        self.tag.as_ref().map(ModelTag::as_str)
    }

    /// Returns the exact unescaped bytes of this instruction's optional Stim tag.
    pub fn tag_bytes(&self) -> Option<&[u8]> {
        self.tag.as_ref().map(ModelTag::as_bytes)
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

    /// Returns [`Self::target_groups`]'s length without materializing the groups.
    pub(crate) fn target_group_count(&self) -> usize {
        match self.gate.target_group_kind() {
            GateTargetGroupKind::None => 0,
            GateTargetGroupKind::Singles => self.targets.len(),
            GateTargetGroupKind::Pairs => self.targets.len() / 2,
            GateTargetGroupKind::PauliProducts => pauli_product_target_group_count(&self.targets),
            GateTargetGroupKind::AllTargets => usize::from(!self.targets.is_empty()),
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

    pub(crate) fn without_tag(&self) -> Self {
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
            tag.write_escaped_text(out);
            out.push(']');
        }
        if !self.args.is_empty() {
            out.push('(');
            for (index, arg) in self.args.iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                out.push_str(printing::format_float(*arg).as_str());
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
            tag.write_escaped_bytes(out)?;
            out.write_all(b"]")?;
        }
        if !self.args.is_empty() {
            out.write_all(b"(")?;
            for (index, arg) in self.args.iter().enumerate() {
                if index > 0 {
                    out.write_all(b", ")?;
                }
                out.write_all(printing::format_float(*arg).as_bytes())?;
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
    tag: Option<ModelTag>,
}

impl RepeatBlock {
    /// Creates a `REPEAT` block, normalizing empty tags to no tag.
    pub fn new(repeat_count: RepeatCount, body: Circuit, tag: Option<String>) -> Self {
        Self {
            repeat_count,
            body,
            tag: tag.and_then(ModelTag::from_string),
        }
    }

    pub(crate) fn new_with_tag_bytes(
        repeat_count: RepeatCount,
        body: Circuit,
        tag: Option<&[u8]>,
    ) -> Self {
        Self {
            repeat_count,
            body,
            tag: tag.and_then(ModelTag::from_slice),
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

    /// Returns the non-empty tag attached to this `REPEAT` block as UTF-8 display text.
    ///
    /// Opaque bytes are represented with the UTF-8 replacement character. Use
    /// [`Self::tag_bytes`] when exact metadata preservation matters.
    pub fn tag(&self) -> Option<&str> {
        self.tag.as_ref().map(ModelTag::as_str)
    }

    /// Returns the exact unescaped bytes of this repeat block's optional Stim tag.
    pub fn tag_bytes(&self) -> Option<&[u8]> {
        self.tag.as_ref().map(ModelTag::as_bytes)
    }

    fn write_stim(&self, out: &mut String, indent: usize) {
        write_indent(out, indent);
        out.push_str("REPEAT");
        if let Some(tag) = &self.tag {
            out.push('[');
            tag.write_escaped_text(out);
            out.push(']');
        }
        out.push(' ');
        printing::push_u64(out, self.repeat_count.get());
        out.push_str(" {\n");
        self.body.write_stim(out, indent + 4);
        if self.body.is_empty() {
            out.push('\n');
        }
        write_indent(out, indent);
        out.push_str("}\n");
    }

    fn write_stim_io(&self, out: &mut impl Write, indent: usize) -> io::Result<()> {
        write_indent_io(out, indent)?;
        out.write_all(b"REPEAT")?;
        if let Some(tag) = &self.tag {
            out.write_all(b"[")?;
            tag.write_escaped_bytes(out)?;
            out.write_all(b"]")?;
        }
        writeln!(out, " {} {{", self.repeat_count.get())?;
        self.body.write_stim_io_indented(out, indent + 4)?;
        if self.body.is_empty() {
            out.write_all(b"\n")?;
        }
        write_indent_io(out, indent)?;
        out.write_all(b"}\n")
    }
}

impl Display for Circuit {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_stim_string())
    }
}

pub(crate) mod printing;

mod drop_impl;
mod parser;

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

/// Counts [`pauli_product_target_groups`]'s groups without materializing them.
///
/// A group is one Pauli target followed by any number of combiner-target pairs, so on the
/// combiner-validated target lists every constructed instruction carries, both walks agree.
fn pauli_product_target_group_count(targets: &[Target]) -> usize {
    let mut group_count = 0;
    let mut previous_was_combiner = false;
    for target in targets {
        if target.is_combiner() {
            previous_was_combiner = true;
        } else {
            if !previous_was_combiner {
                group_count += 1;
            }
            previous_was_combiner = false;
        }
    }
    group_count
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
    // Mirrors pinned Stim's write_targets (gate_target.cc:214-226): a
    // combiner prints immediately with no space before it and suppresses the
    // space before the following target, so dangling and doubled combiners
    // reprint exactly as stored.
    let mut skip_space = false;
    for target in targets {
        if target.is_combiner() {
            skip_space = true;
        } else if skip_space {
            skip_space = false;
        } else {
            out.push(' ');
        }
        printing::write_target(out, target);
    }
}

fn write_targets_io(out: &mut impl Write, targets: &[Target]) -> io::Result<()> {
    let mut skip_space = false;
    for target in targets {
        if target.is_combiner() {
            skip_space = true;
        } else if skip_space {
            skip_space = false;
        } else {
            out.write_all(b" ")?;
        }
        printing::write_target_io(out, target)?;
    }
    Ok(())
}

fn probability_from_validated_arg(gate: &'static str, arg: f64) -> ModelResult<Probability> {
    Probability::try_new(arg).map_err(|_| {
        ValidationError::InvalidArgument {
            gate,
            argument: arg.to_string(),
        }
        .into()
    })
}

fn observable_id_from_validated_arg(gate: &'static str, arg: f64) -> ModelResult<ObservableId> {
    const U64_EXCLUSIVE_UPPER_BOUND: f64 = f64::from_bits(0x43f0_0000_0000_0000);

    if !arg.is_finite() || arg < 0.0 || arg.fract() != 0.0 || arg >= U64_EXCLUSIVE_UPPER_BOUND {
        return Err(ValidationError::InvalidArgument {
            gate,
            argument: arg.to_string(),
        }
        .into());
    }
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the finite, integral, nonnegative, and exclusive 2^64 checks prove this cast exact"
    )]
    Ok(ObservableId::new(arg as u64))
}
