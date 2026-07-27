use crate::{ByteSpan, CircuitError, CircuitResult, FormatErrorCode, FormatErrorContext};

pub(crate) fn b8_bytes_per_record(
    input_len: usize,
    bits_per_record: usize,
) -> CircuitResult<usize> {
    let bytes_per_record = bits_per_record.div_ceil(8);
    if bytes_per_record == 0 {
        return Err(format_error(
            FormatErrorCode::InvalidRecordWidth,
            "b8 input cannot represent zero-width records",
            None,
            FormatErrorContext::MinimumRecordWidth {
                actual_bits: 0,
                minimum_bits: 1,
            },
        ));
    }
    let trailing_bytes = input_len % bytes_per_record;
    if trailing_bytes != 0 {
        return Err(format_error(
            FormatErrorCode::InvalidPackedLength,
            format!(
                "b8 input length {input_len} is not a multiple of record byte width {bytes_per_record}"
            ),
            ByteSpan::try_new(input_len - trailing_bytes, trailing_bytes),
            FormatErrorContext::InputLengthMultiple {
                actual_bytes: input_len,
                byte_multiple: bytes_per_record,
            },
        ));
    }
    Ok(bytes_per_record)
}

pub(crate) fn ptb64_prefix_layout(
    input_len: usize,
    bits_per_record: usize,
    max_shots: usize,
) -> CircuitResult<(usize, usize)> {
    if bits_per_record == 0 {
        return Err(format_error(
            FormatErrorCode::InvalidRecordWidth,
            "ptb64 input cannot represent a nonzero number of zero-width records",
            None,
            FormatErrorContext::MinimumRecordWidth {
                actual_bits: 0,
                minimum_bits: 1,
            },
        ));
    }
    let bytes_per_group = ptb64_bytes_per_group(bits_per_record)?;
    let shot_groups = max_shots / 64;
    let expected_bytes = shot_groups.checked_mul(bytes_per_group).ok_or_else(|| {
        format_error(
            FormatErrorCode::ArithmeticOverflow,
            "ptb64 expected byte count overflowed",
            None,
            FormatErrorContext::None,
        )
    })?;
    if input_len < expected_bytes {
        return Err(format_error(
            FormatErrorCode::UnexpectedEndOfInput,
            format!(
                "ptb64 input expected at least {expected_bytes} bytes for {max_shots} records with {bits_per_record} bits each, got {input_len}"
            ),
            ByteSpan::try_new(input_len, 0),
            FormatErrorContext::MinimumInputLength {
                actual_bytes: input_len,
                minimum_bytes: expected_bytes,
            },
        ));
    }
    Ok((bytes_per_group, expected_bytes))
}

pub(crate) fn ptb64_record_count(input_len: usize, bits_per_record: usize) -> CircuitResult<usize> {
    if bits_per_record == 0 {
        return Err(format_error(
            FormatErrorCode::InvalidRecordWidth,
            "ptb64 input cannot infer a shot count for zero-width records",
            None,
            FormatErrorContext::MinimumRecordWidth {
                actual_bits: 0,
                minimum_bits: 1,
            },
        ));
    }
    let bytes_per_group = ptb64_bytes_per_group(bits_per_record)?;
    let trailing_bytes = input_len % bytes_per_group;
    if trailing_bytes != 0 {
        return Err(format_error(
            FormatErrorCode::InvalidPackedLength,
            format!(
                "ptb64 input length {input_len} is not a multiple of shot-group byte width {bytes_per_group}"
            ),
            ByteSpan::try_new(input_len - trailing_bytes, trailing_bytes),
            FormatErrorContext::InputLengthMultiple {
                actual_bytes: input_len,
                byte_multiple: bytes_per_group,
            },
        ));
    }
    let shot_groups = input_len / bytes_per_group;
    shot_groups.checked_mul(64).ok_or_else(|| {
        format_error(
            FormatErrorCode::ArithmeticOverflow,
            "ptb64 shot count overflowed",
            None,
            FormatErrorContext::None,
        )
    })
}

fn ptb64_bytes_per_group(bits_per_record: usize) -> CircuitResult<usize> {
    bits_per_record.checked_mul(8).ok_or_else(|| {
        format_error(
            FormatErrorCode::ArithmeticOverflow,
            "ptb64 record byte width overflowed",
            None,
            FormatErrorContext::None,
        )
    })
}

#[inline]
pub(crate) fn decode_next_r8_record<F>(
    input: &[u8],
    bits_per_record: usize,
    offset: &mut usize,
    mut record_hit: F,
) -> CircuitResult<bool>
where
    F: FnMut(usize) -> CircuitResult<()>,
{
    if *offset == input.len() {
        return Ok(false);
    }
    let mut bit_index = 0usize;
    loop {
        let Some(byte) = input.get(*offset).copied() else {
            return Err(format_error(
                FormatErrorCode::UnexpectedEndOfInput,
                "r8 input ended before record completed",
                ByteSpan::try_new(*offset, 0),
                FormatErrorContext::RecordWidth {
                    actual_bits: bit_index,
                    expected_bits: bits_per_record,
                },
            ));
        };
        let byte_offset = *offset;
        *offset += 1;
        let decoded_bits = bit_index.checked_add(usize::from(byte)).ok_or_else(|| {
            format_error(
                FormatErrorCode::ArithmeticOverflow,
                "r8 decoded bit index overflowed",
                ByteSpan::try_new(byte_offset, 1),
                FormatErrorContext::None,
            )
        })?;
        if decoded_bits > bits_per_record {
            return Err(format_error(
                FormatErrorCode::RunLengthOvershoot,
                "r8 run-length overshot record width",
                ByteSpan::try_new(byte_offset, 1),
                FormatErrorContext::RunLength {
                    decoded_bits,
                    expected_bits: bits_per_record,
                },
            ));
        }
        if byte == u8::MAX {
            bit_index = decoded_bits;
            continue;
        }
        if decoded_bits == bits_per_record {
            return Ok(true);
        }
        record_hit(decoded_bits)?;
        bit_index = decoded_bits + 1;
    }
}

fn format_error(
    code: FormatErrorCode,
    message: impl Into<String>,
    span: Option<ByteSpan>,
    context: FormatErrorContext,
) -> CircuitError {
    CircuitError::invalid_result_format_diagnostic_with_context(code, message, span, context)
}
