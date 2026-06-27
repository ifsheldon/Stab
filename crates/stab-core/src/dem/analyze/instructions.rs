use super::AnalyzerBasis;

pub(super) fn shifted_coordinates(offset: &[f64], local: &[f64]) -> Vec<f64> {
    local
        .iter()
        .copied()
        .enumerate()
        .map(|(index, value)| offset.get(index).copied().unwrap_or(0.0) + value)
        .collect()
}

pub(super) fn measurement_basis(name: &str) -> Option<AnalyzerBasis> {
    match name {
        "M" | "MR" => Some(AnalyzerBasis::Z),
        "MX" | "MRX" => Some(AnalyzerBasis::X),
        "MY" | "MRY" => Some(AnalyzerBasis::Y),
        _ => None,
    }
}

pub(super) fn pair_measurement_basis(name: &str) -> Option<AnalyzerBasis> {
    match name {
        "MXX" => Some(AnalyzerBasis::X),
        "MYY" => Some(AnalyzerBasis::Y),
        "MZZ" => Some(AnalyzerBasis::Z),
        _ => None,
    }
}

pub(super) fn is_measurement_instruction(name: &str) -> bool {
    matches!(
        name,
        "MXX" | "MYY" | "MZZ" | "MPP" | "HERALDED_PAULI_CHANNEL_1"
    )
}

pub(super) fn is_noise_instruction(name: &str) -> bool {
    matches!(
        name,
        "DEPOLARIZE1"
            | "DEPOLARIZE2"
            | "I_ERROR"
            | "II_ERROR"
            | "PAULI_CHANNEL_1"
            | "PAULI_CHANNEL_2"
            | "ELSE_CORRELATED_ERROR"
            | "E"
    )
}
