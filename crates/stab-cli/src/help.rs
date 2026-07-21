use std::io::Write;

use clap::Args;
use stab_core::{Gate, GateCategory};

use crate::{CliError, RecordFormatArg, write_output};

#[derive(Debug, Args)]
pub(crate) struct HelpArgs {
    /// Optional help topic: commands, formats, gates, a command, a result format, or a gate.
    topic: Option<String>,
}

const COMMANDS: &[(&str, &str)] = &[
    ("gen", "Generates supported example circuits."),
    ("convert", "Converts result data between supported formats."),
    ("sample", "Samples measurements from a circuit."),
    (
        "detect",
        "Samples detector events and observable flips from a circuit.",
    ),
    ("m2d", "Converts measurements into detector events."),
    (
        "analyze_errors",
        "Converts a circuit into a detector error model.",
    ),
    (
        "sample_dem",
        "Samples detection events from a detector error model.",
    ),
    ("help", "Prints help for commands, formats, and gates."),
];

const FORMATS: &[RecordFormatArg] = &[
    RecordFormatArg::ZeroOne,
    RecordFormatArg::B8,
    RecordFormatArg::R8,
    RecordFormatArg::Hits,
    RecordFormatArg::Dets,
    RecordFormatArg::Ptb64,
];

pub(crate) fn run_help<W>(args: HelpArgs, stdout: &mut W) -> Result<(), CliError>
where
    W: Write,
{
    let text = match args.topic.as_deref() {
        None | Some("commands") => command_list_help(),
        Some("formats") => format_list_help(),
        Some("gates") => gate_list_help(),
        Some(topic) => topic_help(topic)?,
    };
    write_output(None, stdout, text.as_bytes())
}

fn topic_help(topic: &str) -> Result<String, CliError> {
    if let Some(text) = command_help(topic) {
        return Ok(text);
    }
    if let Some(format) = FORMATS
        .iter()
        .copied()
        .find(|format| format.name() == topic)
    {
        return Ok(format_help(format));
    }
    if let Ok(gate) = Gate::from_name(topic) {
        return Ok(gate_help(gate));
    }
    Err(CliError::UnknownHelpTopic {
        topic: topic.to_string(),
    })
}

fn command_list_help() -> String {
    let mut out = String::new();
    out.push_str(
        "Stab(ilizer): an agent-native simulation toolkit for quantum error correction research,\nwritten in safe Rust. It is now a drop-in replacement for the Stim v1.16 with future extensions coming soon.\ncommand-line tool: the same .stim circuit and .dem detector error model\nformats, and the same 01, b8, r8, hits, dets, and ptb64 result formats.\n\n",
    );
    out.push_str("Available stab commands:\n\n");
    for (name, summary) in COMMANDS {
        out.push_str("    stab ");
        out.push_str(name);
        out.push_str("    # ");
        out.push_str(summary);
        out.push('\n');
    }
    out.push_str("\nExample:\n");
    out.push_str("    stab gen --code repetition_code --task memory --distance 3 --rounds 3 --out circuit.stim\n");
    out.push_str("    stab sample --shots 100 --in circuit.stim --out shots.01\n");
    out.push_str("\nUse `stab help [topic]` for help on commands, formats, and gates.\n");
    out.push_str("Useful topics: commands, formats, gates, convert, sample, 01, ptb64, H.\n");
    out.push_str("\nDocs and sources: ");
    out.push_str(env!("CARGO_PKG_REPOSITORY"));
    out.push('\n');
    out
}

fn format_list_help() -> String {
    let mut out = String::new();
    out.push_str("Supported result formats:\n\n");
    for format in FORMATS {
        out.push_str("    ");
        out.push_str(format.name());
        out.push_str("    # ");
        out.push_str(format_summary(*format));
        out.push('\n');
    }
    out
}

fn gate_list_help() -> String {
    let mut out = String::new();
    out.push_str("Supported circuit gates:\n\n");
    for gate in Gate::all() {
        out.push_str(gate.canonical_name());
        out.push('\n');
    }
    out
}

fn command_help(command: &str) -> Option<String> {
    let text = match command {
        "gen" => {
            "stab gen\n\nGenerates supported repetition, surface, and color-code circuits.\n\nKey flags: --code, --task, --distance, --rounds, --out, and supported noise probabilities.\n"
        }
        "convert" => {
            "stab convert\n\nConverts result data between 01, b8, r8, hits, dets, and ptb64 formats.\n\nLayout flags: --num_measurements, --num_detectors, --num_observables, --bits_per_shot, --circuit, --dem, and --types.\n\nI/O flags: --in_format, --out_format, --in, --out, --obs_out, and --obs_out_format.\n\nThe Stab extension `--in_format=stim --out_format=stim` canonicalizes .stim circuit text.\n"
        }
        "sample" => {
            "stab sample\n\nSamples measurements from a circuit.\n\nKey flags: --shots, --in, --out, --out_format, --seed, --skip_reference_sample, --frame0, and --skip_loop_folding.\n"
        }
        "detect" => {
            "stab detect\n\nSamples detector events and observable flips from a circuit.\n\nKey flags: --shots, --in, --out, --out_format, --obs_out, --obs_out_format, --append_observables, --prepend_observables, and --seed.\n"
        }
        "m2d" => {
            "stab m2d\n\nConverts measurement records into detector-event records using a circuit.\n\nKey flags: --circuit, --in_format, --out_format, --in, --out, --sweep, --sweep_format, --obs_out, --obs_out_format, --append_observables, --skip_reference_sample, and --ran_without_feedback.\n"
        }
        "analyze_errors" => {
            "stab analyze_errors\n\nConverts a supported circuit into a detector error model.\n\nKey flags: --in, --out, --decompose_errors, --fold_loops, --allow_gauge_detectors, --approximate_disjoint_errors, --block_decompose_from_introducing_remnant_edges, and --ignore_decomposition_failures.\n"
        }
        "sample_dem" => {
            "stab sample_dem\n\nSamples detection events from a detector error model.\n\nKey flags: --shots, --in, --out, --out_format, --obs_out, --obs_out_format, --err_out, --err_out_format, --replay_err_in, --replay_err_in_format, and --seed.\n"
        }
        "help" => {
            "stab help [topic]\n\nPrints Stab-native help for implemented commands, result formats, and gate names.\n\nExamples: stab help commands, stab help convert, stab help 01, stab help H.\n"
        }
        _ => return None,
    };
    Some(text.to_string())
}

fn format_help(format: RecordFormatArg) -> String {
    let mut out = String::new();
    out.push_str("Result format ");
    out.push_str(format.name());
    out.push_str("\n\n");
    out.push_str(format_summary(format));
    out.push('\n');
    out.push_str(format_details(format));
    out
}

fn format_summary(format: RecordFormatArg) -> &'static str {
    match format {
        RecordFormatArg::ZeroOne => "text records made from 0 and 1 characters",
        RecordFormatArg::B8 => "little-endian bit-packed bytes",
        RecordFormatArg::R8 => "run-length encoded sparse hits",
        RecordFormatArg::Hits => "comma-separated sparse hit indexes",
        RecordFormatArg::Dets => "sparse text records using shot plus M, D, and L prefixes",
        RecordFormatArg::Ptb64 => "64-shot transposed little-endian packed bits",
        RecordFormatArg::Stim => {
            ".stim circuit text, supported only by the Stab canonical convert extension"
        }
    }
}

fn format_details(format: RecordFormatArg) -> &'static str {
    match format {
        RecordFormatArg::ZeroOne => "Each record is one line. The first character is bit 0.\n",
        RecordFormatArg::B8 => {
            "Each record uses ceil(width / 8) bytes. Bit 0 is the low bit of the first byte.\n"
        }
        RecordFormatArg::R8 => {
            "Each record stores zero-run lengths between set bits and ends when the record width is reached.\n"
        }
        RecordFormatArg::Hits => "Each record is a comma-separated list of set bit indexes.\n",
        RecordFormatArg::Dets => {
            "Each record starts with `shot`, followed by sparse terms such as M0, D1, or L2 when the layout includes those result types.\n"
        }
        RecordFormatArg::Ptb64 => {
            "Records are written in groups of exactly 64 shots. For each result bit, one little-endian u64 stores that bit for the 64 shots.\n"
        }
        RecordFormatArg::Stim => {
            "Use `stab convert --in_format=stim --out_format=stim` to parse and canonicalize circuit text.\n"
        }
    }
}

fn gate_help(gate: Gate) -> String {
    let mut out = String::new();
    out.push_str("Gate ");
    out.push_str(gate.canonical_name());
    out.push_str("\n\n");
    out.push_str("Category: ");
    out.push_str(gate_category_label(gate.category()));
    out.push_str(".\n");
    out.push_str("Canonical name: ");
    out.push_str(gate.canonical_name());
    out.push_str(".\n");
    if gate.category() == GateCategory::HadamardLike {
        out.push_str("This is a Hadamard-like Clifford gate.\n");
    }
    out
}

fn gate_category_label(category: GateCategory) -> &'static str {
    match category {
        GateCategory::Annotation => "annotation",
        GateCategory::ControlFlow => "control flow",
        GateCategory::Collapsing => "collapsing measurement or reset",
        GateCategory::Controlled => "controlled Clifford",
        GateCategory::HadamardLike => "Hadamard-like Clifford",
        GateCategory::Noise => "noise",
        GateCategory::HeraldedNoise => "heralded noise",
        GateCategory::Pauli => "Pauli Clifford",
        GateCategory::Period3 => "period-3 Clifford",
        GateCategory::Period4 => "period-4 Clifford",
        GateCategory::ParityPhasing => "parity phasing",
        GateCategory::PauliProduct => "Pauli product",
        GateCategory::Swap => "swap",
        GateCategory::PairMeasurement => "pair measurement",
    }
}
