use crate::target::TargetVec;
use crate::{CircuitResult, Gate, Target};

use super::{
    CircuitInstruction, parse_common_pair_instruction, parse_common_single_qubit_instruction,
    wrap_line,
};

pub(super) fn parse_common_plain_instruction(
    line_number: usize,
    line: &str,
) -> Option<CircuitResult<CircuitInstruction>> {
    if line == "TICK" {
        return Some(Ok(CircuitInstruction::from_validated_parts(
            Gate::plain_tick(),
            Vec::new(),
            TargetVec::new(),
            None,
        )));
    }
    if let Some(rest) = line.strip_prefix("H ") {
        return parse_common_single_qubit_instruction(line_number, Gate::plain_h(), rest);
    }
    if let Some(rest) = line.strip_prefix("S ") {
        return parse_common_single_qubit_instruction(line_number, Gate::plain_s(), rest);
    }
    if let Some(rest) = line.strip_prefix("M ").or_else(|| line.strip_prefix("MZ ")) {
        return parse_common_single_qubit_instruction(line_number, Gate::plain_m(), rest);
    }
    if let Some(rest) = line
        .strip_prefix("CX ")
        .or_else(|| line.strip_prefix("CNOT "))
    {
        return parse_common_pair_instruction(line_number, Gate::plain_cx(), rest);
    }
    if let Some(rest) = line.strip_prefix("DETECTOR ") {
        return parse_common_detector_instruction(line_number, rest);
    }
    None
}

fn parse_common_detector_instruction(
    line_number: usize,
    rest: &str,
) -> Option<CircuitResult<CircuitInstruction>> {
    if rest.chars().any(char::is_whitespace) || !rest.starts_with("rec[-") || !rest.ends_with(']') {
        return None;
    }
    let target = match rest.parse::<Target>() {
        Ok(target) if target.is_measurement_record_target() => target,
        Ok(_) => return None,
        Err(error) => return Some(Err(wrap_line(line_number, error))),
    };
    let mut targets = TargetVec::new();
    targets.push(target);
    Some(Ok(CircuitInstruction::from_validated_parts(
        Gate::plain_detector(),
        Vec::new(),
        targets,
        None,
    )))
}

#[cfg(test)]
mod tests {
    use super::parse_common_plain_instruction;
    use crate::circuit::parse_instruction_fully_generic;

    #[test]
    #[allow(
        clippy::expect_used,
        reason = "internal parser equivalence fixtures require both selected paths to succeed"
    )]
    fn exact_paths_match_fully_generic_instruction_parsing() {
        for line in ["S 1", "TICK", "DETECTOR rec[-1]"] {
            let fast = parse_common_plain_instruction(1, line)
                .expect("selected exact fast path")
                .expect("parse exact fast path");
            let generic =
                parse_instruction_fully_generic(1, line).expect("parse fully generic path");
            assert_eq!(fast, generic, "{line}");
        }
    }
}
