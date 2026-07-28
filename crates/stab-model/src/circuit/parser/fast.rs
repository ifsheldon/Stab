use crate::target::TargetVec;
use crate::{Gate, ModelResult, Target};

use super::CircuitInstruction;

pub(super) fn parse_common_plain_instruction(
    line: &str,
) -> Option<ModelResult<CircuitInstruction>> {
    if line == "TICK" {
        return Some(Ok(CircuitInstruction::from_validated_parts(
            Gate::plain_tick(),
            Vec::new(),
            TargetVec::new(),
            None,
        )));
    }
    if let Some(rest) = line.strip_prefix("H ") {
        return parse_common_single_qubit_instruction(Gate::plain_h(), rest);
    }
    if let Some(rest) = line.strip_prefix("S ") {
        return parse_common_single_qubit_instruction(Gate::plain_s(), rest);
    }
    if let Some(rest) = line.strip_prefix("M ").or_else(|| line.strip_prefix("MZ ")) {
        return parse_common_single_qubit_instruction(Gate::plain_m(), rest);
    }
    if let Some(rest) = line
        .strip_prefix("CX ")
        .or_else(|| line.strip_prefix("CNOT "))
    {
        return parse_common_pair_instruction(Gate::plain_cx(), rest);
    }
    if let Some(rest) = line.strip_prefix("DETECTOR ") {
        return parse_common_detector_instruction(rest);
    }
    None
}

fn parse_common_single_qubit_instruction(
    gate: Gate,
    rest: &str,
) -> Option<ModelResult<CircuitInstruction>> {
    let target = parse_common_qubit_id(rest)?;
    let mut targets = TargetVec::new();
    targets.push(Target::qubit(target, false));
    Some(Ok(CircuitInstruction::from_validated_parts(
        gate,
        Vec::new(),
        targets,
        None,
    )))
}

fn parse_common_pair_instruction(
    gate: Gate,
    rest: &str,
) -> Option<ModelResult<CircuitInstruction>> {
    let (left, right) = rest.split_once(' ')?;
    let left = parse_common_qubit_id(left)?;
    let right = parse_common_qubit_id(right)?;
    if left == right {
        return None;
    }
    let mut targets = TargetVec::new();
    targets.push(Target::qubit(left, false));
    targets.push(Target::qubit(right, false));
    Some(Ok(CircuitInstruction::from_validated_parts(
        gate,
        Vec::new(),
        targets,
        None,
    )))
}

fn parse_common_qubit_id(text: &str) -> Option<crate::QubitId> {
    if text.is_empty() || !text.as_bytes().iter().all(u8::is_ascii_digit) {
        return None;
    }
    let mut value = 0u32;
    for byte in text.bytes() {
        let digit = u32::from(byte - b'0');
        value = value
            .checked_mul(10)
            .and_then(|value| value.checked_add(digit))?;
        if value >= crate::ids::STIM_TARGET_VALUE_LIMIT {
            return None;
        }
    }
    crate::QubitId::new(value).ok()
}

fn parse_common_detector_instruction(rest: &str) -> Option<ModelResult<CircuitInstruction>> {
    if rest.chars().any(char::is_whitespace) || !rest.starts_with("rec[-") || !rest.ends_with(']') {
        return None;
    }
    let target = match rest.parse::<Target>() {
        Ok(target) if target.is_measurement_record_target() => target,
        Ok(_) | Err(_) => return None,
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
    use super::super::parse_instruction_fully_generic;
    use super::parse_common_plain_instruction;

    #[test]
    #[allow(
        clippy::expect_used,
        reason = "internal parser equivalence fixtures require both selected paths to succeed"
    )]
    fn exact_paths_match_fully_generic_instruction_parsing() {
        for line in ["S 1", "TICK", "DETECTOR rec[-1]"] {
            let fast = parse_common_plain_instruction(line)
                .expect("selected exact fast path")
                .expect("parse exact fast path");
            let generic =
                parse_instruction_fully_generic(1, line).expect("parse fully generic path");
            assert_eq!(fast, generic, "{line}");
        }
    }
}
