use std::hint::black_box;

use stab_core::DetectorErrorModel;

use super::{WorkerError, byte_digest, semantic_digest};

pub(in crate::qualification::runtime) const DEM_CYCLE_ITEMS: u64 = 8;
pub(in crate::qualification::runtime) const DEM_MAX_ITEMS: u64 = 524_288;

const DEM_CYCLE_TEXT: &str = concat!(
    "error(0.125) D0\n",
    "error[edge](0.25) D1 D2 L0 ^ D3\n",
    "detector(0.5, 1) D4\n",
    "logical_observable L1\n",
    "shift_detectors(1.5, 3) 5\n",
    "detector[tagged] D2\n",
    "repeat[loop] 3 {\n",
    "    error(0.375) D0 D1\n",
    "    shift_detectors 2\n",
    "}\n",
    "error(0.0625) D5 ^ L2\n",
);
const DEM_CYCLE_BYTES: u64 = 222;

pub(super) struct DemFixture {
    text: String,
}

impl DemFixture {
    pub(super) fn prepare(top_level_items: u64) -> Result<Self, WorkerError> {
        if top_level_items > DEM_MAX_ITEMS {
            return Err(WorkerError::DemItemLimit {
                actual: top_level_items,
                maximum: DEM_MAX_ITEMS,
            });
        }
        if top_level_items == 0 || !top_level_items.is_multiple_of(DEM_CYCLE_ITEMS) {
            return Err(WorkerError::DemItemShape {
                actual: top_level_items,
                cycle: DEM_CYCLE_ITEMS,
            });
        }
        let cycles = top_level_items / DEM_CYCLE_ITEMS;
        let byte_count = cycles
            .checked_mul(DEM_CYCLE_BYTES)
            .ok_or(WorkerError::DemFixtureOverflow)?;
        let capacity =
            usize::try_from(byte_count).map_err(|_| WorkerError::DemItemRange(top_level_items))?;
        let cycle_count =
            usize::try_from(cycles).map_err(|_| WorkerError::DemItemRange(top_level_items))?;
        let mut text = String::new();
        text.try_reserve_exact(capacity)
            .map_err(WorkerError::DemFixtureAllocation)?;
        for _ in 0..cycle_count {
            text.push_str(DEM_CYCLE_TEXT);
        }
        if text.len() != capacity {
            return Err(WorkerError::DemFixtureSize {
                actual: text.len(),
                expected: capacity,
            });
        }
        Ok(Self { text })
    }

    pub(super) fn text(&self) -> &str {
        &self.text
    }

    pub(super) fn input_bytes(&self) -> Result<u64, WorkerError> {
        u64::try_from(self.text.len()).map_err(|_| WorkerError::InputSizeRange)
    }

    pub(super) fn input_digest(&self) -> String {
        semantic_digest(byte_digest(self.text.as_bytes()))
    }

    pub(super) fn validate_canonical(&self, canonical: &str) -> Result<String, WorkerError> {
        let expected = self
            .text
            .strip_suffix('\n')
            .ok_or(WorkerError::DemFixtureTerminalNewline)?;
        let actual = canonical.strip_suffix('\n').unwrap_or(canonical);
        if actual != expected {
            let first_difference = actual
                .bytes()
                .zip(expected.bytes())
                .position(|(left, right)| left != right)
                .unwrap_or_else(|| actual.len().min(expected.len()));
            return Err(WorkerError::DemCanonicalMismatch {
                actual_bytes: actual.len(),
                expected_bytes: expected.len(),
                first_difference,
            });
        }
        Ok(semantic_digest(byte_digest(actual.as_bytes())))
    }
}

pub(super) fn parse(
    iterations: u64,
    fixture: &DemFixture,
) -> Result<DetectorErrorModel, WorkerError> {
    let mut parsed = DetectorErrorModel::new();
    for _ in 0..iterations {
        parsed = DetectorErrorModel::from_dem_str(black_box(fixture.text()))?;
        black_box(&parsed);
    }
    Ok(parsed)
}

pub(super) fn serialize(iterations: u64, model: &DetectorErrorModel) -> String {
    let mut canonical = String::new();
    for _ in 0..iterations {
        canonical = black_box(black_box(model).to_dem_string());
    }
    canonical
}

#[cfg(test)]
mod tests {
    use super::*;

    const CASES: [(u64, u64, u64, u64, &str, &str); 4] = [
        (
            64,
            1_776,
            88,
            1_775,
            "fe2dab309c0d63109124cbaae8fadfe7b72ec523bd1c2252e1a7fc20f1b0d773",
            "02ad6cd3910a69ae416bdaadeb16126fdf813aba8154bb682bf75a01c609093f",
        ),
        (
            4_096,
            113_664,
            5_632,
            113_663,
            "9de340076c00f2c1cae6130f3393c556e8d892d2dc25519b0b93cda239d0e01c",
            "c8a5116b4e1748d63c0baf8b9eb378d1c53e986b983b405aab7cc7da417561a9",
        ),
        (
            65_536,
            1_818_624,
            90_112,
            1_818_623,
            "240d4c9e8e0d7a24e5ad6dea5421fe19906942430c4d994c9b1fcf55fa939716",
            "bf2206ba69567e3a48c9b74a0cd22b97ef7a5d11bd0297afc428462c237fef38",
        ),
        (
            DEM_MAX_ITEMS,
            14_548_992,
            720_896,
            14_548_991,
            "127e88c725aa88acdea3be1aed5369af43166e27365e1dbd11dbe89c8e807789",
            "5bd41410a3ee8859fa7589abe6a20fa61d4e5c06e08105f60a5f3aa474d478b2",
        ),
    ];

    #[test]
    fn fixture_cycle_is_exact_and_scale_identities_are_frozen() {
        assert_eq!(DEM_CYCLE_TEXT.lines().count(), 11);
        assert_eq!(DEM_CYCLE_TEXT.len(), 222);
        assert!(DEM_CYCLE_TEXT.contains("error[edge](0.25) D1 D2 L0 ^ D3\n"));
        assert!(DEM_CYCLE_TEXT.contains("repeat[loop] 3 {\n"));

        for (items, bytes, physical_lines, output_bytes, input_digest, output_digest) in CASES {
            let fixture = DemFixture::prepare(items).expect("valid DEM fixture");
            assert_eq!(fixture.input_bytes().expect("input bytes"), bytes);
            assert_eq!(fixture.text().lines().count() as u64, physical_lines);
            let canonical_bytes = fixture
                .text()
                .strip_suffix('\n')
                .expect("terminal newline")
                .len();
            assert_eq!(
                u64::try_from(canonical_bytes).expect("canonical byte count"),
                output_bytes
            );
            assert_eq!(fixture.input_digest(), input_digest);
            assert_eq!(
                fixture
                    .validate_canonical(fixture.text())
                    .expect("canonical fixture"),
                output_digest
            );
        }
    }

    #[test]
    fn parse_and_serialize_bind_odd_even_and_normalized_output() {
        let fixture = DemFixture::prepare(64).expect("fixture");
        let odd = parse(1, &fixture).expect("odd parse");
        let even = parse(2, &fixture).expect("even parse");
        let odd_text = serialize(1, &odd);
        let even_text = serialize(2, &even);

        assert_eq!(odd, even);
        assert_eq!(odd_text, even_text);
        assert_eq!(odd_text.len(), fixture.text().len());
        assert_eq!(
            fixture.validate_canonical(&odd_text).expect("odd output"),
            CASES[0].5
        );
        assert_eq!(
            fixture
                .validate_canonical(odd_text.trim_end_matches('\n'))
                .expect("Stim-style terminal newline"),
            CASES[0].5
        );
    }

    #[test]
    fn fixture_rejects_invalid_shapes_before_allocation() {
        assert!(matches!(
            DemFixture::prepare(0),
            Err(WorkerError::DemItemShape { .. })
        ));
        assert!(matches!(
            DemFixture::prepare(65),
            Err(WorkerError::DemItemShape { .. })
        ));
        assert!(matches!(
            DemFixture::prepare(DEM_MAX_ITEMS + 1),
            Err(WorkerError::DemItemLimit { .. })
        ));
    }

    #[test]
    fn accepted_maximum_constructs_parses_and_serializes() {
        let fixture = DemFixture::prepare(DEM_MAX_ITEMS).expect("maximum fixture");
        let model = parse(1, &fixture).expect("maximum parse");
        let canonical = serialize(1, &model);

        assert_eq!(
            fixture
                .validate_canonical(&canonical)
                .expect("maximum canonical output"),
            CASES[3].5
        );
    }

    #[test]
    fn canonical_validation_rejects_nonterminal_differences() {
        let fixture = DemFixture::prepare(64).expect("fixture");
        let changed = fixture.text().replacen("D0", "D9", 1);

        assert!(matches!(
            fixture.validate_canonical(&changed),
            Err(WorkerError::DemCanonicalMismatch { .. })
        ));
    }
}
