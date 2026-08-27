use std::hint::black_box;
use std::sync::atomic::{Ordering, compiler_fence};

use stab_core::{Circuit, Estimate, ModelFingerprint, RecordFormat, ResourceEstimate};
use stab_engine::{
    CompilationRequestFingerprint, PlanFingerprint, SamplingCompiler, estimate_sampling_request,
};

use super::{
    WorkerError, byte_digest, circuit_parse_fixture, semantic_digest, workload::WorkerWorkload,
};

const ESTIMATE_SHOTS: usize = 4_096;
const ESTIMATE_OUTPUT_FORMAT: RecordFormat = RecordFormat::B8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AgentDiagnosticKind {
    CircuitModelFingerprint,
    SamplingRequestFingerprint,
    SamplingRequestEstimate,
    SamplerCompile,
}

impl AgentDiagnosticKind {
    pub(super) const fn from_workload(workload: WorkerWorkload) -> Option<Self> {
        match workload {
            WorkerWorkload::CircuitModelFingerprint => Some(Self::CircuitModelFingerprint),
            WorkerWorkload::SamplingRequestFingerprint => Some(Self::SamplingRequestFingerprint),
            WorkerWorkload::SamplingRequestEstimate => Some(Self::SamplingRequestEstimate),
            WorkerWorkload::SamplerCompile => Some(Self::SamplerCompile),
            _ => None,
        }
    }

    const fn marker(self) -> u8 {
        match self {
            Self::CircuitModelFingerprint => 1,
            Self::SamplingRequestFingerprint => 2,
            Self::SamplingRequestEstimate => 3,
            Self::SamplerCompile => 4,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum AgentDiagnosticOutput {
    ModelFingerprint(ModelFingerprint),
    CompilationRequestFingerprint(CompilationRequestFingerprint),
    SamplingRequestEstimate(ResourceEstimate),
    SamplingPlanFingerprint(PlanFingerprint),
    CompileRelease { completed_iterations: u64 },
}

pub(super) struct AgentDiagnosticFixture {
    kind: AgentDiagnosticKind,
    input: String,
    circuit: Circuit,
    expected: AgentDiagnosticOutput,
}

impl AgentDiagnosticFixture {
    pub(super) fn prepare(workload: WorkerWorkload, work_items: u64) -> Result<Self, WorkerError> {
        let kind = AgentDiagnosticKind::from_workload(workload)
            .ok_or(WorkerError::PreparedWorkloadKind(workload.id()))?;
        let input = circuit_parse_fixture(work_items)?;
        let circuit = Circuit::from_stim_str(&input)?;
        let expected = execute_once(kind, &circuit)?;
        Ok(Self {
            kind,
            input,
            circuit,
            expected,
        })
    }

    pub(super) fn execute(&self, iterations: u64) -> Result<AgentDiagnosticOutput, WorkerError> {
        if iterations == 0 {
            return Err(WorkerError::AgentDiagnosticMissingOutput);
        }
        if self.kind == AgentDiagnosticKind::SamplerCompile {
            for _ in 0..iterations {
                compiler_fence(Ordering::SeqCst);
                let compiled = SamplingCompiler::new().compile(black_box(&self.circuit))?;
                drop(black_box(compiled));
            }
            return Ok(AgentDiagnosticOutput::CompileRelease {
                completed_iterations: iterations,
            });
        }

        let mut output = None;
        for _ in 0..iterations {
            compiler_fence(Ordering::SeqCst);
            let next = execute_once(self.kind, black_box(&self.circuit))?;
            black_box(&next);
            output = Some(next);
        }
        output.ok_or(WorkerError::AgentDiagnosticMissingOutput)
    }

    pub(super) fn validate(
        &self,
        output: AgentDiagnosticOutput,
        iterations: u64,
        _work_items: u64,
    ) -> Result<String, WorkerError> {
        if self.kind == AgentDiagnosticKind::SamplerCompile {
            let AgentDiagnosticOutput::CompileRelease {
                completed_iterations,
            } = output
            else {
                return Err(WorkerError::AgentDiagnosticWitness(self.kind_name()));
            };
            let AgentDiagnosticOutput::SamplingPlanFingerprint(expected) = &self.expected else {
                return Err(WorkerError::AgentDiagnosticWitness(self.kind_name()));
            };
            if completed_iterations != iterations
                || SamplingCompiler::new()
                    .compile(&self.circuit)?
                    .fingerprint()
                    != *expected
            {
                return Err(WorkerError::AgentDiagnosticWitness(self.kind_name()));
            }
        } else if output != self.expected {
            return Err(WorkerError::AgentDiagnosticWitness(self.kind_name()));
        }
        let mut material = Vec::with_capacity(128);
        material.push(self.kind.marker());
        encode_output(&output, &self.circuit, &mut material);
        Ok(semantic_digest(byte_digest(&material)))
    }

    pub(super) fn input(&self) -> &str {
        &self.input
    }

    const fn kind_name(&self) -> &'static str {
        match self.kind {
            AgentDiagnosticKind::CircuitModelFingerprint => "circuit-model-fingerprint",
            AgentDiagnosticKind::SamplingRequestFingerprint => "sampling-request-fingerprint",
            AgentDiagnosticKind::SamplingRequestEstimate => "sampling-request-estimate",
            AgentDiagnosticKind::SamplerCompile => "sampler-compile-release",
        }
    }
}

fn execute_once(
    kind: AgentDiagnosticKind,
    circuit: &Circuit,
) -> Result<AgentDiagnosticOutput, WorkerError> {
    Ok(match kind {
        AgentDiagnosticKind::CircuitModelFingerprint => {
            AgentDiagnosticOutput::ModelFingerprint(circuit.fingerprint())
        }
        AgentDiagnosticKind::SamplingRequestFingerprint => {
            AgentDiagnosticOutput::CompilationRequestFingerprint(
                CompilationRequestFingerprint::for_sampling(circuit),
            )
        }
        AgentDiagnosticKind::SamplingRequestEstimate => {
            AgentDiagnosticOutput::SamplingRequestEstimate(estimate_sampling_request(
                circuit,
                ESTIMATE_SHOTS,
                ESTIMATE_OUTPUT_FORMAT,
            ))
        }
        AgentDiagnosticKind::SamplerCompile => AgentDiagnosticOutput::SamplingPlanFingerprint(
            SamplingCompiler::new().compile(circuit)?.fingerprint(),
        ),
    })
}

fn encode_output(output: &AgentDiagnosticOutput, circuit: &Circuit, material: &mut Vec<u8>) {
    match output {
        AgentDiagnosticOutput::ModelFingerprint(fingerprint) => {
            material.extend_from_slice(&fingerprint.schema_version().to_le_bytes());
            material.extend_from_slice(&fingerprint.digest());
        }
        AgentDiagnosticOutput::CompilationRequestFingerprint(fingerprint) => {
            material.extend_from_slice(&fingerprint.schema_version().to_le_bytes());
            material.extend_from_slice(&fingerprint.compiler_schema_version().to_le_bytes());
            material.extend_from_slice(&fingerprint.digest());
        }
        AgentDiagnosticOutput::SamplingRequestEstimate(estimate) => {
            for value in [
                estimate.input_bytes(),
                estimate.input_items(),
                estimate.expanded_operations(),
                estimate.folded_traversal(),
                estimate.scratch_bytes(),
                estimate.resident_bytes(),
                estimate.output_bytes(),
                estimate.work_units(),
            ] {
                encode_estimate(value, material);
            }
        }
        AgentDiagnosticOutput::SamplingPlanFingerprint(_)
        | AgentDiagnosticOutput::CompileRelease {
            completed_iterations: _,
        } => {
            // Plan identity is validated against an independently prepared Stab plan above.
            // Cross-implementation semantics stay backend-neutral so the pinned Stim worker can
            // prove it compiled the same request without pretending to share Stab's private IR.
            let request = CompilationRequestFingerprint::for_sampling(circuit);
            material.extend_from_slice(&request.digest());
        }
    }
}

fn encode_estimate(estimate: Estimate<usize>, material: &mut Vec<u8>) {
    match estimate {
        Estimate::Exact(value) => {
            material.push(1);
            encode_usize(value, material);
        }
        Estimate::UpperBound(value) => {
            material.push(2);
            encode_usize(value, material);
        }
        Estimate::Unknown => {
            material.push(3);
            encode_usize(0, material);
        }
    }
}

fn encode_usize(value: usize, material: &mut Vec<u8>) {
    let mut encoded = [0; size_of::<u128>()];
    for (output, input) in encoded
        .iter_mut()
        .rev()
        .zip(value.to_be_bytes().iter().rev())
    {
        *output = *input;
    }
    material.extend_from_slice(&encoded);
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCALES: [u64; 3] = [64, 4_096, 65_536];

    #[test]
    fn a2_agent_diagnostics_validate_typed_outputs_at_every_scale() {
        for workload in [
            WorkerWorkload::CircuitModelFingerprint,
            WorkerWorkload::SamplingRequestFingerprint,
            WorkerWorkload::SamplingRequestEstimate,
            WorkerWorkload::SamplerCompile,
        ] {
            for work_items in SCALES {
                let fixture =
                    AgentDiagnosticFixture::prepare(workload, work_items).expect("fixture");
                let output = fixture.execute(2).expect("diagnostic output");
                let digest = fixture
                    .validate(output, 2, work_items)
                    .expect("validated output");
                assert_eq!(digest.len(), 64, "{workload:?} work_items={work_items}");
            }
        }
    }

    #[test]
    fn a2_agent_diagnostic_witnesses_are_repeat_count_independent() {
        for workload in [
            WorkerWorkload::CircuitModelFingerprint,
            WorkerWorkload::SamplingRequestFingerprint,
            WorkerWorkload::SamplingRequestEstimate,
            WorkerWorkload::SamplerCompile,
        ] {
            let digest = |iterations| {
                let fixture =
                    AgentDiagnosticFixture::prepare(workload, SCALES[0]).expect("fixture");
                let output = fixture.execute(iterations).expect("diagnostic output");
                fixture
                    .validate(output, iterations, SCALES[0])
                    .expect("validated output")
            };
            assert_eq!(digest(1), digest(2), "{workload:?}");
        }
    }

    #[cfg(feature = "count-allocations")]
    #[test]
    fn a2_agent_diagnostics_read_only_operations_do_not_allocate_after_setup() {
        let fixture =
            AgentDiagnosticFixture::prepare(WorkerWorkload::CircuitModelFingerprint, SCALES[1])
                .expect("fixture");
        for kind in [
            AgentDiagnosticKind::CircuitModelFingerprint,
            AgentDiagnosticKind::SamplingRequestFingerprint,
            AgentDiagnosticKind::SamplingRequestEstimate,
        ] {
            let allocations = allocation_counter::measure(|| {
                black_box(execute_once(kind, black_box(&fixture.circuit)).expect("operation"));
            });
            assert_eq!(allocations.count_total, 0, "{kind:?} {allocations:?}");
            assert_eq!(allocations.bytes_total, 0, "{kind:?} {allocations:?}");
        }
    }
}
