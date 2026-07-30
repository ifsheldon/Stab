//! Source-compatible materialized and encoded adapters over sampling sessions.

use std::convert::Infallible;

use crate::{
    CircuitError, CircuitResult, FormatError, MeasurementCodecSink, RecordFormat, SampleFormat,
    sampling::{
        CompiledSampler, RunError, SamplingExecutionError, SamplingRunProgress, ShotCount,
        legacy_execution_error, legacy_random_policy, legacy_reference_mode, legacy_shot_count,
    },
};

impl CompiledSampler {
    /// Materializes measurement records while preserving execution and allocation failures.
    pub fn try_sample_zero_one(
        &self,
        shots: usize,
    ) -> Result<Vec<Vec<bool>>, RunError<Infallible>> {
        self.try_sample_zero_one_with_seed(shots, None)
    }

    /// Materializes seeded measurement records while preserving execution and allocation failures.
    pub fn try_sample_zero_one_with_seed(
        &self,
        shots: usize,
        seed: Option<u64>,
    ) -> Result<Vec<Vec<bool>>, RunError<Infallible>> {
        self.try_sample_zero_one_with_seed_and_reference_mode(shots, seed, false)
    }

    /// Materializes seeded measurement records with an explicit reference-sample policy.
    pub fn try_sample_zero_one_with_seed_and_reference_mode(
        &self,
        shots: usize,
        seed: Option<u64>,
        skip_reference_sample: bool,
    ) -> Result<Vec<Vec<bool>>, RunError<Infallible>> {
        let measurement_width = self.plan().measurement_width().get();
        validate_materialized_request(shots, measurement_width).map_err(preflight_run_error)?;

        let mut samples = Vec::new();
        samples.try_reserve_exact(shots).map_err(|error| {
            preflight_run_error(adapter_allocation_error(
                "materialized sample record",
                shots,
                error,
            ))
        })?;
        let result = self.try_for_each_sample_with_seed_and_reference_mode(
            shots,
            seed,
            skip_reference_sample,
            |sample| {
                let mut owned = Vec::new();
                owned.try_reserve_exact(sample.len()).map_err(|error| {
                    adapter_allocation_error("materialized sample bit", sample.len(), error)
                })?;
                owned.extend_from_slice(sample);
                samples.push(owned);
                Ok(())
            },
        );
        finish_adapter_run(result)?;
        Ok(samples)
    }

    /// Encodes `01` measurement records while preserving execution and allocation failures.
    pub fn try_sample_zero_one_bytes(&self, shots: usize) -> Result<Vec<u8>, RunError<Infallible>> {
        self.try_sample_bytes(shots, SampleFormat::ZeroOne)
    }

    /// Encodes measurement records while preserving execution and allocation failures.
    pub fn try_sample_bytes(
        &self,
        shots: usize,
        format: SampleFormat,
    ) -> Result<Vec<u8>, RunError<Infallible>> {
        self.try_sample_bytes_with_seed(shots, format, None)
    }

    /// Encodes seeded measurement records while preserving execution and allocation failures.
    pub fn try_sample_bytes_with_seed(
        &self,
        shots: usize,
        format: SampleFormat,
        seed: Option<u64>,
    ) -> Result<Vec<u8>, RunError<Infallible>> {
        self.try_sample_bytes_with_seed_and_reference_mode(shots, format, seed, false)
    }

    /// Encodes seeded measurement records with an explicit reference-sample policy.
    pub fn try_sample_bytes_with_seed_and_reference_mode(
        &self,
        shots: usize,
        format: SampleFormat,
        seed: Option<u64>,
        skip_reference_sample: bool,
    ) -> Result<Vec<u8>, RunError<Infallible>> {
        let mut encoder =
            FallibleSampleEncoder::try_new(format, shots, self.plan().measurement_width().get())
                .map_err(preflight_run_error)?;
        let result = self.try_for_each_sample_with_seed_and_reference_mode(
            shots,
            seed,
            skip_reference_sample,
            |sample| encoder.write_record(sample),
        );
        finish_adapter_run(result)?;
        Ok(encoder.into_bytes())
    }

    /// Materializes measurement records through the pre-0.2 compatibility contract.
    ///
    /// # Panics
    ///
    /// Panics when sampling execution or materialized-record allocation fails. Use
    /// [`Self::try_sample_zero_one`] to preserve these failures.
    pub fn sample_zero_one(&self, shots: usize) -> Vec<Vec<bool>> {
        legacy_materialization(self.try_sample_zero_one(shots))
    }

    /// Materializes seeded measurement records through the pre-0.2 compatibility contract.
    ///
    /// # Panics
    ///
    /// Panics when sampling execution or materialized-record allocation fails. Use
    /// [`Self::try_sample_zero_one_with_seed`] to preserve these failures.
    pub fn sample_zero_one_with_seed(&self, shots: usize, seed: Option<u64>) -> Vec<Vec<bool>> {
        legacy_materialization(self.try_sample_zero_one_with_seed(shots, seed))
    }

    /// Materializes seeded measurement records with an explicit reference-sample policy.
    ///
    /// # Panics
    ///
    /// Panics when sampling execution or materialized-record allocation fails. Use
    /// [`Self::try_sample_zero_one_with_seed_and_reference_mode`] to preserve these failures.
    pub fn sample_zero_one_with_seed_and_reference_mode(
        &self,
        shots: usize,
        seed: Option<u64>,
        skip_reference_sample: bool,
    ) -> Vec<Vec<bool>> {
        legacy_materialization(self.try_sample_zero_one_with_seed_and_reference_mode(
            shots,
            seed,
            skip_reference_sample,
        ))
    }

    /// Encodes `01` measurement records through the pre-0.2 compatibility contract.
    ///
    /// # Panics
    ///
    /// Panics when sampling execution or output allocation fails. Use
    /// [`Self::try_sample_zero_one_bytes`] to preserve these failures.
    pub fn sample_zero_one_bytes(&self, shots: usize) -> Vec<u8> {
        legacy_materialization(self.try_sample_zero_one_bytes(shots))
    }

    /// Encodes measurement records through the pre-0.2 compatibility contract.
    ///
    /// # Panics
    ///
    /// Panics when sampling execution or output allocation fails. Use [`Self::try_sample_bytes`]
    /// to preserve these failures.
    pub fn sample_bytes(&self, shots: usize, format: SampleFormat) -> Vec<u8> {
        legacy_materialization(self.try_sample_bytes(shots, format))
    }

    /// Encodes seeded measurement records through the pre-0.2 compatibility contract.
    ///
    /// # Panics
    ///
    /// Panics when sampling execution or output allocation fails. Use
    /// [`Self::try_sample_bytes_with_seed`] to preserve these failures.
    pub fn sample_bytes_with_seed(
        &self,
        shots: usize,
        format: SampleFormat,
        seed: Option<u64>,
    ) -> Vec<u8> {
        legacy_materialization(self.try_sample_bytes_with_seed(shots, format, seed))
    }

    /// Encodes seeded records with an explicit reference-sample policy.
    ///
    /// # Panics
    ///
    /// Panics when sampling execution or output allocation fails. Use
    /// [`Self::try_sample_bytes_with_seed_and_reference_mode`] to preserve these failures.
    pub fn sample_bytes_with_seed_and_reference_mode(
        &self,
        shots: usize,
        format: SampleFormat,
        seed: Option<u64>,
        skip_reference_sample: bool,
    ) -> Vec<u8> {
        legacy_materialization(self.try_sample_bytes_with_seed_and_reference_mode(
            shots,
            format,
            seed,
            skip_reference_sample,
        ))
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

fn validate_materialized_request(
    shots: usize,
    measurement_width: usize,
) -> Result<(), SamplingExecutionError> {
    ShotCount::try_from(shots)?;
    shots.checked_mul(measurement_width).ok_or_else(|| {
        adapter_size_error(format!(
            "materialized sample bit count overflowed for {shots} records of width {measurement_width}"
        ))
    })?;
    shots.checked_mul(size_of::<Vec<bool>>()).ok_or_else(|| {
        adapter_size_error(format!(
            "materialized sample record storage overflowed for {shots} records"
        ))
    })?;
    Ok(())
}

#[derive(Debug)]
struct FallibleSampleEncoder {
    format: SampleFormat,
    width: usize,
    output: Vec<u8>,
}

impl FallibleSampleEncoder {
    fn try_new(
        format: SampleFormat,
        shots: usize,
        width: usize,
    ) -> Result<Self, SamplingExecutionError> {
        ShotCount::try_from(shots)?;
        let minimum_per_record = match format {
            SampleFormat::ZeroOne => width.checked_add(1).ok_or_else(|| {
                adapter_size_error("01 encoded record length overflowed".to_owned())
            })?,
            SampleFormat::B8 => width.div_ceil(8),
            SampleFormat::R8 | SampleFormat::Hits => 1,
            SampleFormat::Dets => 5,
        };
        let minimum_output = shots.checked_mul(minimum_per_record).ok_or_else(|| {
            adapter_size_error(format!(
                "{} encoded output length overflowed for {shots} records of width {width}",
                sample_format_name(format)
            ))
        })?;
        let mut output = Vec::new();
        output.try_reserve_exact(minimum_output).map_err(|error| {
            adapter_allocation_error(
                sample_format_allocation_label(format),
                minimum_output,
                error,
            )
        })?;
        Ok(Self {
            format,
            width,
            output,
        })
    }

    fn write_record(&mut self, record: &[bool]) -> Result<(), SamplingExecutionError> {
        if record.len() != self.width {
            return Err(SamplingExecutionError::InternalInvariant {
                message: format!(
                    "{} sample encoder expected width {}, got {}",
                    sample_format_name(self.format),
                    self.width,
                    record.len()
                ),
            });
        }
        let encoded_len = encoded_record_len(self.format, record)?;
        self.output
            .try_reserve_exact(encoded_len)
            .map_err(|error| {
                adapter_allocation_error(
                    sample_format_allocation_label(self.format),
                    encoded_len,
                    error,
                )
            })?;
        let start = self.output.len();
        match self.format {
            SampleFormat::ZeroOne => encode_zero_one(&mut self.output, record),
            SampleFormat::B8 => encode_b8(&mut self.output, record),
            SampleFormat::R8 => encode_r8(&mut self.output, record),
            SampleFormat::Hits => encode_hits(&mut self.output, record),
            SampleFormat::Dets => encode_dets(&mut self.output, record),
        }
        let actual_len = self.output.len().checked_sub(start).ok_or_else(|| {
            SamplingExecutionError::InternalInvariant {
                message: "sample encoder output length moved backwards".to_owned(),
            }
        })?;
        if actual_len != encoded_len {
            return Err(SamplingExecutionError::InternalInvariant {
                message: format!(
                    "{} sample encoder wrote {actual_len} bytes after reserving {encoded_len}",
                    sample_format_name(self.format)
                ),
            });
        }
        Ok(())
    }

    fn into_bytes(self) -> Vec<u8> {
        self.output
    }
}

fn encoded_record_len(
    format: SampleFormat,
    record: &[bool],
) -> Result<usize, SamplingExecutionError> {
    match format {
        SampleFormat::ZeroOne => record
            .len()
            .checked_add(1)
            .ok_or_else(|| adapter_size_error("01 encoded record length overflowed".to_owned())),
        SampleFormat::B8 => Ok(record.len().div_ceil(8)),
        SampleFormat::R8 => r8_record_len(record),
        SampleFormat::Hits => sparse_record_len(record, false),
        SampleFormat::Dets => sparse_record_len(record, true),
    }
}

fn r8_record_len(record: &[bool]) -> Result<usize, SamplingExecutionError> {
    let mut encoded_len = 1_usize;
    let mut false_run = 0_u8;
    for bit in record {
        if *bit {
            if false_run == u8::MAX {
                encoded_len = checked_encoded_add(encoded_len, 1, "r8")?;
            }
            encoded_len = checked_encoded_add(encoded_len, 1, "r8")?;
            false_run = 0;
        } else {
            if false_run == u8::MAX {
                encoded_len = checked_encoded_add(encoded_len, 1, "r8")?;
                false_run = 0;
            }
            false_run += 1;
        }
    }
    if false_run == u8::MAX {
        encoded_len = checked_encoded_add(encoded_len, 1, "r8")?;
    }
    Ok(encoded_len)
}

fn sparse_record_len(record: &[bool], dets: bool) -> Result<usize, SamplingExecutionError> {
    let mut encoded_len = if dets { 5_usize } else { 1_usize };
    let mut has_hit = false;
    for (index, bit) in record.iter().copied().enumerate() {
        if !bit {
            continue;
        }
        let separator_len = if dets || has_hit { 1 } else { 0 };
        let type_len = usize::from(dets);
        encoded_len = checked_encoded_add(
            encoded_len,
            separator_len + type_len + decimal_len(index),
            if dets { "dets" } else { "hits" },
        )?;
        has_hit = true;
    }
    Ok(encoded_len)
}

fn checked_encoded_add(
    current: usize,
    additional: usize,
    format: &'static str,
) -> Result<usize, SamplingExecutionError> {
    current
        .checked_add(additional)
        .ok_or_else(|| adapter_size_error(format!("{format} encoded record length overflowed")))
}

fn encode_zero_one(output: &mut Vec<u8>, record: &[bool]) {
    output.extend(record.iter().map(|bit| if *bit { b'1' } else { b'0' }));
    output.push(b'\n');
}

fn encode_b8(output: &mut Vec<u8>, record: &[bool]) {
    let mut byte = 0_u8;
    let mut bit_index = 0_u8;
    for bit in record {
        if *bit {
            byte |= 1_u8 << bit_index;
        }
        bit_index += 1;
        if bit_index == 8 {
            output.push(byte);
            byte = 0;
            bit_index = 0;
        }
    }
    if bit_index != 0 {
        output.push(byte);
    }
}

fn encode_r8(output: &mut Vec<u8>, record: &[bool]) {
    let mut false_run = 0_u8;
    for bit in record {
        if *bit {
            if false_run == u8::MAX {
                output.push(u8::MAX);
                false_run = 0;
            }
            output.push(false_run);
            false_run = 0;
        } else {
            if false_run == u8::MAX {
                output.push(u8::MAX);
                false_run = 0;
            }
            false_run += 1;
        }
    }
    if false_run == u8::MAX {
        output.push(u8::MAX);
        false_run = 0;
    }
    output.push(false_run);
}

fn encode_hits(output: &mut Vec<u8>, record: &[bool]) {
    let mut first = true;
    for (index, bit) in record.iter().copied().enumerate() {
        if !bit {
            continue;
        }
        if !first {
            output.push(b',');
        }
        first = false;
        append_usize_decimal(output, index);
    }
    output.push(b'\n');
}

fn encode_dets(output: &mut Vec<u8>, record: &[bool]) {
    output.extend_from_slice(b"shot");
    for (index, bit) in record.iter().copied().enumerate() {
        if !bit {
            continue;
        }
        output.extend_from_slice(b" M");
        append_usize_decimal(output, index);
    }
    output.push(b'\n');
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "a decimal digit modulo 10 is always representable as u8"
)]
fn append_usize_decimal(output: &mut Vec<u8>, mut value: usize) {
    let mut digits = [0_u8; size_of::<usize>() * 3];
    let mut used = 0_usize;
    for digit in digits.iter_mut().rev() {
        *digit = b'0' + (value % 10) as u8;
        used += 1;
        value /= 10;
        if value == 0 {
            break;
        }
    }
    let start = digits.len() - used;
    output.extend(digits.into_iter().skip(start));
}

fn decimal_len(mut value: usize) -> usize {
    let mut digits = 1;
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    digits
}

fn sample_format_name(format: SampleFormat) -> &'static str {
    match format {
        SampleFormat::ZeroOne => "01",
        SampleFormat::B8 => "b8",
        SampleFormat::R8 => "r8",
        SampleFormat::Hits => "hits",
        SampleFormat::Dets => "dets",
    }
}

fn sample_format_allocation_label(format: SampleFormat) -> &'static str {
    match format {
        SampleFormat::ZeroOne => "01 sample output",
        SampleFormat::B8 => "b8 sample output",
        SampleFormat::R8 => "r8 sample output",
        SampleFormat::Hits => "hits sample output",
        SampleFormat::Dets => "dets sample output",
    }
}

fn adapter_allocation_error(
    label: &'static str,
    requested: usize,
    error: std::collections::TryReserveError,
) -> SamplingExecutionError {
    SamplingExecutionError::SessionStorageAllocation {
        message: format!("{label} capacity {requested}: {error}"),
    }
}

fn adapter_size_error(message: String) -> SamplingExecutionError {
    SamplingExecutionError::SessionStorageAllocation { message }
}

fn preflight_run_error(source: SamplingExecutionError) -> RunError<Infallible> {
    RunError::Engine {
        source,
        progress: SamplingRunProgress::default(),
    }
}

fn finish_adapter_run(
    result: Result<crate::SamplingRunSummary, RunError<SamplingExecutionError>>,
) -> Result<(), RunError<Infallible>> {
    match result {
        Ok(_) => Ok(()),
        Err(RunError::Engine { source, progress }) => Err(RunError::Engine { source, progress }),
        Err(RunError::Sink {
            source, progress, ..
        }) => Err(RunError::Engine { source, progress }),
    }
}

fn legacy_materialization<T>(result: Result<T, RunError<Infallible>>) -> T {
    match result {
        Ok(value) => value,
        Err(error) => legacy_materialization_failure(error),
    }
}

#[allow(
    clippy::panic,
    reason = "pre-0.2 materialization signatures cannot represent engine or allocation failures"
)]
fn legacy_materialization_failure(error: RunError<Infallible>) -> ! {
    panic!("legacy CompiledSampler materialization failed: {error}")
}

fn legacy_run_error(error: RunError<stab_records::FormatError>) -> CircuitError {
    match error {
        RunError::Engine { source, .. } => legacy_execution_error(source),
        RunError::Sink { source, .. } => CircuitError::from(FormatError::from(source)),
    }
}
