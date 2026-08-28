use std::fmt::Write as _;

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use stab_records::{RecordFormat as StabRecordFormat, read_records, write_records};

use super::model::{
    DataRecipe, OutputContract, RecordFormat, RecordPattern, SemanticWork, WorkUnit,
};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OutputWitness {
    pub(super) bytes: u64,
    pub(super) sha256: String,
    pub(super) one_bits: Option<u64>,
}

pub(super) fn materialize(
    recipe: &DataRecipe,
    mut generated_circuit: impl FnMut(&[String]) -> Result<Vec<u8>, String>,
) -> Result<Vec<u8>, String> {
    match recipe {
        DataRecipe::Empty => Ok(Vec::new()),
        DataRecipe::GeneratedCircuit { args } => generated_circuit(args),
        DataRecipe::FoldedCircuit {
            qubits,
            repeat_blocks,
            repeat_count,
            error_probability,
        } => folded_circuit(*qubits, *repeat_blocks, *repeat_count, *error_probability),
        DataRecipe::Records {
            format,
            records: record_count,
            bits,
            pattern,
        } => records(*format, *record_count, *bits, *pattern),
        DataRecipe::TypedDets {
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
        DataRecipe::M2dCircuit { bits } => m2d_circuit(*bits),
        DataRecipe::Dem {
            detectors,
            mechanisms,
            repeat_count,
            error_probability,
        } => dem(*detectors, *mechanisms, *repeat_count, *error_probability),
    }
}

pub(super) fn validate_semantic_work(
    work: &SemanticWork,
    stdin: &[u8],
    stdout: &OutputWitness,
) -> Result<(), String> {
    let actual = match work.unit {
        WorkUnit::GeneratedBytes => stdout.bytes,
        WorkUnit::InputBytes => u64::try_from(stdin.len())
            .map_err(|_| "input byte count does not fit in u64".to_string())?,
        WorkUnit::Records | WorkUnit::Shots => return Ok(()),
    };
    if actual == work.amount {
        Ok(())
    } else {
        Err(format!(
            "semantic work declares {} {:?}, actual value is {actual}",
            work.amount, work.unit
        ))
    }
}

pub(super) fn validate_output(
    contract: &OutputContract,
    bytes: &[u8],
) -> Result<OutputWitness, String> {
    let one_bits = match contract {
        OutputContract::Exact { minimum_bytes } => {
            if bytes.len() < *minimum_bytes {
                return Err(format!(
                    "output has {} bytes, expected at least {minimum_bytes}",
                    bytes.len()
                ));
            }
            None
        }
        OutputContract::Records {
            format,
            records,
            bits,
            minimum_one_bits,
            maximum_one_fraction,
        } => {
            let decoded = read_records(bytes, stab_format(*format)?, *bits)
                .map_err(|source| format!("record output is invalid: {source}"))?;
            if decoded.len() != *records {
                return Err(format!(
                    "record output has {} records, expected {records}",
                    decoded.len()
                ));
            }
            let ones = decoded.iter().try_fold(0_u64, |total, record| {
                let record_ones = u64::try_from(record.iter().filter(|bit| **bit).count())
                    .map_err(|_| "record one-bit count does not fit in u64".to_string())?;
                total
                    .checked_add(record_ones)
                    .ok_or_else(|| "record one-bit count overflow".to_string())
            })?;
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
            Some(ones)
        }
    };
    Ok(OutputWitness {
        bytes: u64::try_from(bytes.len())
            .map_err(|_| "output byte count does not fit in u64".to_string())?,
        sha256: hex::encode(Sha256::digest(bytes)),
        one_bits,
    })
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
    let mut records = Vec::new();
    records
        .try_reserve_exact(record_count)
        .map_err(|source| format!("cannot reserve record fixture: {source}"))?;
    for record_index in 0..record_count {
        let mut record = Vec::new();
        record
            .try_reserve_exact(bits)
            .map_err(|source| format!("cannot reserve record bits: {source}"))?;
        record.resize_with(bits, || false);
        match pattern {
            RecordPattern::Alternating => {
                for (bit_index, bit) in record.iter_mut().enumerate() {
                    *bit = (record_index + bit_index * 3).is_multiple_of(7);
                }
            }
            RecordPattern::Sparse => {
                let hit_count = bits.min(4);
                for hit in 0..hit_count {
                    let index = (record_index * 17 + hit * 97) % bits;
                    if let Some(bit) = record.get_mut(index) {
                        *bit = true;
                    }
                }
            }
        }
        records.push(record);
    }
    write_records(&records, stab_format(format)?)
        .map_err(|source| format!("cannot encode record fixture: {source}"))
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
            let decoded =
                read_records(&bytes, stab_format(format).expect("format"), 65).expect("round trip");
            assert_eq!(decoded.len(), record_count);
            assert!(decoded.iter().flatten().any(|bit| *bit));
        }
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
