//! Source-compatible materialized and encoded adapters over sampling sessions.

use crate::{
    CircuitError, CircuitResult, FormatError, MeasurementCodecSink, RecordFormat, SampleFormat,
    result_formats::MeasureRecordWriter,
    sampling::{
        CompiledSampler, RunError, legacy_execution_error, legacy_random_policy,
        legacy_reference_mode, legacy_shot_count,
    },
};

impl CompiledSampler {
    pub fn sample_zero_one(&self, shots: usize) -> Vec<Vec<bool>> {
        self.sample_zero_one_with_seed(shots, None)
    }

    pub fn sample_zero_one_with_seed(&self, shots: usize, seed: Option<u64>) -> Vec<Vec<bool>> {
        self.sample_zero_one_with_seed_and_reference_mode(shots, seed, false)
    }

    pub fn sample_zero_one_with_seed_and_reference_mode(
        &self,
        shots: usize,
        seed: Option<u64>,
        skip_reference_sample: bool,
    ) -> Vec<Vec<bool>> {
        let mut samples = Vec::with_capacity(shots);
        let result = self.for_each_sample_with_seed_and_reference_mode(
            shots,
            seed,
            skip_reference_sample,
            |sample| {
                samples.push(sample.to_vec());
                Ok::<(), std::convert::Infallible>(())
            },
        );
        if let Err(error) = result {
            match error {}
        }
        samples
    }

    pub fn sample_zero_one_bytes(&self, shots: usize) -> Vec<u8> {
        self.sample_bytes(shots, SampleFormat::ZeroOne)
    }

    pub fn sample_bytes(&self, shots: usize, format: SampleFormat) -> Vec<u8> {
        self.sample_bytes_with_seed(shots, format, None)
    }

    pub fn sample_bytes_with_seed(
        &self,
        shots: usize,
        format: SampleFormat,
        seed: Option<u64>,
    ) -> Vec<u8> {
        self.sample_bytes_with_seed_and_reference_mode(shots, format, seed, false)
    }

    pub fn sample_bytes_with_seed_and_reference_mode(
        &self,
        shots: usize,
        format: SampleFormat,
        seed: Option<u64>,
        skip_reference_sample: bool,
    ) -> Vec<u8> {
        let mut writer = MeasureRecordWriter::with_capacity(
            format,
            estimated_sample_bytes_capacity(format, shots, self.plan().measurement_width().get()),
        );
        let result = self.for_each_sample_with_seed_and_reference_mode(
            shots,
            seed,
            skip_reference_sample,
            |sample| {
                writer.write_bits(sample);
                writer.write_end();
                Ok::<(), std::convert::Infallible>(())
            },
        );
        if let Err(error) = result {
            match error {}
        }
        writer.into_bytes()
    }

    pub fn sample_ptb64_bytes_with_seed(
        &self,
        shots: usize,
        seed: Option<u64>,
    ) -> CircuitResult<Vec<u8>> {
        self.sample_ptb64_bytes_with_seed_and_reference_mode(shots, seed, false)
    }

    pub fn sample_ptb64_bytes_with_seed_and_reference_mode(
        &self,
        shots: usize,
        seed: Option<u64>,
        skip_reference_sample: bool,
    ) -> CircuitResult<Vec<u8>> {
        if !shots.is_multiple_of(64) {
            return Err(CircuitError::invalid_sampler_compilation(
                "shots must be a multiple of 64 to use ptb64 format",
            ));
        }
        let mut session = self
            .plan()
            .session_with_reference_mode(
                legacy_random_policy(seed),
                legacy_reference_mode(skip_reference_sample),
            )
            .map_err(legacy_execution_error)?;
        let mut sink =
            MeasurementCodecSink::try_new(RecordFormat::Ptb64, self.plan().measurement_width())
                .map_err(|error| CircuitError::from(FormatError::from(error)))?;
        session
            .run(legacy_shot_count(shots)?, &mut sink)
            .map_err(legacy_run_error)?;
        sink.into_bytes()
            .map_err(|error| CircuitError::from(FormatError::from(error)))
    }
}

fn estimated_sample_bytes_capacity(
    format: SampleFormat,
    shots: usize,
    bits_per_shot: usize,
) -> usize {
    let bytes_per_shot = match format {
        SampleFormat::ZeroOne => bits_per_shot.checked_add(1),
        SampleFormat::B8 => Some(bits_per_shot.div_ceil(8)),
        SampleFormat::R8 | SampleFormat::Hits | SampleFormat::Dets => None,
    };
    bytes_per_shot
        .and_then(|bytes_per_shot| shots.checked_mul(bytes_per_shot))
        .unwrap_or(0)
}

fn legacy_run_error(error: RunError<stab_records::FormatError>) -> CircuitError {
    match error {
        RunError::Engine { source, .. } => legacy_execution_error(source),
        RunError::Sink { source, .. } => CircuitError::from(FormatError::from(source)),
    }
}
