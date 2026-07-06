use rand::rngs::SmallRng;
use rand::{Rng, RngExt as _, SeedableRng as _};

use crate::{
    CircuitError, CircuitResult, DemInstruction, DemInstructionKind, DemItem, DemTarget,
    DetectionConversionOutput, DetectionEventRecord, DetectorErrorModel,
    dem::MAX_DEM_REPEAT_NESTING,
};

const MAX_DEM_SAMPLER_BUFFER_UNITS: usize = 64_000_000;
const MAX_DEM_SAMPLER_BUFFER_BYTES: usize = 64 * 1024 * 1024;
const MAX_DEM_SAMPLER_SAMPLE_ERROR_APPLICATIONS: usize = 64_000_000;

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledDemSampler {
    detector_count: usize,
    observable_count: usize,
    operations: DemSampleBlock,
}

impl CompiledDemSampler {
    pub fn compile(model: &DetectorErrorModel) -> CircuitResult<Self> {
        let operations = compile_items(model.items(), 0)?;
        let detector_count = usize_from_u64(model.count_detectors()?, "detector count")?;
        let observable_count = usize_from_u64(model.count_observables()?, "observable count")?;
        Ok(Self {
            detector_count,
            observable_count,
            operations,
        })
    }

    pub fn sample_detection_events(
        &self,
        shots: usize,
    ) -> CircuitResult<DetectionConversionOutput> {
        self.sample_detection_events_with_seed(shots, None)
    }

    pub fn sample_detection_events_with_seed(
        &self,
        shots: usize,
        seed: Option<u64>,
    ) -> CircuitResult<DetectionConversionOutput> {
        self.validate_sample_buffer_units(shots, false)?;
        let mut records = Vec::with_capacity(shots);
        self.try_for_each_detection_event_with_seed(shots, seed, |record| {
            records.push(record.clone());
            Ok::<(), CircuitError>(())
        })?;
        Ok(DetectionConversionOutput {
            records,
            detector_count: self.detector_count,
            observable_count: self.observable_count,
        })
    }

    pub fn error_count(&self) -> usize {
        self.operations.error_count
    }

    pub fn validate_sample_buffer_units(
        &self,
        shots: usize,
        include_error_records: bool,
    ) -> CircuitResult<()> {
        let mut units_per_shot = self
            .detector_count
            .checked_add(self.observable_count)
            .ok_or_else(|| {
                CircuitError::invalid_sampler_compilation(
                    "DEM sampler output width overflowed while validating buffer size",
                )
            })?;
        if include_error_records {
            units_per_shot = units_per_shot.checked_add(self.error_count()).ok_or_else(|| {
                CircuitError::invalid_sampler_compilation(
                    "DEM sampler output and error width overflowed while validating buffer size",
                )
            })?;
        }
        let units_per_shot = units_per_shot.max(1);
        let total_units = shots.checked_mul(units_per_shot).ok_or_else(|| {
            CircuitError::invalid_sampler_compilation("DEM sampler buffer size overflowed")
        })?;
        if total_units > MAX_DEM_SAMPLER_BUFFER_UNITS {
            return Err(CircuitError::invalid_sampler_compilation(format!(
                "DEM sampler would require {total_units} buffered units; current limit is {MAX_DEM_SAMPLER_BUFFER_UNITS}"
            )));
        }
        let bytes_per_shot = self.materialized_bytes_per_shot(include_error_records)?;
        let total_bytes = shots.checked_mul(bytes_per_shot).ok_or_else(|| {
            CircuitError::invalid_sampler_compilation("DEM sampler buffer byte size overflowed")
        })?;
        if total_bytes > MAX_DEM_SAMPLER_BUFFER_BYTES {
            return Err(CircuitError::invalid_sampler_compilation(format!(
                "DEM sampler would require at least {total_bytes} materialized bytes; current limit is {MAX_DEM_SAMPLER_BUFFER_BYTES}"
            )));
        }
        Ok(())
    }

    fn materialized_bytes_per_shot(&self, include_error_records: bool) -> CircuitResult<usize> {
        let detector_observable_bytes = self
            .detector_count
            .checked_add(self.observable_count)
            .ok_or_else(|| {
                CircuitError::invalid_sampler_compilation(
                    "DEM sampler output width overflowed while validating buffer bytes",
                )
            })?;
        let mut bytes = std::mem::size_of::<DetectionEventRecord>()
            .checked_add(detector_observable_bytes)
            .ok_or_else(|| {
                CircuitError::invalid_sampler_compilation(
                    "DEM sampler per-shot output byte size overflowed",
                )
            })?;
        if include_error_records {
            bytes = bytes
                .checked_add(std::mem::size_of::<Vec<bool>>())
                .and_then(|value| value.checked_add(self.error_count()))
                .ok_or_else(|| {
                    CircuitError::invalid_sampler_compilation(
                        "DEM sampler per-shot error byte size overflowed",
                    )
                })?;
        }
        Ok(bytes.max(1))
    }

    fn validate_detector_sample_work_units(&self, shots: usize) -> CircuitResult<()> {
        self.validate_sample_work_units(shots, self.operations.direct_sample_work_count)
    }

    fn validate_sampled_error_work_units(&self, shots: usize) -> CircuitResult<()> {
        self.validate_sample_work_units(shots, self.error_count())
    }

    fn validate_sample_work_units(
        &self,
        shots: usize,
        error_applications_per_shot: usize,
    ) -> CircuitResult<()> {
        if error_applications_per_shot == 0 || shots == 0 {
            return Ok(());
        }
        let work_units = shots
            .checked_mul(error_applications_per_shot)
            .ok_or_else(|| {
                CircuitError::invalid_sampler_compilation("DEM sampler sample work overflowed")
            })?;
        if work_units > MAX_DEM_SAMPLER_SAMPLE_ERROR_APPLICATIONS {
            return Err(CircuitError::invalid_sampler_compilation(format!(
                "DEM sampler would apply {work_units} sampled errors; current limit is {MAX_DEM_SAMPLER_SAMPLE_ERROR_APPLICATIONS}"
            )));
        }
        Ok(())
    }

    pub fn sample_detection_events_and_errors_with_seed(
        &self,
        shots: usize,
        seed: Option<u64>,
    ) -> CircuitResult<(DetectionConversionOutput, Vec<Vec<bool>>)> {
        self.validate_sample_buffer_units(shots, true)?;
        let mut records = Vec::with_capacity(shots);
        let mut error_records = Vec::with_capacity(shots);
        self.try_for_each_detection_event_and_error_with_seed(
            shots,
            seed,
            |record, error_record| {
                records.push(record.clone());
                error_records.push(error_record.to_vec());
                Ok::<(), CircuitError>(())
            },
        )?;
        Ok((
            DetectionConversionOutput {
                records,
                detector_count: self.detector_count,
                observable_count: self.observable_count,
            },
            error_records,
        ))
    }

    pub fn sample_detection_events_from_error_records(
        &self,
        error_records: &[Vec<bool>],
    ) -> CircuitResult<DetectionConversionOutput> {
        self.validate_sample_buffer_units(error_records.len(), true)?;
        let mut records = Vec::with_capacity(error_records.len());
        self.try_for_each_detection_event_from_error_records(
            error_records.iter().map(Vec::as_slice),
            |record, _error_record| {
                records.push(record.clone());
                Ok::<(), CircuitError>(())
            },
        )?;
        Ok(DetectionConversionOutput {
            records,
            detector_count: self.detector_count,
            observable_count: self.observable_count,
        })
    }

    pub fn try_for_each_detection_event_with_seed<E, F>(
        &self,
        shots: usize,
        seed: Option<u64>,
        mut visit: F,
    ) -> Result<(), E>
    where
        E: From<CircuitError>,
        F: FnMut(&DetectionEventRecord) -> Result<(), E>,
    {
        self.validate_detector_sample_work_units(shots)?;
        let mut rng = dem_sampler_rng(seed);
        let mut record = self.reusable_detection_record();
        for _ in 0..shots {
            self.sample_detection_record_into(&mut rng, &mut record)?;
            visit(&record)?;
        }
        Ok(())
    }

    pub fn try_for_each_detection_event_and_error_with_seed<E, F>(
        &self,
        shots: usize,
        seed: Option<u64>,
        mut visit: F,
    ) -> Result<(), E>
    where
        E: From<CircuitError>,
        F: FnMut(&DetectionEventRecord, &[bool]) -> Result<(), E>,
    {
        self.validate_sample_buffer_units(1, true)?;
        self.validate_sampled_error_work_units(shots)?;
        let mut rng = dem_sampler_rng(seed);
        let mut error_record = Vec::with_capacity(self.error_count());
        let mut record = self.reusable_detection_record();
        for _ in 0..shots {
            self.sample_detection_record_and_error_record_into(
                &mut rng,
                &mut record,
                &mut error_record,
            )?;
            visit(&record, &error_record)?;
        }
        Ok(())
    }

    pub fn try_for_each_detection_event_from_error_records<'a, E, I, F>(
        &self,
        error_records: I,
        mut visit: F,
    ) -> Result<(), E>
    where
        E: From<CircuitError>,
        I: IntoIterator<Item = &'a [bool]>,
        F: FnMut(&DetectionEventRecord, &[bool]) -> Result<(), E>,
    {
        let mut record = self.reusable_detection_record();
        for (shot_index, error_record) in error_records.into_iter().enumerate() {
            self.validate_error_record_width(error_record, Some(shot_index))?;
            self.detection_record_from_error_record_into(error_record, &mut record)?;
            visit(&record, error_record)?;
        }
        Ok(())
    }

    fn reusable_detection_record(&self) -> DetectionEventRecord {
        DetectionEventRecord {
            detectors: vec![false; self.detector_count],
            observables: vec![false; self.observable_count],
        }
    }

    fn sample_detection_record_into<R>(
        &self,
        rng: &mut R,
        record: &mut DetectionEventRecord,
    ) -> CircuitResult<()>
    where
        R: Rng,
    {
        reset_detection_record(record, self.detector_count, self.observable_count);
        sample_block_into(
            &self.operations,
            0,
            rng,
            record,
            SampledErrorOutput::Discard,
        )
    }

    fn sample_detection_record_and_error_record_into<R>(
        &self,
        rng: &mut R,
        record: &mut DetectionEventRecord,
        error_record: &mut Vec<bool>,
    ) -> CircuitResult<()>
    where
        R: Rng,
    {
        reset_detection_record(record, self.detector_count, self.observable_count);
        error_record.clear();
        sample_block_into(
            &self.operations,
            0,
            rng,
            record,
            SampledErrorOutput::Record(error_record),
        )
    }

    fn validate_error_record_width(
        &self,
        error_record: &[bool],
        shot_index: Option<usize>,
    ) -> CircuitResult<()> {
        if error_record.len() == self.error_count() {
            return Ok(());
        }
        if let Some(shot_index) = shot_index {
            return Err(CircuitError::invalid_result_format(format!(
                "DEM error record {shot_index} expected {} bits, got {}",
                self.error_count(),
                error_record.len()
            )));
        }
        Err(CircuitError::invalid_result_format(format!(
            "DEM error record expected {} bits, got {}",
            self.error_count(),
            error_record.len()
        )))
    }

    fn detection_record_from_error_record_into(
        &self,
        error_record: &[bool],
        record: &mut DetectionEventRecord,
    ) -> CircuitResult<()> {
        self.validate_error_record_width(error_record, None)?;
        reset_detection_record(record, self.detector_count, self.observable_count);
        let mut cursor = 0;
        apply_error_record_block(&self.operations, 0, error_record, &mut cursor, record)?;
        if cursor != error_record.len() {
            return Err(CircuitError::invalid_result_format(
                "DEM error record had unused trailing bits",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
struct DemSampleBlock {
    operations: Vec<DemSampleOperation>,
    detector_shift: u64,
    error_count: usize,
    direct_sample_effect_count: usize,
    direct_sample_work_count: usize,
    direct_sample_has_stochastic_error: bool,
}

#[derive(Clone, Debug, PartialEq)]
enum DemSampleOperation {
    Error(DemSampleError),
    Repeat(DemSampleRepeat),
}

#[derive(Clone, Debug, PartialEq)]
struct DemSampleError {
    probability: f64,
    detectors: Vec<u64>,
    observables: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq)]
struct DemSampleRepeat {
    start_detector_shift: u64,
    repeat_count: u64,
    body: DemSampleBlock,
}

impl DemSampleError {
    fn sample_occurs<R>(&self, rng: &mut R) -> bool
    where
        R: Rng,
    {
        if self.probability <= 0.0 {
            return false;
        }
        if self.probability >= 1.0 {
            return true;
        }
        rng.random::<f64>() < self.probability
    }
}

fn compile_items(items: &[DemItem], depth: usize) -> CircuitResult<DemSampleBlock> {
    if depth > MAX_DEM_REPEAT_NESTING {
        return Err(CircuitError::invalid_sampler_compilation(format!(
            "DEM repeat nesting exceeds current limit {MAX_DEM_REPEAT_NESTING}"
        )));
    }
    let mut block = DemSampleBlock::default();
    let mut current_shift = 0;
    for item in items {
        match item {
            DemItem::Instruction(instruction) => {
                compile_instruction(instruction, current_shift, &mut block)?;
                if instruction.kind() == DemInstructionKind::ShiftDetectors {
                    current_shift = current_shift
                        .checked_add(instruction.detector_shift()?)
                        .ok_or_else(|| {
                            CircuitError::invalid_sampler_compilation(
                                "DEM sampler detector shift overflowed",
                            )
                        })?;
                }
            }
            DemItem::RepeatBlock(repeat) => {
                let repeat_count = repeat.repeat_count().get();
                let body = compile_items(repeat.body().items(), depth + 1)?;
                let repeat_start_shift = current_shift;
                let repeated_shift =
                    body.detector_shift
                        .checked_mul(repeat_count)
                        .ok_or_else(|| {
                            CircuitError::invalid_sampler_compilation(
                                "DEM sampler repeat detector shift overflowed",
                            )
                        })?;
                current_shift = current_shift.checked_add(repeated_shift).ok_or_else(|| {
                    CircuitError::invalid_sampler_compilation(
                        "DEM sampler detector shift overflowed",
                    )
                })?;
                block.error_count = checked_repeated_count(
                    block.error_count,
                    body.error_count,
                    repeat_count,
                    "DEM sampler error count",
                )?;
                block.direct_sample_effect_count = checked_repeated_count(
                    block.direct_sample_effect_count,
                    body.direct_sample_effect_count,
                    repeat_count,
                    "DEM sampler direct sample effect count",
                )?;
                block.direct_sample_has_stochastic_error |= body.direct_sample_has_stochastic_error;
                let repeated_work = folded_direct_sample_repeat_work_count(&body, repeat_count)?;
                block.direct_sample_work_count = block
                    .direct_sample_work_count
                    .checked_add(repeated_work)
                    .ok_or_else(|| {
                        CircuitError::invalid_sampler_compilation(
                            "DEM sampler direct sample work count overflowed",
                        )
                    })?;
                block
                    .operations
                    .push(DemSampleOperation::Repeat(DemSampleRepeat {
                        start_detector_shift: repeat_start_shift,
                        repeat_count,
                        body,
                    }));
            }
        }
    }
    block.detector_shift = current_shift;
    Ok(block)
}

fn compile_instruction(
    instruction: &DemInstruction,
    detector_shift: u64,
    block: &mut DemSampleBlock,
) -> CircuitResult<()> {
    if instruction.kind() != DemInstructionKind::Error {
        return Ok(());
    }
    let probability =
        instruction.args().first().copied().ok_or_else(|| {
            CircuitError::invalid_sampler_compilation("error is missing probability")
        })?;
    let mut operation = DemSampleError {
        probability,
        detectors: Vec::new(),
        observables: Vec::new(),
    };
    for target in instruction.targets() {
        match target {
            DemTarget::RelativeDetector(detector) => {
                let shifted = detector_shift
                    .checked_add(detector.get())
                    .ok_or_else(|| detector_index_overflow_error("detector"))?;
                operation.detectors.push(shifted);
            }
            DemTarget::LogicalObservable(observable) => {
                operation
                    .observables
                    .push(usize_from_u64(observable.get(), "observable index")?);
            }
            DemTarget::Separator => {}
            DemTarget::Numeric(_) => {
                return Err(CircuitError::invalid_sampler_compilation(
                    "error targets cannot include numeric DEM targets",
                ));
            }
        }
    }
    block.error_count = block.error_count.checked_add(1).ok_or_else(|| {
        CircuitError::invalid_sampler_compilation("DEM sampler error count overflowed")
    })?;
    block.direct_sample_work_count =
        block
            .direct_sample_work_count
            .checked_add(1)
            .ok_or_else(|| {
                CircuitError::invalid_sampler_compilation(
                    "DEM sampler direct sample work count overflowed",
                )
            })?;
    if probability > 0.0 {
        block.direct_sample_effect_count = block
            .direct_sample_effect_count
            .checked_add(1)
            .ok_or_else(|| {
                CircuitError::invalid_sampler_compilation(
                    "DEM sampler direct sample effect count overflowed",
                )
            })?;
    }
    if probability > 0.0 && probability < 1.0 {
        block.direct_sample_has_stochastic_error = true;
    }
    block.operations.push(DemSampleOperation::Error(operation));
    Ok(())
}

enum SampledErrorOutput<'a> {
    Discard,
    Record(&'a mut Vec<bool>),
}

impl SampledErrorOutput<'_> {
    fn is_discard(&self) -> bool {
        matches!(self, Self::Discard)
    }
}

fn sample_block_into<R>(
    block: &DemSampleBlock,
    detector_shift: u64,
    rng: &mut R,
    record: &mut DetectionEventRecord,
    mut error_output: SampledErrorOutput<'_>,
) -> CircuitResult<()>
where
    R: Rng,
{
    for operation in &block.operations {
        match operation {
            DemSampleOperation::Error(error) => {
                let occurred = error.sample_occurs(rng);
                if let SampledErrorOutput::Record(error_record) = &mut error_output {
                    error_record.push(occurred);
                }
                if occurred {
                    apply_error_to_record(error, detector_shift, record)?;
                }
            }
            DemSampleOperation::Repeat(repeat) => {
                if error_output.is_discard() {
                    if repeat.body.direct_sample_effect_count == 0 {
                        continue;
                    }
                    if repeat.body.detector_shift == 0
                        && !repeat.body.direct_sample_has_stochastic_error
                    {
                        if repeat.repeat_count.is_multiple_of(2) {
                            continue;
                        }
                        let iteration_shift = detector_shift
                            .checked_add(repeat.start_detector_shift)
                            .ok_or_else(detector_shift_overflow_error)?;
                        sample_block_into(
                            &repeat.body,
                            iteration_shift,
                            rng,
                            record,
                            SampledErrorOutput::Discard,
                        )?;
                        continue;
                    }
                    if let Some(error) = single_stochastic_zero_shift_error(&repeat.body) {
                        let probability =
                            odd_parity_probability(error.probability, repeat.repeat_count);
                        let occurred = sample_probability(probability, rng);
                        if occurred {
                            let iteration_shift = detector_shift
                                .checked_add(repeat.start_detector_shift)
                                .ok_or_else(detector_shift_overflow_error)?;
                            apply_error_to_record(error, iteration_shift, record)?;
                        }
                        continue;
                    }
                }
                let mut iteration_shift = detector_shift
                    .checked_add(repeat.start_detector_shift)
                    .ok_or_else(detector_shift_overflow_error)?;
                for _ in 0..repeat.repeat_count {
                    sample_block_into(
                        &repeat.body,
                        iteration_shift,
                        rng,
                        record,
                        match &mut error_output {
                            SampledErrorOutput::Discard => SampledErrorOutput::Discard,
                            SampledErrorOutput::Record(error_record) => {
                                SampledErrorOutput::Record(error_record)
                            }
                        },
                    )?;
                    iteration_shift = iteration_shift
                        .checked_add(repeat.body.detector_shift)
                        .ok_or_else(detector_shift_overflow_error)?;
                }
            }
        }
    }
    Ok(())
}

fn apply_error_record_block(
    block: &DemSampleBlock,
    detector_shift: u64,
    error_record: &[bool],
    cursor: &mut usize,
    record: &mut DetectionEventRecord,
) -> CircuitResult<()> {
    for operation in &block.operations {
        match operation {
            DemSampleOperation::Error(error) => {
                let occurred = *error_record.get(*cursor).ok_or_else(|| {
                    CircuitError::invalid_result_format("DEM error record ended early")
                })?;
                *cursor = cursor.checked_add(1).ok_or_else(|| {
                    CircuitError::invalid_result_format("DEM error record cursor overflowed")
                })?;
                if occurred {
                    apply_error_to_record(error, detector_shift, record)?;
                }
            }
            DemSampleOperation::Repeat(repeat) => {
                let mut iteration_shift = detector_shift
                    .checked_add(repeat.start_detector_shift)
                    .ok_or_else(detector_shift_overflow_error)?;
                for _ in 0..repeat.repeat_count {
                    apply_error_record_block(
                        &repeat.body,
                        iteration_shift,
                        error_record,
                        cursor,
                        record,
                    )?;
                    iteration_shift = iteration_shift
                        .checked_add(repeat.body.detector_shift)
                        .ok_or_else(detector_shift_overflow_error)?;
                }
            }
        }
    }
    Ok(())
}

fn apply_error_to_record(
    error: &DemSampleError,
    detector_shift: u64,
    record: &mut DetectionEventRecord,
) -> CircuitResult<()> {
    for detector in &error.detectors {
        let shifted = detector_shift
            .checked_add(*detector)
            .ok_or_else(|| detector_index_overflow_error("detector"))?;
        toggle_bit(
            &mut record.detectors,
            usize_from_u64(shifted, "detector index")?,
            "detector",
        )?;
    }
    for observable in &error.observables {
        toggle_bit(&mut record.observables, *observable, "observable")?;
    }
    Ok(())
}

fn checked_repeated_count(
    current: usize,
    body_count: usize,
    repeat_count: u64,
    kind: &'static str,
) -> CircuitResult<usize> {
    let repeat_count = usize_from_u64(repeat_count, "DEM sampler repeat count")?;
    let repeated = body_count.checked_mul(repeat_count).ok_or_else(|| {
        CircuitError::invalid_sampler_compilation(format!("repeated {kind} overflowed"))
    })?;
    current
        .checked_add(repeated)
        .ok_or_else(|| CircuitError::invalid_sampler_compilation(format!("{kind} overflowed")))
}

fn folded_direct_sample_repeat_work_count(
    body: &DemSampleBlock,
    repeat_count: u64,
) -> CircuitResult<usize> {
    if body.direct_sample_effect_count == 0 {
        return Ok(0);
    }
    if body.detector_shift == 0 && !body.direct_sample_has_stochastic_error {
        if repeat_count.is_multiple_of(2) {
            return Ok(0);
        }
        return Ok(body.direct_sample_work_count);
    }
    if single_stochastic_zero_shift_error(body).is_some() {
        return Ok(1);
    }
    checked_repeated_count(
        0,
        body.direct_sample_work_count,
        repeat_count,
        "DEM sampler direct sample work count",
    )
}

fn single_stochastic_zero_shift_error(block: &DemSampleBlock) -> Option<&DemSampleError> {
    if block.detector_shift != 0 || block.operations.len() != 1 {
        return None;
    }
    let Some(DemSampleOperation::Error(error)) = block.operations.first() else {
        return None;
    };
    (error.probability > 0.0 && error.probability < 1.0).then_some(error)
}

fn odd_parity_probability(probability: f64, repeat_count: u64) -> f64 {
    if repeat_count == 0 || probability <= 0.0 {
        return 0.0;
    }
    if probability >= 1.0 {
        return if repeat_count.is_multiple_of(2) {
            0.0
        } else {
            1.0
        };
    }
    if probability == 0.5 {
        return 0.5;
    }

    if probability < 0.5 {
        let log_bias = (repeat_count as f64) * (-2.0 * probability).ln_1p();
        return (-0.5 * log_bias.exp_m1()).clamp(0.0, 0.5);
    }

    let complement = 1.0 - probability;
    let log_magnitude = (repeat_count as f64) * (-2.0 * complement).ln_1p();
    if repeat_count.is_multiple_of(2) {
        (-0.5 * log_magnitude.exp_m1()).clamp(0.0, 0.5)
    } else {
        (1.0 + 0.5 * log_magnitude.exp_m1()).clamp(0.5, 1.0)
    }
}

fn sample_probability<R>(probability: f64, rng: &mut R) -> bool
where
    R: Rng,
{
    if probability <= 0.0 {
        return false;
    }
    if probability >= 1.0 {
        return true;
    }
    rng.random::<f64>() < probability
}

fn reset_detection_record(
    record: &mut DetectionEventRecord,
    detector_count: usize,
    observable_count: usize,
) {
    record.detectors.clear();
    record.detectors.resize(detector_count, false);
    record.observables.clear();
    record.observables.resize(observable_count, false);
}

fn toggle_bit(bits: &mut [bool], index: usize, kind: &'static str) -> CircuitResult<()> {
    let bit = bits.get_mut(index).ok_or_else(|| {
        CircuitError::invalid_sampler_compilation(format!("{kind} index {index} is out of range"))
    })?;
    *bit = !*bit;
    Ok(())
}

fn usize_from_u64(value: u64, kind: &'static str) -> CircuitResult<usize> {
    usize::try_from(value).map_err(|_| {
        CircuitError::invalid_sampler_compilation(format!("{kind} {value} does not fit in usize"))
    })
}

fn detector_index_overflow_error(kind: &'static str) -> CircuitError {
    CircuitError::invalid_sampler_compilation(format!("{kind} index overflowed"))
}

fn detector_shift_overflow_error() -> CircuitError {
    CircuitError::invalid_sampler_compilation("DEM sampler detector shift overflowed")
}

fn dem_sampler_rng(seed: Option<u64>) -> SmallRng {
    SmallRng::seed_from_u64(seed.unwrap_or_else(rand::random))
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        reason = "DEM sampler tests use direct fixture assertions for compact diagnostics"
    )]

    use super::*;

    fn collect_streamed_samples(
        sampler: &CompiledDemSampler,
        shots: usize,
        seed: Option<u64>,
    ) -> CircuitResult<(Vec<DetectionEventRecord>, Vec<Vec<bool>>)> {
        let mut records = Vec::new();
        let mut errors = Vec::new();
        sampler.try_for_each_detection_event_and_error_with_seed(
            shots,
            seed,
            |record, error_record| {
                records.push(record.clone());
                errors.push(error_record.to_vec());
                Ok::<(), CircuitError>(())
            },
        )?;
        Ok((records, errors))
    }

    #[test]
    fn odd_parity_probability_matches_repeated_independent_error_parity() {
        assert_eq!(odd_parity_probability(0.0, 1_000_000), 0.0);
        assert_eq!(odd_parity_probability(1.0, 4), 0.0);
        assert_eq!(odd_parity_probability(1.0, 5), 1.0);
        assert!((odd_parity_probability(0.25, 2) - 0.375).abs() < 1e-12);
        assert!((odd_parity_probability(0.5, 64_000_001) - 0.5).abs() < 1e-12);

        let tiny_probability = odd_parity_probability(1e-18, 1_000_000_000_000_000_000);
        let expected_tiny_probability = -0.5 * (-2.0_f64).exp_m1();
        assert!((tiny_probability - expected_tiny_probability).abs() < 1e-12);

        let near_one = 1.0 - 1e-12;
        let near_one_probability = odd_parity_probability(near_one, 1_000_000_000_001);
        let expected_near_one_probability =
            1.0 - odd_parity_probability(1.0 - near_one, 1_000_000_000_001);
        assert!((near_one_probability - expected_near_one_probability).abs() < 1e-12);
    }

    #[test]
    fn dem_streaming_samples_match_materialized_seeded_samples() {
        for dem_text in [
            "error(1) D0\n",
            "error(0.25) D0\n",
            "error(0.25) L2\n",
            "error(0.25) D0 D2\nerror(0.25) D2 D3\n",
            "error(0.25) D0\nshift_detectors 1\nrepeat 2 {\n    error(0.25) D0\n    shift_detectors 1\n}\nerror(0) D0\n",
        ] {
            let model = DetectorErrorModel::from_dem_str(dem_text).expect("parse DEM");
            let sampler = CompiledDemSampler::compile(&model).expect("compile DEM sampler");
            let (materialized, materialized_errors) = sampler
                .sample_detection_events_and_errors_with_seed(65, Some(7))
                .expect("materialized samples");
            let (streamed, streamed_errors) =
                collect_streamed_samples(&sampler, 65, Some(7)).expect("streamed samples");

            assert_eq!(streamed, materialized.records);
            assert_eq!(streamed_errors, materialized_errors);
            let replayed = sampler
                .sample_detection_events_from_error_records(&streamed_errors)
                .expect("materialized replay");
            let mut streamed_replay = Vec::new();
            sampler
                .try_for_each_detection_event_from_error_records(
                    streamed_errors.iter().map(Vec::as_slice),
                    |record, _error_record| {
                        streamed_replay.push(record.clone());
                        Ok::<(), CircuitError>(())
                    },
                )
                .expect("streamed replay");
            assert_eq!(streamed_replay, replayed.records);
        }
    }
}
