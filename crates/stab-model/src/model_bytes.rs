use std::borrow::Cow;
use std::ops::Range;

use crate::{
    ModelDialect, ModelError, ModelResult, ParseError, ParseLimits,
    advanced::invalid_utf8_parse_error, parse_limits::ParseAdmission,
};

pub(crate) struct PreparedModelText<'a> {
    text: Cow<'a, str>,
    tags: Option<Vec<Vec<u8>>>,
    invalid_utf8: Option<ParseError>,
}

impl<'a> PreparedModelText<'a> {
    pub(crate) fn new(
        input: &'a [u8],
        dialect: ModelDialect,
        limits: ParseLimits,
    ) -> ModelResult<Self> {
        ParseAdmission::admit_source_bytes(dialect, input.len(), limits)?;
        let input = admitted_source_prefix(input, limits.source_line_limit().get());
        if let Ok(text) = std::str::from_utf8(input) {
            return Ok(Self {
                text: Cow::Borrowed(text),
                tags: None,
                invalid_utf8: None,
            });
        }

        let (metadata_ranges, tags) = scan_metadata(input);
        let mut sanitized = input.to_vec();
        let mut invalid_utf8 = None;
        let mut cursor = 0usize;
        let mut metadata_range_index = 0usize;

        while cursor < sanitized.len() {
            let remaining = sanitized.get(cursor..).ok_or_else(|| {
                ModelError::invalid_domain_value(
                    "model byte input",
                    "UTF-8 scan cursor escaped input",
                )
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
                .unwrap_or_else(|| sanitized.len().saturating_sub(byte_start))
                .max(1);
            let byte_end = byte_start.checked_add(byte_length).ok_or_else(|| {
                ModelError::invalid_domain_value("model byte input", "UTF-8 error span overflow")
            })?;
            let is_opaque_metadata = byte_range_is_opaque_metadata(
                &metadata_ranges,
                &mut metadata_range_index,
                byte_start,
                byte_end,
            );
            if !is_opaque_metadata && invalid_utf8.is_none() {
                invalid_utf8 = Some(invalid_utf8_parse_error(
                    dialect,
                    byte_start,
                    byte_length,
                    error.error_len(),
                ));
            }
            let Some(region) = sanitized.get_mut(byte_start..byte_end) else {
                return Err(ModelError::invalid_domain_value(
                    "model byte input",
                    "UTF-8 error span escaped input",
                ));
            };
            region.fill(b'?');
            cursor = byte_end;
        }

        let text = ParseError::decode_utf8(dialect, &sanitized)?.to_owned();
        Ok(Self {
            text: Cow::Owned(text),
            tags: Some(tags),
            invalid_utf8,
        })
    }

    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    pub(crate) fn requires_tag_restore(&self) -> bool {
        self.tags.is_some()
    }

    pub(crate) fn into_tags(self) -> Option<Vec<Vec<u8>>> {
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

fn byte_range_is_opaque_metadata(
    metadata_ranges: &[Range<usize>],
    range_index: &mut usize,
    byte_start: usize,
    byte_end: usize,
) -> bool {
    while let Some(range) = metadata_ranges.get(*range_index) {
        if range.end <= byte_start {
            *range_index = range_index.saturating_add(1);
            continue;
        }
        return range.start <= byte_start && byte_end <= range.end;
    }
    false
}

fn admitted_source_prefix(input: &[u8], admitted_lines: usize) -> &[u8] {
    let mut line_start = 0usize;
    for _ in 0..admitted_lines {
        let Some(remaining) = input.get(line_start..) else {
            return input;
        };
        let Some(relative_newline) = remaining.iter().position(|byte| *byte == b'\n') else {
            return input;
        };
        line_start = line_start
            .saturating_add(relative_newline)
            .saturating_add(1);
    }
    if line_start >= input.len() {
        return input;
    }
    input.get(..line_start.saturating_add(1)).unwrap_or(input)
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

fn scan_metadata(input: &[u8]) -> (Vec<Range<usize>>, Vec<Vec<u8>>) {
    let mut ranges = Vec::new();
    let mut tags = Vec::new();
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
        scan_line(input, line_start, line_end, &mut ranges, &mut tags);
        line_start = line_end.saturating_add(usize::from(line_end < input.len()));
    }
    (ranges, tags)
}

fn scan_line(
    input: &[u8],
    line_start: usize,
    line_end: usize,
    ranges: &mut Vec<Range<usize>>,
    tags: &mut Vec<Vec<u8>>,
) {
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
                ranges.push(cursor..line_end);
                break;
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
                    ranges.push(content_start..cursor);
                    let Some(raw_tag) = input.get(content_start..cursor) else {
                        break;
                    };
                    let tag = unescape_tag(raw_tag);
                    if !tag.is_empty() {
                        tags.push(tag);
                    }
                    cursor += 1;
                    terminated = true;
                    break;
                }
                cursor += 1;
            }
            if !terminated {
                ranges.push(content_start..line_end);
            }
        }

        let mut in_arguments = false;
        while cursor < line_end {
            match input.get(cursor) {
                Some(b'#') => {
                    ranges.push(cursor..line_end);
                    return;
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
}

fn unescape_tag(raw: &[u8]) -> Vec<u8> {
    let mut tag = Vec::with_capacity(raw.len());
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
    tag
}

#[cfg(test)]
mod tests {
    use std::ops::Range;

    use crate::{ParseLimits, SourceLineLimit};

    use super::{PreparedModelText, byte_range_is_opaque_metadata, scan_metadata};

    #[test]
    fn metadata_scanner_distinguishes_tags_comments_and_target_brackets() {
        let input = b"REPEAT[x] 2 { H[y] 0 # z\n} M rec[-1]\n";
        let (ranges, tags) = scan_metadata(input);
        assert_eq!(tags, [b"x".to_vec(), b"y".to_vec()]);
        assert!(
            ranges
                .iter()
                .any(|range| input.get(range.clone()) == Some(b"x".as_slice()))
        );
        assert!(
            ranges
                .iter()
                .any(|range| input.get(range.clone()) == Some(b"y".as_slice()))
        );
        assert!(ranges.iter().any(|range| {
            input
                .get(range.clone())
                .is_some_and(|bytes| bytes.starts_with(b"#"))
        }));
        assert!(
            !ranges
                .iter()
                .any(|range| input.get(range.clone()) == Some(b"-1".as_slice()))
        );
    }

    #[test]
    fn byte_preparation_stops_after_the_first_rejected_physical_line() {
        let limits = ParseLimits::default().with_source_line_limit(SourceLineLimit::new(1));
        let prepared = PreparedModelText::new(
            b"H 0\nM 1\nM 2\nM 3\n",
            crate::ModelDialect::StimCircuit,
            limits,
        );
        assert!(prepared.is_ok());
        if let Ok(prepared) = prepared {
            assert_eq!(prepared.text(), "H 0\nM");
        }
    }

    #[test]
    fn opaque_metadata_classification_advances_monotonically() {
        let ranges = (0..100_000usize)
            .map(|index| {
                let start = index.saturating_mul(2);
                Range {
                    start,
                    end: start.saturating_add(1),
                }
            })
            .collect::<Vec<_>>();
        let mut range_index = 0usize;
        for (expected_index, range) in ranges.iter().enumerate() {
            assert!(byte_range_is_opaque_metadata(
                &ranges,
                &mut range_index,
                range.start,
                range.end,
            ));
            assert_eq!(range_index, expected_index);
        }
        assert!(!byte_range_is_opaque_metadata(
            &ranges,
            &mut range_index,
            ranges.len().saturating_mul(2),
            ranges.len().saturating_mul(2).saturating_add(1),
        ));
        assert_eq!(range_index, ranges.len());
    }
}
