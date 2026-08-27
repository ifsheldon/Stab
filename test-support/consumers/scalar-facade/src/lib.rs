//! Nightly facade fixture with portable SIMD explicitly disabled.

use std::error::Error;

pub use stab_core::experimental;

use stab_core::advanced::{
    backend::BackendPreference,
    compat::CompiledSampler,
    records::{
        DetsLayout, FormatError as RecordFormatError, MeasurementCodecSink, RecordResult,
        RecordStreamReader,
    },
    storage::BitVec,
    traversal::CircuitFlattenedInstructionIter,
};
use stab_core::{
    Circuit, MeasurementBatchView, MeasurementSink, MeasurementWidth, PackedShotBatch,
    RecordFormat, SamplingCompiler,
};

pub fn exercise_scalar_facade() -> Result<(usize, usize, usize), Box<dyn Error>> {
    let mut left = BitVec::from_words_truncated(257, vec![0x55aa; 5]);
    let right = BitVec::from_words_truncated(257, vec![0xaa55; 5]);
    left.xor_assign(&right.as_bitslice())?;

    let circuit = Circuit::from_stim_str("M 0\nDETECTOR rec[-1]\n")?;
    let _: CircuitFlattenedInstructionIter<'_> = circuit.iter_flattened_instructions();
    let layout = DetsLayout::try_new(1, 1, 0)?;
    let plan = SamplingCompiler::new()
        .backend(BackendPreference::Scalar)
        .compile(&circuit)?;
    let adapter = CompiledSampler::compile(&circuit)?;

    let encoded = encode_streamed_records(b"10\n01\n")?;
    if encoded != [0x01, 0x02] {
        return Err(std::io::Error::other("facade-encoded records changed bytes").into());
    }

    Ok((
        left.len(),
        layout.total_bits(),
        plan.measurement_width().get() + adapter.plan().measurement_width().get(),
    ))
}

/// Names the component sink and stream-reader error identity through stab-core paths only: the
/// sink's associated `Error` type is the facade-re-exported `stab_core::advanced::records`
/// `FormatError`, without any direct `stab-records` dependency.
fn encode_streamed_records(input: &[u8]) -> RecordResult<Vec<u8>> {
    let mut reader =
        RecordStreamReader::measurements(input, RecordFormat::ZeroOne, 2, 1024);
    let mut records = Vec::new();
    loop {
        match reader.next_record() {
            Ok(Some(record)) => records.push(record.to_vec()),
            Ok(None) => break,
            Err(error) => return Err(stream_error_to_sink_error(error)),
        }
    }
    let mut sink = MeasurementCodecSink::try_new(RecordFormat::B8, MeasurementWidth::new(2))?;
    let batch = PackedShotBatch::from_records(&records, 2)?;
    let sink_error_names_the_component_type: Result<(), RecordFormatError> =
        sink.write_batch(MeasurementBatchView::new(batch.view()));
    sink_error_names_the_component_type?;
    sink.into_bytes()
}

fn stream_error_to_sink_error(
    error: stab_core::advanced::records::RecordStreamReadError,
) -> RecordFormatError {
    match error {
        stab_core::advanced::records::RecordStreamReadError::Format(error) => error,
        stab_core::advanced::records::RecordStreamReadError::Io(error) => {
            RecordFormatError::new(
                stab_core::advanced::records::FormatErrorCode::UnexpectedEndOfInput,
                error.to_string(),
                None,
            )
        }
    }
}
