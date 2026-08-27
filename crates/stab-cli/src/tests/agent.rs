use std::ffi::OsString;
use std::fs::File;

use clap::CommandFactory;
use serde_json::Value;
use stab_model::Gate;
use stab_records::RecordFormat;
use tempfile::tempdir;

use crate::{Cli, run_from};

fn run_cli<I, S>(args: I, input: &[u8]) -> (i32, Vec<u8>, Vec<u8>)
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
{
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(args, input, &mut stdout, &mut stderr);
    (status, stdout, stderr)
}

fn json_stdout(stdout: &[u8]) -> Value {
    serde_json::from_slice(stdout).expect("stdout is one JSON value")
}

fn pointer<'a>(value: &'a Value, pointer: &str) -> &'a Value {
    value.pointer(pointer).expect("JSON pointer exists")
}

fn assert_exact_keys(object: &Value, expected: &[&str]) {
    let actual = object
        .as_object()
        .expect("schema node is an object")
        .keys()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    let expected = expected
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(actual, expected);
}

fn assert_capability_schema(report: &Value) {
    assert_exact_keys(
        report,
        &[
            "schema_version",
            "stab_version",
            "stim_compatibility_version",
            "commands",
            "dialects",
            "gates",
            "codecs",
            "compilers",
        ],
    );
    for command in pointer(report, "/commands")
        .as_array()
        .expect("commands are an array")
    {
        assert_exact_keys(command, &["name", "summary"]);
    }
    for dialect in pointer(report, "/dialects")
        .as_array()
        .expect("dialects are an array")
    {
        assert_exact_keys(dialect, &["name", "default_parse_limits"]);
        assert_exact_keys(
            pointer(dialect, "/default_parse_limits"),
            &[
                "source_bytes",
                "source_lines",
                "represented_instructions",
                "represented_targets",
                "repeat_nesting",
            ],
        );
    }
    for gate in pointer(report, "/gates")
        .as_array()
        .expect("gates are an array")
    {
        assert_exact_keys(
            gate,
            &[
                "canonical_name",
                "aliases",
                "category",
                "argument_rule",
                "target_rule",
                "target_grouping",
                "support_scope",
            ],
        );
        assert_exact_keys(pointer(gate, "/argument_rule"), &["kind", "count"]);
    }
    for codec in pointer(report, "/codecs")
        .as_array()
        .expect("codecs are an array")
    {
        assert_exact_keys(
            codec,
            &[
                "name",
                "encoding",
                "can_decode",
                "can_encode",
                "requires_typed_layout",
                "records_per_group",
            ],
        );
    }
    for compiler in pointer(report, "/compilers")
        .as_array()
        .expect("compilers are an array")
    {
        assert_exact_keys(
            compiler,
            &[
                "operation",
                "input_dialect",
                "compiler_schema_version",
                "request_fingerprint_schema_version",
                "configurable_limits",
            ],
        );
    }
}

fn assert_estimate_schema(estimate: &Value) {
    assert_exact_keys(
        estimate,
        &[
            "input_bytes",
            "input_items",
            "expanded_operations",
            "folded_traversal",
            "scratch_bytes",
            "resident_bytes",
            "output_bytes",
            "work_units",
        ],
    );
    for value in estimate
        .as_object()
        .expect("estimate is an object")
        .values()
    {
        assert_exact_keys(value, &["class", "value"]);
    }
}

fn assert_inspect_schema(report: &Value, model_keys: &[&str]) {
    assert_exact_keys(
        report,
        &[
            "schema_version",
            "executes",
            "source",
            "parse_estimate",
            "model",
        ],
    );
    assert_exact_keys(pointer(report, "/source"), &["bytes", "physical_lines"]);
    assert_estimate_schema(pointer(report, "/parse_estimate"));
    assert_exact_keys(pointer(report, "/model"), model_keys);
    assert_exact_keys(
        pointer(report, "/model/fingerprint"),
        &["schema_version", "algorithm", "dialect", "digest"],
    );
}

fn assert_sample_plan_schema(report: &Value) {
    assert_exact_keys(
        report,
        &[
            "schema_version",
            "operation",
            "executes",
            "source",
            "model",
            "compilation",
            "run",
            "estimates",
        ],
    );
    assert_exact_keys(pointer(report, "/source"), &["bytes", "physical_lines"]);
    assert_exact_keys(
        pointer(report, "/model"),
        &["schema_version", "algorithm", "dialect", "digest"],
    );
    assert_exact_keys(
        pointer(report, "/compilation"),
        &[
            "request_fingerprint",
            "plan_fingerprint",
            "compiler_schema_version",
            "normalized_options",
            "configurable_limits",
            "validated",
        ],
    );
    assert_exact_keys(
        pointer(report, "/compilation/request_fingerprint"),
        &["schema_version", "algorithm", "digest"],
    );
    assert_exact_keys(
        pointer(report, "/compilation/plan_fingerprint"),
        &[
            "schema_version",
            "algorithm",
            "backend",
            "executable_contract_schema_version",
            "executable_contract_digest",
            "digest",
        ],
    );
    assert_exact_keys(
        pointer(report, "/run"),
        &[
            "shots",
            "random_policy",
            "seed",
            "reference_mode",
            "output_format",
            "skip_loop_folding_requested",
            "skip_loop_folding_effect",
        ],
    );
    assert_estimate_schema(pointer(report, "/estimates"));
}

#[test]
fn capabilities_json_is_generated_from_product_and_clap_descriptors() {
    fn collect_commands(prefix: &str, command: &clap::Command, output: &mut Vec<String>) {
        for subcommand in command.get_subcommands() {
            let name = if prefix.is_empty() {
                subcommand.get_name().to_string()
            } else {
                format!("{prefix} {}", subcommand.get_name())
            };
            output.push(name.clone());
            collect_commands(&name, subcommand, output);
        }
    }

    let (status, stdout, stderr) = run_cli(["stab", "capabilities", "--format=json"], b"ignored");
    assert_eq!(status, 0);
    assert_eq!(stderr, b"");

    let report = json_stdout(&stdout);
    assert_capability_schema(&report);
    assert_eq!(pointer(&report, "/schema_version"), 5);
    assert_eq!(pointer(&report, "/stim_compatibility_version"), "1.16.0");
    for limits in pointer(&report, "/dialects")
        .as_array()
        .expect("dialects are an array")
        .iter()
        .map(|dialect| pointer(dialect, "/default_parse_limits"))
    {
        assert_eq!(pointer(limits, "/source_bytes"), 64 * 1024 * 1024);
        assert_eq!(pointer(limits, "/source_lines"), 1_000_000);
        assert_eq!(pointer(limits, "/represented_instructions"), 1_000_000);
        assert_eq!(pointer(limits, "/represented_targets"), 32_000_000);
        assert_eq!(pointer(limits, "/repeat_nesting"), 256);
    }

    let command_names = pointer(&report, "/commands")
        .as_array()
        .expect("commands are an array")
        .iter()
        .map(|command| {
            pointer(command, "/name")
                .as_str()
                .expect("command name")
                .to_string()
        })
        .collect::<Vec<_>>();
    let mut expected_commands = Vec::new();
    collect_commands("", &Cli::command(), &mut expected_commands);
    assert_eq!(command_names, expected_commands);
    for expected in ["capabilities", "inspect", "plan", "plan sample", "sample"] {
        assert!(command_names.iter().any(|name| name == expected));
    }

    let gate_names = pointer(&report, "/gates")
        .as_array()
        .expect("gates are an array")
        .iter()
        .map(|gate| {
            pointer(gate, "/canonical_name")
                .as_str()
                .expect("gate name")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        gate_names,
        Gate::all().map(Gate::canonical_name).collect::<Vec<_>>()
    );
    assert!(
        pointer(&report, "/gates/0/support_scope")
            .as_str()
            .is_some_and(|scope| scope == "accepted-circuit-syntax")
    );

    let codec_names = pointer(&report, "/codecs")
        .as_array()
        .expect("codecs are an array")
        .iter()
        .map(|codec| pointer(codec, "/name").as_str().expect("codec name"))
        .collect::<Vec<_>>();
    assert_eq!(
        codec_names,
        RecordFormat::all()
            .map(RecordFormat::as_str)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        pointer(&report, "/compilers"),
        &serde_json::json!([
            {
                "operation": "sample",
                "input_dialect": "stim-circuit",
                "compiler_schema_version": 1,
                "request_fingerprint_schema_version": 1,
                "configurable_limits": false
            },
            {
                "operation": "m2d",
                "input_dialect": "stim-circuit",
                "compiler_schema_version": 1,
                "request_fingerprint_schema_version": null,
                "configurable_limits": true
            },
            {
                "operation": "detect",
                "input_dialect": "stim-circuit",
                "compiler_schema_version": 1,
                "request_fingerprint_schema_version": null,
                "configurable_limits": true
            },
            {
                "operation": "sample_dem",
                "input_dialect": "detector-error-model",
                "compiler_schema_version": 1,
                "request_fingerprint_schema_version": null,
                "configurable_limits": false
            }
        ])
    );
}

#[test]
fn capabilities_human_output_is_concise_and_structural() {
    let (status, stdout, stderr) = run_cli(["stab", "capabilities"], b"");
    assert_eq!(status, 0);
    assert_eq!(stderr, b"");
    let output = std::str::from_utf8(&stdout).expect("human output");
    for expected in [
        "Stim 1.16.0",
        "commands:",
        "gates:",
        "result codecs: 6",
        "compilers: 4",
    ] {
        assert!(output.contains(expected), "{output}");
    }
}

#[test]
fn inspect_json_reports_exact_circuit_and_dem_structure_without_execution() {
    let circuit = b"M 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n";
    let (status, stdout, stderr) =
        run_cli(["stab", "inspect", "--type=stim", "--format=json"], circuit);
    assert_eq!(status, 0);
    assert_eq!(stderr, b"");
    let report = json_stdout(&stdout);
    assert_eq!(pointer(&report, "/schema_version"), 2);
    assert_eq!(pointer(&report, "/executes"), false);
    assert_eq!(pointer(&report, "/source/bytes"), circuit.len());
    assert_eq!(pointer(&report, "/source/physical_lines"), 3);
    assert_eq!(pointer(&report, "/model/dialect"), "stim-circuit");
    assert_eq!(pointer(&report, "/model/qubits"), 1);
    assert_eq!(pointer(&report, "/model/measurements"), 1);
    assert_eq!(pointer(&report, "/model/detectors"), 1);
    assert_eq!(pointer(&report, "/model/observables"), 1);
    assert_eq!(pointer(&report, "/model/sweep_bits"), 0);
    assert_eq!(
        pointer(&report, "/parse_estimate/input_bytes/class"),
        "exact"
    );
    assert_eq!(
        pointer(&report, "/parse_estimate/expanded_operations/class"),
        "unknown"
    );
    assert_inspect_schema(
        &report,
        &[
            "dialect",
            "fingerprint",
            "top_level_items",
            "qubits",
            "measurements",
            "detectors",
            "observables",
            "sweep_bits",
        ],
    );

    let dem = b"error(0.125) D3 L2\n";
    let (status, stdout, stderr) = run_cli(["stab", "inspect", "--type=dem", "--format=json"], dem);
    assert_eq!(status, 0);
    assert_eq!(stderr, b"");
    let report = json_stdout(&stdout);
    assert_eq!(pointer(&report, "/executes"), false);
    assert_eq!(pointer(&report, "/model/dialect"), "detector-error-model");
    assert_eq!(pointer(&report, "/model/detectors"), 4);
    assert_eq!(pointer(&report, "/model/observables"), 3);
    assert!(report.pointer("/model/qubits").is_none());
    assert_inspect_schema(
        &report,
        &[
            "dialect",
            "fingerprint",
            "top_level_items",
            "detectors",
            "observables",
        ],
    );

    let compile_rejected = b"CX sweep[0] 0\nM 0\n";
    let (status, stdout, stderr) = run_cli(
        ["stab", "inspect", "--type=stim", "--format=json"],
        compile_rejected,
    );
    assert_eq!(status, 0);
    assert_eq!(stderr, b"");
    assert_eq!(
        pointer(&json_stdout(&stdout), "/model/sweep_bits"),
        1,
        "inspection must stop before sampler compilation"
    );
}

#[test]
fn inspect_infers_file_type_and_explicit_type_overrides_unknown_extension() {
    let temp = tempdir().expect("temp dir");
    let circuit_path = temp.path().join("model.stim");
    let dem_path = temp.path().join("model.dem");
    let unknown_path = temp.path().join("model.data");
    std::fs::write(&circuit_path, "M 0\n").expect("write circuit");
    std::fs::write(&dem_path, "error(0.25) D0\n").expect("write DEM");
    std::fs::write(&unknown_path, "M 0\n").expect("write unknown extension");

    for (path, dialect) in [
        (&circuit_path, "stim-circuit"),
        (&dem_path, "detector-error-model"),
    ] {
        let args = vec![
            OsString::from("stab"),
            OsString::from("inspect"),
            path.as_os_str().to_owned(),
            OsString::from("--format=json"),
        ];
        let (status, stdout, stderr) = run_cli(args, b"ignored");
        assert_eq!(status, 0, "{path:?}");
        assert_eq!(stderr, b"", "{path:?}");
        assert_eq!(pointer(&json_stdout(&stdout), "/model/dialect"), dialect);
    }

    let args = vec![
        OsString::from("stab"),
        OsString::from("inspect"),
        unknown_path.as_os_str().to_owned(),
        OsString::from("--error-format=json"),
    ];
    let (status, stdout, stderr) = run_cli(args, b"ignored");
    assert_eq!(status, 1);
    assert_eq!(stdout, b"");
    let diagnostic = json_stdout(&stderr);
    assert_eq!(pointer(&diagnostic, "/code"), "unknown-inspect-model-type");
    assert_eq!(
        pointer(&diagnostic, "/context/path"),
        unknown_path.to_string_lossy().as_ref()
    );

    let args = vec![
        OsString::from("stab"),
        OsString::from("inspect"),
        unknown_path.as_os_str().to_owned(),
        OsString::from("--type=stim"),
        OsString::from("--format=json"),
    ];
    let (status, stdout, stderr) = run_cli(args, b"ignored");
    assert_eq!(status, 0);
    assert_eq!(stderr, b"");
    assert_eq!(
        pointer(&json_stdout(&stdout), "/model/dialect"),
        "stim-circuit"
    );
}

#[test]
fn inspect_requires_an_explicit_type_when_inference_is_impossible() {
    let (status, stdout, stderr) = run_cli(
        ["stab", "inspect", "--format=json", "--error-format=json"],
        b"M 0\n",
    );
    assert_eq!(status, 1);
    assert_eq!(stdout, b"");
    let diagnostic = json_stdout(&stderr);
    assert_eq!(pointer(&diagnostic, "/code"), "missing-inspect-model-type");
    assert_eq!(pointer(&diagnostic, "/context/input"), "stdin");
}

#[test]
fn agent_commands_route_parse_and_compile_failures_through_json_diagnostics() {
    let (status, stdout, stderr) = run_cli(
        ["stab", "inspect", "--type=stim", "--error-format=json"],
        b"\xff",
    );
    assert_eq!(status, 1);
    assert_eq!(stdout, b"");
    let diagnostic = json_stdout(&stderr);
    assert_eq!(pointer(&diagnostic, "/code"), "invalid-utf8-input");
    assert_eq!(pointer(&diagnostic, "/span/byte_start"), 0);
    assert_eq!(pointer(&diagnostic, "/span/byte_length"), 1);
    assert_eq!(pointer(&diagnostic, "/context/dialect"), "stim-circuit");

    let (status, stdout, stderr) = run_cli(
        ["stab", "plan", "sample", "--error-format=json"],
        b"CX sweep[0] 0\nM 0\n",
    );
    assert_eq!(status, 1);
    assert_eq!(stdout, b"");
    assert_eq!(
        pointer(&json_stdout(&stderr), "/code"),
        "invalid-sampler-compilation"
    );
}

#[test]
fn inspect_rejects_oversized_files_before_reading_them() {
    let temp = tempdir().expect("temp dir");
    let path = temp.path().join("large.stim");
    let file = File::create(&path).expect("create sparse input");
    file.set_len(crate::MAX_CIRCUIT_INPUT_BYTES + 1)
        .expect("extend sparse input");

    let args = vec![
        OsString::from("stab"),
        OsString::from("inspect"),
        path.as_os_str().to_owned(),
        OsString::from("--error-format=json"),
    ];
    let (status, stdout, stderr) = run_cli(args, b"ignored");
    assert_eq!(status, 1);
    assert_eq!(stdout, b"");
    let diagnostic = json_stdout(&stderr);
    assert_eq!(pointer(&diagnostic, "/code"), "input-too-large");
    assert_eq!(
        pointer(&diagnostic, "/context/limit_bytes"),
        crate::MAX_CIRCUIT_INPUT_BYTES
    );
}

#[test]
fn plan_sample_separates_compilation_identity_from_run_configuration() {
    let circuit = b"H 0\nM 0\n";
    let (_, first_stdout, first_stderr) = run_cli(
        [
            "stab",
            "plan",
            "sample",
            "--shots=3",
            "--seed=5",
            "--out_format=b8",
            "--format=json",
        ],
        circuit,
    );
    assert_eq!(first_stderr, b"");
    let first = json_stdout(&first_stdout);

    let (status, second_stdout, second_stderr) = run_cli(
        [
            "stab",
            "plan",
            "sample",
            "--shots=7",
            "--out_format=hits",
            "--skip_reference_sample",
            "--skip_loop_folding",
            "--format=json",
        ],
        circuit,
    );
    assert_eq!(status, 0);
    assert_eq!(second_stderr, b"");
    let second = json_stdout(&second_stdout);
    assert_sample_plan_schema(&first);
    assert_sample_plan_schema(&second);
    assert_eq!(pointer(&first, "/schema_version"), 3);
    assert_eq!(pointer(&second, "/schema_version"), 3);

    assert_eq!(
        pointer(&first, "/compilation/request_fingerprint/digest"),
        pointer(&second, "/compilation/request_fingerprint/digest")
    );
    assert_eq!(
        pointer(&first, "/compilation/plan_fingerprint/digest"),
        pointer(&second, "/compilation/plan_fingerprint/digest")
    );
    assert_eq!(
        pointer(&first, "/model/digest"),
        pointer(&second, "/model/digest")
    );
    assert_eq!(
        pointer(&first, "/compilation/normalized_options"),
        &serde_json::json!([])
    );
    assert_eq!(
        pointer(&first, "/compilation/configurable_limits"),
        &serde_json::json!([])
    );
    assert_eq!(
        pointer(&first, "/compilation/plan_fingerprint/backend"),
        "scalar"
    );
    assert_eq!(
        pointer(&first, "/compilation/plan_fingerprint/schema_version"),
        1
    );
    assert_eq!(
        pointer(&second, "/run/skip_loop_folding_effect"),
        "accepted-no-op"
    );
    assert_eq!(pointer(&second, "/executes"), false);
    assert_eq!(pointer(&second, "/compilation/validated"), true);
}

#[test]
fn plan_sample_reports_scalar_identity_without_backend_selection() {
    let (status, stdout, stderr) =
        run_cli(["stab", "plan", "sample", "--format=json"], b"H 0\nM 0\n");
    assert_eq!(status, 0);
    assert_eq!(stderr, b"");
    let report = json_stdout(&stdout);
    assert_sample_plan_schema(&report);
    assert_eq!(
        pointer(&report, "/compilation/plan_fingerprint/backend"),
        "scalar"
    );

    let (status, help_stdout, help_stderr) = run_cli(["stab", "plan", "sample", "--help"], b"");
    assert_eq!(status, 0);
    assert_eq!(help_stderr, b"");
    assert!(!String::from_utf8_lossy(&help_stdout).contains("--backend"));

    let (status, stdout, stderr) =
        run_cli(["stab", "plan", "sample", "--backend=scalar"], b"M 0\n");
    assert_eq!(status, 1);
    assert_eq!(stdout, b"");
    assert!(String::from_utf8_lossy(&stderr).contains("--backend"));
}

#[test]
fn plan_sample_is_deterministic_for_noisy_entropy_configuration() {
    let circuit = b"X_ERROR(0.5) 0\nM 0\n";
    let args = ["stab", "plan", "sample", "--shots=100", "--format=json"];
    let first = run_cli(args, circuit);
    let second = run_cli(args, circuit);

    assert_eq!(first.0, 0);
    assert_eq!(second.0, 0);
    assert_eq!(first.1, second.1);
    assert_eq!(first.2, b"");
    assert_eq!(second.2, b"");
    let report = json_stdout(&first.1);
    assert_sample_plan_schema(&report);
    assert_eq!(pointer(&report, "/run/random_policy"), "entropy");
    assert_eq!(pointer(&report, "/run/seed"), &Value::Null);
    assert_eq!(pointer(&report, "/executes"), false);
    assert_eq!(pointer(&report, "/estimates/output_bytes/value"), 200);
}

#[test]
fn plan_sample_matches_one_shot_herald_filtering_output_width() {
    let circuit = b"HERALDED_ERASE(0) 0\nM 0\n";
    let (status, plan_stdout, plan_stderr) =
        run_cli(["stab", "plan", "sample", "--format=json"], circuit);
    assert_eq!(status, 0);
    assert_eq!(plan_stderr, b"");
    let plan = json_stdout(&plan_stdout);

    let (status, sample_stdout, sample_stderr) =
        run_cli(["stab", "sample", "--shots=1", "--seed=7"], circuit);
    assert_eq!(status, 0);
    assert_eq!(sample_stderr, b"");
    assert_eq!(
        pointer(&plan, "/estimates/output_bytes/value"),
        sample_stdout.len()
    );
    assert_eq!(sample_stdout.len(), 2);
}

#[test]
fn plan_sample_counts_heralded_repeat_width_without_expanding_the_repeat() {
    let circuit = b"REPEAT 10000000000 {\nHERALDED_ERASE(0) 0\nM 0\n}\n";
    let (status, stdout, stderr) = run_cli(["stab", "plan", "sample", "--format=json"], circuit);

    assert_eq!(status, 0);
    assert_eq!(stderr, b"");
    let report = json_stdout(&stdout);
    assert_eq!(
        pointer(&report, "/estimates/output_bytes/value"),
        10_000_000_001u64
    );
}

#[test]
fn plan_sample_rejects_invalid_output_groups_and_uncompilable_circuits() {
    let (status, stdout, stderr) = run_cli(
        ["stab", "plan", "sample", "--shots=65", "--out_format=ptb64"],
        b"M 0\n",
    );
    assert_eq!(status, 1);
    assert_eq!(stdout, b"");
    assert!(
        std::str::from_utf8(&stderr)
            .expect("stderr")
            .contains("multiple of 64")
    );

    let (status, stdout, stderr) = run_cli(["stab", "plan", "sample"], b"CX sweep[0] 0\nM 0\n");
    assert_eq!(status, 1);
    assert_eq!(stdout, b"");
    assert!(
        std::str::from_utf8(&stderr)
            .expect("stderr")
            .contains("cannot compile circuit sampler")
    );
}

#[test]
fn agent_commands_have_structural_human_output_and_clap_rejects_unknown_plans() {
    for (args, input, expected) in [
        (
            &["stab", "inspect", "--type=stim"][..],
            &b"M 0\n"[..],
            "executes: no",
        ),
        (
            &["stab", "plan", "sample"][..],
            &b"M 0\n"[..],
            "validated: yes",
        ),
    ] {
        let (status, stdout, stderr) = run_cli(args.iter().copied(), input);
        assert_eq!(status, 0, "{args:?}");
        assert_eq!(stderr, b"", "{args:?}");
        assert!(
            std::str::from_utf8(&stdout)
                .expect("stdout")
                .contains(expected),
            "{args:?}"
        );
    }

    let (status, stdout, stderr) = run_cli(["stab", "plan", "unknown"], b"");
    assert_eq!(status, 1);
    assert_eq!(stdout, b"");
    assert!(!stderr.is_empty());
}
