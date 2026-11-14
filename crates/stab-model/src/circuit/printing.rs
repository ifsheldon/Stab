use std::fmt::{self, NumBuffer, Write as _};
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
            .saturating_add(escaped_tag_len(tag.as_bytes()));
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
            .saturating_add(escaped_tag_len(tag.as_bytes()));
    }
    capacity
}

fn targets_text_len(targets: &[Target]) -> usize {
    // Slight overcount for combiners, which suppress adjacent separators;
    // this only sizes the output reservation.
    targets.iter().fold(0usize, |len, target| {
        len.saturating_add(1)
            .saturating_add(target_text_len(target))
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
    out.push_str(value.format_into(&mut NumBuffer::new()));
}

pub(super) fn push_u64(out: &mut String, value: u64) {
    out.push_str(value.format_into(&mut NumBuffer::new()));
}

fn write_i32_io(out: &mut impl Write, value: i32) -> io::Result<()> {
    out.write_all(value.format_into(&mut NumBuffer::new()).as_bytes())
}

fn write_u64_io(out: &mut impl Write, value: u64) -> io::Result<()> {
    out.write_all(value.format_into(&mut NumBuffer::new()).as_bytes())
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

fn escaped_tag_len(tag: &[u8]) -> usize {
    tag.iter().fold(0usize, |len, byte| {
        len.saturating_add(match byte {
            b']' | b'\r' | b'\n' | b'\\' => 2,
            _ => 1,
        })
    })
}

pub(crate) struct FormattedFloat<const N: usize = FLOAT_BUFFER_BYTES> {
    text: StackText<N>,
}

impl<const N: usize> FormattedFloat<N> {
    pub(crate) fn as_bytes(&self) -> &[u8] {
        self.text.as_bytes()
    }

    pub(crate) fn as_str(&self) -> &str {
        self.text.as_str()
    }

    pub(super) fn len(&self) -> usize {
        self.text.len()
    }
}

/// Stim prints detector-error-model arguments with `long double` `digits10 + 1`
/// significant digits; Stab fixes the 128-bit `long double` width of the pinned
/// AArch64 baseline on every host.
const DEM_SIGNIFICANT_DIGITS: usize = 34;
const DEM_FLOAT_BUFFER_BYTES: usize = 48;

/// How a printer renders finite doubles that hold an integral value.
#[derive(Clone, Copy)]
enum IntegralDoubles {
    /// The circuit printer casts integral doubles to `int64_t`, so `-0` prints as `0`.
    CastToInt64,
    /// The DEM printer streams every double through `%g`, which keeps the sign of `-0`.
    GeneralFormat,
}

/// Formats a circuit argument like Stim's six-significant-digit circuit printer.
pub(crate) fn format_float(value: f64) -> FormattedFloat {
    format_stim_float(value, 6, IntegralDoubles::CastToInt64)
}

/// Formats a detector-error-model argument like Stim's `%.34g` DEM printer.
pub(crate) fn format_dem_float(value: f64) -> FormattedFloat<DEM_FLOAT_BUFFER_BYTES> {
    format_stim_float(
        value,
        DEM_SIGNIFICANT_DIGITS,
        IntegralDoubles::GeneralFormat,
    )
}

#[allow(
    clippy::expect_used,
    reason = "callers pair each significant-digit count with a buffer that fits its longest f64 form"
)]
fn format_stim_float<const N: usize>(
    value: f64,
    significant_digits: usize,
    integral: IntegralDoubles,
) -> FormattedFloat<N> {
    let mut result = StackText::new();
    let integral_shortcut = match integral {
        IntegralDoubles::CastToInt64 => true,
        IntegralDoubles::GeneralFormat => !(value == 0.0 && value.is_sign_negative()),
    };
    if integral_shortcut && let Some(integer) = stim_integer_like_i64(value) {
        result
            .write_str(integer.format_into(&mut NumBuffer::new()))
            .expect("integer fits in float buffer");
        return FormattedFloat { text: result };
    }

    // `%g` chooses fixed or scientific notation from the exponent after rounding
    // to the requested significant digits. One exact scientific conversion yields
    // both the rounded exponent and every significant digit: fixed notation at
    // `significant_digits - 1 - exponent` decimal places rounds at the same digit
    // position, and when rounding carries into a new power of ten both forms
    // print that power of ten.
    let mantissa_digits = significant_digits.saturating_sub(1);
    let mut scientific = StackText::<N>::new();
    write!(&mut scientific, "{value:.mantissa_digits$e}")
        .expect("scientific f64 fits in float buffer");
    let (mantissa, exponent) = scientific
        .as_str()
        .split_once('e')
        .expect("Rust scientific f64 formatting includes an exponent");
    let exponent = exponent
        .parse::<i32>()
        .expect("Rust scientific f64 formatting uses an i32 exponent");

    let fixed_upper = i32::try_from(significant_digits).expect("significant digits fit i32");
    if (-4..fixed_upper).contains(&exponent) {
        write_fixed_from_scientific(&mut result, mantissa, exponent)
            .expect("fixed f64 fits in float buffer");
    } else {
        result
            .write_str(mantissa)
            .expect("mantissa fits in float buffer");
        result.trim_decimal_float();
        write!(&mut result, "e{exponent:+03}").expect("exponent fits in float buffer");
    }
    FormattedFloat { text: result }
}

/// Rewrites a scientific mantissa such as `-1.2345` with decimal exponent `exponent` in
/// `[-4, significant digits)` as trimmed fixed notation, reusing the rounded digits.
fn write_fixed_from_scientific<const N: usize>(
    out: &mut StackText<N>,
    mantissa: &str,
    exponent: i32,
) -> fmt::Result {
    let (negative, unsigned) = match mantissa.as_bytes().split_first() {
        Some((b'-', rest)) => (true, rest),
        _ => (false, mantissa.as_bytes()),
    };
    let (&leading, rest) = unsigned.split_first().ok_or(fmt::Error)?;
    let fraction = match rest.split_first() {
        Some((b'.', fraction)) => fraction,
        Some(_) => return Err(fmt::Error),
        None => &[],
    };
    if negative {
        out.push_ascii(b"-")?;
    }
    if let Ok(integer_tail) = usize::try_from(exponent) {
        // `exponent < significant digits` keeps the decimal point inside the digits.
        let (integer_rest, fraction) = fraction.split_at_checked(integer_tail).ok_or(fmt::Error)?;
        out.push_ascii(&[leading])?;
        out.push_ascii(integer_rest)?;
        if !fraction.is_empty() {
            out.push_ascii(b".")?;
            out.push_ascii(fraction)?;
        }
    } else {
        out.push_ascii(b"0.")?;
        for _ in 1..exponent.unsigned_abs() {
            out.push_ascii(b"0")?;
        }
        out.push_ascii(&[leading])?;
        out.push_ascii(fraction)?;
    }
    out.trim_decimal_float();
    Ok(())
}

/// Memoizes DEM argument text by bit pattern while one model prints.
///
/// Error analysis emits a few dozen distinct probabilities across many lines, and each
/// 34-digit conversion is an exact big-number computation. The table is direct-mapped and
/// fixed-size, so hostile models with many distinct values only miss the cache.
pub(crate) struct DemFloatCache {
    entries: [DemFloatCacheEntry; DEM_FLOAT_CACHE_SLOTS],
    /// Holds the text when a slot lookup misses the table, which the masked index rules out.
    spill: FormattedFloat<DEM_FLOAT_BUFFER_BYTES>,
}

struct DemFloatCacheEntry {
    bits: Option<u64>,
    text: FormattedFloat<DEM_FLOAT_BUFFER_BYTES>,
}

const DEM_FLOAT_CACHE_SLOTS: usize = 64;

impl DemFloatCache {
    pub(crate) const fn new() -> Self {
        Self {
            entries: [const {
                DemFloatCacheEntry {
                    bits: None,
                    text: FormattedFloat {
                        text: StackText::new(),
                    },
                }
            }; DEM_FLOAT_CACHE_SLOTS],
            spill: FormattedFloat {
                text: StackText::new(),
            },
        }
    }

    /// Returns `value` formatted exactly like [`format_dem_float`].
    #[allow(
        clippy::cast_possible_truncation,
        reason = "the hash is shifted down to six bits before narrowing, so no bits are lost"
    )]
    pub(crate) fn format(&mut self, value: f64) -> &[u8] {
        let bits = value.to_bits();
        let slot = ((bits ^ (bits >> 29)).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 58) as usize;
        match self.entries.get_mut(slot % DEM_FLOAT_CACHE_SLOTS) {
            Some(entry) => {
                if entry.bits != Some(bits) {
                    entry.text = format_dem_float(value);
                    entry.bits = Some(bits);
                }
                entry.text.as_bytes()
            }
            None => {
                self.spill = format_dem_float(value);
                self.spill.as_bytes()
            }
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    reason = "the retained two-pass oracle keeps the original formatter's buffer contracts"
)]
fn format_stim_float_two_pass<const N: usize>(
    value: f64,
    significant_digits: usize,
    integral: IntegralDoubles,
) -> FormattedFloat<N> {
    let mut result = StackText::new();
    let integral_shortcut = match integral {
        IntegralDoubles::CastToInt64 => true,
        IntegralDoubles::GeneralFormat => !(value == 0.0 && value.is_sign_negative()),
    };
    if integral_shortcut && let Some(integer) = stim_integer_like_i64(value) {
        write!(&mut result, "{integer}").expect("integer fits in float buffer");
        return FormattedFloat { text: result };
    }

    let mantissa_digits = significant_digits.saturating_sub(1);
    let mut scientific = StackText::<N>::new();
    write!(&mut scientific, "{value:.mantissa_digits$e}")
        .expect("scientific f64 fits in float buffer");
    let (mantissa, exponent) = scientific
        .as_str()
        .split_once('e')
        .expect("Rust scientific f64 formatting includes an exponent");
    let exponent = exponent
        .parse::<i32>()
        .expect("Rust scientific f64 formatting uses an i32 exponent");

    let fixed_upper = i32::try_from(significant_digits).expect("significant digits fit i32");
    if (-4..fixed_upper).contains(&exponent) {
        let decimal_places = usize::try_from(fixed_upper - 1 - exponent)
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

    /// Appends ASCII bytes, which keeps the buffer valid UTF-8.
    fn push_ascii(&mut self, text: &[u8]) -> fmt::Result {
        if !text.is_ascii() {
            return Err(fmt::Error);
        }
        let end = self.len.checked_add(text.len()).ok_or(fmt::Error)?;
        let destination = self.bytes.get_mut(self.len..end).ok_or(fmt::Error)?;
        destination.copy_from_slice(text);
        self.len = end;
        Ok(())
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

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::{
        DEM_FLOAT_BUFFER_BYTES, DEM_SIGNIFICANT_DIGITS, DemFloatCache, IntegralDoubles,
        format_dem_float, format_float, format_stim_float_two_pass,
    };

    fn two_pass_dem(value: f64) -> String {
        format_stim_float_two_pass::<DEM_FLOAT_BUFFER_BYTES>(
            value,
            DEM_SIGNIFICANT_DIGITS,
            IntegralDoubles::GeneralFormat,
        )
        .as_str()
        .to_owned()
    }

    fn two_pass_circuit(value: f64) -> String {
        format_stim_float_two_pass::<32>(value, 6, IntegralDoubles::CastToInt64)
            .as_str()
            .to_owned()
    }

    fn assert_single_pass_matches(value: f64) {
        assert_eq!(
            format_dem_float(value).as_str(),
            two_pass_dem(value),
            "DEM text for {value:e} ({:#018x})",
            value.to_bits()
        );
        assert_eq!(
            format_float(value).as_str(),
            two_pass_circuit(value),
            "circuit text for {value:e} ({:#018x})",
            value.to_bits()
        );
    }

    /// Finite doubles on both sides of every power of ten reachable by `f64`,
    /// where `%g` switches notation or rounding carries into a new decade.
    fn decade_edge_values() -> Vec<f64> {
        let mut values = vec![0.0, -0.0, f64::MIN_POSITIVE, 5e-324, f64::MAX, -f64::MAX];
        for exponent in -323..=308 {
            let Ok(decade) = format!("1e{exponent}").parse::<f64>() else {
                continue;
            };
            for value in [decade, -decade] {
                let bits = value.to_bits();
                for neighbor in [bits.wrapping_sub(2), bits.wrapping_sub(1), bits, bits + 1] {
                    let neighbor = f64::from_bits(neighbor);
                    if neighbor.is_finite() {
                        values.push(neighbor);
                    }
                }
            }
            // Six-digit circuit rounding carries for values within half an
            // ulp-at-six-digits below a decade, such as 9.999995e-5.
            for value in [
                decade * 0.999_999_5,
                decade * 0.999_999_4,
                decade * 0.999_999_6,
            ] {
                if value.is_finite() {
                    values.push(value);
                    values.push(-value);
                }
            }
        }
        values
    }

    #[test]
    fn single_pass_formatter_matches_two_pass_text_at_decade_edges() {
        for value in decade_edge_values() {
            assert_single_pass_matches(value);
        }
    }

    #[test]
    fn dem_float_cache_matches_uncached_text_across_slot_collisions() {
        let mut cache = DemFloatCache::new();
        let values = decade_edge_values();
        for _ in 0..2 {
            for &value in &values {
                assert_eq!(cache.format(value), format_dem_float(value).as_bytes());
            }
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(4096))]

        #[test]
        fn single_pass_formatter_matches_two_pass_text_on_random_bits(bits in any::<u64>()) {
            let value = f64::from_bits(bits);
            prop_assume!(value.is_finite());
            assert_single_pass_matches(value);
        }

        #[test]
        fn single_pass_formatter_matches_two_pass_text_on_probabilities(value in 0.0_f64..=1.0) {
            assert_single_pass_matches(value);
        }
    }
}
