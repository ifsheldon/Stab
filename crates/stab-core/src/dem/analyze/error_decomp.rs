use crate::{CircuitError, CircuitResult, Probability};

use super::AnalyzerPauli;

#[derive(Clone, Copy, Debug)]
pub(super) struct IndependentPauliProbabilities {
    pub(super) x: Probability,
    pub(super) y: Probability,
    pub(super) z: Probability,
}

pub(super) fn try_disjoint_to_independent_xyz_errors(
    x: Probability,
    y: Probability,
    z: Probability,
) -> CircuitResult<Option<IndependentPauliProbabilities>> {
    let Some([x, y, z]) = solve_disjoint_to_independent_xyz(x.get(), y.get(), z.get(), 50) else {
        return Ok(None);
    };
    Ok(Some(IndependentPauliProbabilities {
        x: Probability::try_new(x)?,
        y: Probability::try_new(y)?,
        z: Probability::try_new(z)?,
    }))
}

fn solve_disjoint_to_independent_xyz(x: f64, y: f64, z: f64, max_steps: usize) -> Option<[f64; 3]> {
    let identity = (1.0 - x - y - z).max(0.0);
    if identity < x {
        let [out_x, out_y, out_z] = solve_disjoint_to_independent_xyz(identity, z, y, max_steps)?;
        return Some([1.0 - out_x, out_y, out_z]);
    }
    if identity < y {
        let [out_x, out_y, out_z] = solve_disjoint_to_independent_xyz(z, identity, x, max_steps)?;
        return Some([out_x, 1.0 - out_y, out_z]);
    }
    if identity < z {
        let [out_x, out_y, out_z] = solve_disjoint_to_independent_xyz(y, x, identity, max_steps)?;
        return Some([out_x, out_y, 1.0 - out_z]);
    }

    if x + z < 0.5 && x + y < 0.5 && y + z < 0.5 {
        let s_xz = (1.0 - 2.0 * x - 2.0 * z).sqrt();
        let s_xy = (1.0 - 2.0 * x - 2.0 * y).sqrt();
        let s_yz = (1.0 - 2.0 * y - 2.0 * z).sqrt();
        let a = 0.5 - 0.5 * s_xz * s_xy / s_yz;
        let b = 0.5 - 0.5 * s_xy * s_yz / s_xz;
        let c = 0.5 - 0.5 * s_xz * s_yz / s_xy;
        if a >= 0.0 && b >= 0.0 && c >= 0.0 {
            return Some([a, b, c]);
        }
    }

    let mut a = x;
    let mut b = y;
    let mut c = z;
    for _ in 0..max_steps {
        let ab = a * b;
        let ac = a * c;
        let bc = b * c;
        let a_i = 1.0 - a;
        let b_i = 1.0 - b;
        let c_i = 1.0 - c;
        let ab_i = a_i * b_i;
        let ac_i = a_i * c_i;
        let bc_i = b_i * c_i;
        let x2 = a * bc_i + a_i * bc;
        let y2 = b * ac_i + b_i * ac;
        let z2 = c * ab_i + c_i * ab;
        let dx = x2 - x;
        let dy = y2 - y;
        let dz = z2 - z;
        if dx.abs() + dy.abs() + dz.abs() < 1e-14 {
            return Some([a, b, c]);
        }

        let da = bc_i - bc;
        let db = ac_i - ac;
        let dc = ab_i - ac;
        a = (a - dx / da).max(0.0);
        b = (b - dy / db).max(0.0);
        c = (c - dz / dc).max(0.0);
    }
    None
}

pub(super) fn depolarize2_independent_channel_probability(
    probability: Probability,
) -> CircuitResult<Probability> {
    if probability.get() > 15.0 / 16.0 {
        return Err(CircuitError::invalid_detector_error_model(
            "cannot analyze over-mixing DEPOLARIZE2 probability above 15/16",
        ));
    }
    Probability::try_new(0.5 - 0.5 * (1.0 - (16.0 * probability.get()) / 15.0).powf(0.125))
}

pub(super) fn pauli_channel2_components(
    probabilities: [Probability; 15],
) -> impl Iterator<Item = (Probability, Option<AnalyzerPauli>, Option<AnalyzerPauli>)> {
    const COMPONENTS: [(Option<AnalyzerPauli>, Option<AnalyzerPauli>); 15] = [
        (None, Some(AnalyzerPauli::X)),
        (None, Some(AnalyzerPauli::Y)),
        (None, Some(AnalyzerPauli::Z)),
        (Some(AnalyzerPauli::X), None),
        (Some(AnalyzerPauli::X), Some(AnalyzerPauli::X)),
        (Some(AnalyzerPauli::X), Some(AnalyzerPauli::Y)),
        (Some(AnalyzerPauli::X), Some(AnalyzerPauli::Z)),
        (Some(AnalyzerPauli::Y), None),
        (Some(AnalyzerPauli::Y), Some(AnalyzerPauli::X)),
        (Some(AnalyzerPauli::Y), Some(AnalyzerPauli::Y)),
        (Some(AnalyzerPauli::Y), Some(AnalyzerPauli::Z)),
        (Some(AnalyzerPauli::Z), None),
        (Some(AnalyzerPauli::Z), Some(AnalyzerPauli::X)),
        (Some(AnalyzerPauli::Z), Some(AnalyzerPauli::Y)),
        (Some(AnalyzerPauli::Z), Some(AnalyzerPauli::Z)),
    ];
    probabilities
        .into_iter()
        .zip(COMPONENTS)
        .map(|(probability, (left, right))| (probability, left, right))
}
