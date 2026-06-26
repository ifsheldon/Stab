use std::ffi::OsString;
use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use stab_core::{BitMatrix, BitVec, Circuit, Gate, SparseXorVec};

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

const STAB_COMPARE_ITERATIONS: usize = 128;
const WORD_BITS: usize = u64::BITS as usize;
const M5_BIT_TABLE_BITS: usize = 128;
const M5_BITVEC_BITS: usize = 10_000;
const M5_POPCOUNT_BITS: usize = 1024 * 256;
const M5_SPARSE_ROWS_USIZE: usize = 1000;
const M5_SPARSE_ROWS_U32: u32 = 1000;
const M4_PARSE_FIXTURE: &str = include_str!("../../../oracle/fixtures/inputs/parser_basic.stim");
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
    pub(crate) rebuild_stim: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct CompareOptions {
    pub(crate) baseline: PathBuf,
    pub(crate) milestone: Option<String>,
    pub(crate) strict: bool,
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
    let rows = manifest.filtered(&options.only)?;
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

pub(crate) fn run_compare(
    root: &RepoRoot,
    manifest: &BenchmarkManifest,
    options: &CompareOptions,
) -> Result<(), BenchError> {
    let baseline_path = root.resolve_relative(&options.baseline);
    let baseline_report = read_baseline_report(&baseline_path)?;
    let rows = manifest
        .rows
        .iter()
        .filter(|row| {
            options
                .milestone
                .as_deref()
                .is_none_or(|milestone| milestone == row.milestone.as_str())
        })
        .collect::<Vec<_>>();
    println!(
        "[{PREFIX}] comparing {} row(s) against {}",
        rows.len(),
        baseline_path.display()
    );
    let mut pending = Vec::new();
    let mut missing_baselines = Vec::new();
    for row in rows {
        let stim_summary = match summarize_baseline_row(&baseline_report, &row.id) {
            BaselineSummary::Present(summary) => summary,
            BaselineSummary::Missing => {
                missing_baselines.push(row.id.clone());
                "missing-baseline".to_string()
            }
        };
        match run_stab_compare_row(row)? {
            Some(measurements) => {
                let note = compare_note(&row.id)
                    .map(|note| format!(" note={note}"))
                    .unwrap_or_default();
                println!(
                    "- {} {} status=measured stab={} stim={}{}",
                    row.milestone.as_str(),
                    row.id,
                    summarize_stab_measurements(&row.id, &measurements),
                    stim_summary,
                    note
                );
            }
            None => {
                println!(
                    "- {} {} status=pending stab=no-runner stim={}",
                    row.milestone.as_str(),
                    row.id,
                    stim_summary
                );
                pending.push(row.id.clone());
            }
        }
    }
    if options.strict && (!pending.is_empty() || !missing_baselines.is_empty()) {
        Err(BenchError::CompareIncomplete {
            details: compare_incomplete_details(&pending, &missing_baselines).into_boxed_str(),
        })
    } else {
        Ok(())
    }
}

fn read_baseline_report(path: &Path) -> Result<BaselineReport, BenchError> {
    let content = std::fs::read_to_string(path).map_err(|source| BenchError::ReadBaseline {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(serde_json::from_str(&content)?)
}

fn run_stab_compare_row(row: &BenchmarkRow) -> Result<Option<Vec<Measurement>>, BenchError> {
    match row.id.as_str() {
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
        "m4-gate-lookup" => {
            let names = Gate::all().map(Gate::canonical_name).collect::<Vec<_>>();
            Ok(Some(vec![measure_stab("stab_gate_lookup_all", || {
                for name in &names {
                    let gate = Gate::from_name(name).map_err(|error| BenchError::StabRunner {
                        row_id: row.id.clone(),
                        message: error.to_string(),
                    })?;
                    black_box(gate);
                }
                Ok(())
            })?]))
        }
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
            let mut xor_target = right.clone();
            Ok(Some(vec![
                measure_stab("stab_bitvec_xor_10k", || {
                    xor_target
                        .xor_assign(&left.as_bitslice())
                        .map_err(|error| stab_runner_error(&row.id, error))?;
                    black_box(xor_target.words());
                    Ok(())
                })?,
                measure_stab("stab_bitvec_not_zero_10k", || {
                    black_box(left.not_zero());
                    Ok(())
                })?,
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
                measure_stab("stab_sparse_xor_item_7", || {
                    for item in xor_items {
                        buf.xor_item(item);
                    }
                    black_box(buf.items().len());
                    Ok(())
                })?,
            ]))
        }
        _ => Ok(None),
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
    let mut timings = Vec::with_capacity(STAB_COMPARE_ITERATIONS);
    for _ in 0..STAB_COMPARE_ITERATIONS {
        let start = Instant::now();
        operation()?;
        timings.push(start.elapsed());
    }
    timings.sort();
    let seconds = timings
        .get(timings.len() / 2)
        .map(Duration::as_secs_f64)
        .unwrap_or_default();
    Ok(Measurement {
        name: name.to_string(),
        seconds,
        iterations: Some(STAB_COMPARE_ITERATIONS),
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum BaselineSummary {
    Present(String),
    Missing,
}

fn summarize_baseline_row(report: &BaselineReport, row_id: &str) -> BaselineSummary {
    let Some(row) = report.rows.iter().find(|row| row.id == row_id) else {
        return BaselineSummary::Missing;
    };
    if row.measurements.is_empty() {
        return BaselineSummary::Present(row.status.clone());
    }
    BaselineSummary::Present(summarize_measurements(&row.measurements))
}

fn compare_incomplete_details(pending: &[String], missing_baselines: &[String]) -> String {
    let mut details = Vec::new();
    if !pending.is_empty() {
        details.push(format!(
            "pending Stab comparison runner(s): {}",
            pending.join(", ")
        ));
    }
    if !missing_baselines.is_empty() {
        details.push(format!(
            "missing baseline row(s): {}",
            missing_baselines.join(", ")
        ));
    }
    details.join("\n")
}

fn summarize_measurements(measurements: &[Measurement]) -> String {
    measurements
        .iter()
        .map(|measurement| format!("{}={:.9}s", measurement.name, measurement.seconds))
        .collect::<Vec<_>>()
        .join(",")
}

fn summarize_stab_measurements(row_id: &str, measurements: &[Measurement]) -> String {
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

fn measurement_work(row_id: &str, name: &str) -> Option<(f64, &'static str)> {
    match (row_id, name) {
        ("m5-simd-bit-table", "stab_bit_matrix_row_xor_128x128_contract") => {
            Some(((M5_BIT_TABLE_BITS - 1) as f64, "row-xors/s"))
        }
        ("m5-simd-bit-table", "stab_bit_matrix_transpose_128x128_contract") => {
            Some(((M5_BIT_TABLE_BITS * M5_BIT_TABLE_BITS) as f64, "bits/s"))
        }
        ("m5-simd-bits", "stab_bitvec_xor_10k")
        | ("m5-simd-bits", "stab_bitvec_not_zero_10k")
        | ("m5-simd-bits", "stab_bitvec_masked_xor_10k_contract")
        | ("m5-simd-bits", "stab_bitvec_copy_10k_contract") => {
            Some((M5_BITVEC_BITS as f64, "bits/s"))
        }
        ("m5-simd-bits", "stab_bitvec_range_xor_4096_contract") => Some((4096.0, "bits/s")),
        ("m5-simd-word", "stab_bitvec_popcount_262144") => {
            Some((M5_POPCOUNT_BITS as f64, "bits/s"))
        }
        ("m5-sparse-xor", "stab_sparse_table_row_xor_1000") => {
            Some(((M5_SPARSE_ROWS_USIZE * 2) as f64, "row-xors/s"))
        }
        ("m5-sparse-xor", "stab_sparse_xor_item_7") => Some((7.0, "items/s")),
        _ => None,
    }
}

fn compare_note(row_id: &str) -> Option<&'static str> {
    match row_id {
        "m5-simd-bit-table" => Some(
            "contract-smoke: Stab transpose/row-xor uses 128x128 until optimized 10k transpose parity is introduced",
        ),
        "m5-simd-bits" => Some(
            "partial-match: xor/not_zero use upstream 10k size; masked/range/copy are Stab M5 contract extras; randomize is not implemented in M5",
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

#[cfg(test)]
mod tests {
    use super::{BaselineSummary, parse_stim_perf_line, summarize_baseline_row};
    use crate::report::{BaselineReport, Measurement};

    #[test]
    fn parses_stim_perf_measurement_line() {
        let measurement = parse_stim_perf_line(
            "[..................*<|....................] 1.3 us (vs 950 ns) circuit_parse",
        )
        .expect("parse line");

        assert_eq!(
            measurement,
            Measurement {
                name: "circuit_parse".to_string(),
                seconds: 0.0000013,
                iterations: None,
            }
        );
    }

    #[test]
    fn summarizes_present_contract_and_missing_baseline_rows() {
        let report = serde_json::from_str::<BaselineReport>(
            r#"{
                "schema_version": 1,
                "generated_unix_epoch_seconds": 0,
                "machine": {
                    "os": "linux",
                    "arch": "x86_64",
                    "family": "unix",
                    "available_parallelism": 1,
                    "rustc_version": "rustc test",
                    "cmake_version": "cmake test"
                },
                "stim": {
                    "source_path": "vendor/stim",
                    "expected_tag": "v1.16.0",
                    "expected_commit": "expected",
                    "actual_tag": "v1.16.0",
                    "actual_commit": "actual"
                },
                "command": {
                    "target_seconds": 0.001,
                    "cli_iterations": 1,
                    "filters": []
                },
                "rows": [
                    {
                        "id": "measured-row",
                        "milestone": "M4",
                        "threshold_class": "report-only",
                        "runner": "stim-perf",
                        "upstream_source": "src/stim/circuit/circuit.perf.cc",
                        "phase": "analysis",
                        "measurement": "parser-throughput",
                        "status": "measured",
                        "command": {
                            "program": "stim_perf",
                            "args": [],
                            "stdin_path": ""
                        },
                        "measurements": [
                            {
                                "name": "circuit_parse",
                                "seconds": 0.0000013,
                                "iterations": null
                            }
                        ]
                    },
                    {
                        "id": "contract-row",
                        "milestone": "M4",
                        "threshold_class": "report-only",
                        "runner": "contract-only",
                        "upstream_source": "src/stim/circuit/circuit.test.cc",
                        "phase": "analysis",
                        "measurement": "canonical-print",
                        "status": "contract-only",
                        "command": {
                            "program": "",
                            "args": [],
                            "stdin_path": ""
                        },
                        "measurements": []
                    }
                ]
            }"#,
        )
        .expect("baseline report");

        assert_eq!(
            summarize_baseline_row(&report, "measured-row"),
            BaselineSummary::Present("circuit_parse=0.000001300s".to_string())
        );
        assert_eq!(
            summarize_baseline_row(&report, "contract-row"),
            BaselineSummary::Present("contract-only".to_string())
        );
        assert_eq!(
            summarize_baseline_row(&report, "missing-row"),
            BaselineSummary::Missing
        );
    }
}
