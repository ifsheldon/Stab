#![allow(
    clippy::panic_in_result_fn,
    reason = "integration tests use direct assertions for semantic mismatch diagnostics"
)]

use stab_core::{
    PauliBasis, PauliSign, PauliStringIterator, StabilizerError, StabilizerResource,
    StabilizerResult,
};
use thiserror::Error;

const RANGE_SPECS: [IteratorSpec; 3] = [
    IteratorSpec::xz_range(5),
    IteratorSpec::xz_range(11),
    IteratorSpec::xz_range(22),
];
const SINGLETON_SPECS: [IteratorSpec; 3] = [
    IteratorSpec::xyz_singleton(1_000),
    IteratorSpec::xyz_singleton(32_000),
    IteratorSpec::xyz_singleton(1_000_000),
];
const WORD_BOUNDARIES: [usize; 6] = [63, 64, 65, 255, 256, 257];

#[test]
fn cq2_algebra_pauli_iterator_runtime_contract_matches_independent_reference() -> TestResult<()> {
    let exhaustive_specs = [
        IteratorSpec::new(3, 0, 0, true, true, true),
        IteratorSpec::new(3, 0, 2, true, true, true),
        IteratorSpec::new(4, 1, 3, true, false, true),
        IteratorSpec::new(4, 0, 4, false, true, true),
        IteratorSpec::new(4, 0, 4, true, true, false),
        IteratorSpec::new(4, 0, 4, false, false, false),
    ];
    for spec in exhaustive_specs {
        assert_eq!(collect_actual(spec)?, collect_reference(spec)?, "{spec:?}");
    }

    let expected_range_counts = [232, 21_604, 972_972];
    for (spec, expected_count) in RANGE_SPECS.into_iter().zip(expected_range_counts) {
        let actual = summarize_actual(spec)?;
        let reference = summarize_reference(spec)?;
        assert_eq!(actual, reference, "{spec:?}");
        assert_eq!(actual.output_count, expected_count, "{spec:?}");
    }

    let expected_singleton_counts = [3_000, 96_000, 3_000_000];
    for (spec, expected_count) in SINGLETON_SPECS.into_iter().zip(expected_singleton_counts) {
        let actual = summarize_actual(spec)?;
        let reference = summarize_reference(spec)?;
        assert_eq!(actual, reference, "{spec:?}");
        assert_eq!(actual.output_count, expected_count, "{spec:?}");
    }

    for width in WORD_BOUNDARIES {
        let spec = IteratorSpec::xyz_singleton(width);
        assert_eq!(summarize_actual(spec)?, summarize_reference(spec)?);
    }

    let restart_spec = IteratorSpec::xyz_singleton(32_000);
    let mut iterator = restart_spec.build()?;
    let first = summarize_existing(&mut iterator, restart_spec)?;
    iterator.restart();
    let restarted = summarize_existing(&mut iterator, restart_spec)?;
    assert_eq!(restarted, first);
    assert_eq!(restarted, summarize_reference(restart_spec)?);

    let resource = StabilizerResource::PauliQubits;
    let accepted = PauliStringIterator::new(resource.limit(), 1, 1, true, true, true)?;
    assert_eq!(accepted.result().len(), resource.limit());
    assert!(matches!(
        PauliStringIterator::new(resource.limit() + 1, 1, 1, true, true, true),
        Err(StabilizerError::ResourceLimitExceeded {
            resource: StabilizerResource::PauliQubits,
            requested,
            limit,
        }) if requested == resource.limit() + 1 && limit == resource.limit()
    ));
    Ok(())
}

type TestResult<T> = Result<T, QualificationTestError>;

#[derive(Debug, Error)]
enum QualificationTestError {
    #[error(transparent)]
    Stabilizer(#[from] StabilizerError),
    #[error(transparent)]
    IntegerRange(#[from] std::num::TryFromIntError),
    #[error("Pauli iterator qualification arithmetic overflowed")]
    ArithmeticOverflow,
}

#[derive(Clone, Copy, Debug)]
struct IteratorSpec {
    num_qubits: usize,
    min_weight: usize,
    max_weight: usize,
    allow_x: bool,
    allow_y: bool,
    allow_z: bool,
}

impl IteratorSpec {
    const fn new(
        num_qubits: usize,
        min_weight: usize,
        max_weight: usize,
        allow_x: bool,
        allow_y: bool,
        allow_z: bool,
    ) -> Self {
        Self {
            num_qubits,
            min_weight,
            max_weight,
            allow_x,
            allow_y,
            allow_z,
        }
    }

    const fn xz_range(num_qubits: usize) -> Self {
        Self::new(num_qubits, 2, 5, true, false, true)
    }

    const fn xyz_singleton(num_qubits: usize) -> Self {
        Self::new(num_qubits, 1, 1, true, true, true)
    }

    fn build(self) -> StabilizerResult<PauliStringIterator> {
        PauliStringIterator::new(
            self.num_qubits,
            self.min_weight,
            self.max_weight,
            self.allow_x,
            self.allow_y,
            self.allow_z,
        )
    }

    fn allowed_bases(self) -> Vec<PauliBasis> {
        let mut bases = Vec::with_capacity(3);
        if self.allow_x {
            bases.push(PauliBasis::X);
        }
        if self.allow_y {
            bases.push(PauliBasis::Y);
        }
        if self.allow_z {
            bases.push(PauliBasis::Z);
        }
        bases
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SequenceSummary {
    output_count: u64,
    total_result_width: u64,
    digest: [u64; 4],
    first: Vec<(usize, PauliBasis)>,
    last: Vec<(usize, PauliBasis)>,
}

impl SequenceSummary {
    fn new() -> Self {
        Self {
            output_count: 0,
            total_result_width: 0,
            digest: [
                0x243f_6a88_85a3_08d3,
                0x1319_8a2e_0370_7344,
                0xa409_3822_299f_31d0,
                0x082e_fa98_ec4e_6c89,
            ],
            first: Vec::new(),
            last: Vec::new(),
        }
    }

    fn record_terms(
        &mut self,
        result_width: usize,
        term_count: usize,
        terms: impl IntoIterator<Item = (usize, PauliBasis)>,
    ) -> TestResult<()> {
        let is_first = self.output_count == 0;
        self.output_count = self
            .output_count
            .checked_add(1)
            .ok_or(QualificationTestError::ArithmeticOverflow)?;
        let result_width = u64::try_from(result_width)?;
        self.total_result_width = self
            .total_result_width
            .checked_add(result_width)
            .ok_or(QualificationTestError::ArithmeticOverflow)?;
        self.last.clear();

        self.mix(0xfedc_ba98_7654_3210);
        self.mix(result_width);
        self.mix(u64::try_from(term_count)?);
        for (position, basis) in terms {
            if is_first {
                self.first.push((position, basis));
            }
            self.last.push((position, basis));
            self.mix(u64::try_from(position)?);
            self.mix(match basis {
                PauliBasis::I => 0,
                PauliBasis::X => 1,
                PauliBasis::Y => 2,
                PauliBasis::Z => 3,
            });
        }
        Ok(())
    }

    fn mix(&mut self, value: u64) {
        const LANES: [(u64, u32); 4] = [(0, 11), (1, 12), (2, 13), (3, 14)];
        for ((lane, rotation), state) in LANES.into_iter().zip(&mut self.digest) {
            *state ^= value.wrapping_add(lane.wrapping_mul(0x9e37_79b9_7f4a_7c15));
            *state = state
                .rotate_left(rotation)
                .wrapping_mul(0x0100_0000_01b3 + lane * 2);
        }
    }
}

fn collect_actual(spec: IteratorSpec) -> TestResult<Vec<Vec<(usize, PauliBasis)>>> {
    let mut iterator = spec.build()?;
    let result_address = std::ptr::from_ref(iterator.result());
    let mut values = Vec::new();
    while iterator.iter_next() {
        assert_eq!(std::ptr::from_ref(iterator.result()), result_address);
        assert_eq!(iterator.result().sign(), PauliSign::Plus);
        values.push(iterator.result().active_terms().collect());
    }
    Ok(values)
}

fn collect_reference(spec: IteratorSpec) -> TestResult<Vec<Vec<(usize, PauliBasis)>>> {
    let mut values = Vec::new();
    visit_reference(spec, |positions, bases| {
        values.push(
            positions
                .iter()
                .copied()
                .zip(bases.iter().copied())
                .collect(),
        );
        Ok(())
    })?;
    Ok(values)
}

fn summarize_actual(spec: IteratorSpec) -> TestResult<SequenceSummary> {
    summarize_existing(&mut spec.build()?, spec)
}

fn summarize_existing(
    iterator: &mut PauliStringIterator,
    spec: IteratorSpec,
) -> TestResult<SequenceSummary> {
    if spec.min_weight == 1 && spec.max_weight == 1 {
        return summarize_singleton_existing(iterator, spec);
    }
    let mut summary = SequenceSummary::new();
    let mut terms = Vec::with_capacity(5);
    while iterator.iter_next() {
        let result = iterator.result();
        assert_eq!(result.len(), spec.num_qubits);
        assert_eq!(result.sign(), PauliSign::Plus);
        terms.clear();
        terms.extend(result.active_terms());
        summary.record_terms(spec.num_qubits, terms.len(), terms.iter().copied())?;
    }
    Ok(summary)
}

fn summarize_singleton_existing(
    iterator: &mut PauliStringIterator,
    spec: IteratorSpec,
) -> TestResult<SequenceSummary> {
    let allowed_bases = spec.allowed_bases();
    let mut summary = SequenceSummary::new();
    for position in 0..spec.num_qubits {
        for (basis_index, basis) in allowed_bases.iter().copied().enumerate() {
            assert!(
                iterator.iter_next(),
                "{spec:?} stopped at {position}:{basis_index}"
            );
            let result = iterator.result();
            assert_eq!(result.len(), spec.num_qubits);
            assert_eq!(result.sign(), PauliSign::Plus);
            assert_eq!(result.get(position), Some(basis));
            if basis_index == 0 && position > 0 {
                assert_eq!(result.get(position - 1), Some(PauliBasis::I));
            }
            summary.record_terms(spec.num_qubits, 1, std::iter::once((position, basis)))?;
        }
    }
    assert!(!iterator.iter_next(), "{spec:?} produced an extra value");
    let final_terms = iterator.result().active_terms().collect::<Vec<_>>();
    assert_eq!(final_terms, summary.last);
    Ok(summary)
}

fn summarize_reference(spec: IteratorSpec) -> TestResult<SequenceSummary> {
    let mut summary = SequenceSummary::new();
    visit_reference(spec, |positions, bases| {
        summary.record_terms(
            spec.num_qubits,
            positions.len(),
            positions.iter().copied().zip(bases.iter().copied()),
        )
    })?;
    Ok(summary)
}

fn visit_reference(
    spec: IteratorSpec,
    mut visit: impl FnMut(&[usize], &[PauliBasis]) -> TestResult<()>,
) -> TestResult<()> {
    if spec.max_weight < spec.min_weight {
        return Ok(());
    }
    let bases = spec.allowed_bases();
    for weight in spec.min_weight..=spec.max_weight.min(spec.num_qubits) {
        if weight == 0 {
            visit(&[], &[])?;
            continue;
        }
        if bases.is_empty() {
            continue;
        }
        let mut positions = Vec::with_capacity(weight);
        visit_combinations(
            spec.num_qubits,
            weight,
            0,
            &mut positions,
            &bases,
            &mut visit,
        )?;
    }
    Ok(())
}

fn visit_combinations(
    num_qubits: usize,
    weight: usize,
    start: usize,
    positions: &mut Vec<usize>,
    bases: &[PauliBasis],
    visit: &mut impl FnMut(&[usize], &[PauliBasis]) -> TestResult<()>,
) -> TestResult<()> {
    if positions.len() == weight {
        let mut active_bases = Vec::with_capacity(weight);
        return visit_basis_products(positions, bases, &mut active_bases, visit);
    }
    let remaining = weight - positions.len();
    for position in start..=num_qubits - remaining {
        positions.push(position);
        visit_combinations(num_qubits, weight, position + 1, positions, bases, visit)?;
        positions.pop();
    }
    Ok(())
}

fn visit_basis_products(
    positions: &[usize],
    allowed_bases: &[PauliBasis],
    active_bases: &mut Vec<PauliBasis>,
    visit: &mut impl FnMut(&[usize], &[PauliBasis]) -> TestResult<()>,
) -> TestResult<()> {
    if active_bases.len() == positions.len() {
        return visit(positions, active_bases);
    }
    for basis in allowed_bases {
        active_bases.push(*basis);
        visit_basis_products(positions, allowed_bases, active_bases, visit)?;
        active_bases.pop();
    }
    Ok(())
}
