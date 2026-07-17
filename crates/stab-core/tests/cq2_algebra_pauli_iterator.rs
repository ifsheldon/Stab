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
const RANGE_SEQUENCE_DIGESTS: [[u64; 4]; 3] = [
    [
        12_446_789_277_131_027_186,
        13_051_800_182_227_271_736,
        8_913_968_126_161_186_152,
        615_325_163_075_576_630,
    ],
    [
        8_534_633_740_512_535_725,
        4_085_474_010_443_026_331,
        18_206_560_189_976_024_663,
        5_389_641_570_020_862_266,
    ],
    [
        4_583_398_351_909_552_890,
        10_692_845_233_139_100_947,
        16_030_953_713_442_756_085,
        15_103_561_035_003_883_458,
    ],
];
const SINGLETON_SEQUENCE_DIGESTS: [[u64; 4]; 3] = [
    [
        10_062_400_317_628_243_932,
        8_094_961_272_711_778_612,
        3_304_519_595_989_212_289,
        16_690_026_315_542_598_811,
    ],
    [
        4_677_313_449_531_378_386,
        3_680_951_585_136_897_876,
        14_549_473_997_558_646_074,
        16_804_399_793_348_274_468,
    ],
    [
        13_916_549_690_708_414_893,
        12_377_616_979_003_074_289,
        3_252_611_672_401_433_999,
        16_537_992_152_957_202_582,
    ],
];
const WORD_BOUNDARY_SEQUENCE_DIGESTS: [[u64; 4]; 6] = [
    [
        3_090_780_283_937_011_957,
        18_080_691_007_433_190_073,
        12_352_157_874_546_323_577,
        18_307_326_670_971_381_005,
    ],
    [
        7_056_300_548_564_720_962,
        8_403_608_438_459_964_027,
        17_287_770_800_484_319_015,
        13_775_059_398_034_614_954,
    ],
    [
        10_463_140_286_969_830_809,
        15_415_621_748_824_166_523,
        7_485_816_268_919_135_903,
        96_674_164_544_979_753,
    ],
    [
        7_797_370_015_588_931_115,
        8_369_731_014_461_536_336,
        4_511_031_657_567_620_864,
        7_905_674_535_924_389_521,
    ],
    [
        18_225_252_125_383_721_054,
        2_001_592_867_943_306_302,
        12_483_468_837_754_404_859,
        15_310_441_896_764_087_119,
    ],
    [
        4_822_836_063_366_409_570,
        6_190_669_524_495_593_407,
        18_194_907_299_641_788_014,
        5_590_972_172_499_269_586,
    ],
];

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
    for ((spec, expected_count), expected_digest) in RANGE_SPECS
        .into_iter()
        .zip(expected_range_counts)
        .zip(RANGE_SEQUENCE_DIGESTS)
    {
        let actual = summarize_actual(spec)?;
        let reference = summarize_reference(spec)?;
        assert_eq!(actual, reference, "{spec:?}");
        assert_eq!(actual.output_count, expected_count, "{spec:?}");
        assert_frozen_range_summary(&actual, spec, expected_digest)?;
    }

    let expected_singleton_counts = [3_000, 96_000, 3_000_000];
    for ((spec, expected_count), expected_digest) in SINGLETON_SPECS
        .into_iter()
        .zip(expected_singleton_counts)
        .zip(SINGLETON_SEQUENCE_DIGESTS)
    {
        let actual = summarize_actual(spec)?;
        let reference = summarize_reference(spec)?;
        assert_eq!(actual, reference, "{spec:?}");
        assert_eq!(actual.output_count, expected_count, "{spec:?}");
        assert_frozen_singleton_summary(&actual, spec, expected_digest)?;
    }

    for (width, expected_digest) in WORD_BOUNDARIES
        .into_iter()
        .zip(WORD_BOUNDARY_SEQUENCE_DIGESTS)
    {
        let spec = IteratorSpec::xyz_singleton(width);
        let actual = summarize_actual(spec)?;
        assert_eq!(actual, summarize_reference(spec)?);
        assert_frozen_singleton_summary(&actual, spec, expected_digest)?;
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

fn assert_frozen_range_summary(
    summary: &SequenceSummary,
    spec: IteratorSpec,
    expected_digest: [u64; 4],
) -> TestResult<()> {
    assert_eq!(summary.digest, expected_digest, "{spec:?}");
    assert_eq!(
        summary.total_result_width,
        summary
            .output_count
            .checked_mul(u64::try_from(spec.num_qubits)?)
            .ok_or(QualificationTestError::ArithmeticOverflow)?,
        "{spec:?}"
    );
    assert_eq!(
        summary.first,
        [(0, PauliBasis::X), (1, PauliBasis::X)],
        "{spec:?}"
    );
    assert_eq!(
        summary.last,
        (spec.num_qubits - 5..spec.num_qubits)
            .map(|position| (position, PauliBasis::Z))
            .collect::<Vec<_>>(),
        "{spec:?}"
    );
    Ok(())
}

fn assert_frozen_singleton_summary(
    summary: &SequenceSummary,
    spec: IteratorSpec,
    expected_digest: [u64; 4],
) -> TestResult<()> {
    let width = u64::try_from(spec.num_qubits)?;
    assert_eq!(summary.output_count, width * 3, "{spec:?}");
    assert_eq!(summary.total_result_width, width * width * 3, "{spec:?}");
    assert_eq!(summary.digest, expected_digest, "{spec:?}");
    assert_eq!(summary.first, [(0, PauliBasis::X)], "{spec:?}");
    assert_eq!(
        summary.last,
        [(spec.num_qubits - 1, PauliBasis::Z)],
        "{spec:?}"
    );
    Ok(())
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
