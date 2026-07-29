#[cfg(test)]
mod semantic_contract;

#[cfg(test)]
pub(crate) use stab_model::{Gate, GateCategory, GateTargetRule};

#[cfg(test)]
pub(crate) use stab_model::advanced::{validate_gate, validate_gate_targets};
