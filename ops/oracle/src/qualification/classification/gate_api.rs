const GATE_API_ROOTS: &[&str] = &[
    "stab_core::gate",
    "stab_core::gateargumentrule",
    "stab_core::gatecategory",
    "stab_core::gatedecomposition",
    "stab_core::gatetargetgroupkind",
    "stab_core::gatetargetrule",
    "stab_model::gate",
    "stab_model::gateargumentrule",
    "stab_model::gatecategory",
    "stab_model::gatedecomposition",
    "stab_model::gatetargetgroupkind",
    "stab_model::gatetargetrule",
];

const ADVANCED_GATE_APIS: &[&str] = &[
    "stab_model::advanced::gate_decomposition",
    "stab_model::advanced::gate_flow_descriptors",
    "stab_model::advanced::gate_unitary_rows",
    "stab_model::advanced::gateunitaryrows",
    "stab_model::advanced::lookup_gate",
    "stab_model::advanced::lookup_simple_plain_gate",
    "stab_model::advanced::plain_cx_gate",
    "stab_model::advanced::plain_detector_gate",
    "stab_model::advanced::plain_h_gate",
    "stab_model::advanced::plain_m_gate",
    "stab_model::advanced::plain_s_gate",
    "stab_model::advanced::plain_tick_gate",
    "stab_model::advanced::validate_gate",
    "stab_model::advanced::validate_gate_targets",
];

pub(super) fn classifies(api_path: &str) -> bool {
    GATE_API_ROOTS
        .iter()
        .chain(ADVANCED_GATE_APIS)
        .any(|prefix| {
            api_path == *prefix
                || api_path
                    .strip_prefix(prefix)
                    .is_some_and(|suffix| suffix.starts_with("::") || suffix.starts_with(" as "))
        })
}
