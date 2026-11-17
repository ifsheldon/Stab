use crate::diagnostics::bounded_parse_diagnostic_text;
use crate::model_parse::{line_error, validation_error};
use crate::parse_limits::ParseAdmission;
use crate::source_text::{SourceCommands, SourceSlice};
use crate::target::{
    TargetVec, parse_plain_qubit_targets_into, parse_target_token_into, take_exact,
    take_exact_targets,
};
use crate::{
    ByteSpan, Gate, GateCategory, ModelDialect, ModelError, ModelResult, ParseErrorCode,
    ParseErrorContext, ParseLimits, RepeatCount, Target, ValidationError,
};

use super::{Circuit, CircuitInstruction, CircuitItem, RepeatBlock};

const MAX_STIM_NUMBER_TOKEN_BYTES: usize = 63;
const MAX_CIRCUIT_REPEAT_COUNT_EXCLUSIVE: u64 = 1_u64 << 63;

mod fast;
mod items;
mod tag;

use items::BlockItems;
use tag::{DeferredTag, parse_optional_tag};

pub(super) fn parse_circuit(input: &str, limits: ParseLimits) -> ModelResult<Circuit> {
    let admission = ParseAdmission::new(ModelDialect::StimCircuit, input.len(), limits)?;
    Parser::new(input, admission, true).parse()
}

/// Parses `input` under `admission`, which has already charged `input` and any preparation
/// storage, and returns the admission with everything the parse charged.
pub(super) fn parse_circuit_admitted(
    input: &str,
    admission: ParseAdmission,
    fuse_instructions: bool,
) -> ModelResult<(Circuit, ParseAdmission)> {
    let mut parser = Parser::new(input, admission, fuse_instructions);
    let circuit = parser.parse_block()?;
    Ok((circuit, parser.admission))
}

struct Parser<'a> {
    commands: SourceCommands<'a>,
    input_len: usize,
    admission: ParseAdmission,
    scratch: ParseScratch,
    fuse_instructions: bool,
}

/// Reusable per-line storage for parsed arguments and targets.
///
/// Every line is parsed here first and then moved into its instruction with one exact-size
/// allocation (none when the values fit inline), so a line with many targets never walks a
/// reallocation chain per instruction. Each argument and target is admitted as retained before
/// it is pushed, and the scratch capacity itself is charged as transient parse storage before it
/// grows, so the scratch never holds more than admission accepted.
#[derive(Default)]
struct ParseScratch {
    args: Vec<f64>,
    targets: Vec<Target>,
}

/// The smallest capacity the target scratch grows to.
const MIN_SCRATCH_TARGETS: usize = 4;

impl ParseScratch {
    /// Moves the scratch arguments into exact-size instruction storage.
    fn take_args(&mut self, admission: &mut ParseAdmission) -> Vec<f64> {
        let capacity = self.args.capacity();
        let args = take_exact(&mut self.args);
        release_scratch::<f64>(admission, capacity, self.args.capacity());
        args
    }

    /// Moves the scratch targets into exact-size instruction storage.
    fn take_targets(&mut self, admission: &mut ParseAdmission) -> TargetVec {
        let capacity = self.targets.capacity();
        let targets = take_exact_targets(&mut self.targets);
        release_scratch::<Target>(admission, capacity, self.targets.capacity());
        targets
    }
}

/// Returns the transient charge of scratch capacity that a long line handed to its instruction.
fn release_scratch<T>(admission: &mut ParseAdmission, before: usize, after: usize) {
    admission.release_transient(before.saturating_sub(after).saturating_mul(size_of::<T>()));
}

/// Admits `target` as retained and appends it to the scratch, charging scratch growth first.
fn push_scratch_target(
    admission: &mut ParseAdmission,
    targets: &mut Vec<Target>,
    target: Target,
    source_line: usize,
    span: ByteSpan,
) -> ModelResult<()> {
    admission.admit_target(source_line, span)?;
    if targets.len() == targets.capacity() {
        let additional = targets.capacity().max(MIN_SCRATCH_TARGETS);
        admission.admit_transient(
            additional.saturating_mul(size_of::<Target>()),
            source_line,
            span,
        )?;
        targets.try_reserve_exact(additional).map_err(|error| {
            ModelError::invalid_domain_value(
                "circuit target allocation",
                format!("unable to reserve {additional} target slots: {error}"),
            )
        })?;
    }
    targets.push(target);
    Ok(())
}

impl<'a> Parser<'a> {
    fn new(input: &'a str, admission: ParseAdmission, fuse_instructions: bool) -> Self {
        Self {
            commands: SourceCommands::new(input),
            input_len: input.len(),
            admission,
            scratch: ParseScratch::default(),
            fuse_instructions,
        }
    }

    fn parse(mut self) -> ModelResult<Circuit> {
        self.parse_block()
    }

    /// Parses the whole input without recursing: each open `REPEAT` block is a frame holding
    /// its enclosing block's items, and the repeat-nesting admission bounds the frame count.
    fn parse_block(&mut self) -> ModelResult<Circuit> {
        let mut circuit = BlockItems::default();
        let mut enclosing: Vec<OpenRepeat> = Vec::new();
        while let Some(command) = self.commands.next() {
            let line_number = command.line_number();
            let line = command.source().trim_command_space_start();
            let semantic_line = line.trim_ascii_end();
            if semantic_line.text().is_empty() {
                continue;
            }
            if semantic_line.text() == "}" {
                self.close_repeat(&mut circuit, &mut enclosing, semantic_line.span())?;
                continue;
            }
            self.admission
                .admit_instruction(line_number, semantic_line.span())?;
            if let Some(header) = semantic_line.without_suffix('{') {
                self.open_repeat(
                    &mut circuit,
                    &mut enclosing,
                    line_number,
                    header,
                    semantic_line.span(),
                    command.end_error_span(),
                )?;
            } else {
                // Each path charges what its instruction retains, its tag included, before the
                // instruction is built.
                let instruction = parse_instruction(
                    line_number,
                    line,
                    command.end_error_span(),
                    &mut self.admission,
                    &mut self.scratch,
                )?;
                if self.fuse_instructions {
                    circuit.push_instruction(
                        instruction,
                        &mut self.admission,
                        line_number,
                        semantic_line.span(),
                    )?;
                } else {
                    circuit.push(
                        CircuitItem::Instruction(instruction),
                        &mut self.admission,
                        line_number,
                        semantic_line.span(),
                    )?;
                }
            }
        }
        if enclosing.is_empty() {
            Ok(circuit.finish(&mut self.admission))
        } else {
            Err(crate::model_parse::unterminated_repeat_block(
                ModelDialect::StimCircuit,
                self.input_len,
            ))
        }
    }

    /// Parses a `REPEAT` header and starts collecting its body, keeping the enclosing block's
    /// items in a new frame.
    fn open_repeat(
        &mut self,
        circuit: &mut BlockItems,
        enclosing: &mut Vec<OpenRepeat>,
        line_number: usize,
        header: SourceSlice<'a>,
        span: ByteSpan,
        end_error_span: ByteSpan,
    ) -> ModelResult<()> {
        let repeat =
            self.parse_repeat_header(line_number, header, span, end_error_span, enclosing.len())?;
        let tag = admit_deferred_tag(&mut self.admission, repeat.tag, line_number, span)?;
        enclosing.push(OpenRepeat {
            parent: std::mem::take(circuit),
            count: repeat.count,
            tag,
            line_number,
            span,
        });
        Ok(())
    }

    /// Ends the innermost open `REPEAT` block and appends it to its enclosing block.
    fn close_repeat(
        &mut self,
        circuit: &mut BlockItems,
        enclosing: &mut Vec<OpenRepeat>,
        span: ByteSpan,
    ) -> ModelResult<()> {
        let Some(open) = enclosing.pop() else {
            return Err(crate::model_parse::unexpected_repeat_terminator(
                ModelDialect::StimCircuit,
                span,
            ));
        };
        let body = std::mem::replace(circuit, open.parent).finish(&mut self.admission);
        circuit.push(
            CircuitItem::RepeatBlock(RepeatBlock::new(open.count, body, open.tag)),
            &mut self.admission,
            open.line_number,
            open.span,
        )
    }

    fn parse_repeat_header(
        &mut self,
        line_number: usize,
        header: SourceSlice<'a>,
        header_span: ByteSpan,
        end_error_span: ByteSpan,
        depth: usize,
    ) -> ModelResult<ParsedRepeatHeader<'a>> {
        let (name, rest) = parse_name(line_number, header)?;
        if !name.text().eq_ignore_ascii_case("REPEAT") {
            return Err(line_error(
                ModelDialect::StimCircuit,
                line_number,
                ParseErrorCode::InvalidRepeatBlock,
                "repeat blocks must be written as REPEAT <count> {",
                "repeat blocks must be written as REPEAT <count> {",
                header.span(),
                ParseErrorContext::Instruction {
                    dialect: ModelDialect::StimCircuit,
                    instruction: bounded_parse_diagnostic_text(name.text()),
                },
            ));
        }
        let (tag, rest) = parse_optional_tag(line_number, name.text(), rest, end_error_span)?;
        // Pinned Stim lexes parenthesized arguments for every gate before the
        // block brace check and then drops them for block gates, so REPEAT
        // headers accept and discard well-formed arguments while malformed
        // ones keep the shared argument diagnostics (circuit.cc:213-218). The
        // discarded arguments are checked against the budget but never charged.
        let (_args, rest) = parse_optional_args(
            line_number,
            name.text(),
            rest,
            end_error_span,
            &mut self.scratch.args,
            &mut self.admission,
            ArgumentCharge::Discarded,
        )?;
        if !rest
            .text()
            .as_bytes()
            .first()
            .is_some_and(|byte| matches!(byte, b' ' | b'\t' | b'\r'))
        {
            return Err(line_error(
                ModelDialect::StimCircuit,
                line_number,
                ParseErrorCode::MissingRepeatCount,
                "missing repeat count",
                "missing repeat count",
                rest.subspan(0, usize::from(!rest.text().is_empty()))
                    .unwrap_or(end_error_span),
                ParseErrorContext::DomainValue {
                    dialect: ModelDialect::StimCircuit,
                    kind: "repeat count",
                    value: String::new(),
                },
            ));
        }
        let rest = trim_target_space_start(rest);
        if rest.text().is_empty() {
            return Err(line_error(
                ModelDialect::StimCircuit,
                line_number,
                ParseErrorCode::MissingRepeatCount,
                "missing repeat count",
                "missing repeat count",
                end_error_span,
                ParseErrorContext::DomainValue {
                    dialect: ModelDialect::StimCircuit,
                    kind: "repeat count",
                    value: String::new(),
                },
            ));
        }
        let token_end = rest
            .text()
            .as_bytes()
            .iter()
            .position(|byte| matches!(byte, b' ' | b'\t' | b'\r'))
            .unwrap_or(rest.text().len());
        let count_token = rest
            .prefix(token_end)
            .ok_or_else(|| internal_parser_error(line_number, "invalid repeat count"))?;
        let trailing = rest
            .suffix(token_end)
            .ok_or_else(|| internal_parser_error(line_number, "invalid repeat count"))?;
        if !trailing
            .text()
            .as_bytes()
            .iter()
            .all(|byte| matches!(byte, b' ' | b'\t' | b'\r'))
        {
            return Err(line_error(
                ModelDialect::StimCircuit,
                line_number,
                ParseErrorCode::InvalidRepeatBlock,
                "repeat blocks must be written as REPEAT <count> {",
                "repeat blocks must be written as REPEAT <count> {",
                trailing.trim_inline_ascii_start().span(),
                ParseErrorContext::Instruction {
                    dialect: ModelDialect::StimCircuit,
                    instruction: bounded_parse_diagnostic_text(name.text()),
                },
            ));
        }
        let count = parse_repeat_count(line_number, count_token)?;
        let repeat_count = RepeatCount::try_new(count).map_err(|error| {
            validation_error(
                ModelDialect::StimCircuit,
                line_number,
                name.text(),
                ParseErrorCode::InvalidRepeatCount,
                count_token.span(),
                error,
                false,
            )
        })?;
        let actual = depth.checked_add(1).ok_or_else(|| {
            ModelError::invalid_domain_value("circuit repeat nesting", "depth overflow")
        })?;
        self.admission
            .admit_repeat_nesting(line_number, actual, header_span)?;
        Ok(ParsedRepeatHeader {
            count: repeat_count,
            tag,
        })
    }
}

struct ParsedRepeatHeader<'a> {
    count: RepeatCount,
    tag: Option<DeferredTag<'a>>,
}

/// A `REPEAT` block whose body is being parsed.
struct OpenRepeat {
    /// Items of the enclosing block parsed so far.
    parent: BlockItems,
    count: RepeatCount,
    tag: Option<String>,
    line_number: usize,
    span: ByteSpan,
}

/// Charges a declaration's tag, then builds its text in an exact-size allocation.
fn admit_deferred_tag(
    admission: &mut ParseAdmission,
    tag: Option<DeferredTag<'_>>,
    line_number: usize,
    span: ByteSpan,
) -> ModelResult<Option<String>> {
    let Some(tag) = tag else {
        return Ok(None);
    };
    admission.admit_payload(0, tag.len(), line_number, span)?;
    tag.into_string().map(Some)
}

fn parse_instruction(
    line_number: usize,
    line: SourceSlice<'_>,
    end_error_span: ByteSpan,
    admission: &mut ParseAdmission,
    scratch: &mut ParseScratch,
) -> ModelResult<CircuitInstruction> {
    if admission.target_budget_allows_upper_bound(line.text().len())
        && let Some(Ok(instruction)) = fast::parse_common_plain_instruction(line.text())
    {
        admission.admit_targets(instruction.targets().len(), line_number, line.span())?;
        return Ok(instruction);
    }
    let (name, rest) = parse_name(line_number, line)?;
    if let Some(instruction) =
        parse_simple_plain_instruction(line_number, name.text(), rest, admission, scratch)?
    {
        return Ok(instruction);
    }
    parse_instruction_fully_generic_from_parts(
        line_number,
        name,
        rest,
        end_error_span,
        line,
        admission,
        scratch,
    )
}

/// Parses an instruction with the diagnostic parser, charging its tag against the span of
/// `line` without trailing whitespace.
fn parse_instruction_fully_generic_from_parts(
    line_number: usize,
    name: SourceSlice<'_>,
    rest: SourceSlice<'_>,
    end_error_span: ByteSpan,
    line: SourceSlice<'_>,
    admission: &mut ParseAdmission,
    scratch: &mut ParseScratch,
) -> ModelResult<CircuitInstruction> {
    let gate = Gate::lookup_name(name.text()).ok_or_else(|| {
        let name_excerpt = bounded_parse_diagnostic_text(name.text());
        line_error(
            ModelDialect::StimCircuit,
            line_number,
            ParseErrorCode::UnknownInstruction,
            format!("unknown gate {name_excerpt}"),
            format!("unknown gate {name_excerpt}"),
            name.span(),
            ParseErrorContext::Instruction {
                dialect: ModelDialect::StimCircuit,
                instruction: name_excerpt,
            },
        )
    })?;
    if gate.category() == GateCategory::ControlFlow {
        let message = format!("missing '{{' at start of {} block", gate.canonical_name());
        return Err(line_error(
            ModelDialect::StimCircuit,
            line_number,
            ParseErrorCode::InvalidRepeatBlock,
            message.clone(),
            message,
            name.span(),
            ParseErrorContext::Instruction {
                dialect: ModelDialect::StimCircuit,
                instruction: bounded_parse_diagnostic_text(name.text()),
            },
        ));
    }
    let (tag, rest) = parse_optional_tag(line_number, name.text(), rest, end_error_span)?;
    let (args, rest) = parse_optional_args(
        line_number,
        name.text(),
        rest,
        end_error_span,
        &mut scratch.args,
        admission,
        ArgumentCharge::Retained,
    )?;
    let targets = parse_targets(
        line_number,
        name.text(),
        rest,
        end_error_span,
        admission,
        &mut scratch.targets,
    )?;
    gate.validate(&scratch.args, &scratch.targets)
        .map_err(|error| {
            let (code, span) = validation_span(&error, args, targets);
            validation_error(
                ModelDialect::StimCircuit,
                line_number,
                name.text(),
                code,
                span,
                error,
                true,
            )
        })?;
    let args = scratch.take_args(admission);
    let targets = scratch.take_targets(admission);
    let tag = match tag {
        None => None,
        Some(tag) => admit_deferred_tag(
            admission,
            Some(tag),
            line_number,
            line.trim_ascii_end().span(),
        )?,
    };
    Ok(CircuitInstruction::from_validated_parts(
        gate, args, targets, tag,
    ))
}

#[cfg(test)]
fn parse_instruction_fully_generic(
    line_number: usize,
    line: &str,
) -> ModelResult<CircuitInstruction> {
    let mut admission = ParseAdmission::new(
        ModelDialect::StimCircuit,
        line.len(),
        ParseLimits::default(),
    )?;
    let line = SourceSlice::new(line, 0);
    let (name, rest) = parse_name(line_number, line)?;
    parse_instruction_fully_generic_from_parts(
        line_number,
        name,
        rest,
        line.end_span(),
        line,
        &mut admission,
        &mut ParseScratch::default(),
    )
}

fn parse_name(
    line_number: usize,
    line: SourceSlice<'_>,
) -> ModelResult<(SourceSlice<'_>, SourceSlice<'_>)> {
    let mut end = None;
    for (index, byte) in line.text().bytes().enumerate() {
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
    let Some(end) = end else {
        return Err(line_error(
            ModelDialect::StimCircuit,
            line_number,
            ParseErrorCode::MissingInstructionName,
            "missing instruction name",
            "missing instruction name",
            first_character_span(line),
            ParseErrorContext::Model {
                dialect: ModelDialect::StimCircuit,
            },
        ));
    };
    let name = line
        .prefix(end)
        .ok_or_else(|| internal_parser_error(line_number, "missing instruction name"))?;
    let rest = line
        .suffix(end)
        .ok_or_else(|| internal_parser_error(line_number, "missing instruction name"))?;
    Ok((name, rest))
}

/// Source region of a parsed argument list, whose values live in the parse scratch.
#[derive(Clone, Copy)]
struct ParsedArguments {
    region: ByteSpan,
}

/// How parsed arguments are charged.
#[derive(Clone, Copy, Eq, PartialEq)]
enum ArgumentCharge {
    /// The arguments are retained by an instruction and charged as they are parsed.
    Retained,
    /// The arguments of a `REPEAT` header are discarded, so they must fit the remaining budget
    /// but are never charged.
    Discarded,
}

/// Parses an optional parenthesized argument list into `values`, which is cleared first.
///
/// Each argument is admitted under `charge` before it is pushed, and the scratch capacity for
/// the whole list is charged as transient storage before it is reserved.
fn parse_optional_args<'a>(
    line_number: usize,
    instruction: &str,
    rest: SourceSlice<'a>,
    end_error_span: ByteSpan,
    values: &mut Vec<f64>,
    admission: &mut ParseAdmission,
    charge: ArgumentCharge,
) -> ModelResult<(ParsedArguments, SourceSlice<'a>)> {
    values.clear();
    let Some(body) = rest.strip_prefix("(") else {
        return Ok((
            ParsedArguments {
                region: rest.subspan(0, 0).unwrap_or_else(|| rest.end_span()),
            },
            rest,
        ));
    };
    let Some(end) = body.text().find(')') else {
        return Err(line_error(
            ModelDialect::StimCircuit,
            line_number,
            ParseErrorCode::UnterminatedArgumentList,
            "unterminated argument list",
            "unterminated argument list",
            end_error_span,
            instruction_context(instruction),
        ));
    };
    let raw_args = body
        .prefix(end)
        .ok_or_else(|| internal_parser_error(line_number, "unterminated argument list"))?;
    let tail = body
        .suffix(end + 1)
        .ok_or_else(|| internal_parser_error(line_number, "unterminated argument list"))?;
    let argument_count = raw_args
        .text()
        .bytes()
        .filter(|byte| *byte == b',')
        .count()
        .saturating_add(1);
    if values.capacity() < argument_count {
        let additional = argument_count - values.capacity();
        admission.admit_transient(
            additional.saturating_mul(size_of::<f64>()),
            line_number,
            raw_args.span(),
        )?;
        values.try_reserve_exact(argument_count).map_err(|error| {
            ModelError::invalid_domain_value(
                "circuit argument allocation",
                format!("unable to reserve {argument_count} argument slots: {error}"),
            )
        })?;
    }
    let mut first_non_finite = None;
    let mut token_start = 0;
    loop {
        let token_end = raw_args
            .text()
            .get(token_start..)
            .and_then(|tail| tail.find(',').map(|offset| token_start + offset))
            .unwrap_or(raw_args.text().len());
        let token = raw_args
            .slice(token_start, token_end)
            .ok_or_else(|| internal_parser_error(line_number, "invalid argument"))?
            .trim_inline_ascii_start()
            .trim_inline_ascii_end();
        let value = if token.text().is_empty() {
            0.0
        } else if token.text().len() > MAX_STIM_NUMBER_TOKEN_BYTES {
            return Err(invalid_number_error(
                line_number,
                instruction,
                token,
                "invalid argument",
            ));
        } else {
            token.text().parse::<f64>().map_err(|_| {
                invalid_number_error(line_number, instruction, token, "invalid argument")
            })?
        };
        if !value.is_finite() {
            first_non_finite.get_or_insert(token);
        }
        match charge {
            ArgumentCharge::Retained => admission.admit_argument(line_number, token.span())?,
            ArgumentCharge::Discarded => admission.admit_transient_arguments(
                values.len().saturating_add(1),
                line_number,
                token.span(),
            )?,
        }
        values.push(value);
        if token_end == raw_args.text().len() {
            break;
        }
        token_start = token_end + 1;
    }
    if let Some(token) = first_non_finite {
        let token_excerpt = bounded_parse_diagnostic_text(token.text());
        let legacy_detail = Gate::from_name(instruction)
            .and_then(|gate| gate.validate(values, &[]))
            .err()
            .map_or_else(
                || format!("invalid argument {token_excerpt}"),
                |error| error.to_string(),
            );
        return Err(line_error(
            ModelDialect::StimCircuit,
            line_number,
            ParseErrorCode::InvalidNumber,
            format!("invalid argument {token_excerpt}"),
            legacy_detail,
            token.span(),
            ParseErrorContext::Argument {
                dialect: ModelDialect::StimCircuit,
                instruction: bounded_parse_diagnostic_text(instruction),
                argument: token_excerpt,
            },
        ));
    }
    Ok((
        ParsedArguments {
            region: raw_args.span(),
        },
        tail,
    ))
}

/// Source region of a parsed target list, whose targets live in the parse scratch.
#[derive(Clone, Copy)]
struct ParsedTargets {
    region: ByteSpan,
}

/// Parses a target list into `values`, which is cleared first, admitting each target before it
/// is retained.
fn parse_targets(
    line_number: usize,
    instruction: &str,
    rest: SourceSlice<'_>,
    end_error_span: ByteSpan,
    admission: &mut ParseAdmission,
    values: &mut Vec<Target>,
) -> ModelResult<ParsedTargets> {
    values.clear();
    if rest.text().is_empty() {
        return Ok(ParsedTargets {
            region: end_error_span,
        });
    }
    // Pinned Stim waives the target spacing requirement for combiners
    // (circuit.h:302-310), so `E(0.1)*X0` lexes while `E(0.1)X0` rejects.
    if !rest
        .text()
        .as_bytes()
        .first()
        .is_some_and(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'*'))
    {
        let span = first_character_span(rest);
        return Err(line_error(
            ModelDialect::StimCircuit,
            line_number,
            ParseErrorCode::InvalidTargetSyntax,
            "targets must be separated by spacing",
            "targets must be separated by spacing",
            span,
            ParseErrorContext::Target {
                dialect: ModelDialect::StimCircuit,
                instruction: bounded_parse_diagnostic_text(instruction),
                target: bounded_parse_diagnostic_text(rest.text()),
            },
        ));
    }
    let content = trim_target_space_start(rest);
    if content.text().is_empty() {
        return Ok(ParsedTargets {
            region: content.end_span(),
        });
    }

    let mut cursor = 0;
    let mut region_start = None;
    let mut region_end = content.byte_start();
    while cursor < content.text().len() {
        while content
            .text()
            .as_bytes()
            .get(cursor)
            .is_some_and(|byte| matches!(byte, b' ' | b'\t' | b'\r'))
        {
            cursor += 1;
        }
        if cursor == content.text().len() {
            break;
        }
        let start = cursor;
        while content
            .text()
            .as_bytes()
            .get(cursor)
            .is_some_and(|byte| !matches!(byte, b' ' | b'\t' | b'\r'))
        {
            cursor += 1;
        }
        let token = content
            .slice(start, cursor)
            .ok_or_else(|| internal_parser_error(line_number, "invalid target"))?;
        if let Err(error) = parse_target_token_into(token.text(), &mut |target| {
            push_scratch_target(admission, values, target, line_number, token.span())
        }) {
            if error.resource_limit_error().is_some() {
                return Err(error);
            }
            let code = target_parse_error_code(&error);
            return Err(validation_error(
                ModelDialect::StimCircuit,
                line_number,
                instruction,
                code,
                token.span(),
                error,
                true,
            ));
        }
        region_start.get_or_insert(token.byte_start());
        region_end = token.span().byte_end();
    }
    let region = region_start
        .and_then(|start| ByteSpan::try_new(start, region_end.saturating_sub(start)))
        .unwrap_or_else(|| content.end_span());
    Ok(ParsedTargets { region })
}

fn validation_span(
    error: &ModelError,
    args: ParsedArguments,
    targets: ParsedTargets,
) -> (ParseErrorCode, ByteSpan) {
    match error {
        ModelError::Validation(ValidationError::InvalidArgumentCount { .. }) => {
            (ParseErrorCode::InvalidArgumentCount, args.region)
        }
        ModelError::Validation(ValidationError::InvalidArgument { .. }) => {
            (ParseErrorCode::InvalidArgument, args.region)
        }
        ModelError::Validation(ValidationError::InvalidTarget { .. }) => {
            (ParseErrorCode::InvalidTarget, targets.region)
        }
        ModelError::Validation(ValidationError::InvalidTargetCount { .. }) => {
            (ParseErrorCode::InvalidTargetCount, targets.region)
        }
        _ => (ParseErrorCode::InvalidSyntax, targets.region),
    }
}

fn parse_repeat_count(line_number: usize, token: SourceSlice<'_>) -> ModelResult<u64> {
    if token.text().is_empty() {
        return Err(line_error(
            ModelDialect::StimCircuit,
            line_number,
            ParseErrorCode::MissingRepeatCount,
            "missing repeat count",
            "missing repeat count",
            token.end_span(),
            ParseErrorContext::DomainValue {
                dialect: ModelDialect::StimCircuit,
                kind: "repeat count",
                value: String::new(),
            },
        ));
    }
    let mut value = 0_u64;
    for (offset, character) in token.text().char_indices() {
        if !character.is_ascii_digit() {
            return Err(line_error(
                ModelDialect::StimCircuit,
                line_number,
                ParseErrorCode::InvalidRepeatCount,
                "invalid repeat count",
                "invalid repeat count",
                token
                    .subspan(offset, character.len_utf8())
                    .unwrap_or_else(|| token.span()),
                ParseErrorContext::DomainValue {
                    dialect: ModelDialect::StimCircuit,
                    kind: "repeat count",
                    value: bounded_parse_diagnostic_text(token.text()),
                },
            ));
        }
        let digit = u64::from(character as u8 - b'0');
        value = value
            .checked_mul(10)
            .and_then(|value| value.checked_add(digit))
            .ok_or_else(|| {
                line_error(
                    ModelDialect::StimCircuit,
                    line_number,
                    ParseErrorCode::IntegerOutOfRange,
                    "invalid repeat count",
                    "invalid repeat count",
                    token.span(),
                    ParseErrorContext::DomainValue {
                        dialect: ModelDialect::StimCircuit,
                        kind: "repeat count",
                        value: bounded_parse_diagnostic_text(token.text()),
                    },
                )
            })?;
        if value >= MAX_CIRCUIT_REPEAT_COUNT_EXCLUSIVE {
            return Err(line_error(
                ModelDialect::StimCircuit,
                line_number,
                ParseErrorCode::IntegerOutOfRange,
                "invalid repeat count",
                "invalid repeat count",
                token.span(),
                ParseErrorContext::DomainValue {
                    dialect: ModelDialect::StimCircuit,
                    kind: "repeat count",
                    value: bounded_parse_diagnostic_text(token.text()),
                },
            ));
        }
    }
    Ok(value)
}

fn target_parse_error_code(error: &ModelError) -> ParseErrorCode {
    match error {
        ModelError::Validation(ValidationError::InvalidDomainValue { value, .. }) => {
            let digits = value.trim_prefix('-');
            if !digits.is_empty() && digits.as_bytes().iter().all(u8::is_ascii_digit) {
                ParseErrorCode::IntegerOutOfRange
            } else {
                ParseErrorCode::InvalidTargetSyntax
            }
        }
        _ => ParseErrorCode::InvalidTargetSyntax,
    }
}

fn invalid_number_error(
    line_number: usize,
    instruction: &str,
    token: SourceSlice<'_>,
    message: &'static str,
) -> ModelError {
    let span = token.span();
    let token = bounded_parse_diagnostic_text(token.text());
    line_error(
        ModelDialect::StimCircuit,
        line_number,
        ParseErrorCode::InvalidNumber,
        format!("{message} {token}"),
        format!("{message} {token}"),
        span,
        ParseErrorContext::Argument {
            dialect: ModelDialect::StimCircuit,
            instruction: bounded_parse_diagnostic_text(instruction),
            argument: token,
        },
    )
}

fn instruction_context(instruction: &str) -> ParseErrorContext {
    ParseErrorContext::Instruction {
        dialect: ModelDialect::StimCircuit,
        instruction: bounded_parse_diagnostic_text(instruction),
    }
}

fn first_character_span(source: SourceSlice<'_>) -> ByteSpan {
    source
        .text()
        .chars()
        .next()
        .and_then(|character| source.subspan(0, character.len_utf8()))
        .unwrap_or_else(|| source.end_span())
}

fn trim_target_space_start(mut source: SourceSlice<'_>) -> SourceSlice<'_> {
    let count = source
        .text()
        .as_bytes()
        .iter()
        .take_while(|byte| matches!(byte, b' ' | b'\t' | b'\r'))
        .count();
    if let Some(trimmed) = source.suffix(count) {
        source = trimmed;
    }
    source
}

/// Parses a plain qubit-list instruction such as `H 0 1 2`, or returns `None` so the generic
/// parser handles (and diagnoses) the line.
///
/// A plain list of `n` bytes holds at most `ceil(n / 2)` targets, so the scratch is grown to
/// that bound first, charging the growth as transient storage, and the targets are charged as
/// retained once the whole line is accepted; a line this path declines charges no target.
fn parse_simple_plain_instruction(
    line_number: usize,
    name: &str,
    rest: SourceSlice<'_>,
    admission: &mut ParseAdmission,
    scratch: &mut ParseScratch,
) -> ModelResult<Option<CircuitInstruction>> {
    let Some(gate) = Gate::from_simple_plain_name(name) else {
        return Ok(None);
    };
    let text = rest.text();
    if text.starts_with('[')
        || text.starts_with('(')
        || !admission.target_budget_allows_upper_bound(text.len())
    {
        return Ok(None);
    }
    let bound = text.len().div_ceil(2);
    if scratch.targets.capacity() < bound {
        let additional = bound - scratch.targets.capacity();
        if admission
            .admit_transient(
                additional.saturating_mul(size_of::<Target>()),
                line_number,
                rest.span(),
            )
            .is_err()
        {
            return Ok(None);
        }
        scratch.targets.clear();
        scratch.targets.try_reserve_exact(bound).map_err(|error| {
            ModelError::invalid_domain_value(
                "circuit target allocation",
                format!("unable to reserve {bound} target slots: {error}"),
            )
        })?;
    }
    scratch.targets.clear();
    let targets = &mut scratch.targets;
    let parsed = parse_plain_qubit_targets_into(text, &mut |target| {
        if targets.len() == targets.capacity() {
            return Err(internal_parser_error(
                line_number,
                "plain target list exceeded its reserved bound",
            ));
        }
        targets.push(target);
        Ok(())
    });
    if !matches!(parsed, Ok(true)) {
        return Ok(None);
    }
    if gate == Gate::CX
        && validate_simple_plain_pairs(gate.canonical_name(), &scratch.targets).is_err()
    {
        return Ok(None);
    }
    admission.admit_targets(scratch.targets.len(), line_number, rest.span())?;
    let targets = scratch.take_targets(admission);
    Ok(Some(CircuitInstruction::from_validated_parts(
        gate,
        Vec::new(),
        targets,
        None,
    )))
}

fn validate_simple_plain_pairs(gate: &'static str, targets: &[Target]) -> ModelResult<()> {
    if !targets.len().is_multiple_of(2) {
        return Err(ValidationError::InvalidTargetCount {
            gate,
            count: targets.len(),
        }
        .into());
    }
    for [left, right] in targets.as_chunks::<2>().0 {
        if left == right {
            return Err(ValidationError::InvalidTarget {
                gate,
                target: left.to_string(),
            }
            .into());
        }
    }
    Ok(())
}

fn internal_parser_error(line_number: usize, message: impl ToString) -> ModelError {
    ModelError::invalid_domain_value(
        "Stim circuit parser",
        format!("line {line_number}: {}", message.to_string()),
    )
}
