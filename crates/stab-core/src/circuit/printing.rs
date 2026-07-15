use std::fmt::{self, Write as _};
use std::io::{self, Write};

use crate::{Pauli, Target};

use super::{Circuit, CircuitInstruction, CircuitItem, RepeatBlock};

const FLOAT_BUFFER_BYTES: usize = 32;

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
            .saturating_add(instruction.args.iter().fold(0usize, |len, arg| {
                len.saturating_add(format_float(*arg).len())
            }))
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
        .saturating_add(usize::from(repeat.body.is_empty()))
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

pub(super) fn write_target_io(out: &mut impl Write, target: &Target) -> io::Result<()> {
    match target {
        Target::Qubit { id, inverted } => {
            if *inverted {
                out.write_all(b"!")?;
            }
            write_u64_io(out, u64::from(id.get()))
        }
        Target::MeasurementRecord { offset } => {
            out.write_all(b"rec[")?;
            if offset.is_negative_zero() {
                out.write_all(b"-0")?;
            } else {
                write_i32_io(out, offset.get())?;
            }
            out.write_all(b"]")
        }
        Target::SweepBit { id } => {
            out.write_all(b"sweep[")?;
            write_u64_io(out, u64::from(*id))?;
            out.write_all(b"]")
        }
        Target::Pauli {
            pauli,
            id,
            inverted,
        } => {
            if *inverted {
                out.write_all(b"!")?;
            }
            out.write_all(&[match pauli {
                Pauli::X => b'X',
                Pauli::Y => b'Y',
                Pauli::Z => b'Z',
            }])?;
            write_u64_io(out, u64::from(id.get()))
        }
        Target::Combiner => out.write_all(b"*"),
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
pub(super) fn push_u64(out: &mut String, value: u64) {
    let (digits, start) = encode_u64(value);
    let encoded = digits
        .get(start..)
        .expect("decimal cursor remains in bounds");
    out.push_str(std::str::from_utf8(encoded).expect("decimal digits are valid UTF-8"));
}

fn write_i32_io(out: &mut impl Write, value: i32) -> io::Result<()> {
    if value < 0 {
        out.write_all(b"-")?;
    }
    write_u64_io(out, u64::from(value.unsigned_abs()))
}

#[allow(
    clippy::expect_used,
    reason = "the decimal encoder proves its returned cursor is inside the fixed buffer"
)]
fn write_u64_io(out: &mut impl Write, value: u64) -> io::Result<()> {
    let (digits, start) = encode_u64(value);
    out.write_all(
        digits
            .get(start..)
            .expect("decimal cursor remains in bounds"),
    )
}

#[allow(
    clippy::expect_used,
    reason = "a u64 has at most 20 decimal digits and each remainder fits in u8"
)]
fn encode_u64(mut value: u64) -> ([u8; 20], usize) {
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
    (digits, start)
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

pub(super) struct FormattedFloat {
    text: StackText<FLOAT_BUFFER_BYTES>,
}

impl FormattedFloat {
    pub(super) fn as_bytes(&self) -> &[u8] {
        self.text.as_bytes()
    }

    pub(super) fn as_str(&self) -> &str {
        self.text.as_str()
    }

    pub(super) fn len(&self) -> usize {
        self.text.len()
    }
}

#[allow(
    clippy::expect_used,
    reason = "Stim's six-significant-digit f64 forms fit in the fixed 32-byte buffers"
)]
pub(super) fn format_float(value: f64) -> FormattedFloat {
    let mut result = StackText::new();
    if let Some(integer) = stim_integer_like_i64(value) {
        write!(&mut result, "{integer}").expect("integer fits in float buffer");
        return FormattedFloat { text: result };
    }

    let mut scientific = StackText::<FLOAT_BUFFER_BYTES>::new();
    write!(&mut scientific, "{value:.5e}").expect("scientific f64 fits in float buffer");
    let (mantissa, exponent) = scientific
        .as_str()
        .split_once('e')
        .expect("Rust scientific f64 formatting includes an exponent");
    let exponent = exponent
        .parse::<i32>()
        .expect("Rust scientific f64 formatting uses an i32 exponent");

    if (-4..6).contains(&exponent) {
        let decimal_places = usize::try_from(5 - exponent)
            .expect("fixed-format exponent yields nonnegative decimal places");
        write!(&mut result, "{value:.decimal_places$}").expect("fixed f64 fits in float buffer");
        result.trim_decimal_float();
    } else {
        result
            .write_str(mantissa)
            .expect("mantissa fits in float buffer");
        result.trim_decimal_float();
        write!(&mut result, "e{exponent:+03}").expect("exponent fits in float buffer");
    }
    FormattedFloat { text: result }
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

struct StackText<const N: usize> {
    bytes: [u8; N],
    len: usize,
}

#[allow(
    clippy::expect_used,
    reason = "StackText mutators preserve bounds and write only valid UTF-8 formatting input"
)]
impl<const N: usize> StackText<N> {
    const fn new() -> Self {
        Self {
            bytes: [0; N],
            len: 0,
        }
    }

    fn as_bytes(&self) -> &[u8] {
        self.bytes
            .get(..self.len)
            .expect("stack text length remains in bounds")
    }

    fn as_str(&self) -> &str {
        std::str::from_utf8(self.as_bytes()).expect("formatted text is valid UTF-8")
    }

    const fn len(&self) -> usize {
        self.len
    }

    fn trim_decimal_float(&mut self) {
        if !self.as_bytes().contains(&b'.') {
            return;
        }
        while self.as_bytes().last() == Some(&b'0') {
            self.len -= 1;
        }
        if self.as_bytes().last() == Some(&b'.') {
            self.len -= 1;
        }
    }
}

impl<const N: usize> fmt::Write for StackText<N> {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        let end = self.len.checked_add(text.len()).ok_or(fmt::Error)?;
        let destination = self.bytes.get_mut(self.len..end).ok_or(fmt::Error)?;
        destination.copy_from_slice(text.as_bytes());
        self.len = end;
        Ok(())
    }
}
