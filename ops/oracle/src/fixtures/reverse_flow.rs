use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;
use std::str::FromStr;

use serde::Deserialize;
use stab_core::{Flow, TimeReversedForFlowsOptions, circuit_time_reversed_for_flows_with_options};

use super::{FixtureError, FixtureRow, parse_core_circuit};
use crate::{OracleError, RepoRoot};

const REVERSE_FLOW_PROTOCOL_LIMIT_BYTES: usize = crate::process::OUTPUT_LIMIT_BYTES;

#[derive(Debug, Deserialize)]
struct ReverseFlowFixtureCorpus {
    cases: BTreeMap<String, ReverseFlowFixtureCase>,
}

#[derive(Clone, Debug, Deserialize)]
struct ReverseFlowFixtureCase {
    circuit: String,
    flows: Vec<String>,
    #[serde(default)]
    dont_turn_measurements_into_resets: bool,
}

pub(super) fn core_time_reverse_flows_output(
    row: &FixtureRow,
    input: &str,
    tokens: &[String],
) -> Result<crate::ProcessOutput, FixtureError> {
    let case = reverse_flow_case(row, input, tokens)?;
    let circuit = parse_core_circuit(row, "reverse-flow circuit", &case.circuit)?;
    let flows = case
        .flows
        .iter()
        .map(|text| {
            Flow::from_str(text).map_err(|source| FixtureError::CoreFixtureFailed {
                id: row.id.clone(),
                reason: format!("reverse-flow input flow {text:?} failed to parse: {source}"),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let reversed = circuit_time_reversed_for_flows_with_options(
        &circuit,
        &flows,
        TimeReversedForFlowsOptions {
            dont_turn_measurements_into_resets: case.dont_turn_measurements_into_resets,
        },
    );
    let (inverse, inverse_flows) = match reversed {
        Ok(reversed) => reversed,
        Err(source) => {
            return Ok(crate::ProcessOutput {
                status: Some(1),
                stdout: crate::CapturedOutput {
                    bytes: Vec::new(),
                    truncated: false,
                },
                stderr: crate::CapturedOutput {
                    bytes: format!("{source}\n").into_bytes(),
                    truncated: false,
                },
            });
        }
    };
    let mut stdout = String::from("circuit:\n");
    stdout.push_str(&inverse.to_stim_string());
    stdout.push_str("flows:\n");
    for flow in inverse_flows {
        writeln!(stdout, "{flow}").map_err(|source| FixtureError::CoreFixtureFailed {
            id: row.id.clone(),
            reason: format!("reverse-flow output formatting failed: {source}"),
        })?;
    }
    Ok(crate::ProcessOutput {
        status: Some(0),
        stdout: crate::CapturedOutput {
            bytes: stdout.into_bytes(),
            truncated: false,
        },
        stderr: crate::CapturedOutput {
            bytes: Vec::new(),
            truncated: false,
        },
    })
}

pub(super) fn is_reverse_flow_fixture(row: &FixtureRow) -> bool {
    row.argv_tokens()
        .first()
        .is_some_and(|token| token == "core-time-reverse-flows")
}

pub(super) fn run_pinned_stim_reverse_flow(
    root: &RepoRoot,
    row: &FixtureRow,
    input: &[u8],
    helper: &Path,
) -> Result<crate::ProcessOutput, OracleError> {
    let input = super::fixture_utf8(row, "stdin", input)?;
    let tokens = row.argv_tokens();
    let case = reverse_flow_case(row, input, &tokens)?;
    let mut protocol = Vec::new();
    protocol.extend_from_slice(if case.dont_turn_measurements_into_resets {
        b"1\n"
    } else {
        b"0\n"
    });
    protocol.extend_from_slice(case.flows.len().to_string().as_bytes());
    protocol.push(b'\n');
    append_protocol_blob(&mut protocol, case.circuit.as_bytes());
    for flow in &case.flows {
        append_protocol_blob(&mut protocol, flow.as_bytes());
    }
    if protocol.len() > REVERSE_FLOW_PROTOCOL_LIMIT_BYTES {
        return Err(FixtureError::CoreFixtureFailed {
            id: row.id.clone(),
            reason: format!(
                "reverse-flow helper protocol exceeds the {REVERSE_FLOW_PROTOCOL_LIMIT_BYTES}-byte limit"
            ),
        }
        .into());
    }
    crate::run_process(
        helper,
        std::iter::empty::<&str>(),
        &protocol,
        Some(&root.path),
    )
}

fn reverse_flow_case(
    row: &FixtureRow,
    input: &str,
    tokens: &[String],
) -> Result<ReverseFlowFixtureCase, FixtureError> {
    let [_, case_id] = tokens else {
        return Err(FixtureError::CoreFixtureFailed {
            id: row.id.clone(),
            reason: "core-time-reverse-flows requires exactly one case id".to_string(),
        });
    };
    let corpus: ReverseFlowFixtureCorpus =
        serde_json::from_str(input).map_err(|source| FixtureError::CoreFixtureFailed {
            id: row.id.clone(),
            reason: format!("reverse-flow corpus parse failed: {source}"),
        })?;
    corpus
        .cases
        .get(case_id)
        .cloned()
        .ok_or_else(|| FixtureError::CoreFixtureFailed {
            id: row.id.clone(),
            reason: format!("reverse-flow corpus has no case {case_id}"),
        })
}

fn append_protocol_blob(protocol: &mut Vec<u8>, blob: &[u8]) {
    protocol.extend_from_slice(blob.len().to_string().as_bytes());
    protocol.push(b'\n');
    protocol.extend_from_slice(blob);
    protocol.push(b'\n');
}
