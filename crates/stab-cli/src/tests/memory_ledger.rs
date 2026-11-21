//! One `--memory-budget` covers a whole command: each stage admits against the budget less what
//! earlier stages still hold, and dead storage returns its bytes before the next stage.

use stab_engine::{DetectionSamplingCompiler, SamplingCompiler};
use stab_model::{Circuit, MemoryBudget, ParseLimits};
use stab_records::{RecordEncoder, RecordFormat};

use super::support::run;

/// The admission numbers the libraries report for one circuit.
struct Admissions {
    input_bytes: u64,
    model_bytes: u64,
    program_bytes: u64,
}

impl Admissions {
    fn of(text: &str) -> Self {
        let (circuit, model_bytes) =
            Circuit::from_stim_bytes_admitted(text.as_bytes(), ParseLimits::default())
                .expect("parse");
        Self {
            input_bytes: u64::try_from(text.len()).expect("length fits u64"),
            model_bytes,
            program_bytes: smallest_compile_budget(&circuit),
        }
    }
}

/// The smallest budget the sampling compiler admits the circuit's program under, which is the
/// program's admitted size.
fn smallest_compile_budget(circuit: &Circuit) -> u64 {
    let compiles = |bytes: u64| {
        SamplingCompiler::new()
            .memory_budget(MemoryBudget::from_bytes(bytes))
            .compile(circuit)
            .is_ok()
    };
    let (mut refused, mut admitted) = (0_u64, 1_u64 << 32);
    assert!(compiles(admitted), "fixture compiles under 4 GiB");
    while admitted - refused > 1 {
        let middle = refused + (admitted - refused) / 2;
        if compiles(middle) {
            admitted = middle;
        } else {
            refused = middle;
        }
    }
    admitted
}

/// A flat noise-heavy circuit whose compiled program outweighs its source text and whose
/// one-qubit, one-measurement session is small.
fn fixture() -> String {
    let mut text = "DEPOLARIZE2(0.1) 1 2\nZ 0\n".repeat(2000);
    text.push_str("M 0\n");
    text
}

fn sample(budget: u64, error_format: &str) -> super::support::CliRun {
    let budget = format!("--memory-budget={budget}");
    let error_format = format!("--error-format={error_format}");
    let args = [
        "stab",
        &error_format,
        "sample",
        "--shots",
        "1",
        "--skip_reference_sample",
        &budget,
    ];
    run(&args, fixture().as_bytes())
}

#[test]
fn parse_and_compile_share_one_budget_and_name_what_earlier_stages_hold() {
    let numbers = Admissions::of(&fixture());
    // Each stage fits the budget alone: parsing charges the text and the model, and compiling
    // charges the program; only their sum exceeds it.
    let budget = (numbers.input_bytes + numbers.model_bytes).max(numbers.program_bytes);
    assert!(
        numbers.model_bytes + numbers.program_bytes > budget,
        "fixture must make parse and compile exceed the budget only together"
    );

    let human = sample(budget, "human");
    human.assert_err(&[
        "cannot compile circuit sampler",
        &format!(
            "exceeding the {}-byte memory budget",
            budget - numbers.model_bytes
        ),
        &format!(
            "earlier stages of this command hold {} bytes of its {budget}-byte memory budget",
            numbers.model_bytes
        ),
        "help: Raise the budget with `--memory-budget <SIZE>`",
    ]);

    let json = sample(budget, "json");
    assert_eq!(json.status, 1);
    let diagnostic: serde_json::Value =
        serde_json::from_slice(&json.stderr).expect("one JSON diagnostic");
    let field = |pointer: &str| diagnostic.pointer(pointer).cloned();
    assert_eq!(field("/code"), Some("resource-limit-exceeded".into()));
    assert_eq!(
        field("/context/command_memory_budget/held_bytes"),
        Some(numbers.model_bytes.into())
    );
    assert_eq!(
        field("/context/command_memory_budget/budget_bytes"),
        Some(budget.into())
    );
    assert!(
        field("/help")
            .and_then(|help| help.as_str().map(str::to_owned))
            .is_some_and(|help| help.contains("--memory-budget"))
    );
}

#[test]
fn released_source_text_returns_its_bytes_to_later_stages() {
    let numbers = Admissions::of(&fixture());
    // The parsed model and program fit together; holding the source text through compilation
    // as well would not.
    let budget = numbers.model_bytes + numbers.program_bytes;
    assert!(numbers.input_bytes + numbers.model_bytes <= budget);
    assert!(numbers.input_bytes + numbers.model_bytes + numbers.program_bytes > budget);

    sample(budget, "human").assert_ok("0\n");
}

#[test]
fn held_plans_and_inlined_feedback_reduce_the_budget_of_later_stages() {
    // 1. `sample` holds `plan.retained_bytes()` when admitting its session.
    let mut wide_session_circuit = "S 0\nZ 0\n".repeat(400);
    wide_session_circuit.push_str("REPEAT 20000 {\n    M 0\n}\n");
    let (parsed, parsed_bytes) =
        Circuit::from_stim_bytes_admitted(wide_session_circuit.as_bytes(), ParseLimits::default())
            .expect("parse");
    let sample_plan = SamplingCompiler::new().compile(&parsed).expect("compile");
    let plan_bytes = sample_plan.retained_bytes();
    // 64 shots uses a 64-lane materializing session (`8 * (2 * 1 + 20000) = 160,016` bytes).
    let session_bytes = 8 * (2 + 20_000);
    // The session is admitted beside the plan and the 64-shot output encoder.
    let output_bytes = encoder_bytes(RecordFormat::ZeroOne, &[20_000], true, 64);
    let session_refusal_budget = (u64::try_from(wide_session_circuit.len()).expect("len")
        + parsed_bytes)
        .max(parsed_bytes + plan_bytes)
        .max(session_bytes)
        .max(plan_bytes + output_bytes);
    assert!(
        plan_bytes + output_bytes + session_bytes > session_refusal_budget
            && parsed_bytes + plan_bytes <= session_refusal_budget,
        "plan={plan_bytes}, session={session_bytes}, parsed={parsed_bytes}, budget={session_refusal_budget}"
    );
    let sample_json = run(
        &[
            "stab",
            "--error-format=json",
            "sample",
            "--shots",
            "64",
            "--skip_reference_sample",
            &format!("--memory-budget={session_refusal_budget}"),
        ],
        wide_session_circuit.as_bytes(),
    );
    assert_eq!(sample_json.status, 1);
    let diag: serde_json::Value = serde_json::from_slice(&sample_json.stderr).expect("json");
    assert_eq!(
        diag.pointer("/context/command_memory_budget/held_bytes")
            .cloned(),
        Some((plan_bytes + output_bytes).into())
    );

    // 2. `sample_dem` admits its plan against `budget - dem_hold` and holds the plan for session.
    let mut dem_text = String::new();
    for index in 0..300 {
        dem_text.push_str(&format!("error(0.1) D{index}\n"));
    }
    let (dem, dem_bytes) = stab_model::DetectorErrorModel::from_dem_bytes_admitted(
        dem_text.as_bytes(),
        ParseLimits::default(),
    )
    .expect("parse DEM");
    let dem_plan = stab_engine::DemSamplingCompiler::new()
        .compile(&dem)
        .expect("compile DEM");
    let dem_plan_bytes = dem_plan.retained_bytes();
    let dem_compile_refusal =
        (u64::try_from(dem_text.len()).expect("len") + dem_bytes).max(dem_plan_bytes);
    assert!(dem_bytes + dem_plan_bytes > dem_compile_refusal);
    let dem_json = run(
        &[
            "stab",
            "--error-format=json",
            "sample_dem",
            "--shots",
            "1",
            &format!("--memory-budget={dem_compile_refusal}"),
        ],
        dem_text.as_bytes(),
    );
    assert_eq!(dem_json.status, 1);
    let dem_diag: serde_json::Value = serde_json::from_slice(&dem_json.stderr).expect("json");
    assert_eq!(
        dem_diag.pointer("/context/resource").cloned(),
        Some("materialized-bytes".into())
    );
    assert_eq!(
        dem_diag
            .pointer("/context/command_memory_budget/held_bytes")
            .cloned(),
        Some(dem_bytes.into())
    );

    // 3. `m2d --ran_without_feedback` replaces `parsed_bytes` with `inlined.admitted_bytes()`.
    let scratch = super::support::Scratch::new();
    let feedback_stim = "M 0 0 0 0 0 0 0\nREPEAT 40 {\n    CX rec[-7] 0\n    M 0\n    DETECTOR rec[-1]\n}\nOBSERVABLE_INCLUDE(200000) rec[-1]\n";
    let circuit_path = scratch.file("feedback.stim", feedback_stim);
    let (fb_circuit, fb_parsed_bytes) =
        Circuit::from_stim_bytes_admitted(feedback_stim.as_bytes(), ParseLimits::default())
            .expect("parse feedback");
    let inlined = stab_analysis::circuit_with_inlined_feedback(&fb_circuit, &Default::default())
        .expect("inline");
    let inlined_bytes = inlined.admitted_bytes();
    assert!(inlined_bytes > fb_parsed_bytes * 2);
    let m2d_plan_bytes = stab_engine::MeasurementToDetectionCompiler::new()
        .reference_sample_mode(stab_engine::ReferenceSampleMode::SkipReferenceSample)
        .compile(&inlined)
        .expect("compile m2d")
        .retained_bytes();
    let m2d_budget = inlined_bytes + m2d_plan_bytes - 1;
    assert!(fb_parsed_bytes + m2d_plan_bytes < m2d_budget);
    let m2d_json = run(
        &[
            "stab",
            "--error-format=json",
            "m2d",
            "--circuit",
            &circuit_path,
            "--in_format=01",
            "--ran_without_feedback",
            "--skip_reference_sample",
            &format!("--memory-budget={m2d_budget}"),
        ],
        b"",
    );
    assert_eq!(m2d_json.status, 1);
    let m2d_diag: serde_json::Value = serde_json::from_slice(&m2d_json.stderr).expect("json");
    assert_eq!(
        m2d_diag
            .pointer("/context/command_memory_budget/held_bytes")
            .cloned(),
        Some(inlined_bytes.into())
    );
}

/// Scratch the CLI admits for one output encoder of `records` records joining `parts`,
/// transposing bit-plane batches when `bit_planes`.
fn encoder_bytes(format: RecordFormat, parts: &[usize], bit_planes: bool, records: u64) -> u64 {
    let mut bytes =
        RecordEncoder::shot_major_scratch_bytes_for_records(format, parts.iter().sum(), records);
    if bit_planes {
        bytes += RecordEncoder::bit_plane_scratch_bytes(format, parts);
    }
    u64::try_from(bytes).expect("fits u64")
}

#[test]
fn detect_admits_its_output_encoders_and_file_buffers_before_its_session() {
    let scratch = super::support::Scratch::new();
    // Resetting qubit 999 gives a short circuit a session of 1000 frame qubits.
    let text = "R 0 1 999\nX_ERROR(0.25) 0 1\nM 0 1\nDETECTOR rec[-1]\nDETECTOR rec[-2]\n\
                OBSERVABLE_INCLUDE(0) rec[-1]\n";
    let circuit = Circuit::from_stim_str(text).expect("parse");
    let plan_bytes = DetectionSamplingCompiler::new()
        .compile(&circuit)
        .expect("compile")
        .retained_bytes();
    // 256 narrow shots materialize, so both encoders transpose bit-plane batches, and the named
    // output and `--obs_out` files each add one write buffer.
    let output_bytes = encoder_bytes(RecordFormat::B8, &[2], true, 256)
        + encoder_bytes(RecordFormat::ZeroOne, &[1], true, 256)
        + 2 * crate::io_plan::OUTPUT_BUFFER_BYTES as u64;
    let out = scratch.path("dets.b8");
    let obs = scratch.path("obs.01");
    let detect = |budget: Option<u64>| {
        let mut args = vec![
            "stab".to_owned(),
            "detect".to_owned(),
            "--shots=256".to_owned(),
            "--seed=5".to_owned(),
            "--out_format=b8".to_owned(),
            format!("--out={out}"),
            format!("--obs_out={obs}"),
        ];
        args.extend(budget.map(|budget| format!("--memory-budget={budget}")));
        let args = args.iter().map(String::as_str).collect::<Vec<_>>();
        run(&args, text.as_bytes())
    };

    // A session that fits nowhere beside the plan reports its own refusal, as it did before the
    // output storage was admitted.
    detect(Some(plan_bytes + 4096)).assert_err(&[
        "detection session needs an estimated",
        "exceeding the 4096-byte memory budget",
        &format!("earlier stages of this command hold {plan_bytes} bytes"),
    ]);
    // The plan fits, but its encoders and buffers do not fit beside it.
    detect(Some(plan_bytes + output_bytes - 1)).assert_err(&[
        &format!(
            "detect needs at least {output_bytes} bytes to encode its output records, exceeding \
             the {}-byte memory budget",
            output_bytes - 1
        ),
        &format!("earlier stages of this command hold {plan_bytes} bytes"),
    ]);
    // The encoders fit, and the session is then refused with them held.
    detect(Some(plan_bytes + output_bytes)).assert_stderr_contains(&[&format!(
        "earlier stages of this command hold {} bytes",
        plan_bytes + output_bytes
    )]);

    detect(None).assert_ok("");
    let unbudgeted = (scratch.read("dets.b8"), scratch.read("obs.01"));
    detect(Some(plan_bytes + output_bytes + (1 << 20))).assert_ok("");
    assert_eq!(
        (scratch.read("dets.b8"), scratch.read("obs.01")),
        unbudgeted
    );
}

/// A session whose storage does not fit the budget is a budget refusal on every command that
/// creates one, so it carries the resource-limit code instead of a generic execution failure.
#[test]
fn session_storage_refusals_report_the_resource_limit_code() {
    let scratch = super::support::Scratch::new();
    let circuit = "H 0\nREPEAT 1000 {\n    CX 0 1\n    M 0 1\n    DETECTOR rec[-1]\n}\nM 5000\n";
    let circuit_path = scratch.file("wide.stim", circuit);
    // m2d's reference sample of the wide circuit would refuse at compile time, so it converts
    // a narrow circuit whose session is still larger than the budget.
    let narrow_path = scratch.file(
        "narrow.stim",
        "R 0\nREPEAT 100 {\n    H 0\n    M 0\n}\nM 0\nDETECTOR rec[-1]\n",
    );
    let measurements = scratch.file("narrow.01", format!("{}\n", "0".repeat(101)));
    for (command, args) in [
        ("sample", vec!["--shots", "1", "--in", &circuit_path]),
        ("detect", vec!["--shots", "1", "--in", &circuit_path]),
        (
            "m2d",
            vec![
                "--circuit",
                &narrow_path,
                "--in",
                &measurements,
                "--in_format",
                "01",
            ],
        ),
    ] {
        let mut argv = vec!["stab", "--error-format=json", command];
        argv.extend(args);
        argv.push("--memory-budget=16KiB");
        let result = run(&argv, b"");
        assert_eq!(result.status, 1, "{command}: {}", result.stderr_text());
        let diagnostic: serde_json::Value =
            serde_json::from_slice(&result.stderr).expect("one JSON diagnostic");
        assert_eq!(
            diagnostic.pointer("/code").cloned(),
            Some("resource-limit-exceeded".into()),
            "{command}: {diagnostic}"
        );
        assert!(
            diagnostic
                .pointer("/message")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|message| message.contains("session needs an estimated")),
            "{command}: {diagnostic}"
        );
    }
}
