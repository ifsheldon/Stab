use std::hint::black_box;
use std::sync::atomic::{Ordering, compiler_fence};

use stab_analysis::{CircuitPassContext, run_circuit_pass};
use stab_model::{Circuit, Probability};
use stab_reference_noise_pass::{
    XErrorAfterSingleQubitUnitariesOptions, XErrorAfterSingleQubitUnitariesPass,
    XErrorAfterSingleQubitUnitariesReport,
};

use super::{WorkerError, byte_digest, semantic_digest};

const MAX_INPUT_INSTRUCTIONS: u64 = 750_000;
const PROBABILITY: f64 = 0.125;
const WITNESS_DOMAIN: &[u8] = b"stab-a8-external-noise-pass-v1\0";

struct CycleItem {
    input: &'static str,
    output: &'static str,
    insertions: u64,
}

const CYCLE: [CycleItem; 6] = [
    CycleItem {
        input: "H 0\n",
        output: "H 0\nX_ERROR(0.125) 0\n",
        insertions: 1,
    },
    CycleItem {
        input: "S 1\n",
        output: "S 1\nX_ERROR(0.125) 1\n",
        insertions: 1,
    },
    CycleItem {
        input: "CX 0 1\n",
        output: "CX 0 1\n",
        insertions: 0,
    },
    CycleItem {
        input: "M 0\n",
        output: "M 0\n",
        insertions: 0,
    },
    CycleItem {
        input: "DETECTOR rec[-1]\n",
        output: "DETECTOR rec[-1]\n",
        insertions: 0,
    },
    CycleItem {
        input: "TICK\n",
        output: "TICK\n",
        insertions: 0,
    },
];

pub(super) struct NoisePassOutput {
    completed_iterations: u64,
}

pub(super) struct NoisePassFixture {
    input: String,
    circuit: Circuit,
    options: XErrorAfterSingleQubitUnitariesOptions,
    context: CircuitPassContext,
    expected_canonical: String,
    expected_insertions: u64,
    represented_input_items: u64,
}

impl NoisePassFixture {
    pub(super) fn prepare(work_items: u64) -> Result<Self, WorkerError> {
        if work_items == 0 {
            return Err(WorkerError::NoisePassInputMinimum);
        }
        if work_items > MAX_INPUT_INSTRUCTIONS {
            return Err(WorkerError::NoisePassInputLimit {
                actual: work_items,
                maximum: MAX_INPUT_INSTRUCTIONS,
            });
        }
        let instruction_count = usize::try_from(work_items)
            .map_err(|_| WorkerError::NoisePassInputRange(work_items))?;
        let input_capacity = instruction_count
            .checked_mul(18)
            .ok_or(WorkerError::NoisePassFixtureOverflow)?;
        let output_capacity = instruction_count
            .checked_mul(27)
            .ok_or(WorkerError::NoisePassFixtureOverflow)?;
        let mut input = String::new();
        input
            .try_reserve_exact(input_capacity)
            .map_err(WorkerError::NoisePassFixtureAllocation)?;
        let mut expected_canonical = String::new();
        expected_canonical
            .try_reserve_exact(output_capacity)
            .map_err(WorkerError::NoisePassFixtureAllocation)?;
        let mut expected_insertions = 0_u64;
        for item in CYCLE.iter().cycle().take(instruction_count) {
            input.push_str(item.input);
            expected_canonical.push_str(item.output);
            expected_insertions = expected_insertions
                .checked_add(item.insertions)
                .ok_or(WorkerError::NoisePassInsertionOverflow)?;
        }

        let circuit = Circuit::from_stim_str(&input)?;
        let actual_items = u64::try_from(circuit.items().len())
            .map_err(|_| WorkerError::NoisePassInputRange(work_items))?;
        if actual_items != work_items {
            return Err(WorkerError::NoisePassRepresentedItems {
                actual: actual_items,
                expected: work_items,
            });
        }
        let probability = Probability::try_new(PROBABILITY)?;
        Ok(Self {
            input,
            circuit,
            options: XErrorAfterSingleQubitUnitariesOptions::new(probability),
            context: CircuitPassContext::default(),
            expected_canonical,
            expected_insertions,
            represented_input_items: work_items,
        })
    }

    pub(super) fn execute(&self, iterations: u64) -> Result<(), WorkerError> {
        if iterations == 0 {
            return Err(WorkerError::NoisePassMissingOutput);
        }
        for _ in 0..iterations {
            compiler_fence(Ordering::SeqCst);
            let output = run_circuit_pass(
                &XErrorAfterSingleQubitUnitariesPass,
                black_box(&self.circuit),
                black_box(&self.options),
                black_box(&self.context),
            )?;
            black_box(output.circuit());
            black_box(output.report());
            drop(black_box(output));
        }
        Ok(())
    }

    pub(super) const fn completion_marker(iterations: u64) -> NoisePassOutput {
        NoisePassOutput {
            completed_iterations: iterations,
        }
    }

    pub(super) fn validate(
        &self,
        output: NoisePassOutput,
        iterations: u64,
        work_items: u64,
    ) -> Result<String, WorkerError> {
        if output.completed_iterations != iterations {
            return Err(WorkerError::NoisePassWitness("completed iterations"));
        }
        if work_items != self.represented_input_items {
            return Err(WorkerError::NoisePassWitness(
                "represented input instructions",
            ));
        }

        let semantic_output = run_circuit_pass(
            &XErrorAfterSingleQubitUnitariesPass,
            &self.circuit,
            &self.options,
            &self.context,
        )?;
        let canonical = semantic_output.circuit().to_stim_string();
        if canonical != self.expected_canonical {
            return Err(WorkerError::NoisePassWitness("canonical circuit"));
        }
        let report: XErrorAfterSingleQubitUnitariesReport = *semantic_output.report();
        if report.inserted_represented_instruction_count() != self.expected_insertions {
            return Err(WorkerError::NoisePassWitness("inserted instructions"));
        }
        if report.affected_target_count() != self.expected_insertions {
            return Err(WorkerError::NoisePassWitness("affected targets"));
        }

        let mut material = Vec::new();
        material
            .try_reserve_exact(
                WITNESS_DOMAIN
                    .len()
                    .checked_add(canonical.len())
                    .and_then(|size| size.checked_add(24))
                    .ok_or(WorkerError::NoisePassFixtureOverflow)?,
            )
            .map_err(WorkerError::NoisePassFixtureAllocation)?;
        material.extend_from_slice(WITNESS_DOMAIN);
        material.extend_from_slice(&work_items.to_le_bytes());
        material.extend_from_slice(
            &report
                .inserted_represented_instruction_count()
                .to_le_bytes(),
        );
        material.extend_from_slice(&report.affected_target_count().to_le_bytes());
        material.extend_from_slice(canonical.as_bytes());
        Ok(semantic_digest(byte_digest(&material)))
    }

    pub(super) fn input(&self) -> &str {
        &self.input
    }

    #[cfg(test)]
    fn represented_input_items(&self) -> usize {
        self.circuit.items().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCALES: [u64; 3] = [64, 4_096, 65_536];

    #[test]
    fn external_noise_pass_validates_every_exact_instruction_scale() {
        for (work_items, expected_input_bytes, expected_input_digest, expected_output_digest) in [
            (
                64,
                429,
                "c3c0855f4f04402cd1768dee1ca0606d7d1ff8907d6a3a4e3b386fd78ff6c3b6",
                "497dec08e84316b349ec34173208f164e5d990ca8c712d015b85de2af0e6ac59",
            ),
            (
                4_096,
                27_981,
                "7c0a60d24fde2f776143003b987c30cd682d77fee5fd9f17bd9e9b5377a8ad04",
                "6f5cf5b7d020c0b6230fb489fa2cf9a719fe353c3e26a59ef2dde509bedca628",
            ),
            (
                65_536,
                447_821,
                "397e8db6accb8e66a826015e2d5db453271851fa2c49d40a0d98f91748219b60",
                "765bab36c6f7c0dc03a2e2e1821b9d2d2701062554d7f6273c94169c82cc1dca",
            ),
        ] {
            let fixture = NoisePassFixture::prepare(work_items).expect("fixture");
            assert_eq!(fixture.represented_input_items() as u64, work_items);
            assert_eq!(fixture.input().len(), expected_input_bytes);
            assert_eq!(
                semantic_digest(byte_digest(fixture.input().as_bytes())),
                expected_input_digest
            );
            fixture.execute(2).expect("pass execution");
            let digest = fixture
                .validate(NoisePassFixture::completion_marker(2), 2, work_items)
                .expect("validated witness");
            assert_eq!(digest, expected_output_digest);
        }
    }

    #[test]
    fn external_noise_pass_witness_is_iteration_independent() {
        let digest = |iterations| {
            let fixture = NoisePassFixture::prepare(SCALES[0]).expect("fixture");
            fixture.execute(iterations).expect("pass execution");
            fixture
                .validate(
                    NoisePassFixture::completion_marker(iterations),
                    iterations,
                    SCALES[0],
                )
                .expect("validated witness")
        };
        assert_eq!(digest(1), digest(2));
    }

    #[test]
    fn external_noise_pass_rejects_unsupported_work_shapes() {
        assert!(matches!(
            NoisePassFixture::prepare(0),
            Err(WorkerError::NoisePassInputMinimum)
        ));
        assert!(matches!(
            NoisePassFixture::prepare(MAX_INPUT_INSTRUCTIONS + 1),
            Err(WorkerError::NoisePassInputLimit { .. })
        ));
    }

    #[test]
    fn external_noise_pass_rejects_an_incorrect_output_witness() {
        let mut fixture = NoisePassFixture::prepare(SCALES[0]).expect("small fixture");
        fixture.expected_canonical.push_str("TICK\n");
        assert!(matches!(
            fixture.validate(NoisePassFixture::completion_marker(1), 1, SCALES[0]),
            Err(WorkerError::NoisePassWitness("canonical circuit"))
        ));

        let fixture = NoisePassFixture::prepare(SCALES[0]).expect("small fixture");
        assert!(matches!(
            fixture.validate(NoisePassFixture::completion_marker(1), 1, SCALES[0] + 1),
            Err(WorkerError::NoisePassWitness(
                "represented input instructions"
            ))
        ));
    }
}
