use super::{
    DemArgVec, DemInstruction, DemInstructionKind, DemRepeatBlock, DemTag, DemTarget, DemTargetVec,
    DetectorErrorModel,
};
use crate::advanced::{dem_repeat_nesting_limit_error, dem_source_line_limit_error};
use crate::diagnostics::bounded_parse_diagnostic_text;
use crate::model_parse::{line_error, unexpected_repeat_terminator, unterminated_repeat_block};
use crate::source_text::{SourceCommands, SourceSlice};
use crate::{
    ByteSpan, DemRepeatCount, ModelDialect, ModelError, ModelResult, ParseErrorCode,
    ParseErrorContext, ParseLimits, ValidationError,
};

mod fast;

const MAX_DEM_TEXT_INTEGER: u64 = (1_u64 << 60) - 1;
const MAX_DEM_PREALLOCATED_ITEMS: usize = 131_072;
const DEM_PREALLOCATION_SAMPLE_BYTES: usize = 256;
const MAX_STIM_NUMBER_TOKEN_BYTES: usize = 63;

pub(super) fn parse_dem(input: &str, limits: ParseLimits) -> ModelResult<DetectorErrorModel> {
    DemParser::new(input, limits).parse()
}

struct DemParser<'a> {
    commands: SourceCommands<'a>,
    input_len: usize,
    top_level_capacity: usize,
    limits: ParseLimits,
}

impl<'a> DemParser<'a> {
    fn new(input: &'a str, limits: ParseLimits) -> Self {
        Self {
            commands: SourceCommands::new(input),
            input_len: input.len(),
            top_level_capacity: top_level_item_capacity(input, limits),
            limits,
        }
    }

    fn parse(mut self) -> ModelResult<DetectorErrorModel> {
        self.parse_block(false, 0)
    }

    fn parse_block(
        &mut self,
        stop_on_terminator: bool,
        depth: usize,
    ) -> ModelResult<DetectorErrorModel> {
        let mut model = if stop_on_terminator {
            DetectorErrorModel::new()
        } else {
            DetectorErrorModel::with_capacity(self.top_level_capacity)
        };
        while let Some(command) = self.next_command()? {
            let line_number = command.line_number();
            let line = command.source().trim_ascii_start();
            let semantic_line = trim_command_space_end(line);
            if semantic_line.text().is_empty() {
                continue;
            }
            if semantic_line.text() == "}" {
                if stop_on_terminator {
                    return Ok(model);
                }
                return Err(unexpected_repeat_terminator(
                    ModelDialect::DetectorErrorModel,
                    semantic_line.span(),
                ));
            }

            if let Some(header) = semantic_line.without_suffix('{') {
                let header = trim_command_space_end(header);
                let repeat = if let Some((count, tag)) =
                    fast::parse_canonical_repeat_header(header.text())
                {
                    ParsedRepeatHeader {
                        count: DemRepeatCount::new(count),
                        tag,
                    }
                } else {
                    self.parse_repeat_header(line_number, header, command.end_error_span())?
                };
                let limit = self.limits.repeat_nesting_limit().get();
                if depth >= limit {
                    let actual = depth.checked_add(1).ok_or_else(|| {
                        ModelError::invalid_detector_error_model(
                            "DEM repeat nesting depth overflowed",
                        )
                    })?;
                    return Err(dem_repeat_nesting_limit_error(
                        actual,
                        limit,
                        semantic_line.span(),
                    ));
                }
                let body = self.parse_block(true, depth + 1)?;
                model.push_repeat_block(DemRepeatBlock::from_parts(repeat.count, body, repeat.tag));
            } else {
                let instruction =
                    if let Some(instruction) = fast::parse_canonical_instruction(line.text()) {
                        instruction
                    } else {
                        parse_dem_instruction(line_number, line, command.end_error_span())?
                    };
                model.push_instruction(instruction);
            }
        }

        if stop_on_terminator {
            Err(unterminated_repeat_block(
                ModelDialect::DetectorErrorModel,
                self.input_len,
            ))
        } else {
            Ok(model)
        }
    }

    fn next_command(&mut self) -> ModelResult<Option<crate::source_text::SourceCommand<'a>>> {
        let Some(command) = self.commands.next() else {
            return Ok(None);
        };
        let line_number = command.line_number();
        let limit = self.limits.source_line_limit().get();
        if line_number > limit {
            return Err(dem_source_line_limit_error(
                line_number,
                limit,
                command.source().span(),
            ));
        }
        Ok(Some(command))
    }

    fn parse_repeat_header(
        &self,
        line_number: usize,
        header: SourceSlice<'a>,
        end_error_span: ByteSpan,
    ) -> ModelResult<ParsedRepeatHeader> {
        let (name, rest) = parse_name(line_number, header)?;
        if !name.text().eq_ignore_ascii_case("repeat") {
            return Err(line_error(
                ModelDialect::DetectorErrorModel,
                line_number,
                ParseErrorCode::InvalidRepeatBlock,
                "repeat blocks must be written as repeat <count> {",
                "repeat blocks must be written as repeat <count> {",
                header.span(),
                instruction_context(name.text()),
            ));
        }
        let (tag, rest) = parse_optional_tag(line_number, name.text(), rest, end_error_span)?;
        if !starts_with_target_space(rest) {
            return Err(missing_repeat_count_error(
                line_number,
                rest.subspan(0, usize::from(!rest.text().is_empty()))
                    .unwrap_or(end_error_span),
            ));
        }
        let rest = trim_target_space_start(rest);
        if rest.text().is_empty() {
            return Err(missing_repeat_count_error(line_number, end_error_span));
        }

        let token_end = rest
            .text()
            .as_bytes()
            .iter()
            .position(|byte| is_target_space(*byte))
            .unwrap_or(rest.text().len());
        let count_token = rest
            .prefix(token_end)
            .ok_or_else(|| parse_line_error(line_number, "invalid repeat count"))?;
        let trailing = rest
            .suffix(token_end)
            .ok_or_else(|| parse_line_error(line_number, "invalid repeat count"))?;
        if !trailing
            .text()
            .as_bytes()
            .iter()
            .all(|byte| is_target_space(*byte))
        {
            return Err(line_error(
                ModelDialect::DetectorErrorModel,
                line_number,
                ParseErrorCode::InvalidRepeatBlock,
                "repeat blocks must be written as repeat <count> {",
                "repeat blocks must be written as repeat <count> {",
                trim_target_space_start(trailing).span(),
                instruction_context(name.text()),
            ));
        }
        let count = parse_repeat_count(line_number, count_token)?;
        Ok(ParsedRepeatHeader {
            count: DemRepeatCount::new(count),
            tag,
        })
    }
}

struct ParsedRepeatHeader {
    count: DemRepeatCount,
    tag: Option<DemTag>,
}

fn top_level_item_capacity(input: &str, limits: ParseLimits) -> usize {
    let admitted_lines = limits.source_line_limit().get();
    if input.is_empty() || admitted_lines == 0 {
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
        .min(admitted_lines)
}

fn parse_dem_instruction(
    line_number: usize,
    line: SourceSlice<'_>,
    end_error_span: ByteSpan,
) -> ModelResult<DemInstruction> {
    let (kind, name, rest) = parse_instruction_kind(line_number, line)?;
    if name.text().eq_ignore_ascii_case("repeat") {
        return Err(line_error(
            ModelDialect::DetectorErrorModel,
            line_number,
            ParseErrorCode::InvalidRepeatBlock,
            "repeat block is missing an opening brace",
            "invalid detector error model: unknown DEM instruction repeat",
            end_error_span,
            instruction_context(name.text()),
        ));
    }
    let kind = kind.ok_or_else(|| {
        let name_excerpt = bounded_parse_diagnostic_text(name.text());
        line_error(
            ModelDialect::DetectorErrorModel,
            line_number,
            ParseErrorCode::UnknownInstruction,
            format!("unknown DEM instruction {name_excerpt}"),
            format!("invalid detector error model: unknown DEM instruction {name_excerpt}"),
            name.span(),
            ParseErrorContext::Instruction {
                dialect: ModelDialect::DetectorErrorModel,
                instruction: name_excerpt,
            },
        )
    })?;
    let (tag, rest) = parse_optional_tag(line_number, name.text(), rest, end_error_span)?;
    let (args, rest) = parse_optional_args(line_number, name.text(), rest, end_error_span)?;
    let targets = parse_dem_targets(line_number, name.text(), rest, end_error_span)?;
    if let Err(error) = super::validate_dem_instruction(kind, &args.values, &targets.values) {
        let (code, span, context) = dem_validation_location(kind, name.text(), &args, &targets);
        return Err(line_error(
            ModelDialect::DetectorErrorModel,
            line_number,
            code,
            dem_error_message(&error),
            error.to_string(),
            span,
            context,
        ));
    }
    Ok(DemInstruction::from_validated_parts(
        kind,
        args.values,
        targets.values,
        tag,
    ))
}

fn parse_instruction_kind(
    line_number: usize,
    line: SourceSlice<'_>,
) -> ModelResult<(Option<DemInstructionKind>, SourceSlice<'_>, SourceSlice<'_>)> {
    for (name, kind) in [
        ("error", DemInstructionKind::Error),
        ("detector", DemInstructionKind::Detector),
        ("logical_observable", DemInstructionKind::LogicalObservable),
        ("shift_detectors", DemInstructionKind::ShiftDetectors),
    ] {
        if line.text().starts_with(name)
            && line
                .text()
                .as_bytes()
                .get(name.len())
                .is_none_or(|byte| matches!(byte, b'[' | b'(' | b' ' | b'\t' | b'\r'))
        {
            let parsed_name = line
                .prefix(name.len())
                .ok_or_else(|| parse_line_error(line_number, "invalid DEM instruction name"))?;
            let rest = line
                .suffix(name.len())
                .ok_or_else(|| parse_line_error(line_number, "invalid DEM instruction name"))?;
            return Ok((Some(kind), parsed_name, rest));
        }
    }

    let (name, rest) = parse_name(line_number, line)?;
    Ok((DemInstructionKind::lookup_name(name.text()), name, rest))
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
            ModelDialect::DetectorErrorModel,
            line_number,
            ParseErrorCode::MissingInstructionName,
            "missing DEM instruction name",
            "missing DEM instruction name",
            first_character_span(line),
            ParseErrorContext::Model {
                dialect: ModelDialect::DetectorErrorModel,
            },
        ));
    };
    let name = line
        .prefix(end)
        .ok_or_else(|| parse_line_error(line_number, "missing DEM instruction name"))?;
    let rest = line
        .suffix(end)
        .ok_or_else(|| parse_line_error(line_number, "missing DEM instruction name"))?;
    Ok((name, rest))
}

fn parse_optional_tag<'a>(
    line_number: usize,
    instruction: &str,
    rest: SourceSlice<'a>,
    end_error_span: ByteSpan,
) -> ModelResult<(Option<DemTag>, SourceSlice<'a>)> {
    let Some(mut body) = rest.strip_prefix("[") else {
        return Ok((None, rest));
    };
    if let Some(end) = body.text().as_bytes().iter().position(|byte| *byte == b']') {
        let raw_tag = body
            .prefix(end)
            .ok_or_else(|| parse_line_error(line_number, "unterminated tag"))?;
        if !raw_tag
            .text()
            .as_bytes()
            .iter()
            .any(|byte| matches!(byte, b'\\' | b'\r' | b'\n'))
        {
            let tail = body
                .suffix(end + 1)
                .ok_or_else(|| parse_line_error(line_number, "unterminated tag"))?;
            return Ok((DemTag::from_text(raw_tag.text()), tail));
        }
    }
    let mut tag = String::new();
    loop {
        let Some((character, character_len)) = body
            .text()
            .chars()
            .next()
            .map(|character| (character, character.len_utf8()))
        else {
            return Err(line_error(
                ModelDialect::DetectorErrorModel,
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
            .ok_or_else(|| parse_line_error(line_number, "unterminated tag"))?;
        match character {
            ']' => return Ok((DemTag::from_string(tag), body)),
            '\\' => {
                let Some((escaped, escaped_len)) = body
                    .text()
                    .chars()
                    .next()
                    .map(|escaped| (escaped, escaped.len_utf8()))
                else {
                    return Err(line_error(
                        ModelDialect::DetectorErrorModel,
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
                    .ok_or_else(|| parse_line_error(line_number, "unterminated tag escape"))?;
                tag.push(match escaped {
                    'C' => ']',
                    'r' => '\r',
                    'n' => '\n',
                    'B' => '\\',
                    _ => {
                        return Err(line_error(
                            ModelDialect::DetectorErrorModel,
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
                    ModelDialect::DetectorErrorModel,
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

struct ParsedDemArguments<'a> {
    values: DemArgVec,
    source: SourceSlice<'a>,
    region: ByteSpan,
}

fn parse_optional_args<'a>(
    line_number: usize,
    instruction: &str,
    rest: SourceSlice<'a>,
    end_error_span: ByteSpan,
) -> ModelResult<(ParsedDemArguments<'a>, SourceSlice<'a>)> {
    let Some(body) = rest.strip_prefix("(") else {
        return Ok((
            ParsedDemArguments {
                values: DemArgVec::new(),
                source: SourceSlice::new("", rest.byte_start()),
                region: rest.subspan(0, 0).unwrap_or_else(|| rest.end_span()),
            },
            rest,
        ));
    };
    let Some(end) = body.text().find(')') else {
        return Err(line_error(
            ModelDialect::DetectorErrorModel,
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
        .ok_or_else(|| parse_line_error(line_number, "unterminated argument list"))?;
    let tail = body
        .suffix(end + 1)
        .ok_or_else(|| parse_line_error(line_number, "unterminated argument list"))?;
    let mut values = DemArgVec::new();
    let mut token_start = 0;
    loop {
        let token_end = raw_args
            .text()
            .get(token_start..)
            .and_then(|tail| tail.find(',').map(|offset| token_start + offset))
            .unwrap_or(raw_args.text().len());
        let token = raw_args
            .slice(token_start, token_end)
            .ok_or_else(|| parse_line_error(line_number, "invalid argument"))?
            .trim_inline_ascii_start()
            .trim_inline_ascii_end();
        let value = if token.text().is_empty() {
            0.0
        } else if token.text().len() > MAX_STIM_NUMBER_TOKEN_BYTES {
            return Err(invalid_number_error(line_number, instruction, token));
        } else {
            token
                .text()
                .parse::<f64>()
                .map_err(|_| invalid_number_error(line_number, instruction, token))?
        };
        if !value.is_finite() {
            return Err(invalid_number_error(line_number, instruction, token));
        }
        values.push(value);
        if token_end == raw_args.text().len() {
            break;
        }
        token_start = token_end + 1;
    }
    Ok((
        ParsedDemArguments {
            values,
            source: raw_args,
            region: raw_args.span(),
        },
        tail,
    ))
}

struct ParsedDemTargets<'a> {
    values: DemTargetVec,
    source: SourceSlice<'a>,
    region: ByteSpan,
}

fn parse_dem_targets<'a>(
    line_number: usize,
    instruction: &str,
    rest: SourceSlice<'a>,
    end_error_span: ByteSpan,
) -> ModelResult<ParsedDemTargets<'a>> {
    if rest.text().is_empty() {
        return Ok(ParsedDemTargets {
            values: DemTargetVec::new(),
            source: SourceSlice::new("", rest.byte_start()),
            region: end_error_span,
        });
    }
    if !starts_with_target_space(rest) {
        return Err(target_syntax_error(
            line_number,
            instruction,
            rest.text(),
            first_character_span(rest),
            "targets must be separated by spacing",
        ));
    }
    let content = trim_target_space_start(rest);
    if content.text().is_empty() {
        return Ok(ParsedDemTargets {
            values: DemTargetVec::new(),
            source: content,
            region: content.end_span(),
        });
    }

    let mut values = DemTargetVec::new();
    let mut cursor = 0;
    let mut region_start = None;
    let mut region_end = content.byte_start();
    while cursor < content.text().len() {
        while content
            .text()
            .as_bytes()
            .get(cursor)
            .is_some_and(|byte| is_target_space(*byte))
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
            .is_some_and(|byte| !is_target_space(*byte))
        {
            cursor += 1;
        }
        let token = content
            .slice(start, cursor)
            .ok_or_else(|| parse_line_error(line_number, "invalid DEM target"))?;
        let target = parse_arbitrary_target(line_number, instruction, token)?;
        values.push(target);
        region_start.get_or_insert(token.byte_start());
        region_end = token.byte_end();
    }
    let region = region_start
        .and_then(|start| ByteSpan::try_new(start, region_end.saturating_sub(start)))
        .unwrap_or_else(|| content.end_span());
    Ok(ParsedDemTargets {
        values,
        source: content,
        region,
    })
}

fn parse_arbitrary_target(
    line_number: usize,
    instruction: &str,
    token: SourceSlice<'_>,
) -> ModelResult<DemTarget> {
    let Some(prefix) = token.text().as_bytes().first().copied() else {
        return Err(target_syntax_error(
            line_number,
            instruction,
            "",
            token.end_span(),
            "invalid DEM target",
        ));
    };
    match prefix {
        b'D' | b'd' => {
            let number = token
                .suffix(1)
                .ok_or_else(|| parse_line_error(line_number, "invalid DEM target"))?;
            let value = parse_source_uint60(number).map_err(|error| {
                unsigned_target_error(
                    line_number,
                    instruction,
                    token,
                    number,
                    "relative detector target",
                    error,
                )
            })?;
            DemTarget::relative_detector(value).map_err(|error| {
                line_error(
                    ModelDialect::DetectorErrorModel,
                    line_number,
                    ParseErrorCode::IntegerOutOfRange,
                    dem_error_message(&error),
                    error.to_string(),
                    token.span(),
                    target_context(instruction, token.text()),
                )
            })
        }
        b'L' | b'l' => {
            let number = token
                .suffix(1)
                .ok_or_else(|| parse_line_error(line_number, "invalid DEM target"))?;
            let value = parse_source_uint60(number).map_err(|error| {
                unsigned_target_error(
                    line_number,
                    instruction,
                    token,
                    number,
                    "logical observable target",
                    error,
                )
            })?;
            DemTarget::logical_observable(value).map_err(|error| {
                line_error(
                    ModelDialect::DetectorErrorModel,
                    line_number,
                    ParseErrorCode::IntegerOutOfRange,
                    dem_error_message(&error),
                    error.to_string(),
                    token.span(),
                    target_context(instruction, token.text()),
                )
            })
        }
        b'^' if token.text().len() == 1 => Ok(DemTarget::separator()),
        b'0'..=b'9' => {
            let value = parse_source_uint60(token).map_err(|error| {
                unsigned_target_error(
                    line_number,
                    instruction,
                    token,
                    token,
                    "numeric DEM target",
                    error,
                )
            })?;
            Ok(DemTarget::numeric(value))
        }
        _ => Err(line_error(
            ModelDialect::DetectorErrorModel,
            line_number,
            ParseErrorCode::InvalidTargetSyntax,
            "unrecognized DEM target prefix",
            format!(
                "invalid detector error model: invalid DEM target {:?}",
                token.text()
            ),
            first_character_span(token),
            target_context(instruction, token.text()),
        )),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum UnsignedSourceError {
    InvalidDigit { byte_offset: usize },
    OutOfRange,
}

fn parse_source_uint60(token: SourceSlice<'_>) -> Result<u64, UnsignedSourceError> {
    if token.text().is_empty() {
        return Err(UnsignedSourceError::InvalidDigit { byte_offset: 0 });
    }
    let mut value = 0_u64;
    for (byte_offset, byte) in token.text().bytes().enumerate() {
        if !byte.is_ascii_digit() {
            return Err(UnsignedSourceError::InvalidDigit { byte_offset });
        }
        let digit = u64::from(byte - b'0');
        value = value
            .checked_mul(10)
            .and_then(|value| value.checked_add(digit))
            .ok_or(UnsignedSourceError::OutOfRange)?;
        if value > MAX_DEM_TEXT_INTEGER {
            return Err(UnsignedSourceError::OutOfRange);
        }
    }
    Ok(value)
}

fn unsigned_target_error(
    line_number: usize,
    instruction: &str,
    full_token: SourceSlice<'_>,
    number_token: SourceSlice<'_>,
    target_kind: &'static str,
    error: UnsignedSourceError,
) -> ModelError {
    let (code, span, message) = match error {
        UnsignedSourceError::InvalidDigit { byte_offset } => (
            ParseErrorCode::InvalidTargetSyntax,
            character_span_at(number_token, byte_offset.min(number_token.text().len())),
            format!("invalid {target_kind} {:?}", number_token.text()),
        ),
        UnsignedSourceError::OutOfRange => (
            ParseErrorCode::IntegerOutOfRange,
            full_token.span(),
            number_token.text().parse::<u64>().map_or_else(
                |_| format!("invalid {target_kind} {:?}", number_token.text()),
                |value| format!("{target_kind} {value} exceeds {MAX_DEM_TEXT_INTEGER}"),
            ),
        ),
    };
    let legacy = ModelError::invalid_detector_error_model(message.clone()).to_string();
    line_error(
        ModelDialect::DetectorErrorModel,
        line_number,
        code,
        message,
        legacy,
        span,
        target_context(instruction, full_token.text()),
    )
}

fn parse_repeat_count(line_number: usize, token: SourceSlice<'_>) -> ModelResult<u64> {
    parse_source_uint60(token).map_err(|error| {
        let (code, span, message, legacy_message) = match error {
            UnsignedSourceError::InvalidDigit { byte_offset } => (
                ParseErrorCode::InvalidRepeatCount,
                character_span_at(token, byte_offset),
                "invalid repeat count",
                format!("invalid repeat count {:?}", token.text()),
            ),
            UnsignedSourceError::OutOfRange => (
                ParseErrorCode::IntegerOutOfRange,
                token.span(),
                "repeat count is out of range",
                token.text().parse::<u64>().map_or_else(
                    |_| format!("invalid repeat count {:?}", token.text()),
                    |value| format!("repeat count {value} exceeds {MAX_DEM_TEXT_INTEGER}"),
                ),
            ),
        };
        line_error(
            ModelDialect::DetectorErrorModel,
            line_number,
            code,
            message,
            ModelError::invalid_detector_error_model(legacy_message).to_string(),
            span,
            ParseErrorContext::DomainValue {
                dialect: ModelDialect::DetectorErrorModel,
                kind: "repeat count",
                value: bounded_parse_diagnostic_text(token.text()),
            },
        )
    })
}

fn dem_validation_location(
    kind: DemInstructionKind,
    instruction: &str,
    args: &ParsedDemArguments<'_>,
    targets: &ParsedDemTargets<'_>,
) -> (ParseErrorCode, ByteSpan, ParseErrorContext) {
    match kind {
        DemInstructionKind::Error if args.values.len() != 1 => (
            ParseErrorCode::InvalidArgumentCount,
            args.region,
            ParseErrorContext::ArgumentCount {
                dialect: ModelDialect::DetectorErrorModel,
                instruction: bounded_parse_diagnostic_text(instruction),
                expected: "exactly 1",
                actual: args.values.len(),
            },
        ),
        DemInstructionKind::Error
            if args
                .values
                .first()
                .is_some_and(|probability| !(0.0..=1.0).contains(probability)) =>
        {
            let token = argument_token_at(args.source, 0);
            (
                ParseErrorCode::InvalidArgument,
                token.map_or(args.region, SourceSlice::span),
                ParseErrorContext::Argument {
                    dialect: ModelDialect::DetectorErrorModel,
                    instruction: bounded_parse_diagnostic_text(instruction),
                    argument: token.map_or_else(String::new, |token| {
                        bounded_parse_diagnostic_text(token.text())
                    }),
                },
            )
        }
        DemInstructionKind::Error => {
            let invalid_index = targets
                .values
                .iter()
                .enumerate()
                .find_map(|(index, target)| {
                    if !matches!(target, DemTarget::Separator) {
                        return None;
                    }
                    let adjacent = index == 0
                        || index + 1 == targets.values.len()
                        || matches!(targets.values.get(index - 1), Some(DemTarget::Separator));
                    adjacent.then_some(index)
                });
            let token = invalid_index.and_then(|index| target_token_at(targets.source, index));
            (
                ParseErrorCode::InvalidTarget,
                token.map_or(targets.region, SourceSlice::span),
                ParseErrorContext::Target {
                    dialect: ModelDialect::DetectorErrorModel,
                    instruction: bounded_parse_diagnostic_text(instruction),
                    target: token.map_or_else(String::new, |token| {
                        bounded_parse_diagnostic_text(token.text())
                    }),
                },
            )
        }
        DemInstructionKind::LogicalObservable if !args.values.is_empty() => (
            ParseErrorCode::InvalidArgumentCount,
            args.region,
            ParseErrorContext::ArgumentCount {
                dialect: ModelDialect::DetectorErrorModel,
                instruction: bounded_parse_diagnostic_text(instruction),
                expected: "exactly 0",
                actual: args.values.len(),
            },
        ),
        DemInstructionKind::Detector
        | DemInstructionKind::LogicalObservable
        | DemInstructionKind::ShiftDetectors
            if targets.values.len() != 1 =>
        {
            (
                ParseErrorCode::InvalidTargetCount,
                targets.region,
                ParseErrorContext::TargetCount {
                    dialect: ModelDialect::DetectorErrorModel,
                    instruction: bounded_parse_diagnostic_text(instruction),
                    actual: targets.values.len(),
                },
            )
        }
        DemInstructionKind::Detector
            if !matches!(targets.values.first(), Some(DemTarget::RelativeDetector(_))) =>
        {
            invalid_dem_target_location(instruction, targets)
        }
        DemInstructionKind::LogicalObservable
            if !matches!(
                targets.values.first(),
                Some(DemTarget::LogicalObservable(_))
            ) =>
        {
            invalid_dem_target_location(instruction, targets)
        }
        DemInstructionKind::ShiftDetectors
            if !matches!(targets.values.first(), Some(DemTarget::Numeric(_))) =>
        {
            invalid_dem_target_location(instruction, targets)
        }
        _ => (
            ParseErrorCode::InvalidSyntax,
            targets.region,
            instruction_context(instruction),
        ),
    }
}

fn invalid_dem_target_location(
    instruction: &str,
    targets: &ParsedDemTargets<'_>,
) -> (ParseErrorCode, ByteSpan, ParseErrorContext) {
    let token = target_token_at(targets.source, 0);
    (
        ParseErrorCode::InvalidTarget,
        token.map_or(targets.region, SourceSlice::span),
        ParseErrorContext::Target {
            dialect: ModelDialect::DetectorErrorModel,
            instruction: bounded_parse_diagnostic_text(instruction),
            target: token.map_or_else(String::new, |token| {
                bounded_parse_diagnostic_text(token.text())
            }),
        },
    )
}

fn argument_token_at(arguments: SourceSlice<'_>, wanted_index: usize) -> Option<SourceSlice<'_>> {
    let mut start = 0;
    let mut index = 0;
    loop {
        let end = arguments
            .text()
            .get(start..)?
            .find(',')
            .map_or(arguments.text().len(), |offset| start + offset);
        if index == wanted_index {
            return Some(
                arguments
                    .slice(start, end)?
                    .trim_inline_ascii_start()
                    .trim_inline_ascii_end(),
            );
        }
        if end == arguments.text().len() {
            return None;
        }
        start = end.checked_add(1)?;
        index = index.checked_add(1)?;
    }
}

fn target_token_at(targets: SourceSlice<'_>, wanted_index: usize) -> Option<SourceSlice<'_>> {
    let mut cursor = 0;
    let mut index = 0;
    while cursor < targets.text().len() {
        while targets
            .text()
            .as_bytes()
            .get(cursor)
            .is_some_and(|byte| is_target_space(*byte))
        {
            cursor += 1;
        }
        if cursor == targets.text().len() {
            return None;
        }
        let start = cursor;
        while targets
            .text()
            .as_bytes()
            .get(cursor)
            .is_some_and(|byte| !is_target_space(*byte))
        {
            cursor += 1;
        }
        if index == wanted_index {
            return targets.slice(start, cursor);
        }
        index = index.checked_add(1)?;
    }
    None
}

fn invalid_number_error(
    line_number: usize,
    instruction: &str,
    token: SourceSlice<'_>,
) -> ModelError {
    let span = token.span();
    let token = bounded_parse_diagnostic_text(token.text());
    line_error(
        ModelDialect::DetectorErrorModel,
        line_number,
        ParseErrorCode::InvalidNumber,
        format!("invalid argument {token}"),
        format!("invalid argument {token}"),
        span,
        ParseErrorContext::Argument {
            dialect: ModelDialect::DetectorErrorModel,
            instruction: bounded_parse_diagnostic_text(instruction),
            argument: token,
        },
    )
}

fn target_syntax_error(
    line_number: usize,
    instruction: &str,
    target: &str,
    span: ByteSpan,
    message: &'static str,
) -> ModelError {
    line_error(
        ModelDialect::DetectorErrorModel,
        line_number,
        ParseErrorCode::InvalidTargetSyntax,
        message,
        format!("{message}: {target:?}"),
        span,
        target_context(instruction, target),
    )
}

fn missing_repeat_count_error(line_number: usize, span: ByteSpan) -> ModelError {
    line_error(
        ModelDialect::DetectorErrorModel,
        line_number,
        ParseErrorCode::MissingRepeatCount,
        "missing repeat count",
        "missing repeat count",
        span,
        ParseErrorContext::DomainValue {
            dialect: ModelDialect::DetectorErrorModel,
            kind: "repeat count",
            value: String::new(),
        },
    )
}

fn parse_line_error(line_number: usize, message: impl Into<String>) -> ModelError {
    let message = message.into();
    line_error(
        ModelDialect::DetectorErrorModel,
        line_number,
        ParseErrorCode::InvalidSyntax,
        message.clone(),
        message,
        ByteSpan::from_valid_range(0, 0),
        ParseErrorContext::Model {
            dialect: ModelDialect::DetectorErrorModel,
        },
    )
}

fn instruction_context(instruction: &str) -> ParseErrorContext {
    ParseErrorContext::Instruction {
        dialect: ModelDialect::DetectorErrorModel,
        instruction: bounded_parse_diagnostic_text(instruction),
    }
}

fn target_context(instruction: &str, target: &str) -> ParseErrorContext {
    ParseErrorContext::Target {
        dialect: ModelDialect::DetectorErrorModel,
        instruction: bounded_parse_diagnostic_text(instruction),
        target: bounded_parse_diagnostic_text(target),
    }
}

fn dem_error_message(error: &ModelError) -> String {
    match error.validation_error() {
        Some(ValidationError::InvalidDetectorErrorModel { message }) => message.clone(),
        _ => error.to_string(),
    }
}

fn first_character_span(source: SourceSlice<'_>) -> ByteSpan {
    character_span_at(source, 0)
}

fn character_span_at(source: SourceSlice<'_>, byte_offset: usize) -> ByteSpan {
    let Some(tail) = source.suffix(byte_offset) else {
        return source.span();
    };
    tail.text()
        .chars()
        .next()
        .and_then(|character| tail.subspan(0, character.len_utf8()))
        .unwrap_or_else(|| tail.end_span())
}

fn starts_with_target_space(source: SourceSlice<'_>) -> bool {
    source
        .text()
        .as_bytes()
        .first()
        .is_some_and(|byte| is_target_space(*byte))
}

fn is_target_space(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\r')
}

fn trim_target_space_start(mut source: SourceSlice<'_>) -> SourceSlice<'_> {
    let count = source
        .text()
        .as_bytes()
        .iter()
        .take_while(|byte| is_target_space(**byte))
        .count();
    if let Some(trimmed) = source.suffix(count) {
        source = trimmed;
    }
    source
}

fn trim_command_space_end(source: SourceSlice<'_>) -> SourceSlice<'_> {
    let keep = source
        .text()
        .as_bytes()
        .iter()
        .rposition(|byte| !is_target_space(*byte))
        .map_or(0, |index| index + 1);
    source.prefix(keep).unwrap_or(source)
}

pub(super) fn parse_unsigned_dem_text_value(text: &str, kind: &'static str) -> ModelResult<u64> {
    let value = parse_unsigned_dem_value(text, kind)?;
    if value > MAX_DEM_TEXT_INTEGER {
        return Err(ModelError::invalid_detector_error_model(format!(
            "{kind} {value} exceeds {MAX_DEM_TEXT_INTEGER}"
        )));
    }
    Ok(value)
}

fn parse_unsigned_dem_value(text: &str, kind: &'static str) -> ModelResult<u64> {
    if text.is_empty() {
        return Err(ModelError::invalid_detector_error_model(format!(
            "invalid {kind} {text:?}"
        )));
    }
    let mut value = 0_u64;
    for byte in text.bytes() {
        if !byte.is_ascii_digit() {
            return Err(ModelError::invalid_detector_error_model(format!(
                "invalid {kind} {text:?}"
            )));
        }
        let digit = u64::from(byte - b'0');
        value = value
            .checked_mul(10)
            .and_then(|value| value.checked_add(digit))
            .ok_or_else(|| {
                ModelError::invalid_detector_error_model(format!("invalid {kind} {text:?}"))
            })?;
    }
    Ok(value)
}
