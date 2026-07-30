use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};

use crate::{CircuitError, CircuitInstruction, CircuitResult, DemTarget, Gate};
use stab_analysis::advanced::{
    CircuitErrorLocationView, CircuitTargetsInsideInstructionView, write_explained_error,
};

pub use stab_analysis::{
    CircuitErrorLocationStackFrame, DemTargetWithCoords, FlippedMeasurement, GateTargetWithCoords,
};

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
        let mut delegate = self.to_analysis();
        delegate
            .fill_args_and_targets_in_range(actual_op, qubit_coords)
            .map_err(CircuitError::from)?;
        *self = Self::from_analysis(delegate);
        Ok(())
    }

    fn to_analysis(&self) -> stab_analysis::CircuitTargetsInsideInstruction {
        stab_analysis::CircuitTargetsInsideInstruction {
            gate: self.gate,
            gate_tag: self.gate_tag.clone(),
            args: self.args.clone(),
            target_range_start: self.target_range_start,
            target_range_end: self.target_range_end,
            targets_in_range: self.targets_in_range.clone(),
        }
    }

    fn from_analysis(value: stab_analysis::CircuitTargetsInsideInstruction) -> Self {
        Self {
            gate: value.gate,
            gate_tag: value.gate_tag,
            args: value.args,
            target_range_start: value.target_range_start,
            target_range_end: value.target_range_end,
            targets_in_range: value.targets_in_range,
        }
    }

    fn as_analysis_view(&self) -> CircuitTargetsInsideInstructionView<'_> {
        CircuitTargetsInsideInstructionView::new(
            self.gate,
            self.gate_tag.as_deref(),
            &self.args,
            self.target_range_start,
            self.target_range_end,
            &self.targets_in_range,
        )
    }
}

impl Display for CircuitTargetsInsideInstruction {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.as_analysis_view(), f)
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
        let mut delegate = self.to_analysis();
        delegate.canonicalize();
        *self = Self::from_analysis(delegate);
    }

    pub fn is_simpler_than(&self, other: &Self) -> bool {
        self.as_analysis_view()
            .is_simpler_than(other.as_analysis_view())
    }

    fn to_analysis(&self) -> stab_analysis::CircuitErrorLocation {
        stab_analysis::CircuitErrorLocation {
            noise_tag: self.noise_tag.clone(),
            tick_offset: self.tick_offset,
            flipped_pauli_product: self.flipped_pauli_product.clone(),
            flipped_measurement: self.flipped_measurement.clone(),
            instruction_targets: self.instruction_targets.to_analysis(),
            stack_frames: self.stack_frames.clone(),
        }
    }

    pub(crate) fn from_analysis(value: stab_analysis::CircuitErrorLocation) -> Self {
        Self {
            noise_tag: value.noise_tag,
            tick_offset: value.tick_offset,
            flipped_pauli_product: value.flipped_pauli_product,
            flipped_measurement: value.flipped_measurement,
            instruction_targets: CircuitTargetsInsideInstruction::from_analysis(
                value.instruction_targets,
            ),
            stack_frames: value.stack_frames,
        }
    }

    fn as_analysis_view(&self) -> CircuitErrorLocationView<'_> {
        CircuitErrorLocationView::new(
            self.noise_tag.as_deref(),
            self.tick_offset,
            &self.flipped_pauli_product,
            &self.flipped_measurement,
            self.instruction_targets.as_analysis_view(),
            &self.stack_frames,
        )
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
        Display::fmt(&self.as_analysis_view(), f)
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
        let mut delegate = self.to_analysis();
        delegate.fill_in_dem_targets(targets, dem_coords);
        *self = Self::from_analysis(delegate);
    }

    pub fn canonicalize(&mut self) {
        let mut delegate = self.to_analysis();
        delegate.canonicalize();
        *self = Self::from_analysis(delegate);
    }

    fn to_analysis(&self) -> stab_analysis::ExplainedError {
        stab_analysis::ExplainedError {
            dem_error_terms: self.dem_error_terms.clone(),
            circuit_error_locations: self
                .circuit_error_locations
                .iter()
                .map(CircuitErrorLocation::to_analysis)
                .collect(),
        }
    }

    pub(crate) fn from_analysis(value: stab_analysis::ExplainedError) -> Self {
        Self {
            dem_error_terms: value.dem_error_terms,
            circuit_error_locations: value
                .circuit_error_locations
                .into_iter()
                .map(CircuitErrorLocation::from_analysis)
                .collect(),
        }
    }
}

impl Display for ExplainedError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write_explained_error(
            f,
            &self.dem_error_terms,
            self.circuit_error_locations
                .iter()
                .map(CircuitErrorLocation::as_analysis_view),
        )
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        clippy::unwrap_used,
        reason = "facade tests use direct assertions for compact diagnostics"
    )]

    use super::CircuitTargetsInsideInstruction;
    use crate::{CircuitError, CircuitInstruction, Gate, QubitId, Target};
    use std::collections::BTreeMap;

    #[test]
    fn facade_preserves_fill_error_and_success_contracts() {
        let gate = Gate::from_name("X_ERROR").unwrap();
        let actual = CircuitInstruction::new(
            gate,
            vec![0.125],
            vec![Target::qubit(QubitId::new(4).unwrap(), false)],
            None,
        )
        .unwrap();
        let mut targets = CircuitTargetsInsideInstruction {
            gate: None,
            gate_tag: None,
            args: Vec::new(),
            target_range_start: 0,
            target_range_end: 2,
            targets_in_range: Vec::new(),
        };

        let error = targets
            .fill_args_and_targets_in_range(&actual, &BTreeMap::new())
            .expect_err("out-of-range target selection must fail");
        assert!(matches!(
            error,
            CircuitError::InvalidDetectorErrorModel { .. }
        ));

        targets.target_range_end = 1;
        targets
            .fill_args_and_targets_in_range(&actual, &BTreeMap::from([(4, vec![1.0, 2.0])]))
            .expect("valid target selection");
        assert_eq!(targets.to_string(), "X_ERROR(0.125) 4[coords 1,2]");
    }
}
