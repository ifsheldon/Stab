#[cfg(test)]
mod semantic_contract;

#[cfg(test)]
pub(crate) use stab_model::{Gate, GateCategory, GateTargetRule};

pub(crate) use stab_model::advanced::{
    GateUnitaryRows, gate_decomposition, gate_flow_descriptors, gate_unitary_rows,
};

#[cfg(test)]
pub(crate) use stab_model::advanced::{validate_gate, validate_gate_targets};
