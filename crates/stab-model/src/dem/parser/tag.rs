//! Tag parsing for DEM declarations.

use super::super::DemTag;
use super::{instruction_context, parse_line_error};
use crate::model_parse::line_error;
use crate::source_text::SourceSlice;
use crate::{ByteSpan, ModelDialect, ModelResult, ParseErrorCode};

pub(super) fn parse_optional_tag<'a>(
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
