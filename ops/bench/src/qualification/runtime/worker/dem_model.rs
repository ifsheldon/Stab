use std::hint::black_box;

use clap::ValueEnum;
use stab_core::DetectorErrorModel;

use super::{WorkerError, byte_digest, semantic_digest};

pub(in crate::qualification::runtime) const DEM_CYCLE_ITEMS: u64 = 8;
pub(in crate::qualification::runtime) const DEM_MAX_ITEMS: u64 = 524_288;
pub(in crate::qualification::runtime) const DEM_FOLDED_MAX_ITEMS: u64 = 262_144;

const FLAT_ERRORS_CYCLE_TEXT: &str = concat!(
    "error(0.125) D0\n",
    "error(0.25) D1 D2\n",
    "error(0.375) D3 L0\n",
    "error(0.0625) D4 ^ D5\n",
    "error(0.5) D6 D7 D8\n",
    "error(0.03125) D9 L1 ^ D10\n",
    "error(0.75) D11 D12 L2\n",
    "error(0.875) D13 ^ D14 L3\n",
);

const COORDINATE_SPARSE_CYCLE_TEXT: &str = concat!(
    "detector[tag-a](0.5, 1) D1000000\n",
    "logical_observable L100000\n",
    "shift_detectors(1.5, -2, 3) 1000001\n",
    "error[edge](0.25) D0 D1000000 L0 ^ D7\n",
    "detector(2, 3.5) D42\n",
    "error(0.125) D999999 L99999\n",
    "shift_detectors 17\n",
    "detector[tag-b] D1000017\n",
);

const FOLDED_REPEATS_CYCLE_TEXT: &str = concat!(
    "repeat[outer] 1000000 {\n",
    "    repeat[inner] 1024 {\n",
    "        error(0.125) D0 D1000000 L100000\n",
    "        shift_detectors 1000001\n",
    "    }\n",
    "}\n",
);

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(in crate::qualification::runtime) enum DemFamily {
    FlatErrors,
    CoordinateSparse,
    FoldedRepeats,
}

impl DemFamily {
    pub(in crate::qualification::runtime) const ALL: [Self; 3] = [
        Self::FlatErrors,
        Self::CoordinateSparse,
        Self::FoldedRepeats,
    ];

    pub(in crate::qualification::runtime) const fn id(self) -> &'static str {
        match self {
            Self::FlatErrors => "flat-errors",
            Self::CoordinateSparse => "coordinate-sparse",
            Self::FoldedRepeats => "folded-repeats",
        }
    }

    const fn cycle_text(self) -> &'static str {
        match self {
            Self::FlatErrors => FLAT_ERRORS_CYCLE_TEXT,
            Self::CoordinateSparse => COORDINATE_SPARSE_CYCLE_TEXT,
            Self::FoldedRepeats => FOLDED_REPEATS_CYCLE_TEXT,
        }
    }

    pub(in crate::qualification::runtime) const fn cycle_items(self) -> u64 {
        match self {
            Self::FlatErrors | Self::CoordinateSparse => DEM_CYCLE_ITEMS,
            Self::FoldedRepeats => 1,
        }
    }

    pub(in crate::qualification::runtime) const fn maximum_items(self) -> u64 {
        match self {
            Self::FlatErrors | Self::CoordinateSparse => DEM_MAX_ITEMS,
            Self::FoldedRepeats => DEM_FOLDED_MAX_ITEMS,
        }
    }
}

pub(in crate::qualification::runtime) struct DemFixture {
    text: String,
}

impl DemFixture {
    pub(in crate::qualification::runtime) fn prepare(
        family: DemFamily,
        top_level_items: u64,
    ) -> Result<Self, WorkerError> {
        let maximum = family.maximum_items();
        let cycle_items = family.cycle_items();
        let cycle_text = family.cycle_text();
        if top_level_items > maximum {
            return Err(WorkerError::DemItemLimit {
                actual: top_level_items,
                maximum,
            });
        }
        if top_level_items == 0 || !top_level_items.is_multiple_of(cycle_items) {
            return Err(WorkerError::DemItemShape {
                actual: top_level_items,
                cycle: cycle_items,
            });
        }
        let cycles = top_level_items / cycle_items;
        let cycle_bytes =
            u64::try_from(cycle_text.len()).map_err(|_| WorkerError::InputSizeRange)?;
        let byte_count = cycles
            .checked_mul(cycle_bytes)
            .ok_or(WorkerError::DemFixtureOverflow)?;
        let capacity =
            usize::try_from(byte_count).map_err(|_| WorkerError::DemItemRange(top_level_items))?;
        let cycle_count =
            usize::try_from(cycles).map_err(|_| WorkerError::DemItemRange(top_level_items))?;
        let mut text = String::new();
        text.try_reserve_exact(capacity)
            .map_err(WorkerError::DemFixtureAllocation)?;
        for _ in 0..cycle_count {
            text.push_str(cycle_text);
        }
        if text.len() != capacity {
            return Err(WorkerError::DemFixtureSize {
                actual: text.len(),
                expected: capacity,
            });
        }
        Ok(Self { text })
    }

    pub(in crate::qualification::runtime) fn text(&self) -> &str {
        &self.text
    }

    pub(in crate::qualification::runtime) fn input_bytes(&self) -> Result<u64, WorkerError> {
        u64::try_from(self.text.len()).map_err(|_| WorkerError::InputSizeRange)
    }

    pub(in crate::qualification::runtime) fn input_digest(&self) -> String {
        semantic_digest(byte_digest(self.text.as_bytes()))
    }

    pub(in crate::qualification::runtime) fn validate_canonical(
        &self,
        canonical: &str,
    ) -> Result<String, WorkerError> {
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

pub(in crate::qualification::runtime) fn parse(
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

pub(in crate::qualification::runtime) fn serialize(
    iterations: u64,
    model: &DetectorErrorModel,
) -> String {
    let mut canonical = String::new();
    for _ in 0..iterations {
        canonical = black_box(black_box(model).to_dem_string());
    }
    canonical
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn family_cycles_are_distinct_and_canonical() {
        let mut digests = std::collections::BTreeSet::new();
        for family in [
            DemFamily::FlatErrors,
            DemFamily::CoordinateSparse,
            DemFamily::FoldedRepeats,
        ] {
            let fixture = DemFixture::prepare(family, 64).expect("valid DEM fixture");
            assert!(fixture.input_bytes().expect("input bytes") > 0);
            assert!(digests.insert(fixture.input_digest()));
            let model = parse(1, &fixture).expect("parse family");
            let canonical = serialize(1, &model);
            fixture
                .validate_canonical(&canonical)
                .expect("canonical family output");
        }
    }

    #[test]
    fn parse_and_serialize_bind_odd_even_and_normalized_output() {
        let fixture = DemFixture::prepare(DemFamily::FlatErrors, 64).expect("fixture");
        let odd = parse(1, &fixture).expect("odd parse");
        let even = parse(2, &fixture).expect("even parse");
        let odd_text = serialize(1, &odd);
        let even_text = serialize(2, &even);

        assert_eq!(odd, even);
        assert_eq!(odd_text, even_text);
        assert_eq!(odd_text.len(), fixture.text().len());
        let expected = fixture.validate_canonical(&odd_text).expect("odd output");
        let normalized = fixture
            .validate_canonical(odd_text.trim_end_matches('\n'))
            .expect("Stim-style terminal newline");
        assert_eq!(expected, normalized);
    }

    #[test]
    fn fixture_rejects_family_specific_invalid_shapes_before_allocation() {
        assert!(matches!(
            DemFixture::prepare(DemFamily::FlatErrors, 0),
            Err(WorkerError::DemItemShape { .. })
        ));
        assert!(matches!(
            DemFixture::prepare(DemFamily::FlatErrors, 65),
            Err(WorkerError::DemItemShape { .. })
        ));
        assert!(matches!(
            DemFixture::prepare(DemFamily::CoordinateSparse, DEM_MAX_ITEMS + 1),
            Err(WorkerError::DemItemLimit { .. })
        ));
        assert!(matches!(
            DemFixture::prepare(DemFamily::FoldedRepeats, DEM_FOLDED_MAX_ITEMS + 1),
            Err(WorkerError::DemItemLimit { .. })
        ));
    }

    #[test]
    fn folded_repeat_family_remains_compact_after_parsing() {
        let fixture = DemFixture::prepare(DemFamily::FoldedRepeats, 64).expect("folded fixture");
        let model = parse(1, &fixture).expect("folded parse");
        let canonical = serialize(1, &model);

        assert!(canonical.contains("repeat[outer] 1000000"));
        assert!(canonical.contains("repeat[inner] 1024"));
        fixture
            .validate_canonical(&canonical)
            .expect("folded canonical output");
    }

    #[test]
    fn canonical_validation_rejects_nonterminal_differences() {
        let fixture = DemFixture::prepare(DemFamily::FlatErrors, 64).expect("fixture");
        let changed = fixture.text().replacen("D0", "D9", 1);

        assert!(matches!(
            fixture.validate_canonical(&changed),
            Err(WorkerError::DemCanonicalMismatch { .. })
        ));
    }
}
