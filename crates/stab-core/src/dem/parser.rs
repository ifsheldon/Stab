use std::str::Lines;

use super::{
    DemArgVec, DemInstruction, DemInstructionKind, DemRepeatBlock, DemTarget, DemTargetVec,
    DetectorErrorModel, MAX_DEM_REPEAT_NESTING,
};
use crate::{CircuitError, CircuitResult, DemRepeatCount};

const MAX_DEM_TEXT_INTEGER: u64 = (1_u64 << 60) - 1;
const MAX_DEM_PARSE_LINES: usize = 1_000_000;
const MAX_DEM_PREALLOCATED_ITEMS: usize = 131_072;
const DEM_PREALLOCATION_SAMPLE_BYTES: usize = 256;

pub(super) fn parse_dem(input: &str) -> CircuitResult<DetectorErrorModel> {
    DemParser::new(input).parse()
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
        let mut parts = rest
            .split([' ', '\t', '\r'])
            .filter(|part| !part.is_empty());
        let count = parts
            .next()
            .ok_or_else(|| CircuitError::parse_line(line_number, "missing repeat count"))?;
        if parts.next().is_some() {
            return Err(CircuitError::parse_line(
                line_number,
                "repeat blocks must be written as repeat <count> {",
            ));
        }
        let count = parse_unsigned_dem_text_value(count, "repeat count")
            .map_err(|error| wrap_line(line_number, error))?;
        let body = self.parse_block(true, parent_depth + 1)?;
        Ok(DemRepeatBlock::from_parts(
            DemRepeatCount::new(count),
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
    let (kind, rest) = parse_instruction_kind(line_number, line)?;
    let (tag, rest) = parse_optional_tag(line_number, rest)?;
    let (args, rest) = parse_optional_args(line_number, rest)?;
    let targets = parse_dem_targets(rest).map_err(|error| wrap_line(line_number, error))?;
    DemInstruction::from_parts(kind, args, targets, tag)
        .map_err(|error| wrap_line(line_number, error))
}

fn parse_instruction_kind(
    line_number: usize,
    line: &str,
) -> CircuitResult<(DemInstructionKind, &str)> {
    for (name, kind) in [
        ("error", DemInstructionKind::Error),
        ("detector", DemInstructionKind::Detector),
        ("logical_observable", DemInstructionKind::LogicalObservable),
        ("shift_detectors", DemInstructionKind::ShiftDetectors),
    ] {
        if let Some(rest) = line.strip_prefix(name)
            && rest
                .as_bytes()
                .first()
                .is_none_or(|byte| matches!(byte, b'[' | b'(' | b' ' | b'\t' | b'\r'))
        {
            return Ok((kind, rest));
        }
    }
    let (name, rest) = parse_name(line_number, line)?;
    let kind =
        DemInstructionKind::from_name(name).map_err(|error| wrap_line(line_number, error))?;
    Ok((kind, rest))
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

fn parse_optional_tag(
    line_number: usize,
    rest: &str,
) -> CircuitResult<(Option<super::DemTag>, &str)> {
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
            return Ok((super::DemTag::from_text(raw_tag), tail));
        }
    }
    let mut tag = String::new();
    loop {
        let Some((ch, after_ch)) = split_first_char(body) else {
            return Err(CircuitError::parse_line(line_number, "unterminated tag"));
        };
        body = after_ch;
        match ch {
            ']' => return Ok((super::DemTag::from_string(tag), body)),
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
    for arg in raw_args.split(',') {
        let arg = trim_dem_inline(arg);
        args.push(if arg.is_empty() {
            0.0
        } else {
            arg.parse::<f64>().map_err(|_| {
                CircuitError::parse_line(line_number, format!("invalid argument {arg}"))
            })?
        });
    }
    Ok((args, tail))
}

fn parse_dem_targets(rest: &str) -> CircuitResult<DemTargetVec> {
    let mut targets = DemTargetVec::new();
    let bytes = rest.as_bytes();
    let mut cursor = 0;
    while cursor < bytes.len() {
        while bytes
            .get(cursor)
            .is_some_and(|byte| matches!(byte, b' ' | b'\t' | b'\r'))
        {
            cursor += 1;
        }
        let start = cursor;
        while bytes
            .get(cursor)
            .is_some_and(|byte| !matches!(byte, b' ' | b'\t' | b'\r'))
        {
            cursor += 1;
        }
        if start != cursor {
            let raw = rest.get(start..cursor).ok_or_else(|| {
                CircuitError::invalid_detector_error_model("invalid UTF-8 DEM target boundary")
            })?;
            targets.push(parse_dem_target_token(raw)?);
        }
    }
    Ok(targets)
}

fn parse_dem_target_token(raw: &str) -> CircuitResult<DemTarget> {
    let Some((&prefix, _)) = raw.as_bytes().split_first() else {
        return Err(CircuitError::invalid_detector_error_model(
            "invalid DEM target \"\"",
        ));
    };
    match prefix {
        b'D' | b'd' => {
            let value = raw.get(1..).ok_or_else(|| {
                CircuitError::invalid_detector_error_model(format!("invalid DEM target {raw:?}"))
            })?;
            DemTarget::relative_detector(parse_unsigned_dem_text_value(
                value,
                "relative detector target",
            )?)
        }
        b'L' | b'l' => {
            let value = raw.get(1..).ok_or_else(|| {
                CircuitError::invalid_detector_error_model(format!("invalid DEM target {raw:?}"))
            })?;
            DemTarget::logical_observable(parse_unsigned_dem_text_value(
                value,
                "logical observable target",
            )?)
        }
        b'^' if raw.len() == 1 => Ok(DemTarget::separator()),
        b'0'..=b'9' => Ok(DemTarget::numeric(parse_unsigned_dem_text_value(
            raw,
            "numeric DEM target",
        )?)),
        _ => Err(CircuitError::invalid_detector_error_model(format!(
            "invalid DEM target {raw:?}"
        ))),
    }
}

pub(super) fn parse_unsigned_dem_text_value(text: &str, kind: &'static str) -> CircuitResult<u64> {
    let value = parse_unsigned_dem_value(text, kind)?;
    if value > MAX_DEM_TEXT_INTEGER {
        return Err(CircuitError::invalid_detector_error_model(format!(
            "{kind} {value} exceeds {MAX_DEM_TEXT_INTEGER}"
        )));
    }
    Ok(value)
}

fn parse_unsigned_dem_value(text: &str, kind: &'static str) -> CircuitResult<u64> {
    if text.is_empty() {
        return Err(CircuitError::invalid_detector_error_model(format!(
            "invalid {kind} {text:?}"
        )));
    }
    if text.len() <= 18 {
        let mut value = 0_u64;
        for byte in text.bytes() {
            if !byte.is_ascii_digit() {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "invalid {kind} {text:?}"
                )));
            }
            value = value * 10 + u64::from(byte - b'0');
        }
        return Ok(value);
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
    text.trim_ascii_start()
}

fn trim_dem_end(text: &str) -> &str {
    text.trim_ascii_end()
}

fn trim_dem(text: &str) -> &str {
    trim_dem_end(trim_dem_start(text))
}

fn trim_dem_inline(text: &str) -> &str {
    text.trim_matches([' ', '\t'])
}

fn wrap_line(line: usize, error: CircuitError) -> CircuitError {
    CircuitError::parse_line(line, error.to_string())
}
