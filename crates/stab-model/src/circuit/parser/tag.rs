//! Tag syntax of circuit declarations, validated before its text is allocated.
//!
//! A tag is scanned once to validate its escapes and measure its unescaped length, so the
//! parser can charge the tag against the parse budget and then build its text in one exact-size
//! allocation.

use crate::model_parse::line_error;
use crate::source_text::SourceSlice;
use crate::{ByteSpan, ModelDialect, ModelResult, ParseErrorCode};

use super::{instruction_context, internal_parser_error};

/// A validated tag whose text has not been allocated yet.
pub(super) struct DeferredTag<'a> {
    line_number: usize,
    instruction: &'a str,
    /// The source after the opening bracket.
    body: SourceSlice<'a>,
    end_error_span: ByteSpan,
    /// Unescaped UTF-8 length of the tag text.
    len: usize,
}

impl DeferredTag<'_> {
    /// Unescaped bytes the tag text will hold.
    pub(super) fn len(&self) -> usize {
        self.len
    }

    /// Builds the tag text in an allocation of exactly [`Self::len`] bytes; charge it first.
    pub(super) fn into_string(self) -> ModelResult<String> {
        let mut text = String::with_capacity(self.len);
        scan_tag(
            self.line_number,
            self.instruction,
            self.body,
            self.end_error_span,
            Some(&mut text),
        )?;
        Ok(text)
    }
}

/// Parses an optional `[tag]` at the start of `rest` without allocating its text.
#[inline]
pub(super) fn parse_optional_tag<'a>(
    line_number: usize,
    instruction: &'a str,
    rest: SourceSlice<'a>,
    end_error_span: ByteSpan,
) -> ModelResult<(Option<DeferredTag<'a>>, SourceSlice<'a>)> {
    let Some(body) = rest.strip_prefix("[") else {
        return Ok((None, rest));
    };
    let (len, rest) = scan_tag(line_number, instruction, body, end_error_span, None)?;
    Ok((
        Some(DeferredTag {
            line_number,
            instruction,
            body,
            end_error_span,
            len,
        }),
        rest,
    ))
}

/// Scans tag text from `body`, just after its opening bracket, up to its closing bracket,
/// returning the unescaped length and the source after the bracket and appending the unescaped
/// text to `out` when given.
#[inline(never)]
fn scan_tag<'a>(
    line_number: usize,
    instruction: &str,
    mut body: SourceSlice<'a>,
    end_error_span: ByteSpan,
    mut out: Option<&mut String>,
) -> ModelResult<(usize, SourceSlice<'a>)> {
    let mut len = 0_usize;
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
        let unescaped = match character {
            ']' => return Ok((len, body)),
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
                match escaped {
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
                }
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
            _ => character,
        };
        len = len.saturating_add(unescaped.len_utf8());
        if let Some(out) = out.as_deref_mut() {
            out.push(unescaped);
        }
    }
}
