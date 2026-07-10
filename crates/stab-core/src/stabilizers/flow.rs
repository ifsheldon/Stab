use std::cmp::Ordering;
use std::fmt::{Display, Formatter};
use std::str::FromStr;

use super::{
    FlexPauliString, PauliBasis, PauliPhase, PauliSign, PauliString, StabilizerError,
    StabilizerResult,
};

/// Measurement-record term inside a stabilizer flow.
///
/// Nonnegative values refer to absolute measurement indices, and negative values refer to
/// Stim-style relative `rec[...]` offsets.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct FlowMeasurementIndex(i32);

impl FlowMeasurementIndex {
    /// Creates a flow measurement-record term while preserving its absolute or relative sign.
    pub fn new(value: i32) -> Self {
        Self(value)
    }

    /// Returns the raw absolute or relative flow measurement-record value.
    pub fn get(self) -> i32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct FlowObservableIndex(u32);

impl FlowObservableIndex {
    fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Flow {
    input: PauliString,
    output: PauliString,
    measurements: Vec<FlowMeasurementIndex>,
    observables: Vec<FlowObservableIndex>,
}

impl Flow {
    pub fn new(
        input: PauliString,
        output: PauliString,
        measurements: impl IntoIterator<Item = i32>,
        observables: impl IntoIterator<Item = u32>,
    ) -> Self {
        let mut result = Self {
            input,
            output,
            measurements: measurements
                .into_iter()
                .map(FlowMeasurementIndex::new)
                .collect(),
            observables: observables.into_iter().map(FlowObservableIndex).collect(),
        };
        result.canonicalize();
        result
    }

    pub fn input(&self) -> &PauliString {
        &self.input
    }

    pub fn output(&self) -> &PauliString {
        &self.output
    }

    pub fn measurements(&self) -> impl Iterator<Item = i32> + '_ {
        self.measurements
            .iter()
            .copied()
            .map(FlowMeasurementIndex::get)
    }

    pub fn observables(&self) -> impl Iterator<Item = u32> + '_ {
        self.observables
            .iter()
            .copied()
            .map(FlowObservableIndex::get)
    }

    pub fn multiply(&self, rhs: &Self) -> StabilizerResult<Self> {
        let input_product = self.input.multiply(&rhs.input)?;
        let output_product = self.output.multiply(&rhs.output)?;
        let phase_ratio = output_product
            .phase()
            .multiply(inverse_phase(input_product.phase()));
        if phase_ratio.is_imaginary() {
            return Err(StabilizerError::InvalidFlowProduct {
                left: self.to_string(),
                right: rhs.to_string(),
            });
        }

        let input = unsigned_pauli_from_flex(&input_product);
        let output_sign = phase_ratio.sign();
        let output = unsigned_pauli_from_flex(&output_product).with_sign(output_sign);
        let measurements = self.measurements().chain(rhs.measurements());
        let observables = self.observables().chain(rhs.observables());
        Ok(Self::new(input, output, measurements, observables))
    }

    fn canonicalize(&mut self) {
        xor_sort(&mut self.measurements);
        xor_sort(&mut self.observables);
    }
}

impl Display for Flow {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let use_sparse = should_use_sparse(&self.input) || should_use_sparse(&self.output);
        let mut text = String::new();
        if !write_pauli(&mut text, &self.input, use_sparse) {
            text.push('1');
        }
        text.push_str(" -> ");
        let mut has_output = write_pauli(&mut text, &self.output, use_sparse);
        for measurement in &self.measurements {
            if has_output {
                text.push_str(" xor ");
            }
            has_output = true;
            text.push_str("rec[");
            text.push_str(&measurement.get().to_string());
            text.push(']');
        }
        for observable in &self.observables {
            if has_output {
                text.push_str(" xor ");
            }
            has_output = true;
            text.push_str("obs[");
            text.push_str(&observable.get().to_string());
            text.push(']');
        }
        if !has_output {
            text.push('1');
        }
        f.write_str(&text)
    }
}

impl FromStr for Flow {
    type Err = StabilizerError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let (input_text, output_text) = split_flow_arrow(text)?;
        let (input, input_is_imaginary) = parse_flow_pauli(input_text, text)?;
        let mut tokens = output_text.split_whitespace();
        let first_output = tokens.next().ok_or_else(|| invalid_flow_text(text))?;

        let mut output = PauliString::identity(0);
        let mut output_is_imaginary = false;
        let mut measurements = Vec::new();
        let mut observables = Vec::new();
        let mut first_token = first_output;
        let mut flip_output = false;
        if let Some(rest) = first_token.strip_prefix('-') {
            flip_output = true;
            first_token = rest;
        }
        parse_first_output_term(
            first_token,
            text,
            &mut output,
            &mut output_is_imaginary,
            &mut measurements,
            &mut observables,
        )?;
        if flip_output {
            output = output.with_sign(toggled_sign(output.sign()));
        }

        while let Some(separator) = tokens.next() {
            if separator != "xor" {
                return Err(invalid_flow_text(text));
            }
            let token = tokens.next().ok_or_else(|| invalid_flow_text(text))?;
            parse_classical_output_term(token, text, &mut measurements, &mut observables)?;
        }

        if input_is_imaginary != output_is_imaginary {
            return Err(StabilizerError::AntiHermitianFlow);
        }
        Ok(Self::new(input, output, measurements, observables))
    }
}

impl Ord for Flow {
    fn cmp(&self, other: &Self) -> Ordering {
        compare_paulis(&self.input, &other.input)
            .then_with(|| compare_paulis(&self.output, &other.output))
            .then_with(|| self.measurements.cmp(&other.measurements))
            .then_with(|| self.observables.cmp(&other.observables))
    }
}

impl PartialOrd for Flow {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn split_flow_arrow(text: &str) -> StabilizerResult<(&str, &str)> {
    let mut parts = text.split('>');
    let left = parts.next().ok_or_else(|| invalid_flow_text(text))?;
    let right = parts.next().ok_or_else(|| invalid_flow_text(text))?;
    if parts.next().is_some() || left.is_empty() || !left.ends_with('-') {
        return Err(invalid_flow_text(text));
    }
    let input_text = left
        .strip_suffix('-')
        .ok_or_else(|| invalid_flow_text(text))?
        .trim_end();
    if input_text.is_empty() || right.trim().is_empty() {
        return Err(invalid_flow_text(text));
    }
    Ok((input_text, right))
}

fn parse_first_output_term(
    token: &str,
    original_text: &str,
    output: &mut PauliString,
    output_is_imaginary: &mut bool,
    measurements: &mut Vec<i32>,
    observables: &mut Vec<u32>,
) -> StabilizerResult<()> {
    if token.starts_with('r') || token.starts_with('o') {
        parse_classical_output_term(token, original_text, measurements, observables)
    } else {
        let (parsed_output, is_imaginary) = parse_flow_pauli(token, original_text)?;
        *output = parsed_output;
        *output_is_imaginary = is_imaginary;
        Ok(())
    }
}

fn parse_classical_output_term(
    token: &str,
    original_text: &str,
    measurements: &mut Vec<i32>,
    observables: &mut Vec<u32>,
) -> StabilizerResult<()> {
    if token.starts_with('r') {
        measurements.push(parse_rec_index(token, original_text)?);
        Ok(())
    } else if token.starts_with('o') {
        observables.push(parse_obs_index(token, original_text)?);
        Ok(())
    } else {
        Err(invalid_flow_text(original_text))
    }
}

fn parse_flow_pauli(text: &str, original_text: &str) -> StabilizerResult<(PauliString, bool)> {
    match text {
        "1" | "+1" => Ok((PauliString::identity(0), false)),
        "-1" => Ok((PauliString::from_bases(PauliSign::Minus, []), false)),
        "i" | "+i" => Ok((PauliString::identity(0), true)),
        "-i" => Ok((PauliString::from_bases(PauliSign::Minus, []), true)),
        "" => Err(invalid_flow_text(original_text)),
        _ => {
            let flex = text
                .parse::<FlexPauliString>()
                .map_err(|_| invalid_flow_text(original_text))?;
            Ok((flex.value().clone(), flex.phase().is_imaginary()))
        }
    }
}

fn parse_rec_index(token: &str, original_text: &str) -> StabilizerResult<i32> {
    let body = bracket_body(token, "rec", original_text)?;
    let value = body
        .parse::<i64>()
        .map_err(|_| invalid_flow_text(original_text))?;
    i32::try_from(value).map_err(|_| invalid_flow_text(original_text))
}

fn parse_obs_index(token: &str, original_text: &str) -> StabilizerResult<u32> {
    let body = bracket_body(token, "obs", original_text)?;
    let value = body
        .parse::<i64>()
        .map_err(|_| invalid_flow_text(original_text))?;
    u32::try_from(value).map_err(|_| invalid_flow_text(original_text))
}

fn bracket_body<'a>(
    token: &'a str,
    prefix: &str,
    original_text: &str,
) -> StabilizerResult<&'a str> {
    let body = token
        .strip_prefix(prefix)
        .and_then(|rest| rest.strip_prefix('['))
        .and_then(|rest| rest.strip_suffix(']'))
        .ok_or_else(|| invalid_flow_text(original_text))?;
    if body.is_empty() {
        Err(invalid_flow_text(original_text))
    } else {
        Ok(body)
    }
}

fn invalid_flow_text(text: &str) -> StabilizerError {
    StabilizerError::InvalidFlowText {
        text: text.to_owned(),
    }
}

fn should_use_sparse(pauli: &PauliString) -> bool {
    if pauli.len() > 8 && pauli.weight().saturating_mul(8) <= pauli.len() {
        return !pauli_ends_with_identity(pauli);
    }
    false
}

fn pauli_ends_with_identity(pauli: &PauliString) -> bool {
    !pauli.is_empty() && pauli.get(pauli.len() - 1) == Some(PauliBasis::I)
}

fn write_pauli(text: &mut String, pauli: &PauliString, use_sparse: bool) -> bool {
    if pauli.sign().is_negative() {
        text.push('-');
    }
    if use_sparse {
        write_sparse_pauli_terms(text, pauli)
    } else {
        write_dense_pauli_terms(text, pauli)
    }
}

fn write_sparse_pauli_terms(text: &mut String, pauli: &PauliString) -> bool {
    let mut has_term = false;
    for (index, basis) in pauli.active_terms() {
        if has_term {
            text.push('*');
        }
        text.push(pauli_basis_char(basis));
        text.push_str(&index.to_string());
        has_term = true;
    }
    has_term
}

fn write_dense_pauli_terms(text: &mut String, pauli: &PauliString) -> bool {
    for index in 0..pauli.len() {
        text.push(pauli_basis_char(pauli.get(index).unwrap_or(PauliBasis::I)));
    }
    !pauli.is_empty()
}

fn pauli_basis_char(basis: PauliBasis) -> char {
    match basis {
        PauliBasis::I => '_',
        PauliBasis::X => 'X',
        PauliBasis::Y => 'Y',
        PauliBasis::Z => 'Z',
    }
}

fn toggled_sign(sign: PauliSign) -> PauliSign {
    if sign.is_negative() {
        PauliSign::Plus
    } else {
        PauliSign::Minus
    }
}

fn inverse_phase(phase: PauliPhase) -> PauliPhase {
    match phase {
        PauliPhase::Plus => PauliPhase::Plus,
        PauliPhase::PlusI => PauliPhase::MinusI,
        PauliPhase::Minus => PauliPhase::Minus,
        PauliPhase::MinusI => PauliPhase::PlusI,
    }
}

fn unsigned_pauli_from_flex(flex: &FlexPauliString) -> PauliString {
    let bases = (0..flex.len()).map(|index| flex.get(index).unwrap_or(PauliBasis::I));
    PauliString::from_bases(PauliSign::Plus, bases)
}

fn xor_sort<T: Copy + Ord>(values: &mut Vec<T>) {
    values.sort();
    let mut canonical = Vec::with_capacity(values.len());
    let mut pending = None;
    let mut odd_count = false;
    for value in values.iter().copied() {
        if pending == Some(value) {
            odd_count = !odd_count;
        } else {
            if let Some(previous) = pending
                && odd_count
            {
                canonical.push(previous);
            }
            pending = Some(value);
            odd_count = true;
        }
    }
    if let Some(previous) = pending
        && odd_count
    {
        canonical.push(previous);
    }
    *values = canonical;
}

fn compare_paulis(left: &PauliString, right: &PauliString) -> Ordering {
    compare_pauli_bases(left, right)
        .then_with(|| left.len().cmp(&right.len()))
        .then_with(|| left.sign().is_negative().cmp(&right.sign().is_negative()))
}

fn compare_pauli_bases(left: &PauliString, right: &PauliString) -> Ordering {
    for index in 0..left.len().min(right.len()) {
        let ordering = pauli_basis_rank(left.get(index).unwrap_or(PauliBasis::I))
            .cmp(&pauli_basis_rank(right.get(index).unwrap_or(PauliBasis::I)));
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    Ordering::Equal
}

fn pauli_basis_rank(basis: PauliBasis) -> u8 {
    match basis {
        PauliBasis::I => 0,
        PauliBasis::X => 1,
        PauliBasis::Y => 2,
        PauliBasis::Z => 3,
    }
}
