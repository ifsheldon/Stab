use std::ffi::OsString;
use std::hint::black_box;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::error::BenchError;
use crate::manifest::BenchmarkRow;
use crate::report::Measurement;
use crate::root::RepoRoot;

use super::measure_stab;

const CONVERT_RECORDS: f64 = 4096.0;
const CONVERT_WIDE_BITS: f64 = 4096.0 * 2048.0;
const CONVERT_01_128: &[u8] = include_bytes!("../../../../benchmarks/fixtures/convert_01_128.01");
const CONVERT_DL_72: &[u8] = include_bytes!("../../../../benchmarks/fixtures/convert_dl_72.01");
const CONVERT_B8_128: &[u8] = include_bytes!("../../../../benchmarks/fixtures/convert_b8_128.b8");
const CONVERT_B8_2048: &[u8] = include_bytes!("../../../../benchmarks/fixtures/convert_b8_2048.b8");
const CONVERT_B8_DL_72: &[u8] =
    include_bytes!("../../../../benchmarks/fixtures/convert_b8_dl_72.b8");
const CONVERT_PTB64_128: &[u8] =
    include_bytes!("../../../../benchmarks/fixtures/convert_ptb64_128.ptb64");
const CONVERT_DETS_DL_72: &[u8] =
    include_bytes!("../../../../benchmarks/fixtures/convert_dets_dl_72.dets");
const M2D_BASIC_MEASUREMENTS: &[u8] =
    include_bytes!("../../../../oracle/fixtures/inputs/m2d_basic_measurements.01");

pub(super) fn run_convert_compare_row(
    root: &RepoRoot,
    row: &BenchmarkRow,
) -> Result<Option<Vec<Measurement>>, BenchError> {
    let Some(workload) = ConvertWorkload::from_row_id(&row.id) else {
        return Ok(None);
    };
    Ok(Some(vec![measure_stab(
        workload.measurement_name(),
        || run_convert_workload(root, row, workload),
    )?]))
}

pub(super) fn measurement_work(row_id: &str, name: &str) -> Option<(f64, &'static str)> {
    let workload = ConvertWorkload::from_row_id(row_id)?;
    if name != workload.measurement_name() {
        return None;
    }
    match workload {
        ConvertWorkload::ZeroOneToB8
        | ConvertWorkload::B8ToZeroOne
        | ConvertWorkload::Ptb64ToZeroOne
        | ConvertWorkload::ZeroOneToPtb64 => Some((workload.input().len() as f64, "bytes/s")),
        ConvertWorkload::B8ToB8Wide => Some((CONVERT_WIDE_BITS, "bits/s")),
        ConvertWorkload::DetsToB8
        | ConvertWorkload::B8ToDets
        | ConvertWorkload::CircuitDlObsOut
        | ConvertWorkload::DemDetsToZeroOne => Some((CONVERT_RECORDS, "records/s")),
        ConvertWorkload::M9MeasurementsToDets => Some((2.0, "records/s")),
    }
}

pub(super) fn compare_note(row_id: &str) -> Option<&'static str> {
    ConvertWorkload::from_row_id(row_id).map(|workload| workload.compare_note())
}

fn run_convert_workload(
    root: &RepoRoot,
    row: &BenchmarkRow,
    workload: ConvertWorkload,
) -> Result<(), BenchError> {
    let mut stdout = CountingWriter::default();
    let mut stderr = Vec::new();
    let side_output = workload.side_output(root);
    if let Some(path) = side_output.as_ref() {
        create_parent_dir(row, path)?;
    }
    let status = stab_cli::run_from(
        workload.args(root),
        workload.input(),
        &mut stdout,
        &mut stderr,
    );
    if status != 0 {
        return Err(BenchError::StabRunner {
            row_id: row.id.clone(),
            message: format!(
                "stab-cli convert failed with status {status}: {}",
                String::from_utf8_lossy(&stderr)
            ),
        });
    }
    if let Some(path) = side_output.as_ref() {
        let side_bytes = std::fs::read(path).map_err(|source| BenchError::StabRunner {
            row_id: row.id.clone(),
            message: format!(
                "failed to read convert side output {}: {source}",
                path.display()
            ),
        })?;
        black_box((stdout.len(), side_bytes.len()));
    } else {
        black_box(stdout.len());
    }
    Ok(())
}

#[derive(Default)]
struct CountingWriter {
    bytes: usize,
}

impl CountingWriter {
    fn len(&self) -> usize {
        self.bytes
    }
}

impl Write for CountingWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.bytes = self
            .bytes
            .checked_add(buf.len())
            .ok_or_else(|| io::Error::other("convert benchmark output byte count overflowed"))?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
enum ConvertWorkload {
    ZeroOneToB8,
    B8ToZeroOne,
    B8ToB8Wide,
    DetsToB8,
    B8ToDets,
    Ptb64ToZeroOne,
    ZeroOneToPtb64,
    CircuitDlObsOut,
    DemDetsToZeroOne,
    M9MeasurementsToDets,
}

impl ConvertWorkload {
    fn from_row_id(row_id: &str) -> Option<Self> {
        match row_id {
            "m7-convert-01-to-b8" => Some(Self::ZeroOneToB8),
            "m7-convert-b8-to-01" => Some(Self::B8ToZeroOne),
            "m7-convert-b8-to-b8-wide" => Some(Self::B8ToB8Wide),
            "m7-convert-dets-to-b8" => Some(Self::DetsToB8),
            "m7-convert-b8-to-dets" => Some(Self::B8ToDets),
            "m7-convert-ptb64-to-01" => Some(Self::Ptb64ToZeroOne),
            "m7-convert-01-to-ptb64" => Some(Self::ZeroOneToPtb64),
            "m7-convert-circuit-dl-obs-out" => Some(Self::CircuitDlObsOut),
            "m7-convert-dem-dets-to-01" => Some(Self::DemDetsToZeroOne),
            "m9-convert-measurements-dets" => Some(Self::M9MeasurementsToDets),
            _ => None,
        }
    }

    fn measurement_name(self) -> &'static str {
        match self {
            Self::ZeroOneToB8 => "stab_convert_01_to_b8_128",
            Self::B8ToZeroOne => "stab_convert_b8_to_01_128",
            Self::B8ToB8Wide => "stab_convert_b8_to_b8_2048",
            Self::DetsToB8 => "stab_convert_dets_to_b8_dl72",
            Self::B8ToDets => "stab_convert_b8_to_dets_dl72",
            Self::Ptb64ToZeroOne => "stab_convert_ptb64_to_01_128",
            Self::ZeroOneToPtb64 => "stab_convert_01_to_ptb64_128",
            Self::CircuitDlObsOut => "stab_convert_circuit_dl_obs_out",
            Self::DemDetsToZeroOne => "stab_convert_dem_dets_to_01",
            Self::M9MeasurementsToDets => "stab_convert_measurements_to_dets",
        }
    }

    fn input(self) -> &'static [u8] {
        match self {
            Self::ZeroOneToB8 | Self::ZeroOneToPtb64 => CONVERT_01_128,
            Self::B8ToZeroOne => CONVERT_B8_128,
            Self::B8ToB8Wide => CONVERT_B8_2048,
            Self::DetsToB8 | Self::DemDetsToZeroOne => CONVERT_DETS_DL_72,
            Self::B8ToDets => CONVERT_B8_DL_72,
            Self::Ptb64ToZeroOne => CONVERT_PTB64_128,
            Self::CircuitDlObsOut => CONVERT_DL_72,
            Self::M9MeasurementsToDets => M2D_BASIC_MEASUREMENTS,
        }
    }

    fn args(self, root: &RepoRoot) -> Vec<OsString> {
        let mut args = vec![OsString::from("stab"), OsString::from("convert")];
        match self {
            Self::ZeroOneToB8 => {
                push_flags(&mut args, "01", "b8");
                push_bits_per_shot(&mut args, 128);
            }
            Self::B8ToZeroOne => {
                push_flags(&mut args, "b8", "01");
                push_bits_per_shot(&mut args, 128);
            }
            Self::B8ToB8Wide => {
                push_flags(&mut args, "b8", "b8");
                push_bits_per_shot(&mut args, 2048);
            }
            Self::DetsToB8 => {
                push_flags(&mut args, "dets", "b8");
                push_detector_counts(&mut args);
            }
            Self::B8ToDets => {
                push_flags(&mut args, "b8", "dets");
                push_detector_counts(&mut args);
            }
            Self::Ptb64ToZeroOne => {
                push_flags(&mut args, "ptb64", "01");
                push_bits_per_shot(&mut args, 128);
            }
            Self::ZeroOneToPtb64 => {
                push_flags(&mut args, "01", "ptb64");
                push_bits_per_shot(&mut args, 128);
            }
            Self::CircuitDlObsOut => {
                push_flags(&mut args, "01", "dets");
                args.extend([
                    OsString::from("--circuit"),
                    repo_path(root, "benchmarks/fixtures/convert_circuit_dl.stim").into_os_string(),
                    OsString::from("--types"),
                    OsString::from("DL"),
                    OsString::from("--obs_out"),
                    obs_out_path(root).into_os_string(),
                    OsString::from("--obs_out_format"),
                    OsString::from("b8"),
                ]);
            }
            Self::DemDetsToZeroOne => {
                push_flags(&mut args, "dets", "01");
                args.extend([
                    OsString::from("--dem"),
                    repo_path(root, "benchmarks/fixtures/convert_dem_dl.dem").into_os_string(),
                ]);
            }
            Self::M9MeasurementsToDets => {
                push_flags(&mut args, "01", "dets");
                args.extend([
                    OsString::from("--circuit"),
                    repo_path(root, "oracle/fixtures/inputs/m2d_basic.stim").into_os_string(),
                    OsString::from("--types"),
                    OsString::from("M"),
                ]);
            }
        }
        args
    }

    fn side_output(self, root: &RepoRoot) -> Option<PathBuf> {
        matches!(self, Self::CircuitDlObsOut).then(|| obs_out_path(root))
    }

    fn compare_note(self) -> &'static str {
        match self {
            Self::Ptb64ToZeroOne => {
                "cli-baseline: Stab measures in-process convert through the public CLI path against pinned Stim convert on the same ptb64-compatible fixture"
            }
            Self::ZeroOneToPtb64 => {
                "report-only: Stab measures in-process convert 01 to ptb64 output because pinned Stim v1.16.0 rejects SAMPLE_FORMAT_PTB64 output for this convert shape"
            }
            Self::M9MeasurementsToDets => {
                "cli-baseline: Stab measures in-process convert --circuit --types=M against pinned Stim convert on the same measurement fixture"
            }
            Self::CircuitDlObsOut => {
                "cli-baseline: Stab measures in-process convert --circuit --types=DL with observable side output against pinned Stim convert on the same fixture"
            }
            Self::DemDetsToZeroOne => {
                "cli-baseline: Stab measures in-process convert --dem layout handling against pinned Stim convert on the same dets fixture"
            }
            _ => {
                "cli-baseline: Stab measures in-process convert through the public CLI path against pinned Stim convert on the same result fixture"
            }
        }
    }
}

fn push_flags(args: &mut Vec<OsString>, in_format: &'static str, out_format: &'static str) {
    args.extend([
        OsString::from("--in_format"),
        OsString::from(in_format),
        OsString::from("--out_format"),
        OsString::from(out_format),
    ]);
}

fn push_bits_per_shot(args: &mut Vec<OsString>, bits_per_shot: usize) {
    args.extend([
        OsString::from("--bits_per_shot"),
        OsString::from(bits_per_shot.to_string()),
    ]);
}

fn push_detector_counts(args: &mut Vec<OsString>) {
    args.extend([
        OsString::from("--num_detectors"),
        OsString::from("64"),
        OsString::from("--num_observables"),
        OsString::from("8"),
    ]);
}

fn create_parent_dir(row: &BenchmarkRow, path: &Path) -> Result<(), BenchError> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    std::fs::create_dir_all(parent).map_err(|source| BenchError::StabRunner {
        row_id: row.id.clone(),
        message: format!(
            "failed to create convert side-output directory {}: {source}",
            parent.display()
        ),
    })
}

fn obs_out_path(root: &RepoRoot) -> PathBuf {
    repo_path(
        root,
        "target/benchmarks/cli-scratch/m7-convert-circuit-dl-obs-out.obs.b8",
    )
}

fn repo_path(root: &RepoRoot, relative: &str) -> PathBuf {
    root.path.join(relative)
}
