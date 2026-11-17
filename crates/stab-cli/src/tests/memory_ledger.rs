//! One `--memory-budget` covers a whole command: each stage admits against the budget less what
//! earlier stages still hold, and dead storage returns its bytes before the next stage.

use stab_engine::SamplingCompiler;
use stab_model::{Circuit, MemoryBudget, ParseLimits};

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
    let session_refusal_budget = (u64::try_from(wide_session_circuit.len()).expect("len")
        + parsed_bytes)
        .max(parsed_bytes + plan_bytes)
        .max(session_bytes);
    assert!(
        plan_bytes + session_bytes > session_refusal_budget
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
        Some(plan_bytes.into())
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
    let inlined = stab_analysis::circuit_with_inlined_feedback(&fb_circuit).expect("inline");
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
