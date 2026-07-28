#[cfg(test)]
mod semantic_contract;

pub(crate) use stab_model::GateTargetGroupKind;

#[cfg(test)]
pub(crate) use stab_model::{Gate, GateCategory, GateTargetRule};

pub(crate) use stab_model::advanced::{
    GateUnitaryRows, gate_decomposition, gate_flow_descriptors, gate_unitary_rows,
    lookup_gate as gate_from_name, lookup_simple_plain_gate, plain_cx_gate, plain_detector_gate,
    plain_h_gate, plain_m_gate, plain_s_gate, plain_tick_gate, validate_gate,
};

#[cfg(test)]
pub(crate) use stab_model::advanced::validate_gate_targets;
