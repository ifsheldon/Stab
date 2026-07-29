use crate::{
    Circuit, CircuitResult, Flow,
    sampling::{legacy_random_policy, legacy_shot_count},
};

/// Probabilistically checks signed stabilizer flows by sampling augmented noiseless circuits.
pub fn sample_if_circuit_has_stabilizer_flows(
    circuit: &Circuit,
    flows: &[Flow],
    sample_count: usize,
    seed: Option<u64>,
) -> CircuitResult<Vec<bool>> {
    stab_engine::sample_if_circuit_has_stabilizer_flows(
        circuit,
        flows,
        legacy_shot_count(sample_count)?,
        legacy_random_policy(seed),
    )
    .map_err(Into::into)
}
