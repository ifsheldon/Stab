use crate::diagnostics::bounded_parse_diagnostic_text;
use crate::model_parse::{line_error, validation_error};
use crate::parse_limits::ParseAdmission;
use crate::source_text::{SourceCommands, SourceSlice};
use crate::target::{TargetVec, parse_plain_qubit_target_text, parse_target_token_into};
use crate::{
    ByteSpan, Gate, GateCategory, ModelDialect, ModelError, ModelResult, ParseErrorCode,
    ParseErrorContext, ParseLimits, RepeatCount, Target, ValidationError,
};

use super::{Circuit, CircuitInstruction, CircuitItem, RepeatBlock};

const MAX_STIM_NUMBER_TOKEN_BYTES: usize = 63;
const MAX_CIRCUIT_REPEAT_COUNT_EXCLUSIVE: u64 = 1_u64 << 63;

mod fast;

pub(super) fn parse_circuit(input: &str, limits: ParseLimits) -> ModelResult<Circuit> {
    Parser::new(input, limits, true)?.parse()
}

pub(super) fn parse_circuit_unfused(input: &str, limits: ParseLimits) -> ModelResult<Circuit> {
    Parser::new(input, limits, false)?.parse()
}

struct Parser<'a> {
    commands: SourceCommands<'a>,
    input_len: usize,
    admission: ParseAdmission,
    fuse_instructions: bool,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str, limits: ParseLimits, fuse_instructions: bool) -> ModelResult<Self> {
        let admission = ParseAdmission::new(ModelDialect::StimCircuit, input.len(), limits)?;
        Ok(Self {
            commands: SourceCommands::new(input),
            input_len: input.len(),
            admission,
            fuse_instructions,
        })
    }

    fn parse(mut self) -> ModelResult<Circuit> {
        self.parse_block(false, 0)
    }

    fn parse_block(&mut self, stop_on_terminator: bool, depth: usize) -> ModelResult<Circuit> {
        let mut circuit = Circuit::new();
        while let Some(command) = self.commands.next() {
            let line_number = command.line_number();
            self.admission
                .admit_source_line(line_number, command.source().span())?;

            let line = command.source().trim_ascii_start();
            let semantic_line = line.trim_ascii_end();
            if semantic_line.text().is_empty() {
                continue;
            }
            if semantic_line.text() == "}" {
                if stop_on_terminator {
                    return Ok(circuit);
                }
                return Err(crate::model_parse::unexpected_repeat_terminator(
                    ModelDialect::StimCircuit,
                    semantic_line.span(),
                ));
            }
            self.admission
                .admit_instruction(line_number, semantic_line.span())?;
            if let Some(header) = semantic_line.without_suffix('{') {
                let repeat = self.parse_repeat_header(
                    line_number,
                    header,
                    semantic_line.span(),
                    command.end_error_span(),
                    depth,
                )?;
                let body = self.parse_block(true, depth + 1)?;
                circuit.push(CircuitItem::RepeatBlock(RepeatBlock::new(
                    repeat.count,
                    body,
                    repeat.tag,
                )));
            } else {
                let instruction = parse_instruction(
                    line_number,
                    line,
                    command.end_error_span(),
                    &mut self.admission,
                )?;
                if self.fuse_instructions {
                    circuit.push_instruction(instruction);
                } else {
                    circuit.push(CircuitItem::Instruction(instruction));
                }
            }
        }
        if stop_on_terminator {
            Err(crate::model_parse::unterminated_repeat_block(
                ModelDialect::StimCircuit,
                self.input_len,
            ))
        } else {
            Ok(circuit)
        }
    }

    fn parse_repeat_header(
        &self,
        line_number: usize,
        header: SourceSlice<'a>,
        header_span: ByteSpan,
        end_error_span: ByteSpan,
        depth: usize,
    ) -> ModelResult<ParsedRepeatHeader> {
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
        // ones keep the shared argument diagnostics (circuit.cc:213-218).
        let (_args, rest) = parse_optional_args(line_number, name.text(), rest, end_error_span)?;
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

struct ParsedRepeatHeader {
    count: RepeatCount,
    tag: Option<String>,
}

fn parse_instruction(
    line_number: usize,
    line: SourceSlice<'_>,
    end_error_span: ByteSpan,
    admission: &mut ParseAdmission,
) -> ModelResult<CircuitInstruction> {
    if admission.target_budget_allows_upper_bound(line.text().len())
        && let Some(Ok(instruction)) = fast::parse_common_plain_instruction(line.text())
    {
        admission.admit_targets(instruction.targets().len(), line_number, line.span())?;
        return Ok(instruction);
    }
    let (name, rest) = parse_name(line_number, line)?;
    if admission.target_budget_allows_upper_bound(rest.text().len())
        && let Some(Ok(instruction)) = parse_simple_plain_instruction(name.text(), rest.text())
    {
        admission.admit_targets(instruction.targets().len(), line_number, rest.span())?;
        return Ok(instruction);
    }
    parse_instruction_fully_generic_from_parts(line_number, name, rest, end_error_span, admission)
}

fn parse_instruction_fully_generic_from_parts(
    line_number: usize,
    name: SourceSlice<'_>,
    rest: SourceSlice<'_>,
    end_error_span: ByteSpan,
    admission: &mut ParseAdmission,
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
    let (args, rest) = parse_optional_args(line_number, name.text(), rest, end_error_span)?;
    let targets = parse_targets(line_number, name.text(), rest, end_error_span, admission)?;
    gate.validate(&args.values, &targets.values)
        .map_err(|error| {
            let (code, span) = validation_span(&error, &args, &targets);
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
    Ok(CircuitInstruction::from_validated_parts(
        gate,
        args.values,
        targets.values,
        tag,
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
        &mut admission,
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

fn parse_optional_tag<'a>(
    line_number: usize,
    instruction: &str,
    rest: SourceSlice<'a>,
    end_error_span: ByteSpan,
) -> ModelResult<(Option<String>, SourceSlice<'a>)> {
    let Some(mut body) = rest.strip_prefix("[") else {
        return Ok((None, rest));
    };
    let mut tag = String::new();
    loop {
        let Some((character, character_len)) = body
            .text()
            .chars()
            .next()
            .map(|character| (character, character.len_utf8()))
        else {
            return Err(line_error(
                ModelDialect::StimCircuit,
                line_number,
                ParseErrorCode::UnterminatedTag,
                "unterminated tag",
                "unterminated tag",
                end_error_span,
                instruction_context(instruction),
            ));
        };
        let character_span = body
            .subspan(0, character_len)
            .unwrap_or_else(|| body.span());
        body = body
            .suffix(character_len)
            .ok_or_else(|| internal_parser_error(line_number, "unterminated tag"))?;
        match character {
            ']' => return Ok((Some(tag), body)),
            '\\' => {
                let Some((escaped, escaped_len)) = body
                    .text()
                    .chars()
                    .next()
                    .map(|escaped| (escaped, escaped.len_utf8()))
                else {
                    return Err(line_error(
                        ModelDialect::StimCircuit,
                        line_number,
                        ParseErrorCode::UnterminatedTagEscape,
                        "unterminated tag escape",
                        "unterminated tag escape",
                        end_error_span,
                        instruction_context(instruction),
                    ));
                };
                let escape_span = ByteSpan::try_new(
                    character_span.byte_start(),
                    character_span.byte_length() + escaped_len,
                )
                .unwrap_or(character_span);
                body = body
                    .suffix(escaped_len)
                    .ok_or_else(|| internal_parser_error(line_number, "unterminated tag escape"))?;
                tag.push(match escaped {
                    'C' => ']',
                    'r' => '\r',
                    'n' => '\n',
                    'B' => '\\',
                    _ => {
                        return Err(line_error(
                            ModelDialect::StimCircuit,
                            line_number,
                            ParseErrorCode::InvalidTagEscape,
                            format!("invalid tag escape \\{escaped}"),
                            format!("invalid tag escape \\{escaped}"),
                            escape_span,
                            instruction_context(instruction),
                        ));
                    }
                });
            }
            '\r' | '\n' => {
                return Err(line_error(
                    ModelDialect::StimCircuit,
                    line_number,
                    ParseErrorCode::UnterminatedTag,
                    "invalid tag newline",
                    "invalid tag newline",
                    character_span,
                    instruction_context(instruction),
                ));
            }
            _ => tag.push(character),
        }
    }
}

struct ParsedArguments {
    values: Vec<f64>,
    region: ByteSpan,
}

fn parse_optional_args<'a>(
    line_number: usize,
    instruction: &str,
    rest: SourceSlice<'a>,
    end_error_span: ByteSpan,
) -> ModelResult<(ParsedArguments, SourceSlice<'a>)> {
    let Some(body) = rest.strip_prefix("(") else {
        return Ok((
            ParsedArguments {
                values: Vec::new(),
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
    let mut values = Vec::new();
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
        values.push(value);
        if token_end == raw_args.text().len() {
            break;
        }
        token_start = token_end + 1;
    }
    if let Some(token) = first_non_finite {
        let token_excerpt = bounded_parse_diagnostic_text(token.text());
        let legacy_detail = Gate::from_name(instruction)
            .and_then(|gate| gate.validate(&values, &TargetVec::new()))
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
            values,
            region: raw_args.span(),
        },
        tail,
    ))
}

struct ParsedTargets {
    values: TargetVec,
    region: ByteSpan,
}

fn parse_targets(
    line_number: usize,
    instruction: &str,
    rest: SourceSlice<'_>,
    end_error_span: ByteSpan,
    admission: &mut ParseAdmission,
) -> ModelResult<ParsedTargets> {
    if rest.text().is_empty() {
        return Ok(ParsedTargets {
            values: TargetVec::new(),
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
            values: TargetVec::new(),
            region: content.end_span(),
        });
    }

    let mut values = TargetVec::new();
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
        if let Err(error) = parse_target_token_into(token.text(), &mut values, || {
            admission.admit_target(line_number, token.span())
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
    Ok(ParsedTargets { values, region })
}

fn validation_span(
    error: &ModelError,
    args: &ParsedArguments,
    targets: &ParsedTargets,
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
            let digits = value.strip_prefix('-').unwrap_or(value);
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

fn parse_simple_plain_instruction(
    name: &str,
    rest: &str,
) -> Option<ModelResult<CircuitInstruction>> {
    let gate = Gate::from_simple_plain_name(name)?;
    if rest.starts_with('[') || rest.starts_with('(') {
        return None;
    }
    if !rest
        .as_bytes()
        .iter()
        .all(|byte| byte.is_ascii_digit() || matches!(byte, b' ' | b'\t' | b'\r'))
    {
        return None;
    }
    let targets = match parse_plain_qubit_target_text(rest) {
        Ok(Some(targets)) => targets,
        Ok(None) | Err(_) => return None,
    };
    if gate == Gate::CX && validate_simple_plain_pairs(gate.canonical_name(), &targets).is_err() {
        return None;
    }
    Some(Ok(CircuitInstruction::from_validated_parts(
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
    for pair in targets.chunks_exact(2) {
        if let [left, right] = pair
            && left == right
        {
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
