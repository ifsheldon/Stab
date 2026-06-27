use crate::{
    Circuit, CircuitError, CircuitInstruction, CircuitItem, CircuitResult, CompiledSampler,
    RepeatBlock, SampleFormat,
    result_formats::{MeasureRecordWriter, write_ptb64_records_checked},
};

const MAX_DETECTION_RECORD_BITS: usize = 1_000_000;
const MAX_DETECTION_BUFFER_BITS: usize = 64_000_000;
const MAX_DETECTION_REPEAT_UNROLL: u64 = 100_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DetectionConversionOptions {
    pub skip_reference_sample: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DetectionEventRecord {
    pub detectors: Vec<bool>,
    pub observables: Vec<bool>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DetectionConversionOutput {
    pub records: Vec<DetectionEventRecord>,
    pub detector_count: usize,
    pub observable_count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DetectionObservableOutputMode {
    DetectorsOnly,
    Append,
    Prepend,
}

pub fn convert_measurements_to_detection_events(
    circuit: &Circuit,
    measurements: &[Vec<bool>],
    options: DetectionConversionOptions,
) -> CircuitResult<DetectionConversionOutput> {
    let plan = ConversionPlan::from_circuit(circuit)?;
    plan.validate_shot_count(measurements.len())?;
    let reference_sample = if options.skip_reference_sample {
        vec![false; plan.measurement_count]
    } else {
        reference_sample(circuit, plan.measurement_count)?
    };

    let mut records = Vec::with_capacity(measurements.len());
    convert_measurements_with_plan(&plan, measurements, &reference_sample, &mut records)?;

    Ok(DetectionConversionOutput {
        records,
        detector_count: plan.detector_terms.len(),
        observable_count: plan.observable_terms.len(),
    })
}

fn convert_measurements_with_plan(
    plan: &ConversionPlan,
    measurements: &[Vec<bool>],
    reference_sample: &[bool],
    records: &mut Vec<DetectionEventRecord>,
) -> CircuitResult<()> {
    for (shot_index, measurement_record) in measurements.iter().enumerate() {
        if measurement_record.len() != plan.measurement_count {
            return Err(CircuitError::invalid_result_format(format!(
                "measurement record {shot_index} expected {} bits, got {}",
                plan.measurement_count,
                measurement_record.len()
            )));
        }
        records.push(plan.convert_record(measurement_record, reference_sample)?);
    }
    Ok(())
}

pub fn sample_detection_events(
    circuit: &Circuit,
    shots: usize,
    seed: Option<u64>,
) -> CircuitResult<DetectionConversionOutput> {
    validate_detection_sampling_circuit(circuit)?;
    let plan = ConversionPlan::from_circuit(circuit)?;
    plan.validate_shot_count(shots)?;
    let sampler = CompiledSampler::compile(circuit)?;
    let reference_sample = sampler.reference_sample();
    validate_reference_sample_len(&reference_sample, plan.measurement_count)?;
    let measurements = sampler.sample_zero_one_with_seed(shots, seed);
    let mut records = Vec::with_capacity(measurements.len());
    convert_measurements_with_plan(&plan, &measurements, &reference_sample, &mut records)?;
    Ok(DetectionConversionOutput {
        records,
        detector_count: plan.detector_terms.len(),
        observable_count: plan.observable_terms.len(),
    })
}

pub fn measurement_record_count(circuit: &Circuit) -> CircuitResult<usize> {
    Ok(ConversionPlan::from_circuit(circuit)?.measurement_count)
}

pub fn detection_record_width(circuit: &Circuit) -> CircuitResult<usize> {
    ConversionPlan::from_circuit(circuit)?.output_bit_count()
}

pub fn validate_detection_sampling_circuit(circuit: &Circuit) -> CircuitResult<()> {
    validate_sampling_observables(circuit)
}

pub fn write_detection_records(
    output: &DetectionConversionOutput,
    observable_mode: DetectionObservableOutputMode,
    format: SampleFormat,
) -> CircuitResult<Vec<u8>> {
    let mut writer = MeasureRecordWriter::new(format);
    for record in &output.records {
        validate_record_widths(output, record)?;
        if format == SampleFormat::Dets {
            if observable_mode == DetectionObservableOutputMode::Prepend {
                writer.begin_result_type(b'L');
                writer.write_bits(&record.observables);
            }
            writer.begin_result_type(b'D');
            writer.write_bits(&record.detectors);
            if observable_mode == DetectionObservableOutputMode::Append {
                writer.begin_result_type(b'L');
                writer.write_bits(&record.observables);
            }
        } else {
            if observable_mode == DetectionObservableOutputMode::Prepend {
                writer.write_bits(&record.observables);
            }
            writer.write_bits(&record.detectors);
            if observable_mode == DetectionObservableOutputMode::Append {
                writer.write_bits(&record.observables);
            }
        }
        writer.write_end();
    }
    Ok(writer.into_bytes())
}

pub fn write_observable_records(
    output: &DetectionConversionOutput,
    format: SampleFormat,
) -> CircuitResult<Vec<u8>> {
    let mut writer = MeasureRecordWriter::new(format);
    for record in &output.records {
        validate_record_widths(output, record)?;
        if format == SampleFormat::Dets {
            writer.begin_result_type(b'L');
        }
        writer.write_bits(&record.observables);
        writer.write_end();
    }
    Ok(writer.into_bytes())
}

pub fn write_ptb64_detection_records(
    output: &DetectionConversionOutput,
    observable_mode: DetectionObservableOutputMode,
) -> CircuitResult<Vec<u8>> {
    let records = detection_records_as_bits(output, observable_mode)?;
    write_ptb64_records_checked(&records)
}

pub fn write_ptb64_observable_records(
    output: &DetectionConversionOutput,
) -> CircuitResult<Vec<u8>> {
    let records = observable_records_as_bits(output)?;
    write_ptb64_records_checked(&records)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ConversionPlan {
    measurement_count: usize,
    detector_terms: Vec<Vec<usize>>,
    observable_terms: Vec<Vec<usize>>,
}

impl ConversionPlan {
    fn from_circuit(circuit: &Circuit) -> CircuitResult<Self> {
        let mut plan = Self {
            measurement_count: 0,
            detector_terms: Vec::new(),
            observable_terms: Vec::new(),
        };
        plan.visit_circuit(circuit)?;
        Ok(plan)
    }

    fn visit_circuit(&mut self, circuit: &Circuit) -> CircuitResult<()> {
        for item in circuit.items() {
            match item {
                CircuitItem::Instruction(instruction) => self.visit_instruction(instruction)?,
                CircuitItem::RepeatBlock(repeat) => self.visit_repeat(repeat)?,
            }
        }
        Ok(())
    }

    fn visit_repeat(&mut self, repeat: &RepeatBlock) -> CircuitResult<()> {
        let repeat_count = repeat.repeat_count().get();
        if repeat_count > MAX_DETECTION_REPEAT_UNROLL {
            return Err(CircuitError::invalid_sampler_compilation(format!(
                "detection conversion currently supports repeat counts up to {MAX_DETECTION_REPEAT_UNROLL}, got {repeat_count}"
            )));
        }
        for _ in 0..repeat.repeat_count().get() {
            self.visit_circuit(repeat.body())?;
        }
        Ok(())
    }

    fn visit_instruction(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        match instruction.gate().canonical_name() {
            "DETECTOR" => self.record_detector(instruction),
            "OBSERVABLE_INCLUDE" => self.record_observable(instruction),
            _ => self.add_measurements(instruction),
        }
    }

    fn record_detector(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        let terms = instruction
            .targets()
            .iter()
            .map(|target| {
                target
                    .measurement_record_offset()
                    .ok_or_else(|| {
                        CircuitError::invalid_result_format(format!(
                            "DETECTOR target {target} is not a measurement record"
                        ))
                    })
                    .and_then(|offset| self.measurement_index_from_offset(offset.get()))
            })
            .collect::<CircuitResult<Vec<_>>>()?;
        self.detector_terms.push(terms);
        self.validate_record_width()?;
        Ok(())
    }

    fn record_observable(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        let observable_id = instruction
            .observable_id_argument()?
            .ok_or_else(|| CircuitError::invalid_result_format("OBSERVABLE_INCLUDE missing id"))?;
        let observable_id = usize::try_from(observable_id.get()).map_err(|_| {
            CircuitError::invalid_result_format(format!(
                "observable id {} does not fit usize",
                observable_id.get()
            ))
        })?;
        self.ensure_observable(observable_id)?;

        let mut terms = Vec::new();
        for target in instruction.targets() {
            if let Some(offset) = target.measurement_record_offset() {
                terms.push(self.measurement_index_from_offset(offset.get())?);
            } else if target.is_pauli_target() {
                continue;
            } else {
                return Err(CircuitError::invalid_result_format(format!(
                    "OBSERVABLE_INCLUDE target {target} is not supported"
                )));
            }
        }
        let observable_terms = self
            .observable_terms
            .get_mut(observable_id)
            .ok_or_else(|| {
                CircuitError::invalid_result_format(format!(
                    "observable id {observable_id} was not initialized"
                ))
            })?;
        observable_terms.extend(terms);
        Ok(())
    }

    fn ensure_observable(&mut self, observable_id: usize) -> CircuitResult<()> {
        if observable_id >= MAX_DETECTION_RECORD_BITS {
            return Err(CircuitError::invalid_result_format(format!(
                "observable id {observable_id} exceeds current detection record limit {MAX_DETECTION_RECORD_BITS}"
            )));
        }
        while self.observable_terms.len() <= observable_id {
            self.observable_terms.push(Vec::new());
        }
        self.validate_record_width()
    }

    fn add_measurements(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        let measurement_count = instruction_measurement_count(instruction);
        self.measurement_count = self
            .measurement_count
            .checked_add(measurement_count)
            .ok_or_else(|| {
                CircuitError::invalid_result_format(
                    "measurement record count overflowed during detection conversion planning",
                )
            })?;
        if self.measurement_count > MAX_DETECTION_RECORD_BITS {
            return Err(CircuitError::invalid_result_format(format!(
                "measurement record width {} exceeds current detection conversion limit {MAX_DETECTION_RECORD_BITS}",
                self.measurement_count
            )));
        }
        Ok(())
    }

    fn output_bit_count(&self) -> CircuitResult<usize> {
        self.detector_terms
            .len()
            .checked_add(self.observable_terms.len())
            .ok_or_else(|| {
                CircuitError::invalid_result_format(
                    "detection record width overflowed while planning conversion",
                )
            })
    }

    fn validate_record_width(&self) -> CircuitResult<()> {
        let width = self.output_bit_count()?;
        if width > MAX_DETECTION_RECORD_BITS {
            return Err(CircuitError::invalid_result_format(format!(
                "detection record width {width} exceeds current limit {MAX_DETECTION_RECORD_BITS}"
            )));
        }
        Ok(())
    }

    fn validate_shot_count(&self, shots: usize) -> CircuitResult<()> {
        validate_buffer_bits("measurement samples", shots, self.measurement_count)?;
        validate_buffer_bits("detection records", shots, self.output_bit_count()?)
    }

    fn measurement_index_from_offset(&self, offset: i32) -> CircuitResult<usize> {
        let current = i64::try_from(self.measurement_count).map_err(|_| {
            CircuitError::invalid_result_format("measurement count does not fit i64")
        })?;
        let index = current
            .checked_add(i64::from(offset))
            .ok_or_else(|| CircuitError::invalid_result_format("measurement reference overflow"))?;
        let index = usize::try_from(index).map_err(|_| {
            CircuitError::invalid_result_format(format!(
                "measurement record target rec[{offset}] is not available"
            ))
        })?;
        if index >= self.measurement_count {
            return Err(CircuitError::invalid_result_format(format!(
                "measurement record target rec[{offset}] is not available"
            )));
        }
        Ok(index)
    }

    fn convert_record(
        &self,
        measurement_record: &[bool],
        reference_sample: &[bool],
    ) -> CircuitResult<DetectionEventRecord> {
        let detectors = self
            .detector_terms
            .iter()
            .map(|terms| parity_of_terms(terms, measurement_record, reference_sample))
            .collect::<CircuitResult<Vec<_>>>()?;
        let observables = self
            .observable_terms
            .iter()
            .map(|terms| parity_of_terms(terms, measurement_record, reference_sample))
            .collect::<CircuitResult<Vec<_>>>()?;
        Ok(DetectionEventRecord {
            detectors,
            observables,
        })
    }
}

fn reference_sample(circuit: &Circuit, measurement_count: usize) -> CircuitResult<Vec<bool>> {
    let reference_sample = CompiledSampler::compile(circuit)?.reference_sample();
    validate_reference_sample_len(&reference_sample, measurement_count)?;
    Ok(reference_sample)
}

fn validate_reference_sample_len(
    reference_sample: &[bool],
    measurement_count: usize,
) -> CircuitResult<()> {
    if reference_sample.len() == measurement_count {
        return Ok(());
    }
    Err(CircuitError::invalid_result_format(format!(
        "reference sample has {} measurement bits but detection conversion expected {measurement_count}",
        reference_sample.len()
    )))
}

fn validate_buffer_bits(kind: &str, shots: usize, bits_per_shot: usize) -> CircuitResult<()> {
    let total = shots.checked_mul(bits_per_shot).ok_or_else(|| {
        CircuitError::invalid_result_format(format!("{kind} bit count overflowed"))
    })?;
    if total > MAX_DETECTION_BUFFER_BITS {
        return Err(CircuitError::invalid_result_format(format!(
            "{kind} would require {total} buffered bits; current limit is {MAX_DETECTION_BUFFER_BITS}"
        )));
    }
    Ok(())
}

fn detection_records_as_bits(
    output: &DetectionConversionOutput,
    observable_mode: DetectionObservableOutputMode,
) -> CircuitResult<Vec<Vec<bool>>> {
    output
        .records
        .iter()
        .map(|record| {
            validate_record_widths(output, record)?;
            let capacity = match observable_mode {
                DetectionObservableOutputMode::DetectorsOnly => output.detector_count,
                DetectionObservableOutputMode::Append | DetectionObservableOutputMode::Prepend => {
                    output
                        .detector_count
                        .checked_add(output.observable_count)
                        .ok_or_else(|| {
                            CircuitError::invalid_result_format(
                                "detection record width overflowed while writing ptb64 output",
                            )
                        })?
                }
            };
            let mut bits = Vec::with_capacity(capacity);
            if observable_mode == DetectionObservableOutputMode::Prepend {
                bits.extend_from_slice(&record.observables);
            }
            bits.extend_from_slice(&record.detectors);
            if observable_mode == DetectionObservableOutputMode::Append {
                bits.extend_from_slice(&record.observables);
            }
            Ok(bits)
        })
        .collect()
}

fn observable_records_as_bits(output: &DetectionConversionOutput) -> CircuitResult<Vec<Vec<bool>>> {
    output
        .records
        .iter()
        .map(|record| {
            validate_record_widths(output, record)?;
            Ok(record.observables.clone())
        })
        .collect()
}

fn validate_sampling_observables(circuit: &Circuit) -> CircuitResult<()> {
    for item in circuit.items() {
        match item {
            CircuitItem::Instruction(instruction)
                if instruction.gate().canonical_name() == "OBSERVABLE_INCLUDE"
                    && instruction
                        .targets()
                        .iter()
                        .any(crate::Target::is_pauli_target) =>
            {
                return Err(CircuitError::invalid_sampler_compilation(
                    "detect does not yet support OBSERVABLE_INCLUDE Pauli targets",
                ));
            }
            CircuitItem::Instruction(_) => {}
            CircuitItem::RepeatBlock(repeat) => validate_sampling_observables(repeat.body())?,
        }
    }
    Ok(())
}

fn instruction_measurement_count(instruction: &CircuitInstruction) -> usize {
    match instruction.gate().canonical_name() {
        "M"
        | "MX"
        | "MY"
        | "MR"
        | "MRX"
        | "MRY"
        | "MPAD"
        | "HERALDED_ERASE"
        | "HERALDED_PAULI_CHANNEL_1" => instruction.targets().len(),
        "MXX" | "MYY" | "MZZ" | "MPP" => instruction.target_groups().len(),
        _ => 0,
    }
}

fn parity_of_terms(
    terms: &[usize],
    measurement_record: &[bool],
    reference_sample: &[bool],
) -> CircuitResult<bool> {
    let mut parity = false;
    for index in terms {
        let measurement = measurement_record.get(*index).copied().ok_or_else(|| {
            CircuitError::invalid_result_format(format!(
                "measurement index {index} is out of range"
            ))
        })?;
        let reference = reference_sample.get(*index).copied().ok_or_else(|| {
            CircuitError::invalid_result_format(format!(
                "reference sample index {index} is out of range"
            ))
        })?;
        parity ^= measurement ^ reference;
    }
    Ok(parity)
}

fn validate_record_widths(
    output: &DetectionConversionOutput,
    record: &DetectionEventRecord,
) -> CircuitResult<()> {
    if record.detectors.len() != output.detector_count {
        return Err(CircuitError::invalid_result_format(format!(
            "detection record has {} detector bits but expected {}",
            record.detectors.len(),
            output.detector_count
        )));
    }
    if record.observables.len() != output.observable_count {
        return Err(CircuitError::invalid_result_format(format!(
            "detection record has {} observable bits but expected {}",
            record.observables.len(),
            output.observable_count
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        clippy::indexing_slicing,
        clippy::unwrap_used,
        reason = "detection tests use direct fixture assertions for compact diagnostics"
    )]

    use super::*;

    fn convert(
        circuit_text: &str,
        measurements: &[&[bool]],
        skip_reference_sample: bool,
    ) -> DetectionConversionOutput {
        let circuit = Circuit::from_stim_str(circuit_text).expect("parse circuit");
        let measurements = measurements
            .iter()
            .map(|record| record.to_vec())
            .collect::<Vec<_>>();
        convert_measurements_to_detection_events(
            &circuit,
            &measurements,
            DetectionConversionOptions {
                skip_reference_sample,
            },
        )
        .expect("convert measurements")
    }

    #[test]
    fn detection_conversion_uses_reference_sample_for_detectors_and_observables() {
        let output = convert(
            "X 0\nM 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(2) rec[-1]\n",
            &[
                &[false, false],
                &[false, true],
                &[true, false],
                &[true, true],
            ],
            false,
        );

        assert_eq!(output.detector_count, 2);
        assert_eq!(output.observable_count, 3);
        assert_eq!(
            output.records,
            vec![
                DetectionEventRecord {
                    detectors: vec![true, false],
                    observables: vec![false, false, false],
                },
                DetectionEventRecord {
                    detectors: vec![true, true],
                    observables: vec![false, false, true],
                },
                DetectionEventRecord {
                    detectors: vec![false, false],
                    observables: vec![false, false, false],
                },
                DetectionEventRecord {
                    detectors: vec![false, true],
                    observables: vec![false, false, true],
                },
            ],
        );
    }

    #[test]
    fn detection_conversion_can_skip_reference_sample() {
        let output = convert(
            "X 0\nM 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(2) rec[-1]\n",
            &[
                &[false, false],
                &[false, true],
                &[true, false],
                &[true, true],
            ],
            true,
        );

        assert_eq!(
            output.records,
            vec![
                DetectionEventRecord {
                    detectors: vec![false, false],
                    observables: vec![false, false, false],
                },
                DetectionEventRecord {
                    detectors: vec![false, true],
                    observables: vec![false, false, true],
                },
                DetectionEventRecord {
                    detectors: vec![true, false],
                    observables: vec![false, false, false],
                },
                DetectionEventRecord {
                    detectors: vec![true, true],
                    observables: vec![false, false, true],
                },
            ],
        );
    }

    #[test]
    fn detection_conversion_handles_repeats_coordinates_and_empty_detectors() {
        let output = convert(
            "M 0 !1\nSHIFT_COORDS(2, 3)\nDETECTOR(5) rec[-2]\nDETECTOR rec[-1]\nREPEAT 2 {\n    DETECTOR rec[-2] rec[-1]\n}\nDETECTOR\n",
            &[&[false, true]],
            true,
        );

        assert_eq!(
            output.records,
            vec![DetectionEventRecord {
                detectors: vec![false, true, true, true, false],
                observables: Vec::new(),
            }],
        );
    }

    #[test]
    fn detection_conversion_handles_empty_detector_circuits() {
        let output = convert("M 0\n", &[&[false], &[true]], true);

        assert_eq!(output.detector_count, 0);
        assert_eq!(
            output.records,
            vec![
                DetectionEventRecord {
                    detectors: Vec::new(),
                    observables: Vec::new(),
                },
                DetectionEventRecord {
                    detectors: Vec::new(),
                    observables: Vec::new(),
                },
            ],
        );
    }

    #[test]
    fn detection_conversion_rejects_invalid_measurement_references() {
        let circuit = Circuit::from_stim_str("DETECTOR rec[-1]\n").expect("parse circuit");
        let result = convert_measurements_to_detection_events(
            &circuit,
            &[Vec::new()],
            DetectionConversionOptions {
                skip_reference_sample: true,
            },
        );

        assert!(result.is_err());
    }

    #[test]
    fn detection_sampling_rejects_pauli_target_observables_until_supported() {
        let circuit = Circuit::from_stim_str("RZ 0\nOBSERVABLE_INCLUDE(0) X0\n").expect("parse");

        assert!(sample_detection_events(&circuit, 1, Some(5)).is_err());
    }

    #[test]
    fn detection_conversion_rejects_unbounded_record_shapes() {
        let huge_observable =
            Circuit::from_stim_str("M 0\nOBSERVABLE_INCLUDE(1000000) rec[-1]\n").expect("parse");
        assert!(
            convert_measurements_to_detection_events(
                &huge_observable,
                &[vec![false]],
                DetectionConversionOptions {
                    skip_reference_sample: true,
                },
            )
            .is_err()
        );

        let huge_repeat =
            Circuit::from_stim_str("REPEAT 100001 {\n    M 0\n}\n").expect("parse repeat");
        assert!(measurement_record_count(&huge_repeat).is_err());
    }

    #[test]
    fn detection_record_writers_cover_text_and_bit_packed_formats() {
        let output = DetectionConversionOutput {
            detector_count: 2,
            observable_count: 2,
            records: vec![
                DetectionEventRecord {
                    detectors: vec![true, false],
                    observables: vec![false, true],
                },
                DetectionEventRecord {
                    detectors: vec![false, true],
                    observables: vec![true, false],
                },
            ],
        };

        assert_eq!(
            write_detection_records(
                &output,
                DetectionObservableOutputMode::Append,
                SampleFormat::ZeroOne
            )
            .unwrap(),
            b"1001\n0110\n"
        );
        assert_eq!(
            write_detection_records(
                &output,
                DetectionObservableOutputMode::Append,
                SampleFormat::Dets
            )
            .unwrap(),
            b"shot D0 L1\nshot D1 L0\n"
        );
        assert_eq!(
            write_detection_records(
                &output,
                DetectionObservableOutputMode::Prepend,
                SampleFormat::Dets
            )
            .unwrap(),
            b"shot L1 D0\nshot L0 D1\n"
        );
        assert_eq!(
            write_detection_records(
                &output,
                DetectionObservableOutputMode::Append,
                SampleFormat::Hits
            )
            .unwrap(),
            b"0,3\n1,2\n"
        );
        assert_eq!(
            write_detection_records(
                &output,
                DetectionObservableOutputMode::Append,
                SampleFormat::B8
            )
            .unwrap(),
            [0b0000_1001, 0b0000_0110]
        );
        assert_eq!(
            write_observable_records(&output, SampleFormat::B8).unwrap(),
            [0b0000_0010, 0b0000_0001]
        );

        let ptb64_output = DetectionConversionOutput {
            detector_count: 2,
            observable_count: 1,
            records: vec![
                DetectionEventRecord {
                    detectors: vec![true, false],
                    observables: vec![true],
                };
                64
            ],
        };
        assert_eq!(
            write_ptb64_detection_records(&ptb64_output, DetectionObservableOutputMode::Append)
                .unwrap(),
            [
                0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff,
                0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            ]
        );
        assert_eq!(
            write_ptb64_observable_records(&ptb64_output).unwrap(),
            [0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]
        );
    }

    #[test]
    fn detection_sampling_matches_basic_frame_simulator_utility_semantics() {
        let circuit =
            Circuit::from_stim_str("X_ERROR(1) 0\nM 0\nDETECTOR rec[-1]\n").expect("parse");
        let output = sample_detection_events(&circuit, 5, Some(5)).expect("sample detections");

        assert_eq!(output.detector_count, 1);
        assert_eq!(
            output.records,
            vec![
                DetectionEventRecord {
                    detectors: vec![true],
                    observables: Vec::new(),
                };
                5
            ],
        );
    }

    #[test]
    fn detection_sampling_handles_gauge_detectors_structurally() {
        let circuit = Circuit::from_stim_str("MPP Z8*X9\nDETECTOR rec[-1]\n").expect("parse");
        let first = sample_detection_events(&circuit, 1000, Some(5)).expect("sample detections");
        let second = sample_detection_events(&circuit, 1000, Some(5)).expect("sample detections");

        assert_eq!(first, second);
        let hits = first
            .records
            .iter()
            .filter(|record| record.detectors.first().copied().unwrap_or(false))
            .count();
        assert!(
            (350..=650).contains(&hits),
            "expected gauge detector to produce random-looking events, got {hits}"
        );
    }
}
