use crate::{
    ByteSpan, CircuitError, CircuitResult, FormatErrorCode, FormatErrorContext,
    result_formats::{DetsLayout, DetsResultType, DetsToken},
};

pub(crate) fn for_each_zero_one_line<F>(
    input: &[u8],
    bits_per_record: usize,
    mut visit: F,
) -> CircuitResult<()>
where
    F: FnMut(&[u8]) -> CircuitResult<()>,
{
    let mut offset = 0usize;
    while offset < input.len() {
        let start = offset;
        for bit_index in 0..bits_per_record {
            let byte = input.get(offset).copied().ok_or_else(|| {
                invalid_at_with_context(
                    input,
                    offset,
                    FormatErrorCode::UnexpectedEndOfInput,
                    format!(
                        "01 data ended in the middle of a record at bit {bit_index}; expected {bits_per_record} bits"
                    ),
                    FormatErrorContext::RecordWidth {
                        actual_bits: bit_index,
                        expected_bits: bits_per_record,
                    },
                )
            })?;
            if matches!(byte, b'\n' | b'\r') {
                return Err(invalid_at_with_context(
                    input,
                    offset,
                    FormatErrorCode::InvalidRecordWidth,
                    format!(
                        "01 record ended after {bit_index} bits; expected {bits_per_record} bits"
                    ),
                    FormatErrorContext::RecordWidth {
                        actual_bits: bit_index,
                        expected_bits: bits_per_record,
                    },
                ));
            }
            if !matches!(byte, b'0' | b'1') {
                return Err(invalid_at_with_context(
                    input,
                    offset,
                    FormatErrorCode::InvalidByte,
                    format!("01 record contains non-bit byte {byte}"),
                    FormatErrorContext::InvalidByte { byte },
                ));
            }
            offset += 1;
        }

        if bits_per_record == 0 && offset == input.len() {
            return Ok(());
        }
        let end = offset;
        consume_required_newline(input, &mut offset, "01")?;
        let line = input.get(start..end).ok_or_else(|| {
            CircuitError::invalid_result_format("01 record byte range was out of bounds")
        })?;
        visit(line)?;
    }
    Ok(())
}

pub(crate) fn for_each_hits<F>(
    input: &[u8],
    bits_per_record: usize,
    mut visit: F,
) -> CircuitResult<()>
where
    F: FnMut(&[u64]) -> CircuitResult<()>,
{
    let mut offset = 0usize;
    let mut hits = Vec::new();
    while offset < input.len() {
        hits.clear();
        if consume_empty_record_newline(input, &mut offset, "HITS")? {
            visit(&hits)?;
            continue;
        }

        loop {
            let parsed = parse_u64(input, &mut offset, "HITS index")?;
            let value = parsed.value;
            let index = usize::try_from(value).map_err(|_| {
                CircuitError::invalid_result_format_diagnostic(
                    FormatErrorCode::IntegerOverflow,
                    format!("HITS index {value} does not fit usize"),
                    Some(parsed.span),
                )
            })?;
            if index >= bits_per_record {
                return Err(CircuitError::invalid_result_format_diagnostic_with_context(
                    FormatErrorCode::IndexOutOfRange,
                    format!("HITS index {value} exceeds record width {bits_per_record}"),
                    Some(parsed.span),
                    FormatErrorContext::Index {
                        result_type: None,
                        index: value,
                        exclusive_bound: bits_per_record,
                    },
                ));
            }
            hits.push(value);

            let delimiter = input.get(offset).copied().ok_or_else(|| {
                invalid_at(
                    input,
                    offset,
                    FormatErrorCode::InvalidRecordSeparator,
                    "HITS data was not comma-separated integers terminated by a newline",
                )
            })?;
            match delimiter {
                b',' => offset += 1,
                b'\n' => {
                    offset += 1;
                    break;
                }
                b'\r' => {
                    offset += 1;
                    match input.get(offset).copied() {
                        Some(b'\n') => {
                            offset += 1;
                            break;
                        }
                        Some(b',') => offset += 1,
                        _ => {
                            return Err(invalid_at(
                                input,
                                offset,
                                FormatErrorCode::InvalidRecordSeparator,
                                "HITS data was not comma-separated integers terminated by a newline",
                            ));
                        }
                    }
                }
                _ => {
                    return Err(invalid_at(
                        input,
                        offset,
                        FormatErrorCode::InvalidRecordSeparator,
                        "HITS data was not comma-separated integers terminated by a newline",
                    ));
                }
            }
        }
        visit(&hits)?;
    }
    Ok(())
}

pub(crate) fn for_each_dets_tokens<F>(
    input: &[u8],
    layout: DetsLayout,
    mut visit: F,
) -> CircuitResult<()>
where
    F: FnMut(&[DetsToken]) -> CircuitResult<()>,
{
    let mut offset = 0usize;
    let mut tokens = Vec::new();
    loop {
        while input
            .get(offset)
            .is_some_and(|byte| matches!(byte, b' ' | b'\n' | b'\r' | b'\t'))
        {
            offset += 1;
        }
        if offset == input.len() {
            return Ok(());
        }
        let prefix_end = offset.checked_add(4).ok_or_else(|| {
            CircuitError::invalid_result_format("DETS prefix byte offset overflowed")
        })?;
        if input.get(offset..prefix_end) != Some(b"shot".as_slice()) {
            let available = input.len().saturating_sub(offset).min(4);
            return Err(CircuitError::invalid_result_format_diagnostic(
                FormatErrorCode::InvalidPrefix,
                "DETS data did not start with 'shot'",
                ByteSpan::try_new(offset, available),
            ));
        }
        offset = prefix_end;
        tokens.clear();

        loop {
            let Some(mut next) = input.get(offset).copied() else {
                visit(&tokens)?;
                return Ok(());
            };
            if next == b'\r' {
                offset += 1;
                let Some(after_carriage_return) = input.get(offset).copied() else {
                    visit(&tokens)?;
                    return Ok(());
                };
                next = after_carriage_return;
            }
            match next {
                b'\n' => {
                    offset += 1;
                    break;
                }
                b' ' => {
                    offset += 1;
                    let result_type = match input.get(offset).copied() {
                        Some(b'M') => DetsResultType::Measurement,
                        Some(b'D') => DetsResultType::Detector,
                        Some(b'L') => DetsResultType::Observable,
                        _ => {
                            return Err(invalid_at(
                                input,
                                offset,
                                FormatErrorCode::InvalidPrefix,
                                "unrecognized DETS prefix; expected M, D, or L",
                            ));
                        }
                    };
                    offset += 1;
                    let parsed = parse_u64(input, &mut offset, "DETS token index")?;
                    let value = parsed.value;
                    let index = usize::try_from(value).map_err(|_| {
                        CircuitError::invalid_result_format_diagnostic(
                            FormatErrorCode::IntegerOverflow,
                            format!("DETS index {value} does not fit usize"),
                            Some(parsed.span),
                        )
                    })?;
                    let namespace_width = match result_type {
                        DetsResultType::Measurement => layout.measurements(),
                        DetsResultType::Detector => layout.detectors(),
                        DetsResultType::Observable => layout.observables(),
                    };
                    if index >= namespace_width {
                        return Err(CircuitError::invalid_result_format_diagnostic_with_context(
                            FormatErrorCode::IndexOutOfRange,
                            format!(
                                "DETS token {}{index} exceeds namespace width {namespace_width}",
                                char::from(result_type.prefix())
                            ),
                            Some(parsed.span),
                            FormatErrorContext::Index {
                                result_type: Some(result_type),
                                index: value,
                                exclusive_bound: namespace_width,
                            },
                        ));
                    }
                    layout.resolve(result_type, index)?;
                    tokens.push(DetsToken::new(result_type, index));
                }
                _ => {
                    return Err(invalid_at(
                        input,
                        offset,
                        FormatErrorCode::InvalidRecordSeparator,
                        "DETS data was not single-space-separated with no trailing spaces",
                    ));
                }
            }
        }
        visit(&tokens)?;
    }
}

fn consume_required_newline(
    input: &[u8],
    offset: &mut usize,
    format: &'static str,
) -> CircuitResult<()> {
    match input.get(*offset).copied() {
        Some(b'\n') => {
            *offset += 1;
            Ok(())
        }
        Some(b'\r') => {
            *offset += 1;
            require_lf(input, offset, format)
        }
        _ => Err(invalid_at(
            input,
            *offset,
            FormatErrorCode::MissingRecordTerminator,
            format!("{format} data did not end with a newline after the expected record width"),
        )),
    }
}

fn consume_empty_record_newline(
    input: &[u8],
    offset: &mut usize,
    format: &'static str,
) -> CircuitResult<bool> {
    match input.get(*offset).copied() {
        Some(b'\n') => {
            *offset += 1;
            Ok(true)
        }
        Some(b'\r') => {
            *offset += 1;
            require_lf(input, offset, format)?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn require_lf(input: &[u8], offset: &mut usize, format: &'static str) -> CircuitResult<()> {
    if input.get(*offset).copied() != Some(b'\n') {
        return Err(invalid_at(
            input,
            *offset,
            FormatErrorCode::MissingRecordTerminator,
            format!("{format} carriage return was not followed by a line feed"),
        ));
    }
    *offset += 1;
    Ok(())
}

struct ParsedU64 {
    value: u64,
    span: ByteSpan,
}

fn parse_u64(input: &[u8], offset: &mut usize, kind: &'static str) -> CircuitResult<ParsedU64> {
    let start = *offset;
    let mut value = 0u64;
    while let Some(byte @ b'0'..=b'9') = input.get(*offset).copied() {
        let Some(next) = value
            .checked_mul(10)
            .and_then(|value| value.checked_add(u64::from(byte - b'0')))
        else {
            return Err(CircuitError::invalid_result_format_diagnostic(
                FormatErrorCode::IntegerOverflow,
                format!("{kind} overflowed u64"),
                ByteSpan::try_new(start, (*offset).saturating_sub(start).saturating_add(1)),
            ));
        };
        value = next;
        *offset += 1;
    }
    if *offset == start {
        return Err(invalid_at(
            input,
            start,
            FormatErrorCode::MissingIndex,
            format!("{kind} was not followed by an unsigned integer"),
        ));
    }
    Ok(ParsedU64 {
        value,
        span: ByteSpan::try_new(start, *offset - start).ok_or_else(|| {
            CircuitError::invalid_result_format("parsed integer byte span overflowed")
        })?,
    })
}

fn invalid_at(
    input: &[u8],
    offset: usize,
    code: FormatErrorCode,
    message: impl Into<String>,
) -> CircuitError {
    CircuitError::invalid_result_format_diagnostic(
        code,
        message,
        ByteSpan::try_new(offset, usize::from(offset < input.len())),
    )
}

fn invalid_at_with_context(
    input: &[u8],
    offset: usize,
    code: FormatErrorCode,
    message: impl Into<String>,
    context: FormatErrorContext,
) -> CircuitError {
    CircuitError::invalid_result_format_diagnostic_with_context(
        code,
        message,
        ByteSpan::try_new(offset, usize::from(offset < input.len())),
        context,
    )
}
