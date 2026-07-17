use std::hint::black_box;
use std::sync::atomic::{Ordering, compiler_fence};

use stab_core::{PauliStringIterator, StabilizerResource};

use super::{WorkerError, byte_digest_word_pair, byte_digest_words};

pub(super) const PAULI_ITER_RANGE_OUTPUT_CAP: u64 = 1_000_000;
pub(super) const PAULI_ITER_PUBLIC_QUBIT_CAP: u64 = StabilizerResource::PauliQubits.limit() as u64;
pub(super) const PAULI_ITER_SINGLETON_OUTPUT_CAP: u64 = PAULI_ITER_PUBLIC_QUBIT_CAP * 3;

const RANGE_MARKER: u64 = 6;
const SINGLETON_MARKER: u64 = 7;
const X_MASK: u64 = 1;
const Y_MASK: u64 = 2;
const Z_MASK: u64 = 4;
const INPUT_BYTES: u64 = 8 * 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PauliIterKind {
    Range,
    Singleton,
}

impl PauliIterKind {
    pub(super) const fn workload(self) -> &'static str {
        match self {
            Self::Range => "pauli-string-iter-range",
            Self::Singleton => "pauli-string-iter-singleton",
        }
    }

    const fn marker(self) -> u64 {
        match self {
            Self::Range => RANGE_MARKER,
            Self::Singleton => SINGLETON_MARKER,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PauliIterSpec {
    kind: PauliIterKind,
    width: u64,
    min_weight: u64,
    max_weight: u64,
    axis_mask: u64,
    outputs_per_iteration: u64,
    output_cap: u64,
}

impl PauliIterSpec {
    fn from_work_items(kind: PauliIterKind, work_items: u64) -> Result<Self, WorkerError> {
        match kind {
            PauliIterKind::Range => range_spec(work_items),
            PauliIterKind::Singleton => singleton_spec(work_items),
        }
    }

    fn input_fields(self) -> [u64; 8] {
        [
            self.width,
            self.min_weight,
            self.max_weight,
            self.axis_mask,
            self.outputs_per_iteration,
            self.kind.marker(),
            self.output_cap,
            PAULI_ITER_PUBLIC_QUBIT_CAP,
        ]
    }

    fn build(self) -> Result<PauliStringIterator, WorkerError> {
        let width = usize::try_from(self.width)
            .map_err(|_| WorkerError::PauliIterWidthRange(self.width))?;
        let min_weight = usize::try_from(self.min_weight)
            .map_err(|_| WorkerError::PauliIterWidthRange(self.min_weight))?;
        let max_weight = usize::try_from(self.max_weight)
            .map_err(|_| WorkerError::PauliIterWidthRange(self.max_weight))?;
        Ok(PauliStringIterator::new(
            width,
            min_weight,
            max_weight,
            self.axis_mask & X_MASK != 0,
            self.axis_mask & Y_MASK != 0,
            self.axis_mask & Z_MASK != 0,
        )?)
    }
}

pub(super) struct PauliIterFixture {
    spec: PauliIterSpec,
    final_result_digest: [u64; 4],
    observed_outputs: u64,
    observed_width_checksum: u64,
    pub(super) input_bytes: u64,
    pub(super) input_digest: [u64; 4],
}

impl PauliIterFixture {
    pub(super) fn prepare(
        kind: PauliIterKind,
        work_items: u64,
        work_count: u64,
    ) -> Result<Self, WorkerError> {
        let spec = PauliIterSpec::from_work_items(kind, work_items)?;
        work_count
            .checked_mul(spec.width)
            .ok_or(WorkerError::PauliIterWidthChecksumOverflow)?;
        let input_digest = byte_digest_words(&spec.input_fields());
        let mut validation = spec.build()?;
        let (outputs, width_checksum, final_result_digest) =
            validate_traversal(&mut validation, spec.outputs_per_iteration)?;
        let expected_width_checksum = spec
            .outputs_per_iteration
            .checked_mul(spec.width)
            .ok_or(WorkerError::PauliIterWidthChecksumOverflow)?;
        if outputs != spec.outputs_per_iteration || width_checksum != expected_width_checksum {
            return Err(WorkerError::PauliIterValidation {
                workload: kind.workload(),
                expected_outputs: spec.outputs_per_iteration,
                actual_outputs: outputs,
                expected_width_checksum,
                actual_width_checksum: width_checksum,
            });
        }
        Ok(Self {
            spec,
            final_result_digest,
            observed_outputs: 0,
            observed_width_checksum: 0,
            input_bytes: INPUT_BYTES,
            input_digest,
        })
    }

    pub(super) fn execute(&mut self, iterations: u64) -> Result<(), WorkerError> {
        let mut observed_outputs = 0_u64;
        let mut observed_width_checksum = 0_u64;
        for _ in 0..iterations {
            compiler_fence(Ordering::SeqCst);
            let mut iterator = black_box(self.spec).build()?;
            let (outputs, width_checksum) = traverse(black_box(&mut iterator))?;
            compiler_fence(Ordering::SeqCst);
            observed_outputs = observed_outputs
                .checked_add(black_box(outputs))
                .ok_or(WorkerError::WorkOverflow)?;
            observed_width_checksum = observed_width_checksum
                .checked_add(black_box(width_checksum))
                .ok_or(WorkerError::PauliIterWidthChecksumOverflow)?;
        }
        self.observed_outputs = black_box(observed_outputs);
        self.observed_width_checksum = black_box(observed_width_checksum);
        Ok(())
    }

    pub(super) fn output_digest(
        &self,
        iterations: u64,
        work_count: u64,
    ) -> Result<[u64; 4], WorkerError> {
        let expected_width_checksum = work_count
            .checked_mul(self.spec.width)
            .ok_or(WorkerError::PauliIterWidthChecksumOverflow)?;
        if self.observed_outputs != work_count
            || self.observed_width_checksum != expected_width_checksum
        {
            return Err(WorkerError::PauliIterObserved {
                workload: self.spec.kind.workload(),
                expected_outputs: work_count,
                actual_outputs: self.observed_outputs,
                expected_width_checksum,
                actual_width_checksum: self.observed_width_checksum,
            });
        }
        let mut fields = Vec::with_capacity(18);
        fields.extend([
            iterations,
            work_count,
            self.spec.width,
            self.spec.kind.marker(),
            self.spec.min_weight,
            self.spec.max_weight,
            self.spec.axis_mask,
            self.spec.outputs_per_iteration,
            self.observed_outputs,
            self.observed_width_checksum,
        ]);
        fields.extend(self.input_digest);
        fields.extend(self.final_result_digest);
        Ok(byte_digest_words(&fields))
    }
}

fn validate_traversal(
    iterator: &mut PauliStringIterator,
    expected_outputs: u64,
) -> Result<(u64, u64, [u64; 4]), WorkerError> {
    let mut outputs = 0_u64;
    let mut width_checksum = 0_u64;
    let mut final_result_digest = None;
    while iterator.iter_next() {
        outputs = outputs.checked_add(1).ok_or(WorkerError::WorkOverflow)?;
        let result = iterator.result();
        width_checksum = width_checksum
            .checked_add(
                u64::try_from(result.len()).map_err(|_| WorkerError::PauliIterResultWidthRange)?,
            )
            .ok_or(WorkerError::PauliIterWidthChecksumOverflow)?;
        if outputs == expected_outputs {
            final_result_digest = Some(byte_digest_word_pair(result.x_bits(), result.z_bits()));
        }
    }
    let final_result_digest =
        final_result_digest.ok_or(WorkerError::PauliIterMissingFinalResult)?;
    Ok((outputs, width_checksum, final_result_digest))
}

fn traverse(iterator: &mut PauliStringIterator) -> Result<(u64, u64), WorkerError> {
    let mut outputs = 0_u64;
    let mut width_checksum = 0_u64;
    while iterator.iter_next() {
        outputs = outputs.checked_add(1).ok_or(WorkerError::WorkOverflow)?;
        width_checksum = width_checksum
            .checked_add(
                u64::try_from(black_box(iterator.result()).len())
                    .map_err(|_| WorkerError::PauliIterResultWidthRange)?,
            )
            .ok_or(WorkerError::PauliIterWidthChecksumOverflow)?;
    }
    black_box((outputs, width_checksum));
    Ok((outputs, width_checksum))
}

fn range_spec(work_items: u64) -> Result<PauliIterSpec, WorkerError> {
    for width in 2..=23 {
        let outputs = range_output_count(width)?;
        if outputs == work_items {
            if outputs > PAULI_ITER_RANGE_OUTPUT_CAP {
                return Err(WorkerError::PauliIterOutputLimit {
                    workload: PauliIterKind::Range.workload(),
                    actual: outputs,
                    maximum: PAULI_ITER_RANGE_OUTPUT_CAP,
                });
            }
            return Ok(PauliIterSpec {
                kind: PauliIterKind::Range,
                width,
                min_weight: 2,
                max_weight: 5,
                axis_mask: X_MASK | Z_MASK,
                outputs_per_iteration: outputs,
                output_cap: PAULI_ITER_RANGE_OUTPUT_CAP,
            });
        }
    }
    if work_items > PAULI_ITER_RANGE_OUTPUT_CAP {
        Err(WorkerError::PauliIterOutputLimit {
            workload: PauliIterKind::Range.workload(),
            actual: work_items,
            maximum: PAULI_ITER_RANGE_OUTPUT_CAP,
        })
    } else {
        Err(WorkerError::PauliIterWorkShape {
            workload: PauliIterKind::Range.workload(),
            actual: work_items,
        })
    }
}

fn singleton_spec(work_items: u64) -> Result<PauliIterSpec, WorkerError> {
    if !work_items.is_multiple_of(3) {
        return Err(WorkerError::PauliIterWorkShape {
            workload: PauliIterKind::Singleton.workload(),
            actual: work_items,
        });
    }
    let width = work_items / 3;
    if width == 0 {
        return Err(WorkerError::PauliIterWorkShape {
            workload: PauliIterKind::Singleton.workload(),
            actual: work_items,
        });
    }
    if width > PAULI_ITER_PUBLIC_QUBIT_CAP {
        return Err(WorkerError::PauliIterWidthLimit {
            workload: PauliIterKind::Singleton.workload(),
            actual: width,
            maximum: PAULI_ITER_PUBLIC_QUBIT_CAP,
        });
    }
    Ok(PauliIterSpec {
        kind: PauliIterKind::Singleton,
        width,
        min_weight: 1,
        max_weight: 1,
        axis_mask: X_MASK | Y_MASK | Z_MASK,
        outputs_per_iteration: work_items,
        output_cap: PAULI_ITER_SINGLETON_OUTPUT_CAP,
    })
}

fn range_output_count(width: u64) -> Result<u64, WorkerError> {
    let mut outputs = 0_u64;
    for weight in 2..=5_u64.min(width) {
        let combinations = choose(width, weight)?;
        let basis_products = 1_u64
            .checked_shl(u32::try_from(weight).map_err(|_| WorkerError::PauliIterCountOverflow)?)
            .ok_or(WorkerError::PauliIterCountOverflow)?;
        outputs = outputs
            .checked_add(
                combinations
                    .checked_mul(basis_products)
                    .ok_or(WorkerError::PauliIterCountOverflow)?,
            )
            .ok_or(WorkerError::PauliIterCountOverflow)?;
    }
    Ok(outputs)
}

fn choose(n: u64, k: u64) -> Result<u64, WorkerError> {
    let k = k.min(n - k);
    let mut result = 1_u64;
    for index in 1..=k {
        result = result
            .checked_mul(n - k + index)
            .ok_or(WorkerError::PauliIterCountOverflow)?
            / index;
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_counts_and_caps_are_source_owned() {
        assert_eq!(range_output_count(5).unwrap(), 232);
        assert_eq!(range_output_count(11).unwrap(), 21_604);
        assert_eq!(range_output_count(22).unwrap(), 972_972);
        assert_eq!(range_output_count(23).unwrap(), 1_233_628);
        assert!(range_spec(232).is_ok());
        assert!(range_spec(21_604).is_ok());
        assert!(range_spec(972_972).is_ok());
        assert!(matches!(
            range_spec(1_233_628),
            Err(WorkerError::PauliIterOutputLimit { .. })
        ));
        assert!(matches!(
            range_spec(233),
            Err(WorkerError::PauliIterWorkShape { .. })
        ));
    }

    #[test]
    fn singleton_counts_and_caps_are_source_owned() {
        for outputs in [3_000, 96_000, 3_000_000, 3_145_728] {
            assert!(singleton_spec(outputs).is_ok());
        }
        assert!(matches!(
            singleton_spec(3_145_731),
            Err(WorkerError::PauliIterWidthLimit { .. })
        ));
        assert!(matches!(
            singleton_spec(3_001),
            Err(WorkerError::PauliIterWorkShape { .. })
        ));
    }

    #[test]
    fn fixture_output_binds_observed_traversal() {
        for (kind, work_items) in [
            (PauliIterKind::Range, 232),
            (PauliIterKind::Singleton, 3_000),
        ] {
            let mut fixture = PauliIterFixture::prepare(kind, work_items, work_items).unwrap();
            fixture.execute(1).unwrap();
            assert!(fixture.output_digest(1, work_items).is_ok());
        }
    }

    #[cfg(feature = "count-allocations")]
    #[test]
    fn pauli_iterator_callback_allocation_contract_is_bounded_at_every_scale() {
        for (kind, work_items) in [
            (PauliIterKind::Range, 232),
            (PauliIterKind::Range, 21_604),
            (PauliIterKind::Range, 972_972),
            (PauliIterKind::Singleton, 3_000),
            (PauliIterKind::Singleton, 96_000),
            (PauliIterKind::Singleton, 3_000_000),
            (PauliIterKind::Singleton, 3_145_728),
        ] {
            let mut fixture =
                PauliIterFixture::prepare(kind, work_items, work_items).expect("prepare fixture");
            let mut execution = None;
            let allocations = allocation_counter::measure(|| {
                execution = Some(fixture.execute(1));
            });
            execution
                .expect("execution result")
                .expect("execute fixture");
            let (expected_calls, expected_bytes) = match kind {
                PauliIterKind::Range => (5, 120),
                PauliIterKind::Singleton => {
                    let result_bytes = fixture.spec.width.div_ceil(64) * 8 * 2;
                    (4, result_bytes + 40)
                }
            };
            assert_eq!(
                allocations.count_total, expected_calls,
                "kind={kind:?} work_items={work_items} {allocations:?}"
            );
            assert_eq!(
                allocations.bytes_total, expected_bytes,
                "kind={kind:?} work_items={work_items} {allocations:?}"
            );
        }
    }
}
