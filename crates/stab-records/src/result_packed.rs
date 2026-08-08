use crate::{ByteSpan, FormatError, FormatErrorCode, FormatErrorContext, RecordResult};

pub(crate) fn b8_record_byte_width(bits_per_record: usize) -> RecordResult<usize> {
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
    Ok(bytes_per_record)
}

pub(crate) fn b8_length_multiple_error(input_len: usize, bytes_per_record: usize) -> FormatError {
    let trailing_bytes = input_len % bytes_per_record;
    format_error(
        FormatErrorCode::InvalidPackedLength,
        format!(
            "b8 input length {input_len} is not a multiple of record byte width {bytes_per_record}"
        ),
        ByteSpan::try_new(input_len - trailing_bytes, trailing_bytes),
        FormatErrorContext::InputLengthMultiple {
            actual_bytes: input_len,
            byte_multiple: bytes_per_record,
        },
    )
}

pub(crate) fn b8_bytes_per_record(input_len: usize, bits_per_record: usize) -> RecordResult<usize> {
    let bytes_per_record = b8_record_byte_width(bits_per_record)?;
    if !input_len.is_multiple_of(bytes_per_record) {
        return Err(b8_length_multiple_error(input_len, bytes_per_record));
    }
    Ok(bytes_per_record)
}

pub(crate) fn ptb64_prefix_layout(
    input_len: usize,
    bits_per_record: usize,
    max_shots: usize,
) -> RecordResult<(usize, usize)> {
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

pub(crate) fn ptb64_zero_width_count_error() -> FormatError {
    format_error(
        FormatErrorCode::InvalidRecordWidth,
        "ptb64 input cannot infer a shot count for zero-width records",
        None,
        FormatErrorContext::MinimumRecordWidth {
            actual_bits: 0,
            minimum_bits: 1,
        },
    )
}

pub(crate) fn ptb64_length_multiple_error(input_len: usize, bytes_per_group: usize) -> FormatError {
    let trailing_bytes = input_len % bytes_per_group;
    format_error(
        FormatErrorCode::InvalidPackedLength,
        format!(
            "ptb64 input length {input_len} is not a multiple of shot-group byte width {bytes_per_group}"
        ),
        ByteSpan::try_new(input_len - trailing_bytes, trailing_bytes),
        FormatErrorContext::InputLengthMultiple {
            actual_bytes: input_len,
            byte_multiple: bytes_per_group,
        },
    )
}

pub(crate) fn ptb64_record_count(input_len: usize, bits_per_record: usize) -> RecordResult<usize> {
    if bits_per_record == 0 {
        return Err(ptb64_zero_width_count_error());
    }
    let bytes_per_group = ptb64_bytes_per_group(bits_per_record)?;
    if !input_len.is_multiple_of(bytes_per_group) {
        return Err(ptb64_length_multiple_error(input_len, bytes_per_group));
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

pub(crate) fn ptb64_bytes_per_group(bits_per_record: usize) -> RecordResult<usize> {
    bits_per_record.checked_mul(8).ok_or_else(|| {
        format_error(
            FormatErrorCode::ArithmeticOverflow,
            "ptb64 record byte width overflowed",
            None,
            FormatErrorContext::None,
        )
    })
}

/// Unpacks one b8-packed record chunk into a caller-owned dense record buffer.
pub(crate) fn unpack_b8_chunk_into(chunk: &[u8], record: &mut [bool]) {
    for (bit_index, bit) in record.iter_mut().enumerate() {
        *bit = chunk.get(bit_index / 8).copied().unwrap_or(0) & (1u8 << (bit_index % 8)) != 0;
    }
}

/// Reads one ptb64 group's measurement-major words out of its raw group bytes.
pub(crate) fn extend_ptb64_group_words(group_bytes: &[u8], words: &mut Vec<u64>) {
    words.clear();
    words.extend(group_bytes.chunks_exact(8).map(|chunk| {
        let mut word_bytes = [0u8; 8];
        word_bytes.copy_from_slice(chunk);
        u64::from_le_bytes(word_bytes)
    }));
}

/// Fills a dense record with one shot's bits from a ptb64 group's measurement-major words.
pub(crate) fn fill_record_from_ptb64_words(words: &[u64], shot_offset: usize, record: &mut [bool]) {
    for (bit, word) in record.iter_mut().zip(words) {
        *bit = word & (1u64 << shot_offset) != 0;
    }
}

#[inline]
pub(crate) fn decode_next_r8_record<F>(
    input: &[u8],
    bits_per_record: usize,
    offset: &mut usize,
    mut record_hit: F,
) -> RecordResult<bool>
where
    F: FnMut(usize) -> RecordResult<()>,
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
) -> FormatError {
    FormatError::invalid_result_format_diagnostic_with_context(code, message, span, context)
}
