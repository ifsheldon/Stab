use std::ffi::OsString;
use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use stab_core::{
    BitMatrix, BitVec, Circuit, CliffordString, PauliBasis, PauliSign, PauliString,
    PauliStringIterator, SparseXorVec, TableauIterator, stabilizers_to_tableau,
};

use crate::allocations::measure_tracked_memory;
use crate::config::{PREFIX, STIM_COMMIT, STIM_TAG};
use crate::error::BenchError;
use crate::manifest::{BenchmarkManifest, BenchmarkRow, Runner};
use crate::process::{check_success, run_process};
use crate::report::{
    BaselineCommandMetadata, BaselineReport, BaselineRowResult, Measurement, RowCommandMetadata,
    StimMetadata, machine_metadata, render_markdown_report, unix_epoch_seconds,
};
use crate::root::RepoRoot;
use crate::stim::{ensure_stim_binaries, validate_stim_source};

mod m10;
mod m11;
mod m4;
mod m7;
mod m8;
mod m9;
#[cfg(test)]
mod tests;

#[cfg(not(test))]
const STAB_COMPARE_ITERATIONS: usize = 128;
#[cfg(test)]
const STAB_COMPARE_ITERATIONS: usize = 1;
const WORD_BITS: usize = u64::BITS as usize;
const M5_BIT_TABLE_BITS: usize = 128;
const M5_BITVEC_BITS: usize = 10_000;
// Pinned Stim v1.16.0 labels this perf filter "100K" but sets n = 10 * 1000.
const M5_BITVEC_NOT_ZERO_BITS: usize = 10_000;
const M5_BITVEC_NOT_ZERO_SET_INDEX: usize = 600;
const M5_POPCOUNT_BITS: usize = 1024 * 256;
const M5_SPARSE_ROWS_USIZE: usize = 1000;
const M5_SPARSE_ROWS_U32: u32 = 1000;
const TINY_DIRECT_COMPARE_REPETITIONS: usize = 4096;
const M6_CLIFFORD_QUBITS: usize = 10_000;
const M6_PAULI_CASES: [(&str, usize); 3] = [
    ("stab_pauli_string_multiplication_1M", 1_000_000),
    ("stab_pauli_string_multiplication_100K", 100_000),
    ("stab_pauli_string_multiplication_10K", 10_000),
];
const M6_PAULI_ITER_XZ_COUNT: f64 = 232.0;
const M6_PAULI_ITER_XYZ_COUNT: f64 = 3_000.0;
const M6_TABLEAU_QUBITS: usize = 32;
const M6_TABLEAU_ITER_QUBITS: usize = 2;
const M6_STABILIZER_QUBITS: usize = 16;
const M4_PARSE_FIXTURE: &str = include_str!("../../../oracle/fixtures/inputs/parser_basic.stim");
const BASELINE_SCHEMA_VERSION: u32 = 1;
const M4_STIM_PARSE_DENSE_FIXTURE: &str = r#"
H 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0
CNOT 4 5 6 7
M 1 2 3 4 5 6 7 8 9 10 11
"#;

#[derive(Clone, Debug)]
pub(crate) struct BaselineOptions {
    pub(crate) stim: PathBuf,
    pub(crate) out: PathBuf,
    pub(crate) target_seconds: f64,
    pub(crate) cli_iterations: u32,
    pub(crate) only: Vec<String>,
    pub(crate) primary: bool,
    pub(crate) rebuild_stim: bool,
}

pub(crate) fn run_baseline(
    root: &RepoRoot,
    manifest: &BenchmarkManifest,
    options: &BaselineOptions,
) -> Result<(), BenchError> {
    if !options.target_seconds.is_finite() || options.target_seconds <= 0.0 {
        return Err(BenchError::InvalidTargetSeconds);
    }
    if options.cli_iterations == 0 {
        return Err(BenchError::InvalidCliIterations);
    }
    let stim_source = root.resolve_relative(&options.stim);
    let version = validate_stim_source(&stim_source)?;
    let rows = selected_baseline_rows(manifest, &options.only, options.primary)?;
    let needs_stim_perf = rows.iter().any(|row| row.runner == Runner::StimPerf);
    let needs_stim_cli = rows.iter().any(|row| row.runner == Runner::StimCli);
    if needs_stim_perf || needs_stim_cli {
        ensure_stim_binaries(
            root,
            &stim_source,
            needs_stim_perf,
            needs_stim_cli,
            options.rebuild_stim,
        )?;
    }

    let mut results = Vec::new();
    for row in rows {
        results.push(run_baseline_row(
            root,
            row,
            options.target_seconds,
            options.cli_iterations,
        )?);
    }

    let out_dir = root.create_benchmark_output_dir(&options.out)?;
    let report = BaselineReport {
        schema_version: 1,
        generated_unix_epoch_seconds: unix_epoch_seconds(),
        machine: machine_metadata(root)?,
        stim: StimMetadata {
            source_path: stim_source.display().to_string(),
            expected_tag: STIM_TAG.to_string(),
            expected_commit: STIM_COMMIT.to_string(),
            actual_tag: version.tag,
            actual_commit: version.commit,
        },
        command: BaselineCommandMetadata {
            target_seconds: options.target_seconds,
            cli_iterations: options.cli_iterations,
            filters: options.only.clone(),
            primary: options.primary,
        },
        rows: results,
    };
    let json_path = out_dir.join("baseline.json");
    let json = serde_json::to_string_pretty(&report)?;
    std::fs::write(&json_path, json).map_err(|source| BenchError::WriteOutput {
        path: json_path.clone(),
        source,
    })?;
    let report_path = out_dir.join("report.md");
    std::fs::write(&report_path, render_markdown_report(&report)).map_err(|source| {
        BenchError::WriteOutput {
            path: report_path.clone(),
            source,
        }
    })?;
    println!("[{PREFIX}] wrote {}", json_path.display());
    println!("[{PREFIX}] wrote {}", report_path.display());
    Ok(())
}

fn selected_baseline_rows<'a>(
    manifest: &'a BenchmarkManifest,
    only: &[String],
    primary: bool,
) -> Result<Vec<&'a BenchmarkRow>, BenchError> {
    let mut rows = manifest.filtered(only)?;
    if primary {
        rows.retain(|row| row.is_primary());
        if rows.is_empty() {
            return Err(BenchError::UnmatchedFilter("primary".to_string()));
        }
    }
    Ok(rows)
}

pub(crate) fn read_baseline_report(path: &Path) -> Result<BaselineReport, BenchError> {
    let content = std::fs::read_to_string(path).map_err(|source| BenchError::ReadBaseline {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(serde_json::from_str(&content)?)
}

pub(crate) fn validate_baseline_metadata(report: &BaselineReport) -> Result<(), BenchError> {
    let mut details = Vec::new();
    if report.schema_version != BASELINE_SCHEMA_VERSION {
        details.push(format!(
            "schema_version={} expected {}",
            report.schema_version, BASELINE_SCHEMA_VERSION
        ));
    }
    if report.stim.expected_tag != STIM_TAG {
        details.push(format!(
            "expected_tag={} expected {STIM_TAG}",
            report.stim.expected_tag
        ));
    }
    if report.stim.actual_tag != STIM_TAG {
        details.push(format!(
            "actual_tag={} expected {STIM_TAG}",
            report.stim.actual_tag
        ));
    }
    if report.stim.expected_commit != STIM_COMMIT {
        details.push(format!(
            "expected_commit={} expected {STIM_COMMIT}",
            report.stim.expected_commit
        ));
    }
    if report.stim.actual_commit != STIM_COMMIT {
        details.push(format!(
            "actual_commit={} expected {STIM_COMMIT}",
            report.stim.actual_commit
        ));
    }
    if details.is_empty() {
        Ok(())
    } else {
        Err(BenchError::BaselineMetadataMismatch {
            details: details.join("\n").into_boxed_str(),
        })
    }
}

pub(crate) fn run_stab_compare_row(
    row: &BenchmarkRow,
) -> Result<Option<Vec<Measurement>>, BenchError> {
    match row.id.as_str() {
        "m7-cli-dispatch" => Ok(Some(m7::run_cli_dispatch_row(row)?)),
        "m7-convert-stim-canonical" => Ok(Some(m7::run_convert_stim_row(row)?)),
        "m4-circuit-parse" => {
            let sparse_fixture = m4_stim_parse_sparse_fixture();
            Ok(Some(vec![
                measure_stab("stab_circuit_parse", || {
                    let circuit =
                        Circuit::from_stim_str(M4_STIM_PARSE_DENSE_FIXTURE).map_err(|error| {
                            BenchError::StabRunner {
                                row_id: row.id.clone(),
                                message: error.to_string(),
                            }
                        })?;
                    black_box(circuit.items().len());
                    Ok(())
                })?,
                measure_stab("stab_circuit_parse_sparse", || {
                    let circuit = Circuit::from_stim_str(&sparse_fixture).map_err(|error| {
                        BenchError::StabRunner {
                            row_id: row.id.clone(),
                            message: error.to_string(),
                        }
                    })?;
                    black_box(circuit.items().len());
                    Ok(())
                })?,
            ]))
        }
        "m4-circuit-canonical-print" => {
            let circuit = Circuit::from_stim_str(M4_PARSE_FIXTURE).map_err(|error| {
                BenchError::StabRunner {
                    row_id: row.id.clone(),
                    message: error.to_string(),
                }
            })?;
            Ok(Some(vec![measure_stab("stab_print_parser_basic", || {
                let text = circuit.to_stim_string();
                black_box(text.len());
                Ok(())
            })?]))
        }
        "m4-gate-lookup" => Ok(Some(m4::run_gate_lookup_row(row)?)),
        "m5-simd-bit-table" => {
            let matrix = m5_bit_matrix(&row.id)?;
            Ok(Some(vec![
                measure_stab("stab_bit_matrix_row_xor_128x128_contract", || {
                    let mut out = matrix.clone();
                    for target in 1..out.rows() {
                        out.xor_row_into(target - 1, target)
                            .map_err(|error| stab_runner_error(&row.id, error))?;
                    }
                    let last_row = out
                        .row(out.rows() - 1)
                        .map_err(|error| stab_runner_error(&row.id, error))?;
                    black_box(last_row.popcount());
                    Ok(())
                })?,
                measure_stab("stab_bit_matrix_transpose_128x128_contract", || {
                    let mut transposed = matrix.clone();
                    transposed
                        .transpose_square_in_place()
                        .map_err(|error| stab_runner_error(&row.id, error))?;
                    black_box(transposed.rows());
                    Ok(())
                })?,
            ]))
        }
        "m5-simd-bits" => {
            let left = m5_bitvec(M5_BITVEC_BITS, 0x6eed_5eed);
            let right = m5_bitvec(M5_BITVEC_BITS, 0x51ab_51ab);
            let mask = m5_bitvec(M5_BITVEC_BITS, 0xf00d_f00d);
            let not_zero = m5_not_zero_bitvec(&row.id)?;
            let mut xor_target = right.clone();
            Ok(Some(vec![
                measure_stab_batched(
                    "stab_simd_bits_xor_10K",
                    TINY_DIRECT_COMPARE_REPETITIONS,
                    || {
                        xor_target
                            .xor_assign(&left.as_bitslice())
                            .map_err(|error| stab_runner_error(&row.id, error))?;
                        black_box(xor_target.words());
                        Ok(())
                    },
                )?,
                measure_stab_batched(
                    "stab_simd_bits_not_zero_10K",
                    TINY_DIRECT_COMPARE_REPETITIONS,
                    || {
                        black_box(black_box(&not_zero).not_zero());
                        Ok(())
                    },
                )?,
                measure_stab("stab_bitvec_masked_xor_10k_contract", || {
                    let mut out = left.clone();
                    out.masked_xor_assign(&right.as_bitslice(), &mask.as_bitslice())
                        .map_err(|error| stab_runner_error(&row.id, error))?;
                    black_box(out.popcount());
                    Ok(())
                })?,
                measure_stab("stab_bitvec_range_xor_4096_contract", || {
                    let mut out = left.clone();
                    out.xor_range_from(31, &right.as_bitslice(), 17, 4096)
                        .map_err(|error| stab_runner_error(&row.id, error))?;
                    black_box(out.popcount());
                    Ok(())
                })?,
                measure_stab("stab_bitvec_copy_10k_contract", || {
                    let mut out = BitVec::zeros(left.len());
                    out.copy_from_bitslice(&left.as_bitslice())
                        .map_err(|error| stab_runner_error(&row.id, error))?;
                    black_box(out.not_zero());
                    Ok(())
                })?,
            ]))
        }
        "m5-simd-word" => {
            let mut bits = m5_bitvec(M5_POPCOUNT_BITS, 0x5151_5151);
            Ok(Some(vec![measure_stab(
                "stab_bitvec_popcount_262144",
                || {
                    let bit = bits.get(300).ok_or_else(|| {
                        stab_runner_error(&row.id, "benchmark popcount bit index is out of range")
                    })?;
                    bits.set(300, !bit)
                        .map_err(|error| stab_runner_error(&row.id, error))?;
                    black_box(bits.popcount());
                    Ok(())
                },
            )?]))
        }
        "m5-sparse-xor" => {
            let mut table = m5_sparse_xor_table();
            let xor_items = [2, 5, 9, 5, 3, 6, 10];
            let mut buf = SparseXorVec::new();
            Ok(Some(vec![
                measure_stab("stab_sparse_table_row_xor_1000", || {
                    sparse_table_row_xor(&mut table);
                    black_box(
                        table
                            .get(M5_SPARSE_ROWS_USIZE / 2)
                            .map_or(0, |row| row.items().len()),
                    );
                    Ok(())
                })?,
                measure_sparse_xor_items("stab_sparse_xor_item_7", &mut buf, &xor_items)?,
            ]))
        }
        "m6-clifford-string" => {
            let mut left = CliffordString::identity(M6_CLIFFORD_QUBITS);
            let right = CliffordString::identity(M6_CLIFFORD_QUBITS);
            Ok(Some(vec![measure_stab(
                "stab_clifford_string_multiplication_10K",
                || {
                    left.right_multiply_in_place(&right)
                        .map_err(|error| stab_runner_error(&row.id, error))?;
                    black_box(&left);
                    Ok(())
                },
            )?]))
        }
        "m6-pauli-string" => {
            let mut measurements = Vec::with_capacity(M6_PAULI_CASES.len());
            for (name, num_qubits) in M6_PAULI_CASES {
                let mut left = PauliString::identity(num_qubits);
                let right = PauliString::identity(num_qubits);
                measurements.push(measure_stab(name, || {
                    let log_i = left
                        .right_multiply_in_place_returning_log_i_scalar(&right)
                        .map_err(|error| stab_runner_error(&row.id, error))?;
                    black_box((log_i, &left));
                    Ok(())
                })?);
            }
            Ok(Some(measurements))
        }
        "m6-pauli-iter" => Ok(Some(vec![
            measure_pauli_iter("stab_pauli_iter_xz_2_to_5_of_5", 5, 2, 5, true, false, true)?,
            measure_pauli_iter(
                "stab_pauli_iter_xyz_1_of_1000",
                1000,
                1,
                1,
                true,
                true,
                true,
            )?,
        ])),
        "m6-tableau" => {
            let circuit = m6_tableau_circuit(&row.id)?;
            let tableau = circuit
                .to_tableau(false, false, false)
                .map_err(|error| stab_runner_error(&row.id, error))?;
            let pauli = m6_pauli_string(M6_TABLEAU_QUBITS, 0x07ab_1ea7);
            Ok(Some(vec![
                measure_stab("stab_tableau_from_circuit_32q", || {
                    let result = circuit
                        .to_tableau(false, false, false)
                        .map_err(|error| stab_runner_error(&row.id, error))?;
                    black_box(result);
                    Ok(())
                })?,
                measure_stab("stab_tableau_inverse_32q", || {
                    let inverse = tableau
                        .inverse()
                        .map_err(|error| stab_runner_error(&row.id, error))?;
                    black_box(inverse);
                    Ok(())
                })?,
                measure_stab("stab_tableau_apply_32q", || {
                    let output = tableau
                        .apply(&pauli)
                        .map_err(|error| stab_runner_error(&row.id, error))?;
                    black_box(output);
                    Ok(())
                })?,
            ]))
        }
        "m6-tableau-iter" => Ok(Some(vec![measure_stab(
            "stab_tableau_iter_unsigned_2q",
            || {
                let mut count = 0_usize;
                for tableau in TableauIterator::new(M6_TABLEAU_ITER_QUBITS, false)
                    .map_err(|error| stab_runner_error(&row.id, error))?
                {
                    count += 1;
                    black_box(tableau);
                }
                black_box(count);
                Ok(())
            },
        )?])),
        "m6-stabilizers-to-tableau" => {
            let stabilizers = m6_z_stabilizers(M6_STABILIZER_QUBITS);
            Ok(Some(vec![
                measure_stab("stab_stabilizers_to_tableau_16q", || {
                    let tableau = stabilizers_to_tableau(&stabilizers, false, false, false)
                        .map_err(|error| stab_runner_error(&row.id, error))?;
                    black_box(tableau);
                    Ok(())
                })?,
                measure_stab("stab_stabilizers_to_inverse_tableau_16q", || {
                    let tableau = stabilizers_to_tableau(&stabilizers, false, false, true)
                        .map_err(|error| stab_runner_error(&row.id, error))?;
                    black_box(tableau);
                    Ok(())
                })?,
            ]))
        }
        _ => {
            if let Some(measurements) = m8::run_sample_compare_row(row)? {
                Ok(Some(measurements))
            } else if let Some(measurements) = m9::run_detection_compare_row(row)? {
                Ok(Some(measurements))
            } else if let Some(measurements) = m10::run_dem_compare_row(row)? {
                Ok(Some(measurements))
            } else if let Some(measurements) = m11::run_dem_sampling_compare_row(row)? {
                Ok(Some(measurements))
            } else if let Some(measurements) = m7::run_generator_compare_row(row)? {
                Ok(Some(measurements))
            } else if row.runner == Runner::ContractOnly {
                Ok(Some(Vec::new()))
            } else {
                Ok(None)
            }
        }
    }
}

fn m4_stim_parse_sparse_fixture() -> String {
    let mut text = String::new();
    for _ in 0..1000 {
        text.push_str("H 0\nCNOT 1 2\nM 0\n");
    }
    text
}

fn m5_bitvec(bit_len: usize, seed: u64) -> BitVec {
    BitVec::from_words_truncated(bit_len, deterministic_words(words_for_bits(bit_len), seed))
}

fn m5_not_zero_bitvec(row_id: &str) -> Result<BitVec, BenchError> {
    let mut bits = BitVec::zeros(M5_BITVEC_NOT_ZERO_BITS);
    bits.set(M5_BITVEC_NOT_ZERO_SET_INDEX, true)
        .map_err(|error| stab_runner_error(row_id, error))?;
    Ok(bits)
}

fn m5_bit_matrix(row_id: &str) -> Result<BitMatrix, BenchError> {
    let mut matrix = BitMatrix::zeros(M5_BIT_TABLE_BITS, M5_BIT_TABLE_BITS)
        .map_err(|error| stab_runner_error(row_id, error))?;
    for row in 0..M5_BIT_TABLE_BITS {
        for col in 0..M5_BIT_TABLE_BITS {
            if (row * 17 + col * 31) % 11 == 0 {
                matrix
                    .set(row, col, true)
                    .map_err(|error| stab_runner_error(row_id, error))?;
            }
        }
    }
    Ok(matrix)
}

fn m5_sparse_xor_table() -> Vec<SparseXorVec> {
    let mut table = Vec::with_capacity(M5_SPARSE_ROWS_USIZE);
    for row in 0..M5_SPARSE_ROWS_U32 {
        let mut sparse_row = SparseXorVec::new();
        for item in [row, row + 1, row + 4, row + 8, row + 15] {
            sparse_row.xor_item(item);
        }
        table.push(sparse_row);
    }
    table
}

fn sparse_table_row_xor(table: &mut [SparseXorVec]) {
    for row in 1..table.len() {
        let (prefix, suffix) = table.split_at_mut(row);
        if let (Some(target), Some(source)) = (prefix.last_mut(), suffix.first()) {
            target.xor_assign(source);
        }
    }
    for row in (2..table.len()).rev() {
        let (prefix, suffix) = table.split_at_mut(row);
        if let (Some(target), Some(source)) = (prefix.last_mut(), suffix.first()) {
            target.xor_assign(source);
        }
    }
}

fn measure_sparse_xor_items(
    name: &str,
    buf: &mut SparseXorVec,
    xor_items: &[u32],
) -> Result<Measurement, BenchError> {
    let mut timings = Vec::with_capacity(STAB_COMPARE_ITERATIONS);
    for _ in 0..STAB_COMPARE_ITERATIONS {
        let start = Instant::now();
        run_sparse_xor_item_batch(buf, xor_items);
        timings.push(
            start
                .elapsed()
                .div_f64(TINY_DIRECT_COMPARE_REPETITIONS as f64),
        );
    }
    let variance_seconds = duration_variance_seconds(&timings);
    timings.sort();
    let seconds = timings
        .get(timings.len() / 2)
        .map(Duration::as_secs_f64)
        .unwrap_or_default();
    let tracked_memory = measure_tracked_memory(|| {
        run_sparse_xor_item_batch(buf, xor_items);
        Ok(())
    })?;
    Ok(Measurement {
        name: name.to_string(),
        seconds,
        variance_seconds,
        allocation: tracked_memory.allocation,
        resident_bytes: tracked_memory.resident_bytes_max,
        iterations: Some(STAB_COMPARE_ITERATIONS),
    })
}

fn run_sparse_xor_item_batch(buf: &mut SparseXorVec, xor_items: &[u32]) {
    for _ in 0..TINY_DIRECT_COMPARE_REPETITIONS {
        for item in xor_items {
            buf.xor_item(*item);
        }
    }
    black_box(buf.items().len());
}

fn m6_pauli_string(num_qubits: usize, seed: u64) -> PauliString {
    let mut state = seed;
    let bases = (0..num_qubits).map(|_| {
        state = splitmix64(state);
        PauliBasis::from_xz((state & 1) != 0, (state & 2) != 0)
    });
    PauliString::from_bases(PauliSign::Plus, bases)
}

fn m6_tableau_circuit(row_id: &str) -> Result<Circuit, BenchError> {
    let mut text = String::new();
    for index in 0..M6_TABLEAU_QUBITS {
        text.push_str("H ");
        text.push_str(&index.to_string());
        text.push('\n');
    }
    for index in 0..M6_TABLEAU_QUBITS.saturating_sub(1) {
        text.push_str("CX ");
        text.push_str(&index.to_string());
        text.push(' ');
        text.push_str(&(index + 1).to_string());
        text.push('\n');
    }
    Circuit::from_stim_str(&text).map_err(|error| stab_runner_error(row_id, error))
}

fn m6_z_stabilizers(num_qubits: usize) -> Vec<PauliString> {
    (0..num_qubits)
        .map(|target| {
            PauliString::from_bases(
                PauliSign::Plus,
                (0..num_qubits).map(|index| {
                    if index == target {
                        PauliBasis::Z
                    } else {
                        PauliBasis::I
                    }
                }),
            )
        })
        .collect()
}

fn words_for_bits(bit_len: usize) -> usize {
    bit_len.div_ceil(WORD_BITS)
}

fn deterministic_words(word_count: usize, seed: u64) -> Vec<u64> {
    let mut state = seed;
    let mut words = Vec::with_capacity(word_count);
    for _ in 0..word_count {
        state = splitmix64(state);
        words.push(state);
    }
    words
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9E37_79B9_7F4A_7C15);
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

fn measure_pauli_iter(
    name: &str,
    num_qubits: usize,
    min_weight: usize,
    max_weight: usize,
    allow_x: bool,
    allow_y: bool,
    allow_z: bool,
) -> Result<Measurement, BenchError> {
    measure_stab(name, || {
        let mut count = 0_usize;
        let mut total_len = 0_usize;
        let mut iter = PauliStringIterator::new(
            num_qubits, min_weight, max_weight, allow_x, allow_y, allow_z,
        );
        while iter.iter_next() {
            count += 1;
            total_len += iter.result().len();
        }
        black_box((count, total_len));
        Ok(())
    })
}

fn stab_runner_error(row_id: &str, error: impl ToString) -> BenchError {
    BenchError::StabRunner {
        row_id: row_id.to_string(),
        message: error.to_string(),
    }
}

fn measure_stab(
    name: &str,
    mut operation: impl FnMut() -> Result<(), BenchError>,
) -> Result<Measurement, BenchError> {
    measure_stab_iterations(name, STAB_COMPARE_ITERATIONS, &mut operation)
}

fn measure_stab_iterations(
    name: &str,
    iterations: usize,
    mut operation: impl FnMut() -> Result<(), BenchError>,
) -> Result<Measurement, BenchError> {
    let mut timings = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let start = Instant::now();
        operation()?;
        timings.push(start.elapsed());
    }
    let variance_seconds = duration_variance_seconds(&timings);
    timings.sort();
    let seconds = timings
        .get(timings.len() / 2)
        .map(Duration::as_secs_f64)
        .unwrap_or_default();
    let tracked_memory = measure_tracked_memory(&mut operation)?;
    Ok(Measurement {
        name: name.to_string(),
        seconds,
        variance_seconds,
        allocation: tracked_memory.allocation,
        resident_bytes: tracked_memory.resident_bytes_max,
        iterations: Some(iterations),
    })
}

fn measure_stab_batched(
    name: &str,
    repetitions: usize,
    mut operation: impl FnMut() -> Result<(), BenchError>,
) -> Result<Measurement, BenchError> {
    measure_stab_batched_iterations(name, STAB_COMPARE_ITERATIONS, repetitions, &mut operation)
}

fn measure_stab_batched_iterations(
    name: &str,
    iterations: usize,
    repetitions: usize,
    mut operation: impl FnMut() -> Result<(), BenchError>,
) -> Result<Measurement, BenchError> {
    let mut timings = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let start = Instant::now();
        for _ in 0..repetitions {
            operation()?;
        }
        timings.push(start.elapsed().div_f64(repetitions as f64));
    }
    let variance_seconds = duration_variance_seconds(&timings);
    timings.sort();
    let seconds = timings
        .get(timings.len() / 2)
        .map(Duration::as_secs_f64)
        .unwrap_or_default();
    let tracked_memory = measure_tracked_memory(|| {
        for _ in 0..repetitions {
            operation()?;
        }
        Ok(())
    })?;
    Ok(Measurement {
        name: name.to_string(),
        seconds,
        variance_seconds,
        allocation: tracked_memory.allocation,
        resident_bytes: tracked_memory.resident_bytes_max,
        iterations: Some(iterations),
    })
}

pub(crate) fn summarize_measurements(measurements: &[Measurement]) -> String {
    measurements
        .iter()
        .map(|measurement| format!("{}={:.9}s", measurement.name, measurement.seconds))
        .collect::<Vec<_>>()
        .join(",")
}

pub(crate) fn summarize_stab_measurements(row_id: &str, measurements: &[Measurement]) -> String {
    let summary = summarize_measurements(measurements);
    let rates = summarize_measurement_rates(row_id, measurements);
    if rates.is_empty() {
        summary
    } else {
        format!("{summary} rates={rates}")
    }
}

fn summarize_measurement_rates(row_id: &str, measurements: &[Measurement]) -> String {
    measurements
        .iter()
        .filter_map(|measurement| {
            measurement_work(row_id, &measurement.name).map(|(work, unit)| {
                let rate = if measurement.seconds > 0.0 {
                    work / measurement.seconds
                } else {
                    0.0
                };
                format!("{}={rate:.3e}{unit}", measurement.name)
            })
        })
        .collect::<Vec<_>>()
        .join(",")
}

pub(crate) fn measurement_work(row_id: &str, name: &str) -> Option<(f64, &'static str)> {
    if let Some(work) = m4::measurement_work(row_id, name) {
        return Some(work);
    }
    if let Some(work) = m8::measurement_work(row_id, name) {
        return Some(work);
    }
    if let Some(work) = m9::measurement_work(row_id, name) {
        return Some(work);
    }
    if let Some(work) = m10::measurement_work(row_id, name) {
        return Some(work);
    }
    if let Some(work) = m11::measurement_work(row_id, name) {
        return Some(work);
    }
    if row_id.starts_with("m7-gen-") && name.starts_with("stab_gen_") {
        return Some((1.0, "circuits/s"));
    }
    if row_id == "m7-cli-dispatch" && name == "stab_cli_dispatch_gen_d3_r3" {
        return Some((1.0, "dispatches/s"));
    }
    if row_id == "m7-convert-stim-canonical" && name == "stab_convert_stim_canonical" {
        return Some((M4_PARSE_FIXTURE.len() as f64, "bytes/s"));
    }
    match (row_id, name) {
        ("m5-simd-bit-table", "stab_bit_matrix_row_xor_128x128_contract") => {
            Some(((M5_BIT_TABLE_BITS - 1) as f64, "row-xors/s"))
        }
        ("m5-simd-bit-table", "stab_bit_matrix_transpose_128x128_contract") => {
            Some(((M5_BIT_TABLE_BITS * M5_BIT_TABLE_BITS) as f64, "bits/s"))
        }
        ("m5-simd-bits", "stab_simd_bits_xor_10K")
        | ("m5-simd-bits", "stab_bitvec_masked_xor_10k_contract")
        | ("m5-simd-bits", "stab_bitvec_copy_10k_contract") => {
            Some((M5_BITVEC_BITS as f64, "bits/s"))
        }
        ("m5-simd-bits", "stab_simd_bits_not_zero_10K") => {
            Some((M5_BITVEC_NOT_ZERO_BITS as f64, "bits/s"))
        }
        ("m5-simd-bits", "stab_bitvec_range_xor_4096_contract") => Some((4096.0, "bits/s")),
        ("m5-simd-word", "stab_bitvec_popcount_262144") => {
            Some((M5_POPCOUNT_BITS as f64, "bits/s"))
        }
        ("m5-sparse-xor", "stab_sparse_table_row_xor_1000") => {
            Some(((M5_SPARSE_ROWS_USIZE * 2) as f64, "row-xors/s"))
        }
        ("m5-sparse-xor", "stab_sparse_xor_item_7") => Some((7.0, "items/s")),
        ("m6-clifford-string", "stab_clifford_string_multiplication_10K") => {
            Some((M6_CLIFFORD_QUBITS as f64, "single-qubit-products/s"))
        }
        ("m6-pauli-string", "stab_pauli_string_multiplication_1M") => {
            Some((1_000_000.0, "qubits/s"))
        }
        ("m6-pauli-string", "stab_pauli_string_multiplication_100K") => {
            Some((100_000.0, "qubits/s"))
        }
        ("m6-pauli-string", "stab_pauli_string_multiplication_10K") => Some((10_000.0, "qubits/s")),
        ("m6-pauli-iter", "stab_pauli_iter_xz_2_to_5_of_5") => {
            Some((M6_PAULI_ITER_XZ_COUNT, "PauliStrings/s"))
        }
        ("m6-pauli-iter", "stab_pauli_iter_xyz_1_of_1000") => {
            Some((M6_PAULI_ITER_XYZ_COUNT, "PauliStrings/s"))
        }
        ("m6-tableau", "stab_tableau_from_circuit_32q") => {
            Some(((M6_TABLEAU_QUBITS * 2) as f64, "gates/s"))
        }
        ("m6-tableau", "stab_tableau_inverse_32q") | ("m6-tableau", "stab_tableau_apply_32q") => {
            Some((M6_TABLEAU_QUBITS as f64, "qubits/s"))
        }
        ("m6-tableau-iter", "stab_tableau_iter_unsigned_2q") => Some((720.0, "tableaus/s")),
        ("m6-stabilizers-to-tableau", "stab_stabilizers_to_tableau_16q")
        | ("m6-stabilizers-to-tableau", "stab_stabilizers_to_inverse_tableau_16q") => {
            Some((M6_STABILIZER_QUBITS as f64, "stabilizers/s"))
        }
        _ => None,
    }
}

pub(crate) fn compare_note(row_id: &str) -> Option<&'static str> {
    if let Some(note) = m4::compare_note(row_id) {
        return Some(note);
    }
    if let Some(note) = m8::compare_note(row_id) {
        return Some(note);
    }
    if let Some(note) = m9::compare_note(row_id) {
        return Some(note);
    }
    if let Some(note) = m10::compare_note(row_id) {
        return Some(note);
    }
    if let Some(note) = m11::compare_note(row_id) {
        return Some(note);
    }
    match row_id {
        "m4-circuit-parse" => Some(
            "direct-match: Stab measures dense and sparse .stim parser cases against the pinned Stim circuit_parse perf filters",
        ),
        "m7-perf-harness" => Some(
            "contract-only: verifies baseline metadata coverage; no Stab runtime workload is expected",
        ),
        "m7-cli-dispatch" => Some(
            "report-only: Stab measures in-process gen dispatch; upstream baseline is sample-heavy main dispatch",
        ),
        "m7-convert-stim-canonical" => Some(
            "contract-only: Stab measures in-process canonical .stim conversion; pinned Stim has no matching circuit-convert CLI",
        ),
        id if id.starts_with("m7-gen-") => Some(
            "report-only: Stab measures direct Rust generator construction and formatting-independent circuit access",
        ),
        "m5-simd-bit-table" => Some(
            "contract-smoke: Stab transpose/row-xor uses 128x128 until optimized 10k transpose parity is introduced",
        ),
        "m5-simd-bits" => Some(
            "partial-match: direct XOR and not-zero submeasurements pair with pinned Stim simd_bits filters using repeated in-process timing; masked/range/copy are Stab M5 contract extras and randomize is not implemented in M5",
        ),
        "m5-simd-word" => Some(
            "direct-match: Stab measures popcount-like bit-vector work against the pinned Stim simd_compat_popcnt perf filter",
        ),
        "m5-sparse-xor" => Some(
            "direct-match: Stab measures sparse table row XOR and sparse item XOR against the pinned Stim sparse_xor_vec perf filters",
        ),
        "m6-clifford-string" => Some(
            "direct-match: Stab measures in-place 10K CliffordString multiplication against the pinned Stim perf filter",
        ),
        "m6-pauli-string" => Some(
            "direct-match: Stab measures in-place PauliString multiplication at 10K, 100K, and 1M against the pinned Stim perf filters",
        ),
        "m6-pauli-iter" => Some(
            "direct-match: Stab measures borrowed-result PauliStringIterator workloads matching the pinned Stim perf filters",
        ),
        "m6-tableau" => Some(
            "report-only: Stab uses deterministic 32q circuit/tableau operations until M12 defines optimized random 10k-qubit parity thresholds",
        ),
        "m6-tableau-iter" => Some(
            "report-only: deterministic unsigned 2q iterator workload; upstream baseline uses unsigned and signed 3q iterator filters",
        ),
        "m6-stabilizers-to-tableau" => Some(
            "report-only: deterministic 16q conversion workload; exact random/fuzz performance parity remains M12 work",
        ),
        _ => None,
    }
}

fn run_baseline_row(
    root: &RepoRoot,
    row: &BenchmarkRow,
    target_seconds: f64,
    cli_iterations: u32,
) -> Result<BaselineRowResult, BenchError> {
    match row.runner {
        Runner::ContractOnly => Ok(BaselineRowResult {
            id: row.id.clone(),
            milestone: row.milestone,
            threshold_class: row.threshold_class.clone(),
            runner: row.runner,
            upstream_source: row.upstream_source.clone(),
            phase: row.phase.clone(),
            measurement: row.measurement.clone(),
            status: "contract-only".to_string(),
            command: RowCommandMetadata {
                program: String::new(),
                args: Vec::new(),
                stdin_path: row.stdin_path.clone(),
            },
            measurements: Vec::new(),
        }),
        Runner::StimPerf => run_stim_perf_row(root, row, target_seconds),
        Runner::StimCli => run_stim_cli_row(root, row, cli_iterations),
    }
}

fn run_stim_perf_row(
    root: &RepoRoot,
    row: &BenchmarkRow,
    target_seconds: f64,
) -> Result<BaselineRowResult, BenchError> {
    let args = vec![
        OsString::from("--only"),
        OsString::from(row.stim_perf_filter.as_str()),
        OsString::from("--target_seconds"),
        OsString::from(format!("{target_seconds}")),
    ];
    let output = run_process(&root.stim_perf_binary(), &args, b"", &root.path, true)?;
    check_success(&root.stim_perf_binary(), &output)?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let measurements = parse_stim_perf_output(&stdout);
    if measurements.is_empty() {
        return Err(BenchError::MissingPerfMeasurements {
            row_id: row.id.clone(),
        });
    }
    Ok(BaselineRowResult {
        id: row.id.clone(),
        milestone: row.milestone,
        threshold_class: row.threshold_class.clone(),
        runner: row.runner,
        upstream_source: row.upstream_source.clone(),
        phase: row.phase.clone(),
        measurement: row.measurement.clone(),
        status: "measured".to_string(),
        command: RowCommandMetadata {
            program: root.stim_perf_binary().display().to_string(),
            args: args
                .iter()
                .map(|arg| arg.to_string_lossy().to_string())
                .collect(),
            stdin_path: row.stdin_path.clone(),
        },
        measurements,
    })
}

fn run_stim_cli_row(
    root: &RepoRoot,
    row: &BenchmarkRow,
    cli_iterations: u32,
) -> Result<BaselineRowResult, BenchError> {
    let iterations = usize::try_from(cli_iterations)
        .map_err(|_| BenchError::CliIterationsOverflow(cli_iterations))?;
    let args = row
        .argv_tokens()
        .into_iter()
        .map(OsString::from)
        .collect::<Vec<_>>();
    let stdin = row.stdin(root)?;
    let mut timings = Vec::new();
    for _ in 0..iterations {
        let start = Instant::now();
        let output = run_process(&root.stim_binary(), &args, &stdin, &root.path, false)?;
        check_success(&root.stim_binary(), &output)?;
        timings.push(start.elapsed());
    }
    let variance_seconds = duration_variance_seconds(&timings);
    timings.sort();
    let median = timings
        .get(timings.len() / 2)
        .map(Duration::as_secs_f64)
        .unwrap_or_default();
    Ok(BaselineRowResult {
        id: row.id.clone(),
        milestone: row.milestone,
        threshold_class: row.threshold_class.clone(),
        runner: row.runner,
        upstream_source: row.upstream_source.clone(),
        phase: row.phase.clone(),
        measurement: row.measurement.clone(),
        status: "measured".to_string(),
        command: RowCommandMetadata {
            program: root.stim_binary().display().to_string(),
            args: args
                .iter()
                .map(|arg| arg.to_string_lossy().to_string())
                .collect(),
            stdin_path: row.stdin_path.clone(),
        },
        measurements: vec![Measurement {
            name: row.id.clone(),
            seconds: median,
            variance_seconds,
            allocation: None,
            resident_bytes: None,
            iterations: Some(iterations),
        }],
    })
}

fn parse_stim_perf_output(stdout: &str) -> Vec<Measurement> {
    stdout.lines().filter_map(parse_stim_perf_line).collect()
}

fn parse_stim_perf_line(line: &str) -> Option<Measurement> {
    if !line.contains("(vs") {
        return None;
    }
    let name = line.split_whitespace().last()?.to_string();
    let before_expected = line.split("(vs").next()?.trim();
    let mut parts = before_expected.split_whitespace().rev();
    let unit = parts.next()?;
    let value = parts.next()?.parse::<f64>().ok()?;
    let seconds = duration_seconds(value, unit)?;
    Some(Measurement {
        name,
        seconds,
        variance_seconds: None,
        allocation: None,
        resident_bytes: None,
        iterations: None,
    })
}

fn duration_seconds(value: f64, unit: &str) -> Option<f64> {
    match unit {
        "s" => Some(value),
        "ms" => Some(value / 1_000.0),
        "us" => Some(value / 1_000_000.0),
        "ns" => Some(value / 1_000_000_000.0),
        "ps" => Some(value / 1_000_000_000_000.0),
        _ => None,
    }
}

fn duration_variance_seconds(timings: &[Duration]) -> Option<f64> {
    if timings.is_empty() {
        return None;
    }
    let seconds = timings
        .iter()
        .map(Duration::as_secs_f64)
        .collect::<Vec<_>>();
    let mean = seconds.iter().sum::<f64>() / seconds.len() as f64;
    let variance = seconds
        .iter()
        .map(|sample| {
            let delta = sample - mean;
            delta * delta
        })
        .sum::<f64>()
        / seconds.len() as f64;
    Some(variance)
}
