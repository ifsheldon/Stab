use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use stab_records::{
    BitIndex, DetsResultType, PackedShotBatch, RecordBatchPart, RecordEncoder,
    RecordFormat as StabRecordFormat, RecordStreamReader, RecordStreamStorage, ShotIndex,
};

use super::error::{
    CensusError, CensusMismatch, CommandError, DataError, Implementation, InvalidOutputKey,
};
use super::model::{
    DataRecipe, OutputContract, RecordFormat, RecordPattern, SemanticWork, WorkUnit,
};

/// Records decoded or encoded per step, so record fixtures and output validation hold one bounded
/// batch rather than every record.
const RECORD_BATCH: usize = 1024;

/// Budget for the transport buffer that frames one text output record during validation.
const TEXT_RECORD_BUFFER_BYTES: u128 = 256 * 1024 * 1024;

/// Names one witnessed output of a case: `stdout`, `file:NAME`, `step-INDEX` for a pipeline
/// intermediate, or `pipeline` for the Rust worker's semantic output.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum OutputKey {
    Stdout,
    File(String),
    Step(usize),
    Pipeline,
}

impl OutputKey {
    /// Whether the measured output byte count includes this output. Pipeline intermediates are
    /// consumed by the next step and never reach the measured output.
    pub(super) const fn is_measured(&self) -> bool {
        !matches!(self, Self::Step(_))
    }
}

impl std::fmt::Display for OutputKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stdout => formatter.write_str("stdout"),
            Self::File(name) => write!(formatter, "file:{name}"),
            Self::Step(index) => write!(formatter, "step-{index}"),
            Self::Pipeline => formatter.write_str("pipeline"),
        }
    }
}

impl std::str::FromStr for OutputKey {
    type Err = InvalidOutputKey;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text {
            "stdout" => Ok(Self::Stdout),
            "pipeline" => Ok(Self::Pipeline),
            _ => {
                if let Some(name) = text.strip_prefix("file:")
                    && !name.is_empty()
                {
                    return Ok(Self::File(name.to_string()));
                }
                text.strip_prefix("step-")
                    .and_then(|digits| {
                        let index = digits.parse::<usize>().ok()?;
                        // Only the canonical spelling parses, so a key round-trips exactly.
                        (index.to_string() == digits).then_some(index)
                    })
                    .map(Self::Step)
                    .ok_or_else(|| InvalidOutputKey(text.to_string()))
            }
        }
    }
}

/// Orders keys by their spelling, the order committed correctness reports already use.
impl Ord for OutputKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.to_string().cmp(&other.to_string())
    }
}

impl PartialOrd for OutputKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Serialize for OutputKey {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for OutputKey {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OutputWitness {
    pub(super) bytes: u64,
    pub(super) sha256: String,
    pub(super) one_bits: Option<u64>,
    /// Per-record one-bit statistics of a record output, so replay can repeat the preflight
    /// comparison with pinned Stim. Bundles written before this field existed omit it, and an
    /// exact output never has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) census: Option<RecordCensus>,
}

/// Bound, in standard errors, on the difference between Stim's and Stab's mean per-record one-bit
/// counts for one stochastic output contract. Records (shots) are independent, so the difference
/// of the two sample means is asymptotically normal with the per-side sample variances, whatever
/// the correlation between bits inside one record; every contract has at least 4096 records, and
/// a correct implementation exceeds 6 standard errors with probability about 2e-9 per output.
pub(super) const ONE_BIT_TWO_SAMPLE_SIGMA: f64 = 6.0;

/// Per-record one-bit statistics of one record output.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RecordCensus {
    pub(super) records: u64,
    pub(super) ones: u64,
    /// Sum over records of the squared per-record one-bit count.
    pub(super) sum_squares: u128,
}

impl RecordCensus {
    /// Rejects Stab's per-record one-bit mean when it is more than [`ONE_BIT_TWO_SAMPLE_SIGMA`]
    /// standard errors from Stim's.
    pub(super) fn compare_with_stim(self, stim: Self) -> Result<(), CensusMismatch> {
        if self.records != stim.records || self.records == 0 {
            return Err(CensusMismatch::RecordCounts {
                stim_records: stim.records,
                stab_records: self.records,
            });
        }
        let records = self.records as f64;
        let moments = |census: Self| {
            let mean = census.ones as f64 / records;
            let variance = (census.sum_squares as f64 / records - mean * mean).max(0.0);
            (mean, variance)
        };
        let (stim_mean, stim_variance) = moments(stim);
        let (stab_mean, stab_variance) = moments(self);
        let standard_error = ((stim_variance + stab_variance) / records).sqrt();
        let difference = (stab_mean - stim_mean).abs();
        let rejected = if standard_error > 0.0 {
            difference > ONE_BIT_TWO_SAMPLE_SIGMA * standard_error
        } else {
            self.ones != stim.ones
        };
        if rejected {
            return Err(CensusMismatch::OneBits {
                stab_ones: self.ones,
                stim_ones: stim.ones,
                records: self.records,
                standard_error,
            });
        }
        Ok(())
    }
}

/// Compares Stab's per-record one-bit statistics with pinned Stim's for every output both sides
/// witnessed with a census. Preflight runs it on fresh outputs and report derivation reruns it on
/// the committed witnesses; witnesses that predate the census field carry none and are skipped.
pub(super) fn compare_witness_censuses(
    stim: &BTreeMap<OutputKey, OutputWitness>,
    stab: &BTreeMap<OutputKey, OutputWitness>,
) -> Result<(), (OutputKey, CensusError)> {
    for (key, stim_witness) in stim {
        let Some(stab_witness) = stab.get(key) else {
            continue;
        };
        let fail = |error: CensusError| (key.clone(), error);
        match (stim_witness.census, stab_witness.census) {
            (Some(stim_census), Some(stab_census)) => {
                for (implementation, witness, census) in [
                    (Implementation::Stim, stim_witness, stim_census),
                    (Implementation::Stab, stab_witness, stab_census),
                ] {
                    if witness.one_bits != Some(census.ones) {
                        return Err(fail(CensusError::OneBits(implementation)));
                    }
                }
                stab_census
                    .compare_with_stim(stim_census)
                    .map_err(|mismatch| fail(mismatch.into()))?;
            }
            (Some(_), None) => return Err(fail(CensusError::OneSided(Implementation::Stim))),
            (None, Some(_)) => return Err(fail(CensusError::OneSided(Implementation::Stab))),
            (None, None) => {}
        }
    }
    Ok(())
}

/// Produces case inputs that come from running the pinned binaries.
pub(super) trait PinnedInputs {
    /// A circuit generated identically by pinned Stim and Stab.
    fn circuit(&mut self, args: &[String]) -> Result<Vec<u8>, CommandError>;
    /// A generated circuit analyzed into identical DEM text by both binaries.
    fn dem(&mut self, circuit: &[String], analyze: &[String]) -> Result<Vec<u8>, CommandError>;
    /// Records sampled once by pinned Stim from a generated circuit.
    fn stim_records(
        &mut self,
        circuit: &[String],
        sample: &[String],
    ) -> Result<Vec<u8>, CommandError>;
}

impl DataRecipe {
    pub(super) fn materialize(&self, pinned: &mut impl PinnedInputs) -> Result<Vec<u8>, DataError> {
        match self {
            Self::Empty => Ok(Vec::new()),
            Self::GeneratedCircuit { args } => Ok(pinned.circuit(args)?),
            Self::GeneratedDem { circuit, analyze } => Ok(pinned.dem(circuit, analyze)?),
            Self::SampledRecords {
                circuit, sample, ..
            } => Ok(pinned.stim_records(circuit, sample)?),
            Self::FoldedCircuit {
                qubits,
                repeat_blocks,
                repeat_count,
                error_probability,
            } => folded_circuit(*qubits, *repeat_blocks, *repeat_count, *error_probability),
            Self::Records {
                format,
                records: record_count,
                bits,
                pattern,
            } => records(*format, *record_count, *bits, *pattern),
            Self::TypedDets {
                records,
                detectors,
                observables,
                detector_hits,
                observable_hits,
            } => typed_dets(
                *records,
                *detectors,
                *observables,
                *detector_hits,
                *observable_hits,
            ),
            Self::M2dCircuit { bits } => m2d_circuit(*bits),
            Self::Dem {
                detectors,
                mechanisms,
                repeat_count,
                error_probability,
            } => dem(*detectors, *mechanisms, *repeat_count, *error_probability),
        }
    }
}

impl SemanticWork {
    /// Checks byte-denominated work against the case's actual input or generated output.
    pub(super) fn check(&self, stdin: &[u8], stdout: &OutputWitness) -> Result<(), DataError> {
        let actual = match self.unit {
            WorkUnit::GeneratedBytes => stdout.bytes,
            WorkUnit::InputBytes => {
                u64::try_from(stdin.len()).map_err(|_| DataError::InputTooLarge)?
            }
            WorkUnit::Records | WorkUnit::Shots => return Ok(()),
        };
        if actual == self.amount {
            Ok(())
        } else {
            Err(DataError::WorkMismatch {
                declared: self.amount,
                unit: self.unit,
                actual,
            })
        }
    }
}

impl OutputContract {
    /// Validates one output and returns its witness, with a census for a record contract.
    pub(super) fn validate(&self, bytes: &[u8]) -> Result<OutputWitness, DataError> {
        let census = match self {
            Self::Exact { minimum_bytes } => {
                if bytes.len() < *minimum_bytes {
                    return Err(DataError::OutputTooShort {
                        actual: bytes.len(),
                        minimum: *minimum_bytes,
                    });
                }
                None
            }
            Self::Records {
                format,
                records,
                bits,
                minimum_one_bits,
                maximum_one_fraction,
            } => {
                let census = record_census(bytes, stab_format(*format)?, *bits)?;
                let ones = census.ones;
                if usize::try_from(census.records).ok() != Some(*records) {
                    return Err(DataError::RecordCount {
                        actual: census.records,
                        expected: *records,
                    });
                }
                if ones < *minimum_one_bits {
                    return Err(DataError::TooFewOnes {
                        ones,
                        minimum: *minimum_one_bits,
                    });
                }
                let total_bits = u128::try_from(*records)
                    .ok()
                    .and_then(|records| {
                        u128::try_from(*bits)
                            .ok()
                            .and_then(|bits| records.checked_mul(bits))
                    })
                    .ok_or(DataError::BitCountOverflow)?;
                let fraction = if total_bits == 0 {
                    0.0
                } else {
                    ones as f64 / total_bits as f64
                };
                if !maximum_one_fraction.is_finite()
                    || *maximum_one_fraction < 0.0
                    || *maximum_one_fraction > 1.0
                    || fraction > *maximum_one_fraction
                {
                    return Err(DataError::OneFraction {
                        fraction,
                        maximum: *maximum_one_fraction,
                    });
                }
                Some(census)
            }
        };
        Ok(OutputWitness {
            bytes: u64::try_from(bytes.len()).map_err(|_| DataError::OutputTooLarge)?,
            sha256: hex::encode(Sha256::digest(bytes)),
            one_bits: census.map(|census| census.ones),
            census,
        })
    }
}

/// Streams record output through the bounded packed reader, returning its record and one-bit
/// statistics.
fn record_census(
    bytes: &[u8],
    format: StabRecordFormat,
    bits: usize,
) -> Result<RecordCensus, DataError> {
    let mut reader = RecordStreamReader::measurements(
        bytes,
        format,
        bits,
        RecordStreamStorage::max_text_record_bytes(TEXT_RECORD_BUFFER_BYTES),
    );
    let mut census = RecordCensus {
        records: 0,
        ones: 0,
        sum_squares: 0,
    };
    while let Some(batch) = reader
        .read_batch(RECORD_BATCH)
        .map_err(DataError::InvalidRecordStream)?
    {
        census.records = u64::try_from(batch.shot_count())
            .ok()
            .and_then(|count| census.records.checked_add(count))
            .ok_or(DataError::CensusOverflow)?;
        for shot in 0..batch.shot_count() {
            let record_ones = u64::try_from(
                batch
                    .shot(shot)
                    .map_err(DataError::InvalidRecords)?
                    .popcount(),
            )
            .map_err(|_| DataError::CensusOverflow)?;
            census.ones = census
                .ones
                .checked_add(record_ones)
                .ok_or(DataError::CensusOverflow)?;
            census.sum_squares = census
                .sum_squares
                .checked_add(u128::from(record_ones) * u128::from(record_ones))
                .ok_or(DataError::CensusOverflow)?;
        }
    }
    Ok(census)
}

fn records(
    format: RecordFormat,
    record_count: usize,
    bits: usize,
    pattern: RecordPattern,
) -> Result<Vec<u8>, DataError> {
    if record_count == 0 || bits == 0 {
        return Err(DataError::RecordDimensions);
    }
    let mut encoder =
        RecordEncoder::try_new(stab_format(format)?, bits).map_err(DataError::Encode)?;
    let mut batch = PackedShotBatch::zeros(RECORD_BATCH.min(record_count), bits)
        .map_err(DataError::ReserveBatch)?;
    let mut output = Vec::new();
    for batch_start in (0..record_count).step_by(RECORD_BATCH) {
        let batch_records = RECORD_BATCH.min(record_count - batch_start);
        for shot in 0..batch_records {
            let record_index = batch_start + shot;
            for bit_index in 0..bits {
                let bit = match pattern {
                    RecordPattern::Alternating => (record_index + bit_index * 3).is_multiple_of(7),
                    RecordPattern::Sparse => false,
                };
                batch
                    .set(ShotIndex(shot), BitIndex(bit_index), bit)
                    .map_err(DataError::Encode)?;
            }
            if pattern == RecordPattern::Sparse {
                for hit in 0..bits.min(4) {
                    let index = (record_index * 17 + hit * 97) % bits;
                    batch
                        .set(ShotIndex(shot), BitIndex(index), true)
                        .map_err(DataError::Encode)?;
                }
            }
        }
        let view = batch
            .view_prefix(batch_records)
            .map_err(DataError::Encode)?;
        encoder
            .write_batch(
                &[RecordBatchPart::new(DetsResultType::Measurement, view)],
                &mut output,
            )
            .map_err(DataError::EncodeWrite)?;
    }
    if encoder.pending_group_records() != 0 {
        return Err(DataError::PartialPtb64Group);
    }
    Ok(output)
}

fn typed_dets(
    record_count: usize,
    detectors: usize,
    observables: usize,
    detector_hits: usize,
    observable_hits: usize,
) -> Result<Vec<u8>, DataError> {
    if record_count == 0 || detectors == 0 || observables == 0 {
        return Err(DataError::TypedDetsDimensions);
    }
    if detector_hits > detectors || observable_hits > observables {
        return Err(DataError::TypedDetsHits);
    }
    let mut text = String::new();
    for record in 0..record_count {
        text.push_str("shot");
        for hit in 0..detector_hits {
            let index = (record * 17 + hit * 31) % detectors;
            write!(text, " D{index}")?;
        }
        for hit in 0..observable_hits {
            let index = (record * 7 + hit * 11) % observables;
            write!(text, " L{index}")?;
        }
        text.push('\n');
    }
    Ok(text.into_bytes())
}

fn folded_circuit(
    qubits: usize,
    repeat_blocks: usize,
    repeat_count: u64,
    error_probability: f64,
) -> Result<Vec<u8>, DataError> {
    if qubits == 0 || repeat_blocks == 0 || repeat_count == 0 {
        return Err(DataError::FoldedDimensions);
    }
    valid_probability(error_probability)?;
    let targets = joined_indices(qubits);
    let mut text = format!("R {targets}\n");
    for _ in 0..repeat_blocks {
        writeln!(text, "REPEAT {repeat_count} {{")?;
        writeln!(text, "    X_ERROR({error_probability}) {targets}")?;
        writeln!(text, "    M {targets}")?;
        for qubit in 0..qubits {
            let offset = qubits - qubit;
            writeln!(text, "    DETECTOR rec[-{offset}]")?;
        }
        writeln!(text, "    R {targets}")?;
        text.push_str("    SHIFT_COORDS(0, 1)\n}\n");
    }
    Ok(text.into_bytes())
}

fn m2d_circuit(bits: usize) -> Result<Vec<u8>, DataError> {
    if bits == 0 {
        return Err(DataError::M2dWidth);
    }
    let targets = joined_indices(bits);
    let mut text = format!("R {targets}\n");
    for bit in 0..bits {
        writeln!(text, "CX sweep[{bit}] {bit}")?;
    }
    writeln!(text, "M {targets}")?;
    for bit in 0..bits {
        let offset = bits - bit;
        writeln!(text, "DETECTOR rec[-{offset}]")?;
    }
    text.push_str("OBSERVABLE_INCLUDE(0) rec[-1]\n");
    Ok(text.into_bytes())
}

fn dem(
    detectors: usize,
    mechanisms: usize,
    repeat_count: u64,
    error_probability: f64,
) -> Result<Vec<u8>, DataError> {
    if detectors == 0 || mechanisms == 0 || repeat_count == 0 {
        return Err(DataError::DemDimensions);
    }
    valid_probability(error_probability)?;
    let mut text = format!("repeat {repeat_count} {{\n");
    for mechanism in 0..mechanisms {
        let first = (mechanism * 17) % detectors;
        let second = (mechanism * 97 + 1) % detectors;
        if mechanism.is_multiple_of(17) {
            writeln!(text, "    error({error_probability}) D{first} D{second} L0")?;
        } else {
            writeln!(text, "    error({error_probability}) D{first} D{second}")?;
        }
    }
    writeln!(text, "    shift_detectors {detectors}")?;
    text.push_str("}\n");
    Ok(text.into_bytes())
}

fn joined_indices(count: usize) -> String {
    (0..count)
        .map(|index| index.to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

fn valid_probability(value: f64) -> Result<(), DataError> {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(DataError::Probability(value))
    }
}

fn stab_format(format: RecordFormat) -> Result<StabRecordFormat, DataError> {
    match format {
        RecordFormat::ZeroOne => Ok(StabRecordFormat::ZeroOne),
        RecordFormat::B8 => Ok(StabRecordFormat::B8),
        RecordFormat::R8 => Ok(StabRecordFormat::R8),
        RecordFormat::Hits => Ok(StabRecordFormat::Hits),
        RecordFormat::Dets => Err(DataError::DetsFormat),
        RecordFormat::Ptb64 => Ok(StabRecordFormat::Ptb64),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_record_recipes_round_trip_all_generic_formats() {
        for format in [
            RecordFormat::ZeroOne,
            RecordFormat::B8,
            RecordFormat::R8,
            RecordFormat::Hits,
            RecordFormat::Ptb64,
        ] {
            let record_count = if format == RecordFormat::Ptb64 { 64 } else { 7 };
            let bytes =
                records(format, record_count, 65, RecordPattern::Alternating).expect("fixture");
            let census = record_census(&bytes, stab_format(format).expect("format"), 65)
                .expect("round trip");
            assert_eq!(census.records, record_count as u64);
            assert!(census.ones > 0);
            assert!(census.sum_squares >= u128::from(census.ones));
        }
    }

    fn records_contract(format: RecordFormat, records: usize, bits: usize) -> OutputContract {
        OutputContract::Records {
            format,
            records,
            bits,
            minimum_one_bits: 1,
            maximum_one_fraction: 0.75,
        }
    }

    #[test]
    fn record_contracts_reject_wrong_counts_truncation_and_bounds() {
        let b8 = records(RecordFormat::B8, 8, 16, RecordPattern::Alternating).expect("b8");
        let checked = records_contract(RecordFormat::B8, 8, 16)
            .validate(&b8)
            .expect("matching contract");
        assert_eq!(checked.bytes, b8.len() as u64);
        assert_eq!(checked.one_bits, checked.census.map(|census| census.ones));

        let wrong_count = records_contract(RecordFormat::B8, 9, 16)
            .validate(&b8)
            .expect_err("record count mismatch")
            .to_string();
        assert!(
            wrong_count.contains("has 8 records, expected 9"),
            "{wrong_count}"
        );

        let truncated_b8 = records_contract(RecordFormat::B8, 8, 16)
            .validate(&b8[..b8.len() - 1])
            .expect_err("truncated b8")
            .to_string();
        assert!(
            truncated_b8.contains("record output is invalid"),
            "{truncated_b8}"
        );

        let ptb64 = records(RecordFormat::Ptb64, 64, 3, RecordPattern::Alternating).expect("ptb64");
        let truncated_ptb64 = records_contract(RecordFormat::Ptb64, 64, 3)
            .validate(&ptb64[..ptb64.len() - 8])
            .expect_err("truncated ptb64")
            .to_string();
        assert!(
            truncated_ptb64.contains("record output is invalid"),
            "{truncated_ptb64}"
        );

        let dense = b"1111\n1111\n".to_vec();
        let fraction = records_contract(RecordFormat::ZeroOne, 2, 4)
            .validate(&dense)
            .expect_err("one fraction above the bound")
            .to_string();
        assert!(fraction.contains("exceeds 0.750000"), "{fraction}");

        let empty_records = records_contract(RecordFormat::ZeroOne, 2, 4)
            .validate(b"")
            .expect_err("empty record output")
            .to_string();
        assert!(
            empty_records.contains("has 0 records, expected 2"),
            "{empty_records}"
        );
        let empty_exact = OutputContract::Exact { minimum_bytes: 1 }
            .validate(b"")
            .expect_err("empty exact output")
            .to_string();
        assert!(empty_exact.contains("expected at least 1"), "{empty_exact}");
    }

    #[test]
    fn one_bit_comparison_uses_per_record_variance() {
        let census = |records: u64, ones: u64, sum_squares: u128| RecordCensus {
            records,
            ones,
            sum_squares,
        };
        // 10000 records with mean 10 and variance 100 per side: the standard error of the
        // difference is sqrt(200 / 10000) ~= 0.141 per record, so 6 sigma is ~0.85.
        let stim = census(10_000, 100_000, 10_000 * 200);
        let close = census(10_000, 108_000, 10_000 * (100 + 108 * 108 / 100));
        close
            .compare_with_stim(stim)
            .expect("0.8 per record is inside 6 sigma");
        let far = census(10_000, 110_000, 10_000 * (100 + 121));
        assert!(far.compare_with_stim(stim).is_err());

        // A constant per-record count has zero variance: only identical totals pass.
        let constant = census(100, 300, 100 * 9);
        constant
            .compare_with_stim(constant)
            .expect("identical constants");
        assert!(
            census(100, 400, 100 * 16)
                .compare_with_stim(constant)
                .is_err()
        );
        assert!(census(99, 300, 900).compare_with_stim(constant).is_err());
    }

    #[test]
    fn output_keys_round_trip_and_keep_committed_order() {
        for key in [
            OutputKey::Stdout,
            OutputKey::Pipeline,
            OutputKey::File("obs".to_string()),
            OutputKey::Step(12),
        ] {
            assert_eq!(key.to_string().parse::<OutputKey>(), Ok(key));
        }
        for bad in ["file:", "step-", "step-01", "step-+1", "stderr"] {
            assert!(bad.parse::<OutputKey>().is_err(), "{bad}");
        }
        let mut keys = [
            OutputKey::Step(10),
            OutputKey::Stdout,
            OutputKey::Step(2),
            OutputKey::File("obs".to_string()),
            OutputKey::Pipeline,
        ];
        keys.sort();
        let spelled = keys.iter().map(ToString::to_string).collect::<Vec<_>>();
        let mut sorted = spelled.clone();
        sorted.sort();
        assert_eq!(spelled, sorted);
    }

    #[test]
    fn typed_recipes_preserve_namespace_spelling_and_termination() {
        let bytes = typed_dets(2, 8, 2, 2, 1).expect("DETS");
        let text = String::from_utf8(bytes).expect("UTF-8");
        assert!(text.lines().all(|line| line.starts_with("shot D")));
        assert!(text.lines().all(|line| line.contains(" L")));
        assert!(text.ends_with('\n'));
    }

    #[test]
    fn generated_model_recipes_are_folded_and_bounded() {
        let circuit =
            String::from_utf8(folded_circuit(2, 3, 100, 0.01).expect("circuit")).expect("UTF-8");
        assert_eq!(circuit.matches("REPEAT 100").count(), 3);
        assert!(!circuit.contains("REPEAT 100\nREPEAT"));

        let dem = String::from_utf8(dem(32, 64, 4, 0.001).expect("DEM")).expect("UTF-8");
        assert!(dem.starts_with("repeat 4 {\n"));
        assert_eq!(dem.matches("error(").count(), 64);
        assert!(dem.contains("shift_detectors 32"));
    }
}
