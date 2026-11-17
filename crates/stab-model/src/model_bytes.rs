use std::borrow::Cow;

use crate::{
    ByteSpan, ModelDialect, ModelError, ModelResult, ParseError, ParseLimits,
    advanced::invalid_utf8_parse_error, parse_limits::ParseAdmission,
};

pub(crate) struct PreparedModelText<'a> {
    text: Cow<'a, str>,
    admission: ParseAdmission,
    tags: Option<RecoveredTags>,
    invalid_utf8: Option<ParseError>,
}

/// Unescaped tag bytes recovered from non-UTF-8 input, in source order, stored in one arena.
#[derive(Default)]
pub(crate) struct RecoveredTags {
    bytes: Vec<u8>,
    ends: Vec<usize>,
}

impl RecoveredTags {
    pub(crate) fn len(&self) -> usize {
        self.ends.len()
    }

    /// Each recovered tag as an owned byte vector, in source order.
    pub(crate) fn into_owned_tags(self) -> impl Iterator<Item = Vec<u8>> {
        let bytes = self.bytes;
        let mut start = 0usize;
        self.ends.into_iter().map(move |end| {
            let tag = bytes
                .get(start..end)
                .map(<[u8]>::to_vec)
                .unwrap_or_default();
            start = end;
            tag
        })
    }

    /// Appends the unescaped form of non-empty `raw`, which `admission` has already charged.
    fn push_unescaped(&mut self, raw: &[u8]) -> ModelResult<()> {
        let reserve_error = |error: std::collections::TryReserveError| {
            ModelError::invalid_domain_value(
                "model byte input",
                format!("unable to reserve recovered tag storage: {error}"),
            )
        };
        self.bytes.try_reserve(raw.len()).map_err(reserve_error)?;
        self.ends.try_reserve(1).map_err(reserve_error)?;
        unescape_tag_into(raw, &mut self.bytes);
        self.ends.push(self.bytes.len());
        Ok(())
    }
}

impl<'a> PreparedModelText<'a> {
    pub(crate) fn new(
        input: &'a [u8],
        dialect: ModelDialect,
        limits: ParseLimits,
    ) -> ModelResult<Self> {
        let mut admission = ParseAdmission::new(dialect, input.len(), limits)?;
        let first_error = match std::str::from_utf8(input) {
            Ok(text) => {
                return Ok(Self {
                    text: Cow::Borrowed(text),
                    admission,
                    tags: None,
                    invalid_utf8: None,
                });
            }
            Err(error) => error,
        };

        // The sanitized copy and the recovered tags live beside `input` while the model is
        // parsed, so each is admitted before it is allocated.
        let first_invalid = first_error.valid_up_to();
        let first_invalid_line = input
            .get(..first_invalid)
            .map_or(0, |prefix| {
                prefix.iter().filter(|byte| **byte == b'\n').count()
            })
            .saturating_add(1);
        admission.admit_sanitized_copy(
            input.len(),
            first_invalid_line,
            ByteSpan::from_valid_range(first_invalid, first_error.error_len().unwrap_or(1)),
        )?;
        let mut sanitized = Vec::new();
        sanitized.try_reserve_exact(input.len()).map_err(|error| {
            ModelError::invalid_domain_value(
                "model byte input",
                format!("unable to reserve the sanitized text copy: {error}"),
            )
        })?;
        sanitized.extend_from_slice(input);

        // Invalid UTF-8 inside comments and tags is opaque metadata: the scan replaces it in
        // place while recovering the raw tags, so every invalid sequence left afterwards lies
        // outside metadata. Metadata ranges start at or after an ASCII delimiter and end
        // before one, so no invalid sequence straddles a range boundary and replacing a range
        // does not change how the surrounding bytes decode.
        let tags = scan_metadata(input, &mut sanitized, &mut admission)?;
        let mut invalid_utf8 = None;
        replace_invalid_utf8(&mut sanitized, 0, |byte_start, byte_length, error_len| {
            invalid_utf8.get_or_insert_with(|| {
                invalid_utf8_parse_error(dialect, byte_start, byte_length, error_len)
            });
        })?;

        let text = String::from_utf8(sanitized).map_err(|_| {
            ModelError::invalid_domain_value(
                "model byte input",
                "sanitized text is not valid UTF-8",
            )
        })?;
        Ok(Self {
            text: Cow::Owned(text),
            admission,
            tags: Some(tags),
            invalid_utf8,
        })
    }

    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    /// The parse admission, which has charged the source bytes and the preparation storage.
    pub(crate) fn admission(&self) -> ParseAdmission {
        self.admission
    }

    pub(crate) fn requires_tag_restore(&self) -> bool {
        self.tags.is_some()
    }

    pub(crate) fn into_tags(self) -> Option<RecoveredTags> {
        self.tags
    }

    pub(crate) fn resolve<T>(&self, parsed: ModelResult<T>) -> ModelResult<T> {
        let Some(invalid_utf8) = &self.invalid_utf8 else {
            return parsed;
        };
        match parsed {
            Err(error) if error_precedes_invalid_utf8(&error, invalid_utf8) => Err(error),
            _ => Err(invalid_utf8.clone().into()),
        }
    }
}

/// Replaces every invalid UTF-8 sequence of `bytes`, which starts at absolute offset `base`,
/// with `?` bytes, reporting each sequence's absolute start, length, and UTF-8 error length in
/// order.
fn replace_invalid_utf8(
    bytes: &mut [u8],
    base: usize,
    mut on_invalid: impl FnMut(usize, usize, Option<usize>),
) -> ModelResult<()> {
    let mut cursor = 0usize;
    while cursor < bytes.len() {
        let remaining = bytes.get(cursor..).ok_or_else(|| {
            ModelError::invalid_domain_value("model byte input", "UTF-8 scan cursor escaped input")
        })?;
        let error = match std::str::from_utf8(remaining) {
            Ok(_) => break,
            Err(error) => error,
        };
        let byte_start = cursor.checked_add(error.valid_up_to()).ok_or_else(|| {
            ModelError::invalid_domain_value("model byte input", "UTF-8 error offset overflow")
        })?;
        let byte_length = error
            .error_len()
            .unwrap_or_else(|| bytes.len().saturating_sub(byte_start))
            .max(1);
        let byte_end = byte_start.checked_add(byte_length).ok_or_else(|| {
            ModelError::invalid_domain_value("model byte input", "UTF-8 error span overflow")
        })?;
        let absolute_start = base.checked_add(byte_start).ok_or_else(|| {
            ModelError::invalid_domain_value("model byte input", "UTF-8 error offset overflow")
        })?;
        on_invalid(absolute_start, byte_length, error.error_len());
        let Some(region) = bytes.get_mut(byte_start..byte_end) else {
            return Err(ModelError::invalid_domain_value(
                "model byte input",
                "UTF-8 error span escaped input",
            ));
        };
        region.fill(b'?');
        cursor = byte_end;
    }
    Ok(())
}

fn error_precedes_invalid_utf8(error: &ModelError, invalid_utf8: &ParseError) -> bool {
    let invalid_start = invalid_utf8.span().byte_start();
    if let Some(parse_error) = error.parse_error() {
        return parse_error.span().byte_start() < invalid_start;
    }
    error
        .resource_limit_error()
        .is_some_and(|resource_error| resource_error.span().byte_start() <= invalid_start)
}

/// Walks the comments and tags of `input`, replacing invalid UTF-8 inside them in `sanitized`
/// and recovering each non-empty tag, admitted against `admission` before it is stored.
fn scan_metadata(
    input: &[u8],
    sanitized: &mut [u8],
    admission: &mut ParseAdmission,
) -> ModelResult<RecoveredTags> {
    let mut scan = MetadataScan {
        input,
        sanitized,
        admission,
        tags: RecoveredTags::default(),
        line_number: 0,
    };
    let mut line_start = 0usize;
    while line_start < input.len() {
        let Some(remaining) = input.get(line_start..) else {
            break;
        };
        let relative_end = remaining
            .iter()
            .position(|byte| *byte == b'\n')
            .unwrap_or(input.len() - line_start);
        let line_end = line_start + relative_end;
        scan.line_number = scan.line_number.saturating_add(1);
        scan.scan_line(line_start, line_end)?;
        line_start = line_end.saturating_add(usize::from(line_end < input.len()));
    }
    Ok(scan.tags)
}

struct MetadataScan<'a> {
    input: &'a [u8],
    sanitized: &'a mut [u8],
    admission: &'a mut ParseAdmission,
    tags: RecoveredTags,
    line_number: usize,
}

impl MetadataScan<'_> {
    /// Marks `start..end` of the input as opaque metadata.
    fn metadata(&mut self, start: usize, end: usize) -> ModelResult<()> {
        let Some(region) = self.sanitized.get_mut(start..end) else {
            return Err(ModelError::invalid_domain_value(
                "model byte input",
                "metadata range escaped input",
            ));
        };
        replace_invalid_utf8(region, start, |_, _, _| {})
    }

    fn tag(&mut self, start: usize, end: usize) -> ModelResult<()> {
        self.metadata(start, end)?;
        let Some(raw_tag) = self.input.get(start..end) else {
            return Err(ModelError::invalid_domain_value(
                "model byte input",
                "tag range escaped input",
            ));
        };
        if raw_tag.is_empty() {
            return Ok(());
        }
        self.admission.admit_recovered_tag(
            raw_tag.len(),
            self.line_number,
            ByteSpan::from_valid_range(start, raw_tag.len()),
        )?;
        self.tags.push_unescaped(raw_tag)
    }

    fn scan_line(&mut self, line_start: usize, line_end: usize) -> ModelResult<()> {
        let input = self.input;
        let mut cursor = line_start;
        while cursor < line_end {
            while cursor < line_end && matches!(input.get(cursor), Some(b' ' | b'\t' | b'\r')) {
                cursor += 1;
            }
            if cursor >= line_end {
                break;
            }
            match input.get(cursor) {
                Some(b'#') => {
                    return self.metadata(cursor, line_end);
                }
                Some(b'{' | b'}') => {
                    cursor += 1;
                    continue;
                }
                _ => {}
            }

            while cursor < line_end
                && input
                    .get(cursor)
                    .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
            {
                cursor += 1;
            }
            if cursor < line_end && input.get(cursor) == Some(&b'[') {
                let content_start = cursor + 1;
                cursor = content_start;
                let mut escaped = false;
                let mut terminated = false;
                while cursor < line_end {
                    let Some(byte) = input.get(cursor).copied() else {
                        break;
                    };
                    if escaped {
                        escaped = false;
                    } else if byte == b'\\' {
                        escaped = true;
                    } else if byte == b']' {
                        self.tag(content_start, cursor)?;
                        cursor += 1;
                        terminated = true;
                        break;
                    }
                    cursor += 1;
                }
                if !terminated {
                    self.metadata(content_start, line_end)?;
                }
            }

            let mut in_arguments = false;
            while cursor < line_end {
                match input.get(cursor) {
                    Some(b'#') => {
                        return self.metadata(cursor, line_end);
                    }
                    Some(b'(') => in_arguments = true,
                    Some(b')') => in_arguments = false,
                    Some(b'{' | b'}') if !in_arguments => {
                        cursor += 1;
                        break;
                    }
                    _ => {}
                }
                cursor += 1;
            }
        }
        Ok(())
    }
}

fn unescape_tag_into(raw: &[u8], tag: &mut Vec<u8>) {
    let mut cursor = 0usize;
    while cursor < raw.len() {
        let Some(byte) = raw.get(cursor).copied() else {
            break;
        };
        if byte == b'\\' {
            let Some(escaped) = raw.get(cursor + 1).copied() else {
                tag.push(byte);
                break;
            };
            tag.push(match escaped {
                b'C' => b']',
                b'r' => b'\r',
                b'n' => b'\n',
                b'B' => b'\\',
                byte => byte,
            });
            cursor += 2;
        } else {
            tag.push(byte);
            cursor += 1;
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    reason = "the scanner fixture is well-formed and fits the default budget"
)]
mod tests {
    use super::scan_metadata;
    use crate::parse_limits::ParseAdmission;
    use crate::{ModelDialect, ParseLimits};

    #[test]
    fn metadata_scanner_distinguishes_tags_comments_and_target_brackets() {
        let input = b"REPEAT[x\xff] 2 { H[y\\B\xfe] 0 # z\xfd\n} M rec[-1\xfc]\n";
        let mut sanitized = input.to_vec();
        let mut admission = ParseAdmission::new(
            ModelDialect::StimCircuit,
            input.len(),
            ParseLimits::default(),
        )
        .expect("default admission");
        let tags = scan_metadata(input, &mut sanitized, &mut admission).expect("scan");
        assert_eq!(
            tags.into_owned_tags().collect::<Vec<_>>(),
            [b"x\xff".to_vec(), b"y\\\xfe".to_vec()]
        );
        // Invalid bytes inside tags and comments are opaque metadata, replaced in place, while
        // the measurement-record brackets are code and keep their invalid byte for diagnosis.
        assert_eq!(
            sanitized,
            b"REPEAT[x?] 2 { H[y\\B?] 0 # z?\n} M rec[-1\xfc]\n".to_vec()
        );
    }
}
