use crate::{Circuit, CircuitResult, Probability};

pub use stab_analysis::{ColorCodeTask, RepetitionCodeTask, SurfaceCodeTask};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CodeDistance(stab_analysis::CodeDistance);

impl CodeDistance {
    pub fn try_new(value: u32) -> CircuitResult<Self> {
        stab_analysis::CodeDistance::try_new(value)
            .map(Self)
            .map_err(Into::into)
    }

    pub fn get(self) -> u32 {
        self.0.get()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RoundCount(stab_analysis::RoundCount);

impl RoundCount {
    pub fn try_new(value: u64) -> CircuitResult<Self> {
        stab_analysis::RoundCount::try_new(value)
            .map(Self)
            .map_err(Into::into)
    }

    pub fn get(self) -> u64 {
        self.0.get()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RepetitionCodeParams(stab_analysis::RepetitionCodeParams);

impl RepetitionCodeParams {
    pub fn new(
        rounds: RoundCount,
        distance: CodeDistance,
        task: RepetitionCodeTask,
    ) -> CircuitResult<Self> {
        stab_analysis::RepetitionCodeParams::new(rounds.0, distance.0, task)
            .map(Self)
            .map_err(Into::into)
    }

    pub fn rounds(&self) -> RoundCount {
        RoundCount(self.0.rounds())
    }

    pub fn distance(&self) -> CodeDistance {
        CodeDistance(self.0.distance())
    }

    pub fn task(&self) -> RepetitionCodeTask {
        self.0.task()
    }

    pub fn before_round_data_depolarization(&self) -> Probability {
        self.0.before_round_data_depolarization()
    }

    pub fn before_measure_flip_probability(&self) -> Probability {
        self.0.before_measure_flip_probability()
    }

    pub fn after_reset_flip_probability(&self) -> Probability {
        self.0.after_reset_flip_probability()
    }

    pub fn after_clifford_depolarization(&self) -> Probability {
        self.0.after_clifford_depolarization()
    }

    pub fn with_before_round_data_depolarization(mut self, value: Probability) -> Self {
        self.0 = self.0.with_before_round_data_depolarization(value);
        self
    }

    pub fn with_before_measure_flip_probability(mut self, value: Probability) -> Self {
        self.0 = self.0.with_before_measure_flip_probability(value);
        self
    }

    pub fn with_after_reset_flip_probability(mut self, value: Probability) -> Self {
        self.0 = self.0.with_after_reset_flip_probability(value);
        self
    }

    pub fn with_after_clifford_depolarization(mut self, value: Probability) -> Self {
        self.0 = self.0.with_after_clifford_depolarization(value);
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SurfaceCodeParams(stab_analysis::SurfaceCodeParams);

impl SurfaceCodeParams {
    pub fn new(
        rounds: RoundCount,
        distance: CodeDistance,
        task: SurfaceCodeTask,
    ) -> CircuitResult<Self> {
        stab_analysis::SurfaceCodeParams::new(rounds.0, distance.0, task)
            .map(Self)
            .map_err(Into::into)
    }

    pub fn rounds(&self) -> RoundCount {
        RoundCount(self.0.rounds())
    }

    pub fn distance(&self) -> CodeDistance {
        CodeDistance(self.0.distance())
    }

    pub fn task(&self) -> SurfaceCodeTask {
        self.0.task()
    }

    pub fn before_round_data_depolarization(&self) -> Probability {
        self.0.before_round_data_depolarization()
    }

    pub fn before_measure_flip_probability(&self) -> Probability {
        self.0.before_measure_flip_probability()
    }

    pub fn after_reset_flip_probability(&self) -> Probability {
        self.0.after_reset_flip_probability()
    }

    pub fn after_clifford_depolarization(&self) -> Probability {
        self.0.after_clifford_depolarization()
    }

    pub fn with_before_round_data_depolarization(mut self, value: Probability) -> Self {
        self.0 = self.0.with_before_round_data_depolarization(value);
        self
    }

    pub fn with_before_measure_flip_probability(mut self, value: Probability) -> Self {
        self.0 = self.0.with_before_measure_flip_probability(value);
        self
    }

    pub fn with_after_reset_flip_probability(mut self, value: Probability) -> Self {
        self.0 = self.0.with_after_reset_flip_probability(value);
        self
    }

    pub fn with_after_clifford_depolarization(mut self, value: Probability) -> Self {
        self.0 = self.0.with_after_clifford_depolarization(value);
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ColorCodeParams(stab_analysis::ColorCodeParams);

impl ColorCodeParams {
    pub fn new(
        rounds: RoundCount,
        distance: CodeDistance,
        task: ColorCodeTask,
    ) -> CircuitResult<Self> {
        stab_analysis::ColorCodeParams::new(rounds.0, distance.0, task)
            .map(Self)
            .map_err(Into::into)
    }

    pub fn rounds(&self) -> RoundCount {
        RoundCount(self.0.rounds())
    }

    pub fn distance(&self) -> CodeDistance {
        CodeDistance(self.0.distance())
    }

    pub fn task(&self) -> ColorCodeTask {
        self.0.task()
    }

    pub fn before_round_data_depolarization(&self) -> Probability {
        self.0.before_round_data_depolarization()
    }

    pub fn before_measure_flip_probability(&self) -> Probability {
        self.0.before_measure_flip_probability()
    }

    pub fn after_reset_flip_probability(&self) -> Probability {
        self.0.after_reset_flip_probability()
    }

    pub fn after_clifford_depolarization(&self) -> Probability {
        self.0.after_clifford_depolarization()
    }

    pub fn with_before_round_data_depolarization(mut self, value: Probability) -> Self {
        self.0 = self.0.with_before_round_data_depolarization(value);
        self
    }

    pub fn with_before_measure_flip_probability(mut self, value: Probability) -> Self {
        self.0 = self.0.with_before_measure_flip_probability(value);
        self
    }

    pub fn with_after_reset_flip_probability(mut self, value: Probability) -> Self {
        self.0 = self.0.with_after_reset_flip_probability(value);
        self
    }

    pub fn with_after_clifford_depolarization(mut self, value: Probability) -> Self {
        self.0 = self.0.with_after_clifford_depolarization(value);
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GeneratedCircuit(stab_analysis::GeneratedCircuit);

impl GeneratedCircuit {
    pub fn circuit(&self) -> &Circuit {
        self.0.circuit()
    }

    pub fn layout_text(&self) -> &str {
        self.0.layout_text()
    }

    pub fn hint_text(&self) -> &'static str {
        self.0.hint_text()
    }
}

/// Generates Stim-compatible repetition-code memory circuits for the M7 generator subset.
///
/// Returns an error before materialization if the projected circuit exceeds the generator's
/// 131,072-physical-qubit resource limit. Every valid repetition-code distance fits this limit.
pub fn generate_repetition_code_circuit(
    params: &RepetitionCodeParams,
) -> CircuitResult<GeneratedCircuit> {
    stab_analysis::generate_repetition_code_circuit(&params.0)
        .map(GeneratedCircuit)
        .map_err(Into::into)
}

/// Generates Stim-compatible rotated and unrotated surface-code memory circuits.
///
/// Returns an error before materialization if the projected circuit exceeds the generator's
/// 131,072-physical-qubit resource limit. This admits rotated distances through 256 and unrotated
/// distances through 181.
pub fn generate_surface_code_circuit(
    params: &SurfaceCodeParams,
) -> CircuitResult<GeneratedCircuit> {
    stab_analysis::generate_surface_code_circuit(&params.0)
        .map(GeneratedCircuit)
        .map_err(Into::into)
}

/// Generates Stim-compatible triangular color-code memory circuits.
///
/// Returns an error before materialization if the projected circuit exceeds the generator's
/// 131,072-physical-qubit resource limit. This admits valid odd distances through 341.
pub fn generate_color_code_circuit(params: &ColorCodeParams) -> CircuitResult<GeneratedCircuit> {
    stab_analysis::generate_color_code_circuit(&params.0)
        .map(GeneratedCircuit)
        .map_err(Into::into)
}
