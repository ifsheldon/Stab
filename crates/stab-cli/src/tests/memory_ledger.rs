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

/// A flat Clifford-heavy circuit whose compiled program outweighs its source text and whose
/// one-qubit, one-measurement session is small.
fn fixture() -> String {
    let mut text = "S 0\nZ 0\n".repeat(2000);
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
