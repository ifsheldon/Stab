use crate::{
    CircuitError, CircuitResult, SampleFormat,
    bits::{BitSlice, BitVec},
    result_formats::{
        ptb64_record_count as materialized_ptb64_record_count, validate_ptb64_shot_count,
    },
};

pub fn for_each_record<F>(
    input: &[u8],
    format: SampleFormat,
    bits_per_record: usize,
    visit: F,
) -> CircuitResult<()>
where
    F: FnMut(&[bool]) -> CircuitResult<()>,
{
    match format {
        SampleFormat::ZeroOne => for_each_zero_one_record(input, bits_per_record, visit),
        SampleFormat::B8 => for_each_b8_record(input, bits_per_record, visit),
        SampleFormat::R8 => for_each_r8_record(input, bits_per_record, visit),
        SampleFormat::Hits => for_each_hits_record(input, bits_per_record, visit),
        SampleFormat::Dets => for_each_dets_record(input, bits_per_record, visit),
    }
}

pub fn for_each_packed_record<F>(
    input: &[u8],
    format: SampleFormat,
    bits_per_record: usize,
    visit: F,
) -> CircuitResult<()>
where
    F: FnMut(BitSlice<'_>) -> CircuitResult<()>,
{
    match format {
        SampleFormat::ZeroOne => for_each_zero_one_packed_record(input, bits_per_record, visit),
        SampleFormat::B8 => for_each_b8_packed_record(input, bits_per_record, visit),
        SampleFormat::R8 => for_each_r8_packed_record(input, bits_per_record, visit),
        SampleFormat::Hits => for_each_hits_packed_record(input, bits_per_record, visit),
        SampleFormat::Dets => for_each_dets_packed_record(input, bits_per_record, visit),
    }
}

pub fn for_each_sparse_record<F>(
    input: &[u8],
    format: SampleFormat,
    bits_per_record: usize,
    visit: F,
) -> CircuitResult<()>
where
    F: FnMut(&[u64]) -> CircuitResult<()>,
{
    match format {
        SampleFormat::ZeroOne => for_each_zero_one_sparse_record(input, bits_per_record, visit),
        SampleFormat::B8 => for_each_b8_sparse_record(input, bits_per_record, visit),
        SampleFormat::R8 => for_each_r8_sparse_record(input, bits_per_record, visit),
        SampleFormat::Hits => for_each_hits_sparse_record(input, bits_per_record, visit),
        SampleFormat::Dets => for_each_dets_sparse_record(input, bits_per_record, visit),
    }
}

pub fn for_each_ptb64_record_all<F>(
    input: &[u8],
    bits_per_record: usize,
    visit: F,
) -> CircuitResult<()>
where
    F: FnMut(&[bool]) -> CircuitResult<()>,
{
    let shots = ptb64_record_count(input, bits_per_record)?;
    for_each_ptb64_record(input, bits_per_record, shots, visit)
}

pub fn for_each_ptb64_record<F>(
    input: &[u8],
    bits_per_record: usize,
    max_shots: usize,
    mut visit: F,
) -> CircuitResult<()>
where
    F: FnMut(&[bool]) -> CircuitResult<()>,
{
    validate_ptb64_shot_count(max_shots)?;
    if max_shots == 0 {
        return Ok(());
    }
    if bits_per_record == 0 {
        return Err(CircuitError::invalid_result_format(
            "ptb64 input cannot represent a nonzero number of zero-width records",
        ));
    }
    let shot_groups = max_shots / 64;
    let bytes_per_group = bits_per_record
        .checked_mul(8)
        .ok_or_else(|| CircuitError::invalid_result_format("ptb64 record byte width overflowed"))?;
    let expected_bytes = shot_groups.checked_mul(bytes_per_group).ok_or_else(|| {
        CircuitError::invalid_result_format("ptb64 expected byte count overflowed")
    })?;
    if input.len() < expected_bytes {
        return Err(CircuitError::invalid_result_format(format!(
            "ptb64 input expected at least {expected_bytes} bytes for {max_shots} records with {bits_per_record} bits each, got {}",
            input.len()
        )));
    }

    let input = input.get(..expected_bytes).ok_or_else(|| {
        CircuitError::invalid_result_format("ptb64 expected byte range was out of bounds")
    })?;
    let mut record = vec![false; bits_per_record];
    for group_bytes in input.chunks_exact(bytes_per_group) {
        let words = group_bytes
            .chunks_exact(8)
            .map(|chunk| {
                let mut word_bytes = [0u8; 8];
                word_bytes.copy_from_slice(chunk);
                u64::from_le_bytes(word_bytes)
            })
            .collect::<Vec<_>>();
        for shot_offset in 0..64 {
            for (bit_index, word) in words.iter().enumerate() {
                let Some(bit) = record.get_mut(bit_index) else {
                    return Err(CircuitError::invalid_result_format(
                        "ptb64 bit index was out of decoded record bounds",
                    ));
                };
                *bit = word & (1u64 << shot_offset) != 0;
            }
            visit(&record)?;
        }
    }
    Ok(())
}

pub fn ptb64_record_count(input: &[u8], bits_per_record: usize) -> CircuitResult<usize> {
    materialized_ptb64_record_count(input, bits_per_record)
}

fn for_each_zero_one_record<F>(
    input: &[u8],
    bits_per_record: usize,
    mut visit: F,
) -> CircuitResult<()>
where
    F: FnMut(&[bool]) -> CircuitResult<()>,
{
    if input.is_empty() {
        return Ok(());
    }
    let mut record = vec![false; bits_per_record];
    let mut offset = 0usize;
    while offset < input.len() {
        let line_start = offset;
        while input.get(offset).is_some_and(|byte| *byte != b'\n') {
            offset += 1;
        }
        let mut line_end = offset;
        if line_end > line_start && input.get(line_end - 1).is_some_and(|byte| *byte == b'\r') {
            line_end -= 1;
        }
        let line = input.get(line_start..line_end).ok_or_else(|| {
            CircuitError::invalid_result_format("01 record byte range was out of bounds")
        })?;
        if line.len() != bits_per_record {
            return Err(CircuitError::invalid_result_format(format!(
                "01 record expected {bits_per_record} bits, got {}",
                line.len()
            )));
        }
        record.fill(false);
        for (bit, byte) in record.iter_mut().zip(line) {
            match byte {
                b'0' => {}
                b'1' => *bit = true,
                _ => {
                    return Err(CircuitError::invalid_result_format(format!(
                        "01 record contains non-bit byte {byte}"
                    )));
                }
            }
        }
        visit(&record)?;
        if offset < input.len() {
            offset += 1;
        }
    }
    Ok(())
}

fn for_each_b8_record<F>(input: &[u8], bits_per_record: usize, mut visit: F) -> CircuitResult<()>
where
    F: FnMut(&[bool]) -> CircuitResult<()>,
{
    let bytes_per_record = bits_per_record.div_ceil(8);
    if bytes_per_record == 0 {
        return Err(CircuitError::invalid_result_format(
            "b8 input cannot represent zero-width records",
        ));
    }
    if !input.len().is_multiple_of(bytes_per_record) {
        return Err(CircuitError::invalid_result_format(format!(
            "b8 input length {} is not a multiple of record byte width {bytes_per_record}",
            input.len()
        )));
    }
    let mut record = vec![false; bits_per_record];
    for chunk in input.chunks_exact(bytes_per_record) {
        unpack_b8_chunk_into(chunk, &mut record);
        visit(&record)?;
    }
    Ok(())
}

fn for_each_r8_record<F>(input: &[u8], bits_per_record: usize, mut visit: F) -> CircuitResult<()>
where
    F: FnMut(&[bool]) -> CircuitResult<()>,
{
    let mut record = vec![false; bits_per_record];
    let mut offset = 0usize;
    while offset < input.len() {
        record.fill(false);
        let mut bit_index = 0usize;
        loop {
            let byte = *input.get(offset).ok_or_else(|| {
                CircuitError::invalid_result_format("r8 input ended before record completed")
            })?;
            offset += 1;
            if byte == u8::MAX {
                bit_index += usize::from(u8::MAX);
                if bit_index > bits_per_record {
                    return Err(CircuitError::invalid_result_format(
                        "r8 run-length overshot record width",
                    ));
                }
                continue;
            }
            bit_index += usize::from(byte);
            if bit_index > bits_per_record {
                return Err(CircuitError::invalid_result_format(
                    "r8 run-length overshot record width",
                ));
            }
            if bit_index == bits_per_record {
                break;
            }
            let Some(bit) = record.get_mut(bit_index) else {
                return Err(CircuitError::invalid_result_format(format!(
                    "r8 hit index {bit_index} exceeds record width {bits_per_record}"
                )));
            };
            *bit = true;
            bit_index += 1;
        }
        visit(&record)?;
    }
    Ok(())
}

fn for_each_hits_record<F>(input: &[u8], bits_per_record: usize, mut visit: F) -> CircuitResult<()>
where
    F: FnMut(&[bool]) -> CircuitResult<()>,
{
    let text = std::str::from_utf8(input)
        .map_err(|error| CircuitError::invalid_result_format(error.to_string()))?;
    let mut record = vec![false; bits_per_record];
    for line in text.split_terminator('\n') {
        fill_sparse_index_line(strip_trailing_cr(line), bits_per_record, None, &mut record)?;
        visit(&record)?;
    }
    Ok(())
}

fn for_each_dets_record<F>(input: &[u8], bits_per_record: usize, mut visit: F) -> CircuitResult<()>
where
    F: FnMut(&[bool]) -> CircuitResult<()>,
{
    let text = std::str::from_utf8(input)
        .map_err(|error| CircuitError::invalid_result_format(error.to_string()))?;
    let mut record = vec![false; bits_per_record];
    for line in text.split_terminator('\n') {
        let line = strip_trailing_cr(line).trim();
        if line.is_empty() {
            continue;
        }
        let Some(rest) = line.strip_prefix("shot") else {
            return Err(CircuitError::invalid_result_format(format!(
                "dets record does not start with shot: {line:?}"
            )));
        };
        fill_sparse_index_line(rest.trim(), bits_per_record, Some(()), &mut record)?;
        visit(&record)?;
    }
    Ok(())
}

fn for_each_zero_one_packed_record<F>(
    input: &[u8],
    bits_per_record: usize,
    mut visit: F,
) -> CircuitResult<()>
where
    F: FnMut(BitSlice<'_>) -> CircuitResult<()>,
{
    if input.is_empty() {
        return Ok(());
    }
    let mut record = BitVec::zeros(bits_per_record);
    for_each_zero_one_line(input, bits_per_record, |line| {
        record.clear();
        for (index, byte) in line.iter().enumerate() {
            match byte {
                b'0' => {}
                b'1' => record.set(index, true).map_err(bit_error_to_format_error)?,
                _ => {
                    return Err(CircuitError::invalid_result_format(format!(
                        "01 record contains non-bit byte {byte}"
                    )));
                }
            }
        }
        visit(record.as_bitslice())
    })
}

fn for_each_b8_packed_record<F>(
    input: &[u8],
    bits_per_record: usize,
    mut visit: F,
) -> CircuitResult<()>
where
    F: FnMut(BitSlice<'_>) -> CircuitResult<()>,
{
    let bytes_per_record = bits_per_record.div_ceil(8);
    if bytes_per_record == 0 {
        return Err(CircuitError::invalid_result_format(
            "b8 input cannot represent zero-width records",
        ));
    }
    if !input.len().is_multiple_of(bytes_per_record) {
        return Err(CircuitError::invalid_result_format(format!(
            "b8 input length {} is not a multiple of record byte width {bytes_per_record}",
            input.len()
        )));
    }
    let mut record = BitVec::zeros(bits_per_record);
    for chunk in input.chunks_exact(bytes_per_record) {
        unpack_b8_chunk_into_words(chunk, bits_per_record, record.words_mut());
        visit(record.as_bitslice())?;
    }
    Ok(())
}

fn for_each_r8_packed_record<F>(
    input: &[u8],
    bits_per_record: usize,
    mut visit: F,
) -> CircuitResult<()>
where
    F: FnMut(BitSlice<'_>) -> CircuitResult<()>,
{
    let mut record = BitVec::zeros(bits_per_record);
    let mut offset = 0usize;
    while offset < input.len() {
        record.clear();
        fill_r8_packed_record(input, bits_per_record, &mut offset, &mut record)?;
        visit(record.as_bitslice())?;
    }
    Ok(())
}

fn for_each_hits_packed_record<F>(
    input: &[u8],
    bits_per_record: usize,
    mut visit: F,
) -> CircuitResult<()>
where
    F: FnMut(BitSlice<'_>) -> CircuitResult<()>,
{
    let text = std::str::from_utf8(input)
        .map_err(|error| CircuitError::invalid_result_format(error.to_string()))?;
    let mut record = BitVec::zeros(bits_per_record);
    for line in text.split_terminator('\n') {
        fill_sparse_index_line_packed(strip_trailing_cr(line), bits_per_record, None, &mut record)?;
        visit(record.as_bitslice())?;
    }
    Ok(())
}

fn for_each_dets_packed_record<F>(
    input: &[u8],
    bits_per_record: usize,
    mut visit: F,
) -> CircuitResult<()>
where
    F: FnMut(BitSlice<'_>) -> CircuitResult<()>,
{
    let text = std::str::from_utf8(input)
        .map_err(|error| CircuitError::invalid_result_format(error.to_string()))?;
    let mut record = BitVec::zeros(bits_per_record);
    for line in text.split_terminator('\n') {
        let line = strip_trailing_cr(line).trim();
        if line.is_empty() {
            continue;
        }
        let Some(rest) = line.strip_prefix("shot") else {
            return Err(CircuitError::invalid_result_format(format!(
                "dets record does not start with shot: {line:?}"
            )));
        };
        fill_sparse_index_line_packed(rest.trim(), bits_per_record, Some(()), &mut record)?;
        visit(record.as_bitslice())?;
    }
    Ok(())
}

fn for_each_zero_one_sparse_record<F>(
    input: &[u8],
    bits_per_record: usize,
    mut visit: F,
) -> CircuitResult<()>
where
    F: FnMut(&[u64]) -> CircuitResult<()>,
{
    if input.is_empty() {
        return Ok(());
    }
    let mut hits = Vec::new();
    for_each_zero_one_line(input, bits_per_record, |line| {
        hits.clear();
        for (index, byte) in line.iter().enumerate() {
            match byte {
                b'0' => {}
                b'1' => hits.push(u64::try_from(index).map_err(|_| {
                    CircuitError::invalid_result_format(format!(
                        "01 hit index {index} does not fit u64"
                    ))
                })?),
                _ => {
                    return Err(CircuitError::invalid_result_format(format!(
                        "01 record contains non-bit byte {byte}"
                    )));
                }
            }
        }
        visit(&hits)
    })
}

fn for_each_b8_sparse_record<F>(
    input: &[u8],
    bits_per_record: usize,
    mut visit: F,
) -> CircuitResult<()>
where
    F: FnMut(&[u64]) -> CircuitResult<()>,
{
    let bytes_per_record = bits_per_record.div_ceil(8);
    if bytes_per_record == 0 {
        return Err(CircuitError::invalid_result_format(
            "b8 input cannot represent zero-width records",
        ));
    }
    if !input.len().is_multiple_of(bytes_per_record) {
        return Err(CircuitError::invalid_result_format(format!(
            "b8 input length {} is not a multiple of record byte width {bytes_per_record}",
            input.len()
        )));
    }
    let mut hits = Vec::new();
    for chunk in input.chunks_exact(bytes_per_record) {
        collect_b8_chunk_hits(chunk, bits_per_record, &mut hits)?;
        visit(&hits)?;
    }
    Ok(())
}

fn for_each_r8_sparse_record<F>(
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
        fill_r8_sparse_record(input, bits_per_record, &mut offset, &mut hits)?;
        visit(&hits)?;
    }
    Ok(())
}

fn for_each_hits_sparse_record<F>(
    input: &[u8],
    bits_per_record: usize,
    mut visit: F,
) -> CircuitResult<()>
where
    F: FnMut(&[u64]) -> CircuitResult<()>,
{
    let text = std::str::from_utf8(input)
        .map_err(|error| CircuitError::invalid_result_format(error.to_string()))?;
    let mut hits = Vec::new();
    for line in text.split_terminator('\n') {
        parse_sparse_index_line(strip_trailing_cr(line), bits_per_record, None, &mut hits)?;
        visit(&hits)?;
    }
    Ok(())
}

fn for_each_dets_sparse_record<F>(
    input: &[u8],
    bits_per_record: usize,
    mut visit: F,
) -> CircuitResult<()>
where
    F: FnMut(&[u64]) -> CircuitResult<()>,
{
    let text = std::str::from_utf8(input)
        .map_err(|error| CircuitError::invalid_result_format(error.to_string()))?;
    let mut hits = Vec::new();
    for line in text.split_terminator('\n') {
        let line = strip_trailing_cr(line).trim();
        if line.is_empty() {
            continue;
        }
        let Some(rest) = line.strip_prefix("shot") else {
            return Err(CircuitError::invalid_result_format(format!(
                "dets record does not start with shot: {line:?}"
            )));
        };
        parse_sparse_index_line(rest.trim(), bits_per_record, Some(()), &mut hits)?;
        visit(&hits)?;
    }
    Ok(())
}

fn for_each_zero_one_line<F>(
    input: &[u8],
    bits_per_record: usize,
    mut visit: F,
) -> CircuitResult<()>
where
    F: FnMut(&[u8]) -> CircuitResult<()>,
{
    let mut offset = 0usize;
    while offset < input.len() {
        let line_start = offset;
        while input.get(offset).is_some_and(|byte| *byte != b'\n') {
            offset += 1;
        }
        let mut line_end = offset;
        if line_end > line_start && input.get(line_end - 1).is_some_and(|byte| *byte == b'\r') {
            line_end -= 1;
        }
        let line = input.get(line_start..line_end).ok_or_else(|| {
            CircuitError::invalid_result_format("01 record byte range was out of bounds")
        })?;
        if line.len() != bits_per_record {
            return Err(CircuitError::invalid_result_format(format!(
                "01 record expected {bits_per_record} bits, got {}",
                line.len()
            )));
        }
        visit(line)?;
        if offset < input.len() {
            offset += 1;
        }
    }
    Ok(())
}

fn fill_sparse_index_line(
    line: &str,
    bits_per_record: usize,
    dets_tokens: Option<()>,
    record: &mut [bool],
) -> CircuitResult<()> {
    if record.len() != bits_per_record {
        return Err(CircuitError::invalid_result_format(
            "streaming record buffer width did not match requested width",
        ));
    }
    record.fill(false);
    if line.is_empty() {
        return Ok(());
    }
    for token in line.split(if dets_tokens.is_some() { ' ' } else { ',' }) {
        if token.is_empty() {
            continue;
        }
        let index = if dets_tokens.is_some() {
            let mut chars = token.chars();
            let Some('M' | 'D' | 'L') = chars.next() else {
                return Err(CircuitError::invalid_result_format(format!(
                    "invalid dets token {token:?}"
                )));
            };
            parse_sparse_index(chars.as_str())?
        } else {
            parse_sparse_index(token)?
        };
        let index = usize::try_from(index).map_err(|_| {
            CircuitError::invalid_result_format(format!("sparse index {index} does not fit usize"))
        })?;
        let Some(bit) = record.get_mut(index) else {
            return Err(CircuitError::invalid_result_format(format!(
                "sparse index {index} exceeds record width {bits_per_record}"
            )));
        };
        *bit = true;
    }
    Ok(())
}

fn fill_sparse_index_line_packed(
    line: &str,
    bits_per_record: usize,
    dets_tokens: Option<()>,
    record: &mut BitVec,
) -> CircuitResult<()> {
    if record.len() != bits_per_record {
        return Err(CircuitError::invalid_result_format(
            "streaming packed record buffer width did not match requested width",
        ));
    }
    record.clear();
    if line.is_empty() {
        return Ok(());
    }
    for_each_sparse_index(line, bits_per_record, dets_tokens, |index| {
        let index = usize::try_from(index).map_err(|_| {
            CircuitError::invalid_result_format(format!("sparse index {index} does not fit usize"))
        })?;
        record.set(index, true).map_err(bit_error_to_format_error)?;
        Ok(())
    })?;
    Ok(())
}

fn parse_sparse_index_line(
    line: &str,
    bits_per_record: usize,
    dets_tokens: Option<()>,
    hits: &mut Vec<u64>,
) -> CircuitResult<()> {
    hits.clear();
    if line.is_empty() {
        return Ok(());
    }
    for_each_sparse_index(line, bits_per_record, dets_tokens, |index| {
        hits.push(index);
        Ok(())
    })?;
    normalize_sparse_hits(hits);
    Ok(())
}

fn for_each_sparse_index<F>(
    line: &str,
    bits_per_record: usize,
    dets_tokens: Option<()>,
    mut visit: F,
) -> CircuitResult<()>
where
    F: FnMut(u64) -> CircuitResult<()>,
{
    for token in line.split(if dets_tokens.is_some() { ' ' } else { ',' }) {
        if token.is_empty() {
            continue;
        }
        let index = if dets_tokens.is_some() {
            let mut chars = token.chars();
            let Some('M' | 'D' | 'L') = chars.next() else {
                return Err(CircuitError::invalid_result_format(format!(
                    "invalid dets token {token:?}"
                )));
            };
            parse_sparse_index(chars.as_str())?
        } else {
            parse_sparse_index(token)?
        };
        if index
            >= u64::try_from(bits_per_record).map_err(|_| {
                CircuitError::invalid_result_format("record width does not fit sparse index bounds")
            })?
        {
            return Err(CircuitError::invalid_result_format(format!(
                "sparse index {index} exceeds record width {bits_per_record}"
            )));
        }
        visit(index)?;
    }
    Ok(())
}

fn fill_r8_packed_record(
    input: &[u8],
    bits_per_record: usize,
    offset: &mut usize,
    record: &mut BitVec,
) -> CircuitResult<()> {
    let mut bit_index = 0usize;
    loop {
        let byte = *input.get(*offset).ok_or_else(|| {
            CircuitError::invalid_result_format("r8 input ended before record completed")
        })?;
        *offset += 1;
        if byte == u8::MAX {
            bit_index += usize::from(u8::MAX);
            if bit_index > bits_per_record {
                return Err(CircuitError::invalid_result_format(
                    "r8 run-length overshot record width",
                ));
            }
            continue;
        }
        bit_index += usize::from(byte);
        if bit_index > bits_per_record {
            return Err(CircuitError::invalid_result_format(
                "r8 run-length overshot record width",
            ));
        }
        if bit_index == bits_per_record {
            break;
        }
        record
            .set(bit_index, true)
            .map_err(bit_error_to_format_error)?;
        bit_index += 1;
    }
    Ok(())
}

fn fill_r8_sparse_record(
    input: &[u8],
    bits_per_record: usize,
    offset: &mut usize,
    hits: &mut Vec<u64>,
) -> CircuitResult<()> {
    hits.clear();
    let mut bit_index = 0usize;
    loop {
        let byte = *input.get(*offset).ok_or_else(|| {
            CircuitError::invalid_result_format("r8 input ended before record completed")
        })?;
        *offset += 1;
        if byte == u8::MAX {
            bit_index += usize::from(u8::MAX);
            if bit_index > bits_per_record {
                return Err(CircuitError::invalid_result_format(
                    "r8 run-length overshot record width",
                ));
            }
            continue;
        }
        bit_index += usize::from(byte);
        if bit_index > bits_per_record {
            return Err(CircuitError::invalid_result_format(
                "r8 run-length overshot record width",
            ));
        }
        if bit_index == bits_per_record {
            break;
        }
        hits.push(u64::try_from(bit_index).map_err(|_| {
            CircuitError::invalid_result_format(format!(
                "r8 hit index {bit_index} does not fit u64"
            ))
        })?);
        bit_index += 1;
    }
    Ok(())
}

fn unpack_b8_chunk_into(chunk: &[u8], record: &mut [bool]) {
    for (bit_index, bit) in record.iter_mut().enumerate() {
        *bit = chunk.get(bit_index / 8).copied().unwrap_or(0) & (1u8 << (bit_index % 8)) != 0;
    }
}

fn unpack_b8_chunk_into_words(chunk: &[u8], bits_per_record: usize, words: &mut [u64]) {
    words.fill(0);
    let mut word_index = 0usize;
    let mut chunks = chunk.chunks_exact(8);
    for word_bytes in chunks.by_ref() {
        let Some(word) = words.get_mut(word_index) else {
            break;
        };
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(word_bytes);
        *word = u64::from_le_bytes(bytes);
        word_index += 1;
    }
    let remainder = chunks.remainder();
    if !remainder.is_empty()
        && let Some(word) = words.get_mut(word_index)
    {
        let mut tail = 0u64;
        for (byte_index, byte) in remainder.iter().enumerate() {
            tail |= u64::from(*byte) << (byte_index * 8);
        }
        *word = tail;
    }
    if let Some(last) = words.last_mut() {
        let tail = bits_per_record % u64::BITS as usize;
        if tail != 0 {
            *last &= (1_u64 << tail) - 1;
        }
    }
}

fn collect_b8_chunk_hits(
    chunk: &[u8],
    bits_per_record: usize,
    hits: &mut Vec<u64>,
) -> CircuitResult<()> {
    hits.clear();
    for bit_index in 0..bits_per_record {
        if chunk.get(bit_index / 8).copied().unwrap_or(0) & (1u8 << (bit_index % 8)) != 0 {
            hits.push(u64::try_from(bit_index).map_err(|_| {
                CircuitError::invalid_result_format(format!(
                    "b8 hit index {bit_index} does not fit u64"
                ))
            })?);
        }
    }
    Ok(())
}

fn normalize_sparse_hits(hits: &mut Vec<u64>) {
    let already_strictly_ordered = hits
        .windows(2)
        .all(|window| matches!(window, [left, right] if left < right));
    if !already_strictly_ordered {
        hits.sort_unstable();
        hits.dedup();
    }
}

fn strip_trailing_cr(line: &str) -> &str {
    line.strip_suffix('\r').unwrap_or(line)
}

fn parse_sparse_index(text: &str) -> CircuitResult<u64> {
    text.parse::<u64>()
        .map_err(|error| CircuitError::invalid_result_format(error.to_string()))
}

fn bit_error_to_format_error(error: crate::bits::BitError) -> CircuitError {
    CircuitError::invalid_result_format(error.to_string())
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        clippy::unwrap_used,
        reason = "streaming result-format tests use compact fixture assertions"
    )]

    use crate::result_formats::{
        read_ptb64_records_all, read_records, write_ptb64_records_checked, write_records,
    };

    use super::*;

    #[test]
    fn streaming_readers_match_materialized_readers() {
        let records = vec![
            vec![true, false, true, false, false, true, false, false, true],
            vec![false, true, false, true, false, false, true, false, false],
            vec![
                false, false, false, false, false, false, false, false, false,
            ],
        ];

        for format in [
            SampleFormat::ZeroOne,
            SampleFormat::B8,
            SampleFormat::R8,
            SampleFormat::Hits,
            SampleFormat::Dets,
        ] {
            let input = write_records(&records, format);
            let mut streamed = Vec::new();
            for_each_record(&input, format, 9, |record| {
                streamed.push(record.to_vec());
                Ok(())
            })
            .unwrap();
            assert_eq!(streamed, read_records(&input, format, 9).unwrap());
        }
    }

    #[test]
    fn packed_and_sparse_streaming_readers_match_materialized_readers() {
        let records = vec![
            vec![true, false, true, false, false, true, false, false, true],
            vec![false, true, false, true, false, false, true, false, false],
            vec![
                false, false, false, false, false, false, false, false, false,
            ],
        ];

        for format in [
            SampleFormat::ZeroOne,
            SampleFormat::B8,
            SampleFormat::R8,
            SampleFormat::Hits,
            SampleFormat::Dets,
        ] {
            let input = write_records(&records, format);
            let expected = read_records(&input, format, 9).unwrap();
            let mut packed = Vec::new();
            for_each_packed_record(&input, format, 9, |record| {
                packed.push(bitslice_to_vec(record));
                Ok(())
            })
            .unwrap();
            assert_eq!(packed, expected);

            let mut sparse = Vec::new();
            for_each_sparse_record(&input, format, 9, |hits| {
                sparse.push(sparse_hits_to_vec(hits, 9));
                Ok(())
            })
            .unwrap();
            assert_eq!(sparse, expected);
        }
    }

    #[test]
    fn streaming_ptb64_reader_matches_materialized_reader() {
        let records = (0..64)
            .map(|shot_index| {
                (0..17)
                    .map(|bit_index| (shot_index * 7 + bit_index * 11) % 13 == 0)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let input = write_ptb64_records_checked(&records).expect("write ptb64");
        let mut streamed = Vec::new();

        for_each_ptb64_record_all(&input, 17, |record| {
            streamed.push(record.to_vec());
            Ok(())
        })
        .unwrap();

        assert_eq!(streamed, read_ptb64_records_all(&input, 17).unwrap());
    }

    #[test]
    fn streaming_readers_reject_malformed_inputs() {
        assert!(for_each_record(b"0x\n", SampleFormat::ZeroOne, 2, ignore_record).is_err());
        assert!(for_each_record(&[0xff], SampleFormat::B8, 9, ignore_record).is_err());
        assert!(for_each_record(&[3], SampleFormat::R8, 2, ignore_record).is_err());
        assert!(for_each_record(b"3\n", SampleFormat::Hits, 3, ignore_record).is_err());
        assert!(for_each_record(b"shot Q0\n", SampleFormat::Dets, 1, ignore_record).is_err());
        assert!(
            for_each_packed_record(b"shot Q0\n", SampleFormat::Dets, 1, ignore_packed).is_err()
        );
        assert!(
            for_each_sparse_record(b"shot Q0\n", SampleFormat::Dets, 1, ignore_sparse).is_err()
        );
        assert!(for_each_ptb64_record_all(&[0; 7], 1, ignore_record).is_err());
        assert!(for_each_ptb64_record(&[], 0, 64, ignore_record).is_err());
    }

    #[test]
    fn streaming_readers_stop_on_visitor_error() {
        let records = vec![vec![true, false], vec![false, true]];
        let input = write_records(&records, SampleFormat::ZeroOne);
        let mut visited = 0usize;

        let result = for_each_record(&input, SampleFormat::ZeroOne, 2, |_| {
            visited += 1;
            Err(CircuitError::invalid_result_format("visitor stopped"))
        });

        assert!(result.is_err());
        assert_eq!(visited, 1);
    }

    fn ignore_record(_: &[bool]) -> CircuitResult<()> {
        Ok(())
    }

    fn ignore_packed(_: BitSlice<'_>) -> CircuitResult<()> {
        Ok(())
    }

    fn ignore_sparse(_: &[u64]) -> CircuitResult<()> {
        Ok(())
    }

    fn bitslice_to_vec(record: BitSlice<'_>) -> Vec<bool> {
        (0..record.len())
            .map(|index| record.get(index).unwrap())
            .collect()
    }

    fn sparse_hits_to_vec(hits: &[u64], bits_per_record: usize) -> Vec<bool> {
        let mut record = vec![false; bits_per_record];
        for hit in hits {
            *record
                .get_mut(usize::try_from(*hit).unwrap())
                .expect("sparse hit is in range") = true;
        }
        record
    }
}
