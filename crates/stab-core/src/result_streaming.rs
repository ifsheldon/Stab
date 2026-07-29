use crate::{
    BitSlice, CircuitError, CircuitResult, DetsLayout, DetsToken, SampleFormat,
    result_formats::SparseShot,
};
use stab_records::RecordStreamError;

pub fn for_each_record<F>(
    input: &[u8],
    format: SampleFormat,
    bits_per_record: usize,
    visit: F,
) -> CircuitResult<()>
where
    F: FnMut(&[bool]) -> CircuitResult<()>,
{
    stab_records::try_for_each_record(input, format, bits_per_record, visit)
        .map_err(map_stream_error)
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
    stab_records::try_for_each_packed_record(input, format, bits_per_record, visit)
        .map_err(map_stream_error)
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
    stab_records::try_for_each_sparse_record(input, format, bits_per_record, visit)
        .map_err(map_stream_error)
}

pub fn for_each_ptb64_record_all<F>(
    input: &[u8],
    bits_per_record: usize,
    visit: F,
) -> CircuitResult<()>
where
    F: FnMut(&[bool]) -> CircuitResult<()>,
{
    stab_records::try_for_each_ptb64_record_all(input, bits_per_record, visit)
        .map_err(map_stream_error)
}

pub fn for_each_ptb64_record<F>(
    input: &[u8],
    bits_per_record: usize,
    max_shots: usize,
    visit: F,
) -> CircuitResult<()>
where
    F: FnMut(&[bool]) -> CircuitResult<()>,
{
    crate::result_formats::validate_ptb64_shot_count(max_shots)?;
    stab_records::try_for_each_ptb64_record(input, bits_per_record, max_shots, visit)
        .map_err(map_stream_error)
}

pub fn for_each_dets_record<F>(input: &[u8], layout: DetsLayout, visit: F) -> CircuitResult<()>
where
    F: FnMut(&[bool]) -> CircuitResult<()>,
{
    stab_records::try_for_each_dets_record(input, layout, visit).map_err(map_stream_error)
}

pub fn for_each_dets_packed_record<F>(
    input: &[u8],
    layout: DetsLayout,
    visit: F,
) -> CircuitResult<()>
where
    F: FnMut(BitSlice<'_>) -> CircuitResult<()>,
{
    stab_records::try_for_each_dets_packed_record(input, layout, visit).map_err(map_stream_error)
}

pub fn for_each_dets_token_record<F>(
    input: &[u8],
    layout: DetsLayout,
    visit: F,
) -> CircuitResult<()>
where
    F: FnMut(&[DetsToken]) -> CircuitResult<()>,
{
    stab_records::try_for_each_dets_token_record(input, layout, visit).map_err(map_stream_error)
}

pub fn for_each_dets_sparse_shot<F>(input: &[u8], layout: DetsLayout, visit: F) -> CircuitResult<()>
where
    F: FnMut(&SparseShot) -> CircuitResult<()>,
{
    stab_records::try_for_each_dets_sparse_shot(input, layout, visit).map_err(map_stream_error)
}

fn map_stream_error(error: RecordStreamError<CircuitError>) -> CircuitError {
    match error {
        RecordStreamError::Format(error) => CircuitError::from(crate::FormatError::from(error)),
        RecordStreamError::Visitor(error) => error,
    }
}
