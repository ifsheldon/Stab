use crate::{Pauli, Target};

use super::{Circuit, CircuitInstruction, CircuitItem, RepeatBlock};

const MAX_FORMATTED_FLOAT_BYTES: usize = 24;

pub(super) fn stim_text_capacity(circuit: &Circuit, indent: usize) -> usize {
    circuit.items.iter().fold(0usize, |capacity, item| {
        capacity.saturating_add(match item {
            CircuitItem::Instruction(instruction) => instruction_text_capacity(instruction, indent),
            CircuitItem::RepeatBlock(repeat) => repeat_text_capacity(repeat, indent),
        })
    })
}

fn instruction_text_capacity(instruction: &CircuitInstruction, indent: usize) -> usize {
    let mut capacity = indent
        .saturating_add(instruction.gate.canonical_name().len())
        .saturating_add(1);
    if let Some(tag) = &instruction.tag {
        capacity = capacity
            .saturating_add(2)
            .saturating_add(escaped_tag_len(tag));
    }
    if !instruction.args.is_empty() {
        capacity = capacity
            .saturating_add(2)
            .saturating_add(
                instruction
                    .args
                    .len()
                    .saturating_mul(MAX_FORMATTED_FLOAT_BYTES),
            )
            .saturating_add(instruction.args.len().saturating_sub(1).saturating_mul(2));
    }
    capacity.saturating_add(targets_text_len(&instruction.targets))
}

fn repeat_text_capacity(repeat: &RepeatBlock, indent: usize) -> usize {
    let mut capacity = indent
        .saturating_add("REPEAT".len())
        .saturating_add(1)
        .saturating_add(decimal_len_u64(repeat.repeat_count.get()))
        .saturating_add(" {\n".len())
        .saturating_add(stim_text_capacity(&repeat.body, indent.saturating_add(4)))
        .saturating_add(indent)
        .saturating_add("}\n".len());
    if let Some(tag) = &repeat.tag {
        capacity = capacity
            .saturating_add(2)
            .saturating_add(escaped_tag_len(tag));
    }
    capacity
}

fn targets_text_len(targets: &[Target]) -> usize {
    targets.iter().fold(0usize, |len, target| {
        if target.is_combiner() {
            len
        } else {
            len.saturating_add(1)
                .saturating_add(target_text_len(target))
        }
    })
}

fn target_text_len(target: &Target) -> usize {
    match target {
        Target::Qubit { id, inverted } => {
            usize::from(*inverted) + decimal_len_u64(u64::from(id.get()))
        }
        Target::MeasurementRecord { offset } => {
            "rec[]".len()
                + if offset.is_negative_zero() {
                    2
                } else {
                    decimal_len_i32(offset.get())
                }
        }
        Target::SweepBit { id } => "sweep[]".len() + decimal_len_u64(u64::from(*id)),
        Target::Pauli { id, inverted, .. } => {
            usize::from(*inverted) + 1 + decimal_len_u64(u64::from(id.get()))
        }
        Target::Combiner => 1,
    }
}

pub(super) fn write_target(out: &mut String, target: &Target) {
    match target {
        Target::Qubit { id, inverted } => {
            if *inverted {
                out.push('!');
            }
            push_u64(out, u64::from(id.get()));
        }
        Target::MeasurementRecord { offset } => {
            out.push_str("rec[");
            if offset.is_negative_zero() {
                out.push_str("-0");
            } else {
                push_i32(out, offset.get());
            }
            out.push(']');
        }
        Target::SweepBit { id } => {
            out.push_str("sweep[");
            push_u64(out, u64::from(*id));
            out.push(']');
        }
        Target::Pauli {
            pauli,
            id,
            inverted,
        } => {
            if *inverted {
                out.push('!');
            }
            out.push(match pauli {
                Pauli::X => 'X',
                Pauli::Y => 'Y',
                Pauli::Z => 'Z',
            });
            push_u64(out, u64::from(id.get()));
        }
        Target::Combiner => out.push('*'),
    }
}

fn push_i32(out: &mut String, value: i32) {
    if value < 0 {
        out.push('-');
    }
    push_u64(out, u64::from(value.unsigned_abs()));
}

#[allow(
    clippy::expect_used,
    reason = "the local buffer contains only ASCII decimal digits written below"
)]
pub(super) fn push_u64(out: &mut String, mut value: u64) {
    let mut digits = [0_u8; 20];
    let mut start = digits.len();
    loop {
        start -= 1;
        *digits
            .get_mut(start)
            .expect("decimal cursor remains inside the fixed buffer") =
            b'0' + u8::try_from(value % 10).expect("decimal digit fits in u8");
        value /= 10;
        if value == 0 {
            break;
        }
    }
    let encoded = digits
        .get(start..)
        .expect("decimal cursor remains inside the fixed buffer");
    out.push_str(std::str::from_utf8(encoded).expect("decimal digits are valid UTF-8"));
}

fn decimal_len_i32(value: i32) -> usize {
    usize::from(value < 0) + decimal_len_u64(u64::from(value.unsigned_abs()))
}

fn decimal_len_u64(mut value: u64) -> usize {
    let mut len = 1;
    while value >= 10 {
        value /= 10;
        len += 1;
    }
    len
}

fn escaped_tag_len(tag: &str) -> usize {
    tag.chars().fold(0usize, |len, ch| {
        len.saturating_add(match ch {
            ']' | '\r' | '\n' | '\\' => 2,
            _ => ch.len_utf8(),
        })
    })
}

pub(super) fn format_float(value: f64) -> String {
    if let Some(integer) = stim_integer_like_i64(value) {
        return integer.to_string();
    }

    let scientific = format!("{value:.5e}");
    let Some((mantissa, exponent)) = scientific.split_once('e') else {
        return value.to_string();
    };
    let Ok(exponent) = exponent.parse::<i32>() else {
        return value.to_string();
    };

    if (-4..6).contains(&exponent) {
        let decimal_places = usize::try_from(5 - exponent).unwrap_or(0);
        trim_decimal_float(format!("{value:.decimal_places$}"))
    } else {
        format!(
            "{}e{}",
            trim_decimal_float(mantissa.to_string()),
            format_scientific_exponent(exponent)
        )
    }
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "Stim's C++ printer casts integral doubles to int64_t before printing"
)]
fn stim_integer_like_i64(value: f64) -> Option<i64> {
    if value > i64::MIN as f64 && value < i64::MAX as f64 {
        let integer = value as i64;
        if integer as f64 == value {
            return Some(integer);
        }
    }
    None
}

fn trim_decimal_float(mut text: String) -> String {
    if text.contains('.') {
        text = text.trim_end_matches('0').trim_end_matches('.').to_string();
    }
    text
}

fn format_scientific_exponent(exponent: i32) -> String {
    if exponent < 0 {
        format!("-{:02}", exponent.abs())
    } else {
        format!("+{exponent:02}")
    }
}
