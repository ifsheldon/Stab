use std::fmt::Write as _;

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use stab_records::{
    BitIndex, DetsResultType, PackedShotBatch, RecordBatchPart, RecordEncoder,
    RecordFormat as StabRecordFormat, RecordStreamReader, RecordStreamStorage, ShotIndex,
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
pub(super) enum OutputKey {
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
    type Err = String;

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
                    .ok_or_else(|| format!("unknown output key {text:?}"))
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
}

/// Bound, in standard errors, on the difference between Stim's and Stab's mean per-record one-bit
/// counts for one stochastic output contract. Records (shots) are independent, so the difference
/// of the two sample means is asymptotically normal with the per-side sample variances, whatever
/// the correlation between bits inside one record; every contract has at least 4096 records, and
/// a correct implementation exceeds 6 standard errors with probability about 2e-9 per output.
pub(super) const ONE_BIT_TWO_SAMPLE_SIGMA: f64 = 6.0;

/// Per-record one-bit statistics of one record output.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RecordCensus {
    pub(super) records: u64,
    pub(super) ones: u64,
    /// Sum over records of the squared per-record one-bit count.
    pub(super) sum_squares: u128,
}

impl RecordCensus {
    /// Rejects Stab's per-record one-bit mean when it is more than [`ONE_BIT_TWO_SAMPLE_SIGMA`]
    /// standard errors from Stim's.
    pub(super) fn compare_with_stim(self, stim: Self) -> Result<(), String> {
        if self.records != stim.records || self.records == 0 {
            return Err(format!(
                "one-bit comparison needs equal positive record counts, got Stim {} and Stab {}",
                stim.records, self.records
            ));
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
            return Err(format!(
                "Stab one bits {} differ from pinned Stim's {} over {} records by more than {ONE_BIT_TWO_SAMPLE_SIGMA} standard errors ({standard_error:.6} per record)",
                self.ones, stim.ones, self.records
            ));
        }
        Ok(())
    }
}

/// A validated output: its committed witness and, for a record contract, its census.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CheckedOutput {
    pub(super) witness: OutputWitness,
    pub(super) census: Option<RecordCensus>,
}

/// Produces case inputs that come from running the pinned binaries.
pub(super) trait PinnedInputs {
    /// A circuit generated identically by pinned Stim and Stab.
    fn circuit(&mut self, args: &[String]) -> Result<Vec<u8>, String>;
    /// A generated circuit analyzed into identical DEM text by both binaries.
    fn dem(&mut self, circuit: &[String], analyze: &[String]) -> Result<Vec<u8>, String>;
    /// Records sampled once by pinned Stim from a generated circuit.
    fn stim_records(&mut self, circuit: &[String], sample: &[String]) -> Result<Vec<u8>, String>;
}

impl DataRecipe {
    pub(super) fn materialize(&self, pinned: &mut impl PinnedInputs) -> Result<Vec<u8>, String> {
        match self {
            Self::Empty => Ok(Vec::new()),
            Self::GeneratedCircuit { args } => pinned.circuit(args),
            Self::GeneratedDem { circuit, analyze } => pinned.dem(circuit, analyze),
            Self::SampledRecords {
                circuit, sample, ..
            } => pinned.stim_records(circuit, sample),
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
    pub(super) fn check(&self, stdin: &[u8], stdout: &OutputWitness) -> Result<(), String> {
        let actual = match self.unit {
            WorkUnit::GeneratedBytes => stdout.bytes,
            WorkUnit::InputBytes => u64::try_from(stdin.len())
                .map_err(|_| "input byte count does not fit in u64".to_string())?,
            WorkUnit::Records | WorkUnit::Shots => return Ok(()),
        };
        if actual == self.amount {
            Ok(())
        } else {
            Err(format!(
                "semantic work declares {} {:?}, actual value is {actual}",
                self.amount, self.unit
            ))
        }
    }
}

impl OutputContract {
    pub(super) fn validate(&self, bytes: &[u8]) -> Result<CheckedOutput, String> {
        let census = match self {
            Self::Exact { minimum_bytes } => {
                if bytes.len() < *minimum_bytes {
                    return Err(format!(
                        "output has {} bytes, expected at least {minimum_bytes}",
                        bytes.len()
                    ));
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
                    return Err(format!(
                        "record output has {} records, expected {records}",
                        census.records
                    ));
                }
                if ones < *minimum_one_bits {
                    return Err(format!(
                        "record output has {ones} one bits, expected at least {minimum_one_bits}"
                    ));
                }
                let total_bits = u128::try_from(*records)
                    .ok()
                    .and_then(|records| {
                        u128::try_from(*bits)
                            .ok()
                            .and_then(|bits| records.checked_mul(bits))
                    })
                    .ok_or_else(|| "record bit count overflow".to_string())?;
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
                    return Err(format!(
                        "record one fraction {fraction:.6} exceeds {maximum_one_fraction:.6}"
                    ));
                }
                Some(census)
            }
        };
        Ok(CheckedOutput {
            witness: OutputWitness {
                bytes: u64::try_from(bytes.len())
                    .map_err(|_| "output byte count does not fit in u64".to_string())?,
                sha256: hex::encode(Sha256::digest(bytes)),
                one_bits: census.map(|census| census.ones),
            },
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
) -> Result<RecordCensus, String> {
    let invalid = |source: &dyn std::fmt::Display| format!("record output is invalid: {source}");
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
    let overflow = || "record census overflow".to_string();
    while let Some(batch) = reader
        .read_batch(RECORD_BATCH)
        .map_err(|source| invalid(&source))?
    {
        census.records = u64::try_from(batch.shot_count())
            .ok()
            .and_then(|count| census.records.checked_add(count))
            .ok_or_else(overflow)?;
        for shot in 0..batch.shot_count() {
            let record_ones = u64::try_from(
                batch
                    .shot(shot)
                    .map_err(|source| invalid(&source))?
                    .popcount(),
            )
            .map_err(|_| overflow())?;
            census.ones = census.ones.checked_add(record_ones).ok_or_else(overflow)?;
            census.sum_squares = census
                .sum_squares
                .checked_add(u128::from(record_ones) * u128::from(record_ones))
                .ok_or_else(overflow)?;
        }
    }
    Ok(census)
}

fn records(
    format: RecordFormat,
    record_count: usize,
    bits: usize,
    pattern: RecordPattern,
) -> Result<Vec<u8>, String> {
    if record_count == 0 || bits == 0 {
        return Err("record recipes require positive records and bits".to_string());
    }
    let encode_error =
        |source: &dyn std::fmt::Display| format!("cannot encode record fixture: {source}");
    let mut encoder = RecordEncoder::try_new(stab_format(format)?, bits)
        .map_err(|source| encode_error(&source))?;
    let mut batch = PackedShotBatch::zeros(RECORD_BATCH.min(record_count), bits)
        .map_err(|source| format!("cannot reserve record fixture batch: {source}"))?;
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
                    .map_err(|source| encode_error(&source))?;
            }
            if pattern == RecordPattern::Sparse {
                for hit in 0..bits.min(4) {
                    let index = (record_index * 17 + hit * 97) % bits;
                    batch
                        .set(ShotIndex(shot), BitIndex(index), true)
                        .map_err(|source| encode_error(&source))?;
                }
            }
        }
        let view = batch
            .view_prefix(batch_records)
            .map_err(|source| encode_error(&source))?;
        encoder
            .write_batch(
                &[RecordBatchPart::new(DetsResultType::Measurement, view)],
                &mut output,
            )
            .map_err(|source| encode_error(&source))?;
    }
    if encoder.pending_group_records() != 0 {
        return Err(encode_error(
            &"ptb64 record recipes require a multiple of 64 records",
        ));
    }
    Ok(output)
}

fn typed_dets(
    record_count: usize,
    detectors: usize,
    observables: usize,
    detector_hits: usize,
    observable_hits: usize,
) -> Result<Vec<u8>, String> {
    if record_count == 0 || detectors == 0 || observables == 0 {
        return Err("typed DETS recipes require positive dimensions".to_string());
    }
    if detector_hits > detectors || observable_hits > observables {
        return Err("typed DETS hit count exceeds its namespace".to_string());
    }
    let mut text = String::new();
    for record in 0..record_count {
        text.push_str("shot");
        for hit in 0..detector_hits {
            let index = (record * 17 + hit * 31) % detectors;
            write!(text, " D{index}").map_err(|source| source.to_string())?;
        }
        for hit in 0..observable_hits {
            let index = (record * 7 + hit * 11) % observables;
            write!(text, " L{index}").map_err(|source| source.to_string())?;
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
) -> Result<Vec<u8>, String> {
    if qubits == 0 || repeat_blocks == 0 || repeat_count == 0 {
        return Err("folded circuit dimensions must be positive".to_string());
    }
    valid_probability(error_probability)?;
    let targets = joined_indices(qubits);
    let mut text = format!("R {targets}\n");
    for _ in 0..repeat_blocks {
        writeln!(text, "REPEAT {repeat_count} {{").map_err(|source| source.to_string())?;
        writeln!(text, "    X_ERROR({error_probability}) {targets}")
            .map_err(|source| source.to_string())?;
        writeln!(text, "    M {targets}").map_err(|source| source.to_string())?;
        for qubit in 0..qubits {
            let offset = qubits - qubit;
            writeln!(text, "    DETECTOR rec[-{offset}]").map_err(|source| source.to_string())?;
        }
        writeln!(text, "    R {targets}").map_err(|source| source.to_string())?;
        text.push_str("    SHIFT_COORDS(0, 1)\n}\n");
    }
    Ok(text.into_bytes())
}

fn m2d_circuit(bits: usize) -> Result<Vec<u8>, String> {
    if bits == 0 {
        return Err("m2d circuit width must be positive".to_string());
    }
    let targets = joined_indices(bits);
    let mut text = format!("R {targets}\n");
    for bit in 0..bits {
        writeln!(text, "CX sweep[{bit}] {bit}").map_err(|source| source.to_string())?;
    }
    writeln!(text, "M {targets}").map_err(|source| source.to_string())?;
    for bit in 0..bits {
        let offset = bits - bit;
        writeln!(text, "DETECTOR rec[-{offset}]").map_err(|source| source.to_string())?;
    }
    text.push_str("OBSERVABLE_INCLUDE(0) rec[-1]\n");
    Ok(text.into_bytes())
}

fn dem(
    detectors: usize,
    mechanisms: usize,
    repeat_count: u64,
    error_probability: f64,
) -> Result<Vec<u8>, String> {
    if detectors == 0 || mechanisms == 0 || repeat_count == 0 {
        return Err("DEM recipe dimensions must be positive".to_string());
    }
    valid_probability(error_probability)?;
    let mut text = format!("repeat {repeat_count} {{\n");
    for mechanism in 0..mechanisms {
        let first = (mechanism * 17) % detectors;
        let second = (mechanism * 97 + 1) % detectors;
        if mechanism.is_multiple_of(17) {
            writeln!(text, "    error({error_probability}) D{first} D{second} L0")
                .map_err(|source| source.to_string())?;
        } else {
            writeln!(text, "    error({error_probability}) D{first} D{second}")
                .map_err(|source| source.to_string())?;
        }
    }
    writeln!(text, "    shift_detectors {detectors}").map_err(|source| source.to_string())?;
    text.push_str("}\n");
    Ok(text.into_bytes())
}

fn joined_indices(count: usize) -> String {
    (0..count)
        .map(|index| index.to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

fn valid_probability(value: f64) -> Result<(), String> {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(format!("invalid probability {value}"))
    }
}

fn stab_format(format: RecordFormat) -> Result<StabRecordFormat, String> {
    match format {
        RecordFormat::ZeroOne => Ok(StabRecordFormat::ZeroOne),
        RecordFormat::B8 => Ok(StabRecordFormat::B8),
        RecordFormat::R8 => Ok(StabRecordFormat::R8),
        RecordFormat::Hits => Ok(StabRecordFormat::Hits),
        RecordFormat::Dets => Err("generic records cannot infer typed DETS namespaces".to_string()),
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
        assert_eq!(checked.witness.bytes, b8.len() as u64);
        assert_eq!(
            checked.witness.one_bits,
            checked.census.map(|census| census.ones)
        );

        let wrong_count = records_contract(RecordFormat::B8, 9, 16)
            .validate(&b8)
            .expect_err("record count mismatch");
        assert!(
            wrong_count.contains("has 8 records, expected 9"),
            "{wrong_count}"
        );

        let truncated_b8 = records_contract(RecordFormat::B8, 8, 16)
            .validate(&b8[..b8.len() - 1])
            .expect_err("truncated b8");
        assert!(
            truncated_b8.contains("record output is invalid"),
            "{truncated_b8}"
        );

        let ptb64 = records(RecordFormat::Ptb64, 64, 3, RecordPattern::Alternating).expect("ptb64");
        let truncated_ptb64 = records_contract(RecordFormat::Ptb64, 64, 3)
            .validate(&ptb64[..ptb64.len() - 8])
            .expect_err("truncated ptb64");
        assert!(
            truncated_ptb64.contains("record output is invalid"),
            "{truncated_ptb64}"
        );

        let dense = b"1111\n1111\n".to_vec();
        let fraction = records_contract(RecordFormat::ZeroOne, 2, 4)
            .validate(&dense)
            .expect_err("one fraction above the bound");
        assert!(fraction.contains("exceeds 0.750000"), "{fraction}");

        let empty_records = records_contract(RecordFormat::ZeroOne, 2, 4)
            .validate(b"")
            .expect_err("empty record output");
        assert!(
            empty_records.contains("has 0 records, expected 2"),
            "{empty_records}"
        );
        let empty_exact = OutputContract::Exact { minimum_bytes: 1 }
            .validate(b"")
            .expect_err("empty exact output");
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
