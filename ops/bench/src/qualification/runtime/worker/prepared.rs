use std::time::Instant;

use super::bits::{DenseXorFixture, PopcountFixture};
use super::clifford_string::{CliffordDescriptor, CliffordStringFixture};
use super::dem_model::{self, DemFixture};
use super::not_zero::NotZeroFixture;
use super::pauli::PauliMultiplyFixture;
use super::pauli_iter::PauliIterFixture;
use super::sparse_xor::SparseXorFixture;
use super::transpose::TransposeFixture;
use super::workload::WorkerWorkload;
use super::{
    InputDigest, POPCOUNT_TOGGLE_BIT, TimedWorkloadOutput, WorkerError, WorkloadOutput,
    byte_digest, circuit_canonical_print, circuit_parse, circuit_parse_fixture, dense_xor,
    dense_xor_output_digest, gate_hash_names, gate_hash_sweeps, gate_name_hash, gate_table_digest,
    not_zero_fixture, not_zero_output_digest, protocol_smoke, semantic_digest, simd_bits_not_zero,
    simd_word_popcount,
};

pub(super) struct PreparedWorkload {
    state: PreparedState,
    input_bytes: u64,
    input_digest: InputDigest,
}

enum PreparedState {
    ProtocolSmoke,
    CircuitParse(String),
    CircuitCanonicalPrint {
        input: String,
        circuit: stab_core::Circuit,
    },
    DemParse(DemFixture),
    DemCanonicalPrint {
        fixture: DemFixture,
        model: stab_core::DetectorErrorModel,
    },
    GateNameHash {
        names: Vec<String>,
        sweeps: u64,
        table_digest: u64,
    },
    SimdWordPopcount {
        fixture: PopcountFixture,
        toggle_state: bool,
    },
    SimdBitsXor(DenseXorFixture),
    SimdBitsNotZero(NotZeroFixture),
    SparseXor(SparseXorFixture),
    Transpose(TransposeFixture),
    PauliMultiply(PauliMultiplyFixture),
    PauliIter(PauliIterFixture),
    CliffordString(Box<CliffordStringFixture>),
}

impl PreparedWorkload {
    pub(super) fn prepare(
        workload: WorkerWorkload,
        descriptor: Option<CliffordDescriptor>,
        iterations: u64,
        work_items: u64,
        work_count: u64,
    ) -> Result<Self, WorkerError> {
        let clifford_kind = workload.clifford_kind();
        match (clifford_kind, descriptor) {
            (Some(_), None) => return Err(WorkerError::MissingCliffordDescriptor),
            (None, Some(_)) => return Err(WorkerError::UnexpectedCliffordDescriptor),
            _ => {}
        }

        let state = match workload {
            WorkerWorkload::ProtocolSmoke => PreparedState::ProtocolSmoke,
            WorkerWorkload::CircuitParse => {
                PreparedState::CircuitParse(circuit_parse_fixture(work_items)?)
            }
            WorkerWorkload::CircuitCanonicalPrint => {
                let input = circuit_parse_fixture(work_items)?;
                let circuit = stab_core::Circuit::from_stim_str(&input)?;
                PreparedState::CircuitCanonicalPrint { input, circuit }
            }
            WorkerWorkload::DemParse => PreparedState::DemParse(DemFixture::prepare(work_items)?),
            WorkerWorkload::DemCanonicalPrint => {
                let fixture = DemFixture::prepare(work_items)?;
                let model = stab_core::DetectorErrorModel::from_dem_str(fixture.text())?;
                PreparedState::DemCanonicalPrint { fixture, model }
            }
            WorkerWorkload::GateNameHash => {
                let names = gate_hash_names()?;
                let sweeps = gate_hash_sweeps(work_items)?;
                let table_digest = gate_table_digest(&names)?;
                PreparedState::GateNameHash {
                    names,
                    sweeps,
                    table_digest,
                }
            }
            WorkerWorkload::SimdWordPopcount => {
                let fixture = super::popcount_fixture(work_items)?;
                let toggle_state = fixture
                    .bits
                    .get(POPCOUNT_TOGGLE_BIT)
                    .ok_or(WorkerError::MissingPopcountToggleBit)?;
                PreparedState::SimdWordPopcount {
                    fixture,
                    toggle_state,
                }
            }
            WorkerWorkload::SimdBitsXor => {
                PreparedState::SimdBitsXor(super::dense_xor_fixture(work_items)?)
            }
            WorkerWorkload::SimdBitsNotZeroEarly
            | WorkerWorkload::SimdBitsNotZeroZero
            | WorkerWorkload::SimdBitsNotZeroLate => {
                let pattern = workload
                    .not_zero_pattern()
                    .ok_or(WorkerError::PreparedWorkloadKind(workload.id()))?;
                PreparedState::SimdBitsNotZero(not_zero_fixture(work_items, pattern)?)
            }
            WorkerWorkload::SparseXorRow | WorkerWorkload::SparseXorItem => {
                let kind = workload
                    .sparse_xor_kind()
                    .ok_or(WorkerError::PreparedWorkloadKind(workload.id()))?;
                PreparedState::SparseXor(SparseXorFixture::prepare(kind, work_items)?)
            }
            WorkerWorkload::BitMatrixTransposeInPlace
            | WorkerWorkload::BitMatrixTransposeAllocating => {
                let kind = workload
                    .transpose_kind()
                    .ok_or(WorkerError::PreparedWorkloadKind(workload.id()))?;
                PreparedState::Transpose(TransposeFixture::prepare(kind, work_items)?)
            }
            WorkerWorkload::PauliStringRightMultiply => {
                PreparedState::PauliMultiply(PauliMultiplyFixture::prepare(work_items)?)
            }
            WorkerWorkload::PauliStringIterRange | WorkerWorkload::PauliStringIterSingleton => {
                let kind = workload
                    .pauli_iter_kind()
                    .ok_or(WorkerError::PreparedWorkloadKind(workload.id()))?;
                PreparedState::PauliIter(PauliIterFixture::prepare(kind, work_items, work_count)?)
            }
            WorkerWorkload::CliffordStringRightMultiplyIdentity
            | WorkerWorkload::CliffordStringRightMultiplyNonIdentity => {
                let kind = clifford_kind.ok_or(WorkerError::PreparedWorkloadKind(workload.id()))?;
                let descriptor = descriptor.ok_or(WorkerError::MissingCliffordDescriptor)?;
                PreparedState::CliffordString(Box::new(CliffordStringFixture::prepare(
                    kind, descriptor, work_items, iterations,
                )?))
            }
        };
        let (input_bytes, input_digest) = input_evidence(&state)?;
        Ok(Self {
            state,
            input_bytes,
            input_digest,
        })
    }

    pub(super) fn input_evidence(&self) -> (u64, InputDigest) {
        (self.input_bytes, self.input_digest.clone())
    }

    pub(super) fn arm(&mut self) {
        if let PreparedState::CliffordString(fixture) = &mut self.state {
            fixture.reset_execution_state();
        }
    }

    pub(super) fn measure(
        &mut self,
        iterations: u64,
        work_items: u64,
    ) -> Result<(TimedWorkloadOutput, f64), WorkerError> {
        let mut clock = MonotonicClock;
        self.measure_with_clock(iterations, work_items, &mut clock)
    }

    fn measure_with_clock(
        &mut self,
        iterations: u64,
        work_items: u64,
        clock: &mut impl TimingClock,
    ) -> Result<(TimedWorkloadOutput, f64), WorkerError> {
        match &mut self.state {
            PreparedState::ProtocolSmoke => {
                let measured =
                    measure_output(clock, || Ok(protocol_smoke(iterations, work_items)))?;
                Ok((
                    TimedWorkloadOutput::Complete(WorkloadOutput::DigestState(measured.output)),
                    measured.elapsed_seconds,
                ))
            }
            PreparedState::CircuitParse(fixture) => {
                let measured = measure_output(clock, || circuit_parse(iterations, fixture))?;
                Ok((
                    TimedWorkloadOutput::Complete(WorkloadOutput::Circuit(measured.output)),
                    measured.elapsed_seconds,
                ))
            }
            PreparedState::CircuitCanonicalPrint { circuit, .. } => {
                let measured =
                    measure_output(clock, || Ok(circuit_canonical_print(iterations, circuit)))?;
                Ok((
                    TimedWorkloadOutput::Complete(WorkloadOutput::CanonicalCircuitText(
                        measured.output,
                    )),
                    measured.elapsed_seconds,
                ))
            }
            PreparedState::DemParse(fixture) => {
                let measured = measure_output(clock, || dem_model::parse(iterations, fixture))?;
                Ok((
                    TimedWorkloadOutput::DemParsed(measured.output),
                    measured.elapsed_seconds,
                ))
            }
            PreparedState::DemCanonicalPrint { model, .. } => {
                let measured =
                    measure_output(clock, || Ok(dem_model::serialize(iterations, model)))?;
                Ok((
                    TimedWorkloadOutput::DemSerialized(measured.output),
                    measured.elapsed_seconds,
                ))
            }
            PreparedState::GateNameHash {
                names,
                sweeps,
                table_digest,
            } => {
                let measured = measure_output(clock, || {
                    Ok(gate_name_hash(
                        iterations,
                        work_items,
                        *sweeps,
                        names,
                        *table_digest,
                    ))
                })?;
                Ok((
                    TimedWorkloadOutput::Complete(WorkloadOutput::DigestState(measured.output)),
                    measured.elapsed_seconds,
                ))
            }
            PreparedState::SimdWordPopcount {
                fixture,
                toggle_state,
            } => {
                let measured = measure_output(clock, || {
                    simd_word_popcount(iterations, fixture, toggle_state)
                })?;
                Ok((
                    TimedWorkloadOutput::PopcountChecksum(measured.output),
                    measured.elapsed_seconds,
                ))
            }
            PreparedState::SimdBitsXor(fixture) => {
                let elapsed_seconds = measure_mutation(clock, || dense_xor(iterations, fixture))?;
                Ok((TimedWorkloadOutput::DenseXorComplete, elapsed_seconds))
            }
            PreparedState::SimdBitsNotZero(fixture) => {
                let measured =
                    measure_output(clock, || Ok(simd_bits_not_zero(iterations, fixture)))?;
                Ok((
                    TimedWorkloadOutput::NotZeroChecksum(measured.output),
                    measured.elapsed_seconds,
                ))
            }
            PreparedState::SparseXor(fixture) => {
                let elapsed_seconds = measure_mutation(clock, || {
                    fixture.execute(iterations);
                    Ok(())
                })?;
                Ok((TimedWorkloadOutput::SparseXorComplete, elapsed_seconds))
            }
            PreparedState::Transpose(fixture) => {
                let elapsed_seconds = measure_mutation(clock, || fixture.execute(iterations))?;
                Ok((TimedWorkloadOutput::TransposeComplete, elapsed_seconds))
            }
            PreparedState::PauliMultiply(fixture) => {
                let elapsed_seconds = measure_mutation(clock, || fixture.execute(iterations))?;
                Ok((TimedWorkloadOutput::PauliMultiplyComplete, elapsed_seconds))
            }
            PreparedState::PauliIter(fixture) => {
                let elapsed_seconds = measure_mutation(clock, || fixture.execute(iterations))?;
                Ok((TimedWorkloadOutput::PauliIterComplete, elapsed_seconds))
            }
            PreparedState::CliffordString(fixture) => {
                let elapsed_seconds = measure_mutation(clock, || fixture.execute(iterations))?;
                Ok((TimedWorkloadOutput::CliffordStringComplete, elapsed_seconds))
            }
        }
    }

    pub(super) fn output_digest(
        &self,
        output: TimedWorkloadOutput,
        iterations: u64,
        work_items: u64,
        work_count: u64,
    ) -> Result<String, WorkerError> {
        match (output, &self.state) {
            (TimedWorkloadOutput::Complete(output), _) => Ok(output.semantic_digest()),
            (TimedWorkloadOutput::DemParsed(model), PreparedState::DemParse(fixture)) => {
                fixture.validate_canonical(&dem_model::serialize(1, &model))
            }
            (
                TimedWorkloadOutput::DemSerialized(canonical),
                PreparedState::DemCanonicalPrint { fixture, .. },
            ) => fixture.validate_canonical(&canonical),
            (
                TimedWorkloadOutput::PopcountChecksum(checksum),
                PreparedState::SimdWordPopcount { fixture, .. },
            ) => {
                let final_bit = fixture
                    .bits
                    .get(POPCOUNT_TOGGLE_BIT)
                    .ok_or(WorkerError::MissingPopcountToggleBit)?;
                Ok(semantic_digest(super::popcount_output_digest(
                    checksum,
                    iterations,
                    work_items,
                    fixture.input_digest,
                    final_bit,
                )))
            }
            (TimedWorkloadOutput::DenseXorComplete, PreparedState::SimdBitsXor(fixture)) => Ok(
                semantic_digest(dense_xor_output_digest(fixture, iterations, work_items)),
            ),
            (
                TimedWorkloadOutput::NotZeroChecksum(checksum),
                PreparedState::SimdBitsNotZero(fixture),
            ) => Ok(semantic_digest(not_zero_output_digest(
                checksum, iterations, work_items, fixture,
            ))),
            (TimedWorkloadOutput::SparseXorComplete, PreparedState::SparseXor(fixture)) => Ok(
                semantic_digest(fixture.output_digest(iterations, work_items)?),
            ),
            (TimedWorkloadOutput::TransposeComplete, PreparedState::Transpose(fixture)) => Ok(
                semantic_digest(fixture.output_digest(iterations, work_items)?),
            ),
            (TimedWorkloadOutput::PauliMultiplyComplete, PreparedState::PauliMultiply(fixture)) => {
                Ok(semantic_digest(
                    fixture.output_digest(iterations, work_count)?,
                ))
            }
            (TimedWorkloadOutput::PauliIterComplete, PreparedState::PauliIter(fixture)) => Ok(
                semantic_digest(fixture.output_digest(iterations, work_count)?),
            ),
            (
                TimedWorkloadOutput::CliffordStringComplete,
                PreparedState::CliffordString(fixture),
            ) => fixture.output_digest(iterations, work_count),
            _ => Err(WorkerError::PreparedWorkloadOutput),
        }
    }
}

struct MeasuredOutput<T> {
    output: T,
    elapsed_seconds: f64,
}

trait TimingClock {
    type Mark;

    fn start(&mut self) -> Self::Mark;
    fn finish_seconds(&mut self, started: Self::Mark) -> f64;
}

struct MonotonicClock;

impl TimingClock for MonotonicClock {
    type Mark = Instant;

    fn start(&mut self) -> Self::Mark {
        Instant::now()
    }

    fn finish_seconds(&mut self, started: Self::Mark) -> f64 {
        let finished = Instant::now();
        finished.duration_since(started).as_secs_f64()
    }
}

fn measure_output<T>(
    clock: &mut impl TimingClock,
    operation: impl FnOnce() -> Result<T, WorkerError>,
) -> Result<MeasuredOutput<T>, WorkerError> {
    let started = clock.start();
    let output = operation()?;
    let elapsed_seconds = clock.finish_seconds(started);
    Ok(MeasuredOutput {
        output,
        elapsed_seconds,
    })
}

fn measure_mutation(
    clock: &mut impl TimingClock,
    operation: impl FnOnce() -> Result<(), WorkerError>,
) -> Result<f64, WorkerError> {
    let started = clock.start();
    operation()?;
    Ok(clock.finish_seconds(started))
}

fn input_evidence(state: &PreparedState) -> Result<(u64, InputDigest), WorkerError> {
    let (input_bytes, digest) = match state {
        PreparedState::ProtocolSmoke | PreparedState::GateNameHash { .. } => {
            (0, semantic_digest(byte_digest(&[])))
        }
        PreparedState::CircuitParse(input) | PreparedState::CircuitCanonicalPrint { input, .. } => {
            (
                u64::try_from(input.len()).map_err(|_| WorkerError::InputSizeRange)?,
                semantic_digest(byte_digest(input.as_bytes())),
            )
        }
        PreparedState::DemParse(fixture) | PreparedState::DemCanonicalPrint { fixture, .. } => {
            (fixture.input_bytes()?, fixture.input_digest())
        }
        PreparedState::SimdWordPopcount { fixture, .. } => {
            (fixture.input_bytes, semantic_digest(fixture.input_digest))
        }
        PreparedState::SimdBitsXor(fixture) => {
            (fixture.input_bytes, semantic_digest(fixture.input_digest))
        }
        PreparedState::SimdBitsNotZero(fixture) => {
            (fixture.input_bytes, semantic_digest(fixture.input_digest))
        }
        PreparedState::SparseXor(fixture) => {
            (fixture.input_bytes, semantic_digest(fixture.input_digest))
        }
        PreparedState::Transpose(fixture) => {
            (fixture.input_bytes, semantic_digest(fixture.input_digest))
        }
        PreparedState::PauliMultiply(fixture) => {
            (fixture.input_bytes, semantic_digest(fixture.input_digest))
        }
        PreparedState::PauliIter(fixture) => {
            (fixture.input_bytes, semantic_digest(fixture.input_digest))
        }
        PreparedState::CliffordString(fixture) => {
            (fixture.input_bytes, fixture.input_digest.clone())
        }
    };
    Ok((input_bytes, InputDigest::try_new(digest)?))
}

#[cfg(test)]
mod timing_tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::*;

    struct FakeClock {
        events: Rc<RefCell<Vec<&'static str>>>,
    }

    impl TimingClock for FakeClock {
        type Mark = ();

        fn start(&mut self) -> Self::Mark {
            self.events.borrow_mut().push("start");
        }

        fn finish_seconds(&mut self, (): Self::Mark) -> f64 {
            self.events.borrow_mut().push("finish");
            0.25
        }
    }

    #[test]
    fn output_timer_samples_finish_before_protocol_wrapping() {
        let events = Rc::new(RefCell::new(Vec::new()));
        let mut clock = FakeClock {
            events: Rc::clone(&events),
        };

        let measured = measure_output(&mut clock, || {
            events.borrow_mut().push("raw-output-return");
            Ok(String::from("result"))
        })
        .expect("timed output");
        events.borrow_mut().push("protocol-wrap");
        let wrapped = TimedWorkloadOutput::DemSerialized(measured.output);

        assert_eq!(
            *events.borrow(),
            ["start", "raw-output-return", "finish", "protocol-wrap"]
        );
        assert_eq!(measured.elapsed_seconds, 0.25);
        assert!(matches!(wrapped, TimedWorkloadOutput::DemSerialized(_)));
    }

    #[test]
    fn mutation_timer_samples_finish_immediately_after_raw_work() {
        let events = Rc::new(RefCell::new(Vec::new()));
        let mut clock = FakeClock {
            events: Rc::clone(&events),
        };

        let elapsed_seconds = measure_mutation(&mut clock, || {
            events.borrow_mut().push("raw-mutation-return");
            Ok(())
        })
        .expect("timed mutation");
        events.borrow_mut().push("marker-wrap");
        let marker = TimedWorkloadOutput::DenseXorComplete;

        assert_eq!(
            *events.borrow(),
            ["start", "raw-mutation-return", "finish", "marker-wrap"]
        );
        assert_eq!(elapsed_seconds, 0.25);
        assert!(matches!(marker, TimedWorkloadOutput::DenseXorComplete));
    }
}
