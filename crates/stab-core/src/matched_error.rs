use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};

use crate::{CircuitError, CircuitInstruction, CircuitResult, DemTarget, Gate, Target};

#[derive(Clone, Debug, PartialEq)]
pub struct CircuitErrorLocationStackFrame {
    pub instruction_offset: u64,
    pub iteration_index: u64,
    pub instruction_repetitions_arg: u64,
}

impl Display for CircuitErrorLocationStackFrame {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "CircuitErrorLocationStackFrame{{instruction_offset={}, iteration_index={}, instruction_repetitions_arg={}}}",
            self.instruction_offset, self.iteration_index, self.instruction_repetitions_arg
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GateTargetWithCoords {
    pub gate_target: Target,
    pub coords: Vec<f64>,
}

impl Display for GateTargetWithCoords {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.gate_target)?;
        write_optional_coords(f, &self.coords)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DemTargetWithCoords {
    pub dem_target: DemTarget,
    pub coords: Vec<f64>,
}

impl Display for DemTargetWithCoords {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.dem_target)?;
        write_optional_coords(f, &self.coords)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FlippedMeasurement {
    pub measurement_record_index: Option<u64>,
    pub measured_observable: Vec<GateTargetWithCoords>,
}

impl FlippedMeasurement {
    pub fn none() -> Self {
        Self {
            measurement_record_index: None,
            measured_observable: Vec::new(),
        }
    }
}

impl Display for FlippedMeasurement {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("FlippedMeasurement{")?;
        let Some(index) = self.measurement_record_index else {
            return f.write_str("none}");
        };
        write!(f, "{index}, ")?;
        write_pauli_product(f, &self.measured_observable)?;
        f.write_str("}")
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CircuitTargetsInsideInstruction {
    pub gate: Option<Gate>,
    pub gate_tag: Option<String>,
    pub args: Vec<f64>,
    pub target_range_start: usize,
    pub target_range_end: usize,
    pub targets_in_range: Vec<GateTargetWithCoords>,
}

impl CircuitTargetsInsideInstruction {
    pub fn fill_args_and_targets_in_range(
        &mut self,
        actual_op: &CircuitInstruction,
        qubit_coords: &BTreeMap<u64, Vec<f64>>,
    ) -> CircuitResult<()> {
        let take_len = self
            .target_range_end
            .checked_sub(self.target_range_start)
            .ok_or_else(|| {
                CircuitError::invalid_detector_error_model(
                    "instruction target range end is before start",
                )
            })?;
        if self.target_range_end > actual_op.targets().len() {
            return Err(CircuitError::invalid_detector_error_model(
                "instruction target range is outside the actual instruction",
            ));
        }

        self.gate = Some(actual_op.gate());
        self.gate_tag = actual_op.tag().map(ToOwned::to_owned);
        self.args = actual_op.args().to_vec();
        self.targets_in_range.clear();
        for target in actual_op
            .targets()
            .iter()
            .skip(self.target_range_start)
            .take(take_len)
        {
            let coords = target
                .qubit_id()
                .filter(|_| !target.is_classical_bit_target() && !target.is_combiner())
                .and_then(|qubit| qubit_coords.get(&u64::from(qubit.get())))
                .cloned()
                .unwrap_or_default();
            self.targets_in_range.push(GateTargetWithCoords {
                gate_target: target.clone(),
                coords,
            });
        }
        Ok(())
    }
}

impl Display for CircuitTargetsInsideInstruction {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.gate {
            Some(gate) => f.write_str(gate.canonical_name())?,
            None => f.write_str("null")?,
        }
        if let Some(tag) = self.gate_tag.as_deref().filter(|tag| !tag.is_empty()) {
            f.write_str("[")?;
            write_escaped_tag(f, tag)?;
            f.write_str("]")?;
        }
        write_args(f, &self.args)?;

        let mut was_combiner = false;
        for target in &self.targets_in_range {
            let is_combiner = target.gate_target.is_combiner();
            if !is_combiner && !was_combiner {
                f.write_str(" ")?;
            }
            was_combiner = is_combiner;
            write!(f, "{target}")?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct CircuitErrorLocation {
    pub noise_tag: Option<String>,
    pub tick_offset: u64,
    pub flipped_pauli_product: Vec<GateTargetWithCoords>,
    pub flipped_measurement: FlippedMeasurement,
    pub instruction_targets: CircuitTargetsInsideInstruction,
    pub stack_frames: Vec<CircuitErrorLocationStackFrame>,
}

impl CircuitErrorLocation {
    pub fn canonicalize(&mut self) {
        self.flipped_pauli_product
            .sort_by(compare_gate_targets_with_coords);
        self.flipped_measurement
            .measured_observable
            .sort_by(compare_gate_targets_with_coords);
    }

    pub fn is_simpler_than(&self, other: &Self) -> bool {
        if self.flipped_measurement.measured_observable.len()
            != other.flipped_measurement.measured_observable.len()
        {
            return self.flipped_measurement.measured_observable.len()
                < other.flipped_measurement.measured_observable.len();
        }
        if self.flipped_pauli_product.len() != other.flipped_pauli_product.len() {
            return self.flipped_pauli_product.len() < other.flipped_pauli_product.len();
        }
        compare_circuit_error_locations(self, other) == Ordering::Less
    }
}

impl PartialEq for CircuitErrorLocation {
    fn eq(&self, other: &Self) -> bool {
        self.tick_offset == other.tick_offset
            && self.flipped_pauli_product == other.flipped_pauli_product
            && self.flipped_measurement == other.flipped_measurement
            && self.instruction_targets == other.instruction_targets
            && self.stack_frames == other.stack_frames
    }
}

impl Display for CircuitErrorLocation {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write_circuit_error_location(f, self, "")
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExplainedError {
    pub dem_error_terms: Vec<DemTargetWithCoords>,
    pub circuit_error_locations: Vec<CircuitErrorLocation>,
}

impl ExplainedError {
    pub fn fill_in_dem_targets(
        &mut self,
        targets: &[DemTarget],
        dem_coords: &BTreeMap<u64, Vec<f64>>,
    ) {
        self.dem_error_terms.clear();
        for target in targets {
            let coords = match *target {
                DemTarget::RelativeDetector(detector) => {
                    dem_coords.get(&detector.get()).cloned().unwrap_or_default()
                }
                DemTarget::LogicalObservable(_) | DemTarget::Separator | DemTarget::Numeric(_) => {
                    Vec::new()
                }
            };
            self.dem_error_terms.push(DemTargetWithCoords {
                dem_target: *target,
                coords,
            });
        }
    }

    pub fn canonicalize(&mut self) {
        for location in &mut self.circuit_error_locations {
            location.canonicalize();
        }
        self.dem_error_terms
            .sort_by(compare_dem_targets_with_coords);
        self.circuit_error_locations
            .sort_by(compare_circuit_error_locations);
    }
}

impl Display for ExplainedError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("ExplainedError {\n")?;
        f.write_str("    dem_error_terms: ")?;
        write_joined(f, &self.dem_error_terms, " ")?;
        if self.circuit_error_locations.is_empty() {
            f.write_str("\n    [no single circuit error had these exact symptoms]")?;
        }
        for location in &self.circuit_error_locations {
            f.write_str("\n")?;
            write_circuit_error_location(f, location, "    ")?;
        }
        f.write_str("\n}")
    }
}

fn write_circuit_error_location(
    f: &mut Formatter<'_>,
    location: &CircuitErrorLocation,
    indent: &str,
) -> std::fmt::Result {
    writeln!(f, "{indent}CircuitErrorLocation {{")?;
    if let Some(tag) = location.noise_tag.as_deref().filter(|tag| !tag.is_empty()) {
        writeln!(f, "{indent}    noise_tag: {tag}")?;
    }
    if !location.flipped_pauli_product.is_empty() {
        write!(f, "{indent}    flipped_pauli_product: ")?;
        write_pauli_product(f, &location.flipped_pauli_product)?;
        f.write_str("\n")?;
    }
    if let Some(measurement_index) = location.flipped_measurement.measurement_record_index {
        writeln!(
            f,
            "{indent}    flipped_measurement.measurement_record_index: {measurement_index}"
        )?;
    }
    if !location.flipped_measurement.measured_observable.is_empty() {
        write!(f, "{indent}    flipped_measurement.measured_observable: ")?;
        write_pauli_product(f, &location.flipped_measurement.measured_observable)?;
        f.write_str("\n")?;
    }

    writeln!(f, "{indent}    Circuit location stack trace:")?;
    writeln!(f, "{indent}        (after {} TICKs)", location.tick_offset)?;
    for (index, frame) in location.stack_frames.iter().enumerate() {
        if index > 0 {
            writeln!(
                f,
                "{indent}        after {} completed iterations",
                frame.iteration_index
            )?;
        }
        write!(
            f,
            "{indent}        at instruction #{}",
            frame.instruction_offset + 1
        )?;
        if index + 1 < location.stack_frames.len() {
            write!(f, " (a REPEAT {} block)", frame.instruction_repetitions_arg)?;
        } else {
            match location.instruction_targets.gate {
                Some(gate) => write!(f, " ({})", gate.canonical_name())?,
                None => f.write_str(" (null)")?,
            }
        }
        if index > 0 {
            f.write_str(" in the REPEAT block\n")?;
        } else {
            f.write_str(" in the circuit\n")?;
        }
    }

    if location.instruction_targets.target_range_start + 1
        == location.instruction_targets.target_range_end
    {
        write!(
            f,
            "{indent}        at target #{}",
            location.instruction_targets.target_range_start + 1
        )?;
    } else {
        write!(
            f,
            "{indent}        at targets #{} to #{}",
            location.instruction_targets.target_range_start + 1,
            location.instruction_targets.target_range_end
        )?;
    }
    writeln!(f, " of the instruction")?;
    writeln!(
        f,
        "{indent}        resolving to {}",
        location.instruction_targets
    )?;
    write!(f, "{indent}}}")
}

fn write_pauli_product(f: &mut Formatter<'_>, terms: &[GateTargetWithCoords]) -> std::fmt::Result {
    write_joined(f, terms, "*")
}

fn write_joined<T: Display>(
    f: &mut Formatter<'_>,
    values: &[T],
    separator: &str,
) -> std::fmt::Result {
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            f.write_str(separator)?;
        }
        write!(f, "{value}")?;
    }
    Ok(())
}

fn write_optional_coords(f: &mut Formatter<'_>, coords: &[f64]) -> std::fmt::Result {
    if coords.is_empty() {
        return Ok(());
    }
    f.write_str("[coords ")?;
    for (index, coord) in coords.iter().enumerate() {
        if index > 0 {
            f.write_str(",")?;
        }
        f.write_str(&format_float(*coord))?;
    }
    f.write_str("]")
}

fn write_args(f: &mut Formatter<'_>, args: &[f64]) -> std::fmt::Result {
    if args.is_empty() {
        return Ok(());
    }
    f.write_str("(")?;
    for (index, arg) in args.iter().enumerate() {
        if index > 0 {
            f.write_str(", ")?;
        }
        f.write_str(&format_float(*arg))?;
    }
    f.write_str(")")
}

fn write_escaped_tag(f: &mut Formatter<'_>, tag: &str) -> std::fmt::Result {
    for ch in tag.chars() {
        match ch {
            ']' => f.write_str("\\C")?,
            '\r' => f.write_str("\\r")?,
            '\n' => f.write_str("\\n")?,
            '\\' => f.write_str("\\B")?,
            _ => write!(f, "{ch}")?,
        }
    }
    Ok(())
}

fn compare_gate_targets_with_coords(
    left: &GateTargetWithCoords,
    right: &GateTargetWithCoords,
) -> Ordering {
    compare_target(&left.gate_target, &right.gate_target)
        .then_with(|| compare_f64_slices(&left.coords, &right.coords))
}

fn compare_dem_targets_with_coords(
    left: &DemTargetWithCoords,
    right: &DemTargetWithCoords,
) -> Ordering {
    left.dem_target
        .cmp(&right.dem_target)
        .then_with(|| compare_f64_slices(&left.coords, &right.coords))
}

fn compare_circuit_error_locations(
    left: &CircuitErrorLocation,
    right: &CircuitErrorLocation,
) -> Ordering {
    left.tick_offset
        .cmp(&right.tick_offset)
        .then_with(|| {
            compare_slices_by(
                &left.flipped_pauli_product,
                &right.flipped_pauli_product,
                compare_gate_targets_with_coords,
            )
        })
        .then_with(|| {
            compare_flipped_measurements(&left.flipped_measurement, &right.flipped_measurement)
        })
        .then_with(|| {
            compare_instruction_targets(&left.instruction_targets, &right.instruction_targets)
        })
        .then_with(|| {
            compare_slices_by(
                &left.stack_frames,
                &right.stack_frames,
                compare_stack_frames,
            )
        })
}

fn compare_flipped_measurements(left: &FlippedMeasurement, right: &FlippedMeasurement) -> Ordering {
    left.measurement_record_index
        .cmp(&right.measurement_record_index)
        .then_with(|| {
            compare_slices_by(
                &left.measured_observable,
                &right.measured_observable,
                compare_gate_targets_with_coords,
            )
        })
}

fn compare_instruction_targets(
    left: &CircuitTargetsInsideInstruction,
    right: &CircuitTargetsInsideInstruction,
) -> Ordering {
    left.target_range_start
        .cmp(&right.target_range_start)
        .then_with(|| left.target_range_end.cmp(&right.target_range_end))
        .then_with(|| {
            compare_slices_by(
                &left.targets_in_range,
                &right.targets_in_range,
                compare_gate_targets_with_coords,
            )
        })
        .then_with(|| compare_f64_slices(&left.args, &right.args))
        .then_with(|| compare_optional_gate(left.gate, right.gate))
}

fn compare_stack_frames(
    left: &CircuitErrorLocationStackFrame,
    right: &CircuitErrorLocationStackFrame,
) -> Ordering {
    left.instruction_offset
        .cmp(&right.instruction_offset)
        .then_with(|| left.iteration_index.cmp(&right.iteration_index))
        .then_with(|| {
            left.instruction_repetitions_arg
                .cmp(&right.instruction_repetitions_arg)
        })
}

fn compare_slices_by<T>(left: &[T], right: &[T], compare: impl Fn(&T, &T) -> Ordering) -> Ordering {
    for (left_value, right_value) in left.iter().zip(right) {
        let ordering = compare(left_value, right_value);
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    left.len().cmp(&right.len())
}

fn compare_f64_slices(left: &[f64], right: &[f64]) -> Ordering {
    compare_slices_by(left, right, |a, b| a.total_cmp(b))
}

fn compare_optional_gate(left: Option<Gate>, right: Option<Gate>) -> Ordering {
    match (left, right) {
        (Some(left), Some(right)) => left.canonical_name().cmp(right.canonical_name()),
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn compare_target(left: &Target, right: &Target) -> Ordering {
    left.to_string().cmp(&right.to_string())
}

fn format_float(value: f64) -> String {
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

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        reason = "unit tests use direct assertions for compact diagnostics"
    )]

    use std::collections::BTreeMap;

    use crate::{CircuitInstruction, DemTarget, Gate, MeasureRecordOffset, Pauli, QubitId, Target};

    use super::{
        CircuitErrorLocation, CircuitErrorLocationStackFrame, CircuitTargetsInsideInstruction,
        DemTargetWithCoords, ExplainedError, FlippedMeasurement, GateTargetWithCoords,
    };

    fn q(id: u32) -> Target {
        Target::qubit(QubitId::new(id).unwrap(), false)
    }

    fn x(id: u32) -> Target {
        Target::pauli(Pauli::X, QubitId::new(id).unwrap(), false)
    }

    fn y(id: u32) -> Target {
        Target::pauli(Pauli::Y, QubitId::new(id).unwrap(), false)
    }

    fn z(id: u32) -> Target {
        Target::pauli(Pauli::Z, QubitId::new(id).unwrap(), false)
    }

    fn rec(offset: i32) -> Target {
        Target::measurement_record(MeasureRecordOffset::try_new(offset).unwrap())
    }

    fn gate(name: &str) -> Gate {
        Gate::from_name(name).unwrap()
    }

    fn gate_target(target: Target, coords: &[f64]) -> GateTargetWithCoords {
        GateTargetWithCoords {
            gate_target: target,
            coords: coords.to_vec(),
        }
    }

    fn dem_target(target: DemTarget, coords: &[f64]) -> DemTargetWithCoords {
        DemTargetWithCoords {
            dem_target: target,
            coords: coords.to_vec(),
        }
    }

    fn instruction_targets() -> CircuitTargetsInsideInstruction {
        CircuitTargetsInsideInstruction {
            gate: Some(gate("X_ERROR")),
            gate_tag: None,
            args: vec![0.125],
            target_range_start: 11,
            target_range_end: 17,
            targets_in_range: vec![
                gate_target(q(5), &[1.0, 2.0, 3.0]),
                gate_target(x(6), &[]),
                gate_target(Target::combiner(), &[]),
                gate_target(y(9), &[3.0, 4.0]),
                gate_target(rec(-5), &[]),
            ],
        }
    }

    fn location(tick_offset: u64) -> CircuitErrorLocation {
        CircuitErrorLocation {
            noise_tag: None,
            tick_offset,
            flipped_pauli_product: vec![gate_target(x(3), &[11.0, 12.0]), gate_target(z(5), &[])],
            flipped_measurement: FlippedMeasurement {
                measurement_record_index: Some(5),
                measured_observable: vec![gate_target(x(3), &[]), gate_target(y(4), &[14.0, 15.0])],
            },
            instruction_targets: instruction_targets(),
            stack_frames: vec![
                CircuitErrorLocationStackFrame {
                    instruction_offset: 9,
                    iteration_index: 0,
                    instruction_repetitions_arg: 100,
                },
                CircuitErrorLocationStackFrame {
                    instruction_offset: 13,
                    iteration_index: 15,
                    instruction_repetitions_arg: 0,
                },
            ],
        }
    }

    #[test]
    fn matched_error_dem_target_with_coords_matches_upstream() {
        let value = dem_target(DemTarget::relative_detector(5).unwrap(), &[1.0, 2.0, 3.0]);
        let bare = dem_target(DemTarget::relative_detector(5).unwrap(), &[]);

        assert_eq!(value.to_string(), "D5[coords 1,2,3]");
        assert_eq!(bare.to_string(), "D5");
        assert_eq!(
            bare,
            dem_target(DemTarget::relative_detector(5).unwrap(), &[])
        );
        assert_ne!(value, bare);
        assert_eq!(
            value,
            dem_target(DemTarget::relative_detector(5).unwrap(), &[1.0, 2.0, 3.0])
        );
        assert_ne!(
            value,
            dem_target(DemTarget::relative_detector(5).unwrap(), &[1.0, 2.0, 4.0])
        );
        assert_ne!(
            value,
            dem_target(DemTarget::relative_detector(6).unwrap(), &[1.0, 2.0, 3.0])
        );
    }

    #[test]
    fn matched_error_gate_target_with_coords_matches_upstream() {
        let value = gate_target(q(5), &[1.0, 2.0, 3.0]);
        let bare = gate_target(q(5), &[]);
        let pauli = gate_target(x(5), &[1.0, 2.0, 3.0]);

        assert_eq!(value.to_string(), "5[coords 1,2,3]");
        assert_eq!(bare.to_string(), "5");
        assert_eq!(pauli.to_string(), "X5[coords 1,2,3]");
        assert_eq!(bare, gate_target(q(5), &[]));
        assert_ne!(value, bare);
        assert_eq!(value, gate_target(q(5), &[1.0, 2.0, 3.0]));
        assert_ne!(value, gate_target(q(5), &[1.0, 2.0, 4.0]));
        assert_ne!(value, gate_target(q(6), &[1.0, 2.0, 3.0]));
    }

    #[test]
    fn matched_error_stack_frame_matches_upstream() {
        let frame = CircuitErrorLocationStackFrame {
            instruction_offset: 1,
            iteration_index: 2,
            instruction_repetitions_arg: 3,
        };

        assert_eq!(
            frame.to_string(),
            "CircuitErrorLocationStackFrame{instruction_offset=1, iteration_index=2, instruction_repetitions_arg=3}"
        );
        assert_eq!(
            frame,
            CircuitErrorLocationStackFrame {
                instruction_offset: 1,
                iteration_index: 2,
                instruction_repetitions_arg: 3,
            }
        );
        assert_ne!(
            frame,
            CircuitErrorLocationStackFrame {
                instruction_offset: 2,
                iteration_index: 2,
                instruction_repetitions_arg: 3,
            }
        );
        assert_ne!(
            frame,
            CircuitErrorLocationStackFrame {
                instruction_offset: 1,
                iteration_index: 9,
                instruction_repetitions_arg: 3,
            }
        );
        assert_ne!(
            frame,
            CircuitErrorLocationStackFrame {
                instruction_offset: 1,
                iteration_index: 2,
                instruction_repetitions_arg: 9,
            }
        );
    }

    #[test]
    fn matched_error_instruction_targets_match_upstream() {
        let targets = instruction_targets();
        assert_eq!(
            targets.to_string(),
            "X_ERROR(0.125) 5[coords 1,2,3] X6*Y9[coords 3,4] rec[-5]"
        );

        let mut changed = targets.clone();
        changed.target_range_start += 1;
        assert_ne!(targets, changed);
    }

    #[test]
    fn matched_error_instruction_targets_fill_matches_upstream() {
        let mut not_filled = CircuitTargetsInsideInstruction {
            gate: Some(gate("X_ERROR")),
            gate_tag: None,
            args: vec![0.125],
            target_range_start: 2,
            target_range_end: 5,
            targets_in_range: Vec::new(),
        };
        let actual_op =
            CircuitInstruction::new(gate("X_ERROR"), vec![0.125], (0..10).map(q).collect(), None)
                .unwrap();
        let qubit_coords = BTreeMap::from([(4, vec![11.0, 13.0])]);

        not_filled
            .fill_args_and_targets_in_range(&actual_op, &qubit_coords)
            .unwrap();
        assert_eq!(not_filled.to_string(), "X_ERROR(0.125) 2 3 4[coords 11,13]");
    }

    #[test]
    fn matched_error_circuit_error_location_matches_upstream() {
        let loc = location(6);
        assert_eq!(
            loc.to_string(),
            "\
CircuitErrorLocation {
    flipped_pauli_product: X3[coords 11,12]*Z5
    flipped_measurement.measurement_record_index: 5
    flipped_measurement.measured_observable: X3*Y4[coords 14,15]
    Circuit location stack trace:
        (after 6 TICKs)
        at instruction #10 (a REPEAT 100 block) in the circuit
        after 15 completed iterations
        at instruction #14 (X_ERROR) in the REPEAT block
        at targets #12 to #17 of the instruction
        resolving to X_ERROR(0.125) 5[coords 1,2,3] X6*Y9[coords 3,4] rec[-5]
}"
        );
        assert_eq!(loc, loc);
        let changed = location(7);
        assert_ne!(loc, changed);
    }

    #[test]
    fn matched_error_explained_error_matches_upstream() {
        let loc = location(6);
        let loc2 = location(7);
        let err = ExplainedError {
            dem_error_terms: vec![
                dem_target(DemTarget::relative_detector(5).unwrap(), &[1.0, 2.0]),
                dem_target(DemTarget::logical_observable(5).unwrap(), &[]),
            ],
            circuit_error_locations: vec![loc.clone(), loc2.clone()],
        };
        let err2 = ExplainedError {
            dem_error_terms: vec![dem_target(
                DemTarget::relative_detector(5).unwrap(),
                &[1.0, 2.0],
            )],
            circuit_error_locations: vec![loc2, loc],
        };

        assert_eq!(err, err);
        assert_ne!(err, err2);
        assert_eq!(
            err.to_string(),
            "\
ExplainedError {
    dem_error_terms: D5[coords 1,2] L5
    CircuitErrorLocation {
        flipped_pauli_product: X3[coords 11,12]*Z5
        flipped_measurement.measurement_record_index: 5
        flipped_measurement.measured_observable: X3*Y4[coords 14,15]
        Circuit location stack trace:
            (after 6 TICKs)
            at instruction #10 (a REPEAT 100 block) in the circuit
            after 15 completed iterations
            at instruction #14 (X_ERROR) in the REPEAT block
            at targets #12 to #17 of the instruction
            resolving to X_ERROR(0.125) 5[coords 1,2,3] X6*Y9[coords 3,4] rec[-5]
    }
    CircuitErrorLocation {
        flipped_pauli_product: X3[coords 11,12]*Z5
        flipped_measurement.measurement_record_index: 5
        flipped_measurement.measured_observable: X3*Y4[coords 14,15]
        Circuit location stack trace:
            (after 7 TICKs)
            at instruction #10 (a REPEAT 100 block) in the circuit
            after 15 completed iterations
            at instruction #14 (X_ERROR) in the REPEAT block
            at targets #12 to #17 of the instruction
            resolving to X_ERROR(0.125) 5[coords 1,2,3] X6*Y9[coords 3,4] rec[-5]
    }
}"
        );
    }

    #[test]
    fn matched_error_explained_error_fill_matches_upstream() {
        let mut err = ExplainedError {
            dem_error_terms: Vec::new(),
            circuit_error_locations: Vec::new(),
        };
        err.fill_in_dem_targets(
            &[
                DemTarget::relative_detector(5).unwrap(),
                DemTarget::relative_detector(6).unwrap(),
            ],
            &BTreeMap::from([(5, vec![11.0, 13.0])]),
        );
        assert_eq!(
            err.to_string(),
            "\
ExplainedError {
    dem_error_terms: D5[coords 11,13] D6
    [no single circuit error had these exact symptoms]
}"
        );
    }
}
