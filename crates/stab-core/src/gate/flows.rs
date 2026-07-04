use std::str::FromStr;

use crate::{CircuitError, CircuitResult, Flow};

pub(crate) fn gate_flow_metadata(name: &str) -> Option<CircuitResult<Vec<Flow>>> {
    let texts = match name {
        "M" => &["Z -> rec[-1]", "Z -> Z"][..],
        "MX" => &["X -> rec[-1]", "X -> X"],
        "MY" => &["Y -> rec[-1]", "Y -> Y"],
        "R" => &["1 -> Z"],
        "RX" => &["1 -> X"],
        "RY" => &["1 -> Y"],
        "MR" => &["Z -> rec[-1]", "1 -> Z"],
        "MRX" => &["X -> rec[-1]", "1 -> X"],
        "MRY" => &["Y -> rec[-1]", "1 -> Y"],
        "MXX" => &["X_ -> X_", "_X -> _X", "ZZ -> ZZ", "XX -> rec[-1]"],
        "MYY" => &["XX -> XX", "Y_ -> Y_", "_Y -> _Y", "YY -> rec[-1]"],
        "MZZ" => &["XX -> XX", "Z_ -> Z_", "_Z -> _Z", "ZZ -> rec[-1]"],
        "MPP" => &[
            "XYZ__ -> rec[-2]",
            "___XX -> rec[-1]",
            "X____ -> X____",
            "_Y___ -> _Y___",
            "__Z__ -> __Z__",
            "___X_ -> ___X_",
            "____X -> ____X",
            "ZZ___ -> ZZ___",
            "_XX__ -> _XX__",
            "___ZZ -> ___ZZ",
        ],
        "SPP" => &[
            "X__ -> X__",
            "Z__ -> -YYZ",
            "_X_ -> -XZZ",
            "_Z_ -> XXZ",
            "__X -> XYY",
            "__Z -> __Z",
        ],
        "SPP_DAG" => &[
            "X__ -> X__",
            "Z__ -> YYZ",
            "_X_ -> XZZ",
            "_Z_ -> -XXZ",
            "__X -> -XYY",
            "__Z -> __Z",
        ],
        _ => return None,
    };
    Some(
        texts
            .iter()
            .map(|text| {
                Flow::from_str(text).map_err(|error| {
                    CircuitError::invalid_tableau_conversion(format!(
                        "gate {name} flow metadata is invalid: {error}"
                    ))
                })
            })
            .collect(),
    )
}

pub(crate) fn gate_has_flow_metadata(name: &str) -> bool {
    matches!(
        name,
        "M" | "MX"
            | "MY"
            | "R"
            | "RX"
            | "RY"
            | "MR"
            | "MRX"
            | "MRY"
            | "MXX"
            | "MYY"
            | "MZZ"
            | "MPP"
            | "SPP"
            | "SPP_DAG"
    )
}
