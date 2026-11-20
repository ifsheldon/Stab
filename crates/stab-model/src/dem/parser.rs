use super::{
    DemArgVec, DemInstruction, DemInstructionKind, DemRepeatBlock, DemTag, DetectorErrorModel,
};
use crate::diagnostics::bounded_parse_diagnostic_text;
use crate::model_parse::{line_error, unexpected_repeat_terminator, unterminated_repeat_block};
use crate::parse_limits::ParseAdmission;
use crate::source_text::{SourceCommands, SourceSlice};
use crate::{
    ByteSpan, DemRepeatCount, ModelDialect, ModelError, ModelResult, ParseErrorCode,
    ParseErrorContext, ParseLimits,
};

mod diagnostics;
mod fast;
mod items;
mod tag;
mod targets;

use diagnostics::{
    UnsignedSourceError, dem_error_message, first_character_span, instruction_context,
    invalid_number_error, missing_repeat_count_error, parse_line_error, repeat_count_error,
    target_context, target_syntax_error, unsigned_target_error,
};

use items::{OpenDemRepeat, OpenDemRepeatHeader, ParsedDemItems};
use tag::parse_optional_tag;
use targets::{dem_validation_location, parse_dem_targets};

const MAX_DEM_TEXT_INTEGER: u64 = (1_u64 << 60) - 1;
const MAX_STIM_NUMBER_TOKEN_BYTES: usize = 63;

pub(super) fn parse_dem(input: &str, limits: ParseLimits) -> ModelResult<DetectorErrorModel> {
    let admission = ParseAdmission::new(ModelDialect::DetectorErrorModel, input.len(), limits)?;
    DemParser::new(input, admission).parse()
}

/// Parses `input` under `admission`, which has already charged `input` and any preparation
/// storage, and returns the admission holding everything it charged.
pub(super) fn parse_dem_admitted(
    input: &str,
    admission: ParseAdmission,
) -> ModelResult<(DetectorErrorModel, ParseAdmission)> {
    let mut parser = DemParser::new(input, admission);
    let model = parser.parse_block()?;
    Ok((model, parser.admission))
}

struct DemParser<'a> {
    commands: SourceCommands<'a>,
    input_len: usize,
    admission: ParseAdmission,
}

impl<'a> DemParser<'a> {
    fn new(input: &'a str, admission: ParseAdmission) -> Self {
        Self {
            commands: SourceCommands::new(input),
            input_len: input.len(),
            admission,
        }
    }

    fn parse(mut self) -> ModelResult<DetectorErrorModel> {
        self.parse_block()
    }

    /// Parses the whole input without recursing: each open `repeat` block is a frame holding
    /// its enclosing block's items, and the repeat-nesting admission bounds the frame count.
    ///
    /// A block's tag is charged when the block closes, after its body, and the charge reports
    /// the header line, as a body-first recursion would.
    #[inline(never)]
    fn parse_block(&mut self) -> ModelResult<DetectorErrorModel> {
        let mut items = ParsedDemItems::top_level();
        let mut enclosing: Vec<OpenDemRepeat> = Vec::new();
        while let Some(command) = self.commands.next() {
            let line_number = command.line_number();
            let line = command.source().trim_command_space_start();
            let semantic_line = trim_command_space_end(line);
            if semantic_line.text().is_empty() {
                continue;
            }
            if semantic_line.text() == "}" {
                let Some(open) = enclosing.pop() else {
                    return Err(unexpected_repeat_terminator(
                        ModelDialect::DetectorErrorModel,
                        semantic_line.span(),
                    ));
                };
                let body = std::mem::replace(&mut items, open.parent);
                let repeat = self.close_repeat(body, open.header)?;
                items.push_item(super::DemItem::RepeatBlock(repeat));
                continue;
            }
            self.admission
                .admit_instruction(line_number, semantic_line.span())?;

            if let Some(header) = semantic_line.without_suffix('{') {
                let header = self.open_repeat(
                    enclosing.len(),
                    line_number,
                    header,
                    semantic_line.span(),
                    command.end_error_span(),
                )?;
                enclosing.push(OpenDemRepeat {
                    parent: std::mem::replace(&mut items, ParsedDemItems::nested()),
                    header,
                });
            } else {
                // The fast path retains its arguments within the line-length bound it checked
                // and charges them with the tag; the diagnostic parser admits each argument
                // before retaining it.
                let (instruction, unadmitted_arguments) = if self
                    .admission
                    .target_budget_allows_upper_bound(line.text().len())
                    && let Some(instruction) = fast::parse_canonical_instruction(line.text())
                {
                    self.admission.admit_targets(
                        instruction.targets().len(),
                        line_number,
                        line.span(),
                    )?;
                    let arguments = instruction.args().len();
                    (instruction, arguments)
                } else {
                    let instruction = parse_dem_instruction(
                        line_number,
                        line,
                        command.end_error_span(),
                        &mut self.admission,
                    )?;
                    (instruction, 0)
                };
                self.admission.admit_payload(
                    unadmitted_arguments,
                    instruction.tag_bytes().map_or(0, <[u8]>::len),
                    line_number,
                    semantic_line.span(),
                )?;
                items.push_item(super::DemItem::Instruction(instruction));
            }
        }

        if enclosing.is_empty() {
            Ok(items.into_model())
        } else {
            Err(unterminated_repeat_block(
                ModelDialect::DetectorErrorModel,
                self.input_len,
            ))
        }
    }

    /// Parses the header of a `repeat` block opened inside `depth` open blocks and admits the
    /// nesting it reaches.
    #[inline(never)]
    fn open_repeat(
        &mut self,
        depth: usize,
        line_number: usize,
        header: SourceSlice<'a>,
        span: ByteSpan,
        end_error_span: ByteSpan,
    ) -> ModelResult<OpenDemRepeatHeader> {
        let header = trim_command_space_end(header);
        let repeat = if let Some((count, tag)) = fast::parse_canonical_repeat_header(header.text())
        {
            ParsedRepeatHeader {
                count: DemRepeatCount::new(count),
                tag,
            }
        } else {
            self.parse_repeat_header(line_number, header, end_error_span)?
        };
        let actual = depth.checked_add(1).ok_or_else(|| {
            ModelError::invalid_detector_error_model("DEM repeat nesting depth overflowed")
        })?;
        self.admission
            .admit_repeat_nesting(line_number, actual, span)?;
        Ok(OpenDemRepeatHeader {
            count: repeat.count,
            tag: repeat.tag,
            line_number,
            span,
        })
    }

    /// Builds a closed `repeat` block and charges its tag against the header line.
    #[inline(never)]
    fn close_repeat(
        &mut self,
        body: ParsedDemItems,
        header: OpenDemRepeatHeader,
    ) -> ModelResult<DemRepeatBlock> {
        let repeat = DemRepeatBlock::from_parts(header.count, body.into_model(), header.tag);
        self.admission.admit_payload(
            0,
            repeat.tag_bytes().map_or(0, <[u8]>::len),
            header.line_number,
            header.span,
        )?;
        Ok(repeat)
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

// Kept out of line so the parse loop stays compact.
#[inline(never)]
fn parse_dem_instruction(
    line_number: usize,
    line: SourceSlice<'_>,
    end_error_span: ByteSpan,
    admission: &mut ParseAdmission,
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
    let (mut args, rest) =
        parse_optional_args(line_number, name.text(), rest, end_error_span, admission)?;
    let mut targets = parse_dem_targets(line_number, name.text(), rest, end_error_span, admission)?;
    match targets.take_payload(kind, &mut args.values) {
        Ok(payload) => Ok(DemInstruction::from_validated_parts(payload, tag)),
        Err(error) => {
            let (code, span, context) = dem_validation_location(kind, name.text(), &args, &targets);
            Err(line_error(
                ModelDialect::DetectorErrorModel,
                line_number,
                code,
                dem_error_message(&error),
                error.to_string(),
                span,
                context,
            ))
        }
    }
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

struct ParsedDemArguments<'a> {
    values: DemArgVec,
    source: SourceSlice<'a>,
    region: ByteSpan,
}

/// Parses an optional parenthesized argument list, admitting each argument before it is retained.
fn parse_optional_args<'a>(
    line_number: usize,
    instruction: &str,
    rest: SourceSlice<'a>,
    end_error_span: ByteSpan,
    admission: &mut ParseAdmission,
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
        admission.admit_argument(line_number, token.span())?;
        values.try_reserve(1).map_err(|_| {
            ModelError::invalid_detector_error_model("DEM argument allocation failed")
        })?;
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

fn parse_repeat_count(line_number: usize, token: SourceSlice<'_>) -> ModelResult<u64> {
    parse_source_uint60(token).map_err(|error| repeat_count_error(line_number, token, error))
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
