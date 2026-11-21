//! Typed fixture rows: `argv` and `statistical_plan` are parsed once when the
//! manifest loads, so every later use matches on the parsed value instead of
//! re-splitting manifest text.

use serde::Deserialize;

use super::statistical::StatisticalPlan;
use super::two_sample::TwoSamplePlan;
use super::{ExpectedStderrClass, FixtureComparator, FixtureId};

const ARGV_SEPARATOR: char = '|';
const FIXTURE_INPUT_TOKEN_PREFIX: &str = "{fixture_input:";
const FIXTURE_OUTPUT_TOKEN_PREFIX: &str = "{fixture_output:";
const FIXTURE_TOKEN_SUFFIX: &str = "}";

/// A manifest row exactly as the CSV spells it, before typed parsing.
#[derive(Clone, Debug, Deserialize)]
pub(super) struct RawFixtureRow {
    id: FixtureId,
    upstream_source: String,
    comparator: FixtureComparator,
    command_shape: String,
    argv: String,
    stdin_path: String,
    expected_stdout_path: String,
    expected_status: i32,
    expected_stderr_class: ExpectedStderrClass,
    statistical_plan: String,
    source_license_note: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct FixtureRow {
    pub(super) id: FixtureId,
    pub(super) upstream_source: String,
    pub(super) comparator: FixtureComparator,
    pub(super) command_shape: String,
    pub(super) command: RowCommand,
    pub(super) stdin_path: String,
    pub(super) expected_stdout_path: String,
    pub(super) expected_status: i32,
    pub(super) expected_stderr_class: ExpectedStderrClass,
    pub(super) plan: RowPlan,
    pub(super) source_license_note: String,
}

/// What a row executes: a Stim/Stab CLI invocation or an in-process core
/// comparison.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum RowCommand {
    Cli(CliArgv),
    Core(CoreFixture),
}

/// In-process core fixtures. Each kind takes exactly the arguments it uses.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum CoreFixture {
    /// `core-parse-print` or `core-circuit-parse-print`: circuit parse and print.
    CircuitParsePrint,
    /// `core-dem-parse-print`: detector error model parse and print.
    DemParsePrint,
    /// `core-time-reverse-flows|CASE`: one case of the reverse-flow corpus.
    TimeReverseFlows { case_id: String },
}

impl CoreFixture {
    fn parse(command: &str, arguments: &[&str]) -> Option<Result<Self, String>> {
        let kind = match command {
            "core-parse-print" | "core-circuit-parse-print" => Self::CircuitParsePrint,
            "core-dem-parse-print" => Self::DemParsePrint,
            "core-time-reverse-flows" => {
                return Some(match arguments {
                    [case_id] if !case_id.starts_with('{') => Ok(Self::TimeReverseFlows {
                        case_id: (*case_id).to_string(),
                    }),
                    _ => Err(
                        "core-time-reverse-flows requires exactly one case id argument".to_string(),
                    ),
                });
            }
            _ => return None,
        };
        Some(if arguments.is_empty() {
            Ok(kind)
        } else {
            Err(format!(
                "{command} takes no arguments, got {}",
                arguments.len()
            ))
        })
    }
}

/// One CLI argv token after placeholder parsing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum ArgToken {
    Literal(String),
    /// `{fixture_input:REL}`: replaced by the fixture file's path.
    Input(String),
    /// `{fixture_output:REL}`: replaced by a private scratch output path.
    Output(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CliArgv {
    tokens: Vec<ArgToken>,
}

impl CliArgv {
    #[cfg(test)]
    pub(super) fn literals(tokens: &[String]) -> Self {
        Self {
            tokens: tokens.iter().cloned().map(ArgToken::Literal).collect(),
        }
    }

    pub(super) fn tokens(&self) -> &[ArgToken] {
        &self.tokens
    }

    /// The value of `option`, spelled `option VALUE` or `option=VALUE`, among
    /// the literal tokens.
    pub(super) fn option_value(&self, option: &str) -> Option<&str> {
        let with_equals = format!("{option}=");
        let mut tokens = self.tokens.iter();
        while let Some(token) = tokens.next() {
            let ArgToken::Literal(token) = token else {
                continue;
            };
            if token == option {
                return match tokens.next() {
                    Some(ArgToken::Literal(value)) => Some(value),
                    _ => None,
                };
            }
            if let Some(value) = token.strip_prefix(&with_equals) {
                return Some(value);
            }
        }
        None
    }
}

/// A row's `statistical_plan` column, parsed by the grammar its comparator
/// uses.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum RowPlan {
    /// Free text: the structural plan of a structural row, or a note on an
    /// exact-output or help-health row.
    Text(String),
    Statistical(StatisticalPlan),
    TwoSample(TwoSamplePlan),
}

impl TryFrom<RawFixtureRow> for FixtureRow {
    type Error = Vec<String>;

    fn try_from(raw: RawFixtureRow) -> Result<Self, Self::Error> {
        let id = raw.id;
        let mut violations = Vec::new();
        for (field, value) in [
            ("upstream_source", &raw.upstream_source),
            ("command_shape", &raw.command_shape),
            ("argv", &raw.argv),
            ("source_license_note", &raw.source_license_note),
        ] {
            if value.is_empty() {
                violations.push(format!("{id} has empty {field}"));
            }
        }
        let command = parse_command(&id, &raw.argv)
            .map_err(|violation| violations.push(violation))
            .ok();
        let plan = command.as_ref().and_then(|command| {
            parse_plan(raw.comparator, &raw.statistical_plan, command)
                .map_err(|violation| violations.push(format!("{id} {violation}")))
                .ok()
        });
        match (command, plan) {
            (Some(command), Some(plan)) if violations.is_empty() => Ok(Self {
                id,
                upstream_source: raw.upstream_source,
                comparator: raw.comparator,
                command_shape: raw.command_shape,
                command,
                stdin_path: raw.stdin_path,
                expected_stdout_path: raw.expected_stdout_path,
                expected_status: raw.expected_status,
                expected_stderr_class: raw.expected_stderr_class,
                plan,
                source_license_note: raw.source_license_note,
            }),
            _ => Err(violations),
        }
    }
}

fn parse_command(id: &FixtureId, argv: &str) -> Result<RowCommand, String> {
    let words = argv.split(ARGV_SEPARATOR).collect::<Vec<_>>();
    if words.iter().any(|word| word.is_empty()) {
        return Err(format!("{id} has an empty argv token"));
    }
    let Some((command, arguments)) = words.split_first() else {
        return Err(format!("{id} has no argv tokens"));
    };
    if let Some(core) = CoreFixture::parse(command, arguments) {
        return core
            .map(RowCommand::Core)
            .map_err(|reason| format!("{id} has malformed core argv {argv:?}: {reason}"));
    }
    let tokens = words
        .iter()
        .map(|word| parse_arg_token(id, word))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(RowCommand::Cli(CliArgv { tokens }))
}

fn parse_arg_token(id: &FixtureId, token: &str) -> Result<ArgToken, String> {
    if let Some(relative) = parse_prefixed_token(id, token, FIXTURE_INPUT_TOKEN_PREFIX, "input")? {
        return Ok(ArgToken::Input(relative.to_string()));
    }
    if let Some(relative) = parse_prefixed_token(id, token, FIXTURE_OUTPUT_TOKEN_PREFIX, "output")?
    {
        return Ok(ArgToken::Output(relative.to_string()));
    }
    Ok(ArgToken::Literal(token.to_string()))
}

fn parse_prefixed_token<'a>(
    id: &FixtureId,
    token: &'a str,
    prefix: &str,
    label: &str,
) -> Result<Option<&'a str>, String> {
    if let Some(without_prefix) = token.strip_prefix(prefix) {
        if let Some(relative) = without_prefix.strip_suffix(FIXTURE_TOKEN_SUFFIX) {
            if relative.is_empty() {
                return Err(format!("{id} has empty fixture {label} token"));
            }
            return Ok(Some(relative));
        }
        return Err(format!("{id} has malformed fixture {label} token {token}"));
    }
    if token.contains(prefix) {
        return Err(format!("{id} has malformed fixture {label} token {token}"));
    }
    Ok(None)
}

fn parse_plan(
    comparator: FixtureComparator,
    text: &str,
    command: &RowCommand,
) -> Result<RowPlan, String> {
    let cli = match command {
        RowCommand::Cli(argv) => Some(argv),
        RowCommand::Core(_) => None,
    };
    match comparator {
        FixtureComparator::Statistical
        | FixtureComparator::TwoSample
        | FixtureComparator::Structural
            if text.is_empty() =>
        {
            Err("comparator needs structural or statistical plan text".to_string())
        }
        FixtureComparator::Statistical => {
            let argv = cli.ok_or("statistical rows must run the Stim and Stab binaries")?;
            let plan = StatisticalPlan::parse(text)
                .map_err(|reason| format!("invalid statistical plan: {reason}"))?;
            if let Some(reason) = plan.validate_argv(argv) {
                return Err(format!("invalid statistical plan: {reason}"));
            }
            Ok(RowPlan::Statistical(plan))
        }
        FixtureComparator::TwoSample => {
            let argv = cli.ok_or("two-sample rows must run the Stim and Stab binaries")?;
            let plan = TwoSamplePlan::parse(text)
                .map_err(|reason| format!("invalid two-sample plan: {reason}"))?;
            if let Some(reason) = plan.validate_argv(argv) {
                return Err(format!("invalid two-sample plan: {reason}"));
            }
            Ok(RowPlan::TwoSample(plan))
        }
        FixtureComparator::ExactOutput
        | FixtureComparator::HelpHealth
        | FixtureComparator::Structural => Ok(RowPlan::Text(text.to_string())),
    }
}

impl FixtureRow {
    pub(super) fn core(&self) -> Option<&CoreFixture> {
        match &self.command {
            RowCommand::Core(core) => Some(core),
            RowCommand::Cli(_) => None,
        }
    }

    pub(super) fn statistical_plan(&self) -> Option<&StatisticalPlan> {
        match &self.plan {
            RowPlan::Statistical(plan) => Some(plan),
            RowPlan::Text(_) | RowPlan::TwoSample(_) => None,
        }
    }
}
