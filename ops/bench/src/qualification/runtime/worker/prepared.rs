use super::bits::{DenseXorFixture, PopcountFixture};
use super::clifford_string::{CliffordDescriptor, CliffordStringFixture};
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

    pub(super) fn execute(
        &mut self,
        iterations: u64,
        work_items: u64,
    ) -> Result<TimedWorkloadOutput, WorkerError> {
        match &mut self.state {
            PreparedState::ProtocolSmoke => Ok(TimedWorkloadOutput::Complete(
                WorkloadOutput::DigestState(protocol_smoke(iterations, work_items)),
            )),
            PreparedState::CircuitParse(fixture) => circuit_parse(iterations, fixture)
                .map(WorkloadOutput::Circuit)
                .map(TimedWorkloadOutput::Complete),
            PreparedState::CircuitCanonicalPrint { circuit, .. } => Ok(
                TimedWorkloadOutput::Complete(WorkloadOutput::CanonicalCircuitText(
                    circuit_canonical_print(iterations, circuit),
                )),
            ),
            PreparedState::GateNameHash {
                names,
                sweeps,
                table_digest,
            } => Ok(TimedWorkloadOutput::Complete(WorkloadOutput::DigestState(
                gate_name_hash(iterations, work_items, *sweeps, names, *table_digest),
            ))),
            PreparedState::SimdWordPopcount {
                fixture,
                toggle_state,
            } => simd_word_popcount(iterations, fixture, toggle_state)
                .map(TimedWorkloadOutput::PopcountChecksum),
            PreparedState::SimdBitsXor(fixture) => {
                dense_xor(iterations, fixture).map(|()| TimedWorkloadOutput::DenseXorComplete)
            }
            PreparedState::SimdBitsNotZero(fixture) => Ok(TimedWorkloadOutput::NotZeroChecksum(
                simd_bits_not_zero(iterations, fixture),
            )),
            PreparedState::SparseXor(fixture) => {
                fixture.execute(iterations);
                Ok(TimedWorkloadOutput::SparseXorComplete)
            }
            PreparedState::Transpose(fixture) => {
                fixture.execute(iterations)?;
                Ok(TimedWorkloadOutput::TransposeComplete)
            }
            PreparedState::PauliMultiply(fixture) => {
                fixture.execute(iterations)?;
                Ok(TimedWorkloadOutput::PauliMultiplyComplete)
            }
            PreparedState::PauliIter(fixture) => {
                fixture.execute(iterations)?;
                Ok(TimedWorkloadOutput::PauliIterComplete)
            }
            PreparedState::CliffordString(fixture) => {
                fixture.execute(iterations)?;
                Ok(TimedWorkloadOutput::CliffordStringComplete)
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
