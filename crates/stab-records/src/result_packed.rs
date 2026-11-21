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

/// Decodes run bytes of one `r8` record from `input[*offset..]`, continuing a record whose
/// decoded prefix ends at `*bit_index` (the bit after the last run, or zero at the record start).
///
/// Returns `true` once the record's terminating run is consumed and `false` when `input` runs out
/// first, with `*offset` and `*bit_index` advanced past every consumed byte, so decoding can
/// resume in later input. A run of 255 extends the pending run without a hit, so a record ending
/// in such runs needs one more byte, exactly as when decoding it whole. Spans of returned errors
/// are offsets in `input`.
#[inline]
pub(crate) fn resume_r8_record<F>(
    input: &[u8],
    bits_per_record: usize,
    bit_index: &mut usize,
    offset: &mut usize,
    mut record_hit: F,
) -> RecordResult<bool>
where
    F: FnMut(usize) -> RecordResult<()>,
{
    // Work on locals so the loop keeps its cursor in registers across the hit callback.
    let mut bits = *bit_index;
    let mut position = *offset;
    let result = loop {
        let Some(byte) = input.get(position).copied() else {
            break Ok(false);
        };
        let byte_offset = position;
        position += 1;
        let Some(decoded_bits) = bits.checked_add(usize::from(byte)) else {
            break Err(format_error(
                FormatErrorCode::ArithmeticOverflow,
                "r8 decoded bit index overflowed",
                ByteSpan::try_new(byte_offset, 1),
                FormatErrorContext::None,
            ));
        };
        if decoded_bits > bits_per_record {
            break Err(format_error(
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
            bits = decoded_bits;
            continue;
        }
        if decoded_bits == bits_per_record {
            bits = decoded_bits;
            break Ok(true);
        }
        if let Err(error) = record_hit(decoded_bits) {
            break Err(error);
        }
        bits = decoded_bits + 1;
    };
    *bit_index = bits;
    *offset = position;
    result
}

/// The diagnostic for an `r8` record that input ended inside, at input offset `offset` after
/// `bit_index` decoded bits.
pub(crate) fn r8_truncated_error(
    offset: usize,
    bit_index: usize,
    bits_per_record: usize,
) -> FormatError {
    format_error(
        FormatErrorCode::UnexpectedEndOfInput,
        "r8 input ended before record completed",
        ByteSpan::try_new(offset, 0),
        FormatErrorContext::RecordWidth {
            actual_bits: bit_index,
            expected_bits: bits_per_record,
        },
    )
}

fn format_error(
    code: FormatErrorCode,
    message: impl Into<String>,
    span: Option<ByteSpan>,
    context: FormatErrorContext,
) -> FormatError {
    FormatError::invalid_result_format_diagnostic_with_context(code, message, span, context)
}
