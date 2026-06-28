use crate::{CircuitError, CircuitResult, Probability};

use super::AnalyzerPauli;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IndependentPauliProbabilities {
    pub(super) x: Probability,
    pub(super) y: Probability,
    pub(super) z: Probability,
}

impl IndependentPauliProbabilities {
    pub fn x(self) -> Probability {
        self.x
    }

    pub fn y(self) -> Probability {
        self.y
    }

    pub fn z(self) -> Probability {
        self.z
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DisjointPauliProbabilities {
    x: Probability,
    y: Probability,
    z: Probability,
}

impl DisjointPauliProbabilities {
    pub fn x(self) -> Probability {
        self.x
    }

    pub fn y(self) -> Probability {
        self.y
    }

    pub fn z(self) -> Probability {
        self.z
    }
}

#[inline]
pub fn independent_to_disjoint_xyz_errors(
    x: Probability,
    y: Probability,
    z: Probability,
) -> CircuitResult<DisjointPauliProbabilities> {
    let [x, y, z] = independent_to_disjoint_xyz_raw(x.get(), y.get(), z.get());
    Ok(DisjointPauliProbabilities {
        x: Probability::from_valid_probability(x),
        y: Probability::from_valid_probability(y),
        z: Probability::from_valid_probability(z),
    })
}

#[inline]
pub fn try_disjoint_to_independent_xyz_errors(
    x: Probability,
    y: Probability,
    z: Probability,
) -> CircuitResult<Option<IndependentPauliProbabilities>> {
    let Some(solution) = solve_disjoint_to_independent_xyz(x.get(), y.get(), z.get(), 50) else {
        return Ok(None);
    };
    let [x, y, z] = solution.probabilities;
    if solution.proven_probability_bounds {
        Ok(Some(IndependentPauliProbabilities {
            x: Probability::from_valid_probability(x),
            y: Probability::from_valid_probability(y),
            z: Probability::from_valid_probability(z),
        }))
    } else {
        Ok(Some(IndependentPauliProbabilities {
            x: Probability::try_new(x)?,
            y: Probability::try_new(y)?,
            z: Probability::try_new(z)?,
        }))
    }
}

#[derive(Clone, Copy, Debug)]
struct XyzSolution {
    probabilities: [f64; 3],
    proven_probability_bounds: bool,
}

#[inline]
fn solve_disjoint_to_independent_xyz(
    x: f64,
    y: f64,
    z: f64,
    max_steps: usize,
) -> Option<XyzSolution> {
    let identity = (1.0 - x - y - z).max(0.0);
    if identity < x {
        let solution = solve_disjoint_to_independent_xyz(identity, z, y, max_steps)?;
        let [out_x, out_y, out_z] = solution.probabilities;
        return Some(XyzSolution {
            probabilities: [1.0 - out_x, out_y, out_z],
            proven_probability_bounds: solution.proven_probability_bounds,
        });
    }
    if identity < y {
        let solution = solve_disjoint_to_independent_xyz(z, identity, x, max_steps)?;
        let [out_x, out_y, out_z] = solution.probabilities;
        return Some(XyzSolution {
            probabilities: [out_x, 1.0 - out_y, out_z],
            proven_probability_bounds: solution.proven_probability_bounds,
        });
    }
    if identity < z {
        let solution = solve_disjoint_to_independent_xyz(y, x, identity, max_steps)?;
        let [out_x, out_y, out_z] = solution.probabilities;
        return Some(XyzSolution {
            probabilities: [out_x, out_y, 1.0 - out_z],
            proven_probability_bounds: solution.proven_probability_bounds,
        });
    }

    if x + z < 0.5 && x + y < 0.5 && y + z < 0.5 {
        let s_xz = (1.0 - 2.0 * x - 2.0 * z).sqrt();
        let s_xy = (1.0 - 2.0 * x - 2.0 * y).sqrt();
        let s_yz = (1.0 - 2.0 * y - 2.0 * z).sqrt();
        let a = 0.5 - 0.5 * s_xz * s_xy / s_yz;
        let b = 0.5 - 0.5 * s_xy * s_yz / s_xz;
        let c = 0.5 - 0.5 * s_xz * s_yz / s_xy;
        if (0.0..=1.0).contains(&a) && (0.0..=1.0).contains(&b) && (0.0..=1.0).contains(&c) {
            return Some(XyzSolution {
                probabilities: [a, b, c],
                proven_probability_bounds: true,
            });
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
        let [x2, y2, z2] = independent_to_disjoint_xyz_raw(a, b, c);
        let dx = x2 - x;
        let dy = y2 - y;
        let dz = z2 - z;
        if dx.abs() + dy.abs() + dz.abs() < 1e-14 {
            return Some(XyzSolution {
                probabilities: [a, b, c],
                proven_probability_bounds: false,
            });
        }

        let da = bc_i - bc;
        let db = ac_i - ac;
        let dc = ab_i - ab;
        a = (a - dx / da).max(0.0);
        b = (b - dy / db).max(0.0);
        c = (c - dz / dc).max(0.0);
    }
    None
}

#[inline]
fn independent_to_disjoint_xyz_raw(x: f64, y: f64, z: f64) -> [f64; 3] {
    let xy = x * y;
    let xz = x * z;
    let yz = y * z;
    let not_x = 1.0 - x;
    let not_y = 1.0 - y;
    let not_z = 1.0 - z;
    let not_xy = not_x * not_y;
    let not_xz = not_x * not_z;
    let not_yz = not_y * not_z;
    [
        x * not_yz + not_x * yz,
        y * not_xz + not_y * xz,
        z * not_xy + not_z * xy,
    ]
}

pub(super) fn depolarize1_independent_channel_probability(
    probability: Probability,
) -> CircuitResult<Probability> {
    if probability.get() > 0.75 {
        return Err(CircuitError::invalid_detector_error_model(
            "cannot analyze over-mixing DEPOLARIZE1 probability above 3/4",
        ));
    }
    Probability::try_new(0.5 - 0.5 * (1.0 - (4.0 * probability.get()) / 3.0).sqrt())
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

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "unit tests use direct unwraps for compact parity assertions"
)]
mod tests {
    use super::*;

    fn prob(value: f64) -> Probability {
        Probability::try_new(value).unwrap()
    }

    fn assert_near(left: f64, right: f64) {
        assert!(
            (left - right).abs() <= 1e-6,
            "{left} differs from {right} by more than 1e-6"
        );
    }

    fn depolarize1_probability_from_independent_channel(probability: Probability) -> f64 {
        let not_flipped = 1.0 - 2.0 * probability.get();
        3.0 / 4.0 * (1.0 - not_flipped * not_flipped)
    }

    fn depolarize2_probability_from_independent_channel(probability: Probability) -> f64 {
        let not_flipped = 1.0 - 2.0 * probability.get();
        let not_flipped_squared = not_flipped * not_flipped;
        let not_flipped_fourth = not_flipped_squared * not_flipped_squared;
        let not_flipped_eighth = not_flipped_fourth * not_flipped_fourth;
        15.0 / 16.0 * (1.0 - not_flipped_eighth)
    }

    #[test]
    fn error_decomp_independent_to_disjoint_xyz_matches_upstream_cases() {
        let cases = [
            (0.5, 0.5, 0.5, 0.25, 0.25, 0.25),
            (0.1, 0.0, 0.0, 0.1, 0.0, 0.0),
            (0.0, 0.2, 0.0, 0.0, 0.2, 0.0),
            (0.0, 0.0, 0.05, 0.0, 0.0, 0.05),
            (0.1, 0.1, 0.0, 0.09, 0.09, 0.01),
        ];

        for (x, y, z, expected_x, expected_y, expected_z) in cases {
            let disjoint = independent_to_disjoint_xyz_errors(prob(x), prob(y), prob(z)).unwrap();
            assert_near(disjoint.x.get(), expected_x);
            assert_near(disjoint.y.get(), expected_y);
            assert_near(disjoint.z.get(), expected_z);
        }
    }

    #[test]
    fn error_decomp_disjoint_to_independent_xyz_matches_upstream_cases() {
        let exact_cases = [
            (0.4, 0.0, 0.0, 0.4, 0.0, 0.0),
            (0.5, 0.0, 0.0, 0.5, 0.0, 0.0),
            (0.6, 0.0, 0.0, 0.6, 0.0, 0.0),
            (0.25, 0.25, 0.25, 0.5, 0.5, 0.5),
            (0.1, 0.0, 0.0, 0.1, 0.0, 0.0),
            (0.0, 0.2, 0.0, 0.0, 0.2, 0.0),
            (0.0, 0.0, 0.05, 0.0, 0.0, 0.05),
            (0.09, 0.09, 0.01, 0.1, 0.1, 0.0),
            (0.18, 0.28, 0.12, 0.3, 0.4, 0.0),
        ];

        for (x, y, z, expected_x, expected_y, expected_z) in exact_cases {
            let independent = try_disjoint_to_independent_xyz_errors(prob(x), prob(y), prob(z))
                .unwrap()
                .unwrap();
            assert_near(independent.x.get(), expected_x);
            assert_near(independent.y.get(), expected_y);
            assert_near(independent.z.get(), expected_z);
        }

        assert!(
            try_disjoint_to_independent_xyz_errors(prob(0.2), prob(0.2), prob(0.0))
                .unwrap()
                .is_none()
        );
        assert!(
            try_disjoint_to_independent_xyz_errors(prob(0.2), prob(0.1), prob(0.0))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn error_decomp_xyz_round_trips_edge_case_grid() {
        let cases = [
            (0.0, 0.0, 0.0),
            (1e-12, 2e-12, 3e-12),
            (0.1, 0.2, 0.3),
            (0.25, 0.25, 0.25),
            (0.499999, 0.000001, 0.000001),
            (1.0, 0.0, 0.0),
            (0.0, 1.0, 0.0),
            (0.0, 0.0, 1.0),
        ];

        for (x, y, z) in cases {
            let disjoint = independent_to_disjoint_xyz_errors(prob(x), prob(y), prob(z)).unwrap();
            let recovered =
                try_disjoint_to_independent_xyz_errors(disjoint.x(), disjoint.y(), disjoint.z())
                    .unwrap()
                    .unwrap();
            assert_near(recovered.x().get(), x);
            assert_near(recovered.y().get(), y);
            assert_near(recovered.z().get(), z);
        }
    }

    #[test]
    fn error_decomp_depolarize1_conversion_matches_upstream_round_trips() {
        for probability in [0.0, 0.01, 0.125, 0.25, 0.75] {
            let independent =
                depolarize1_independent_channel_probability(prob(probability)).unwrap();
            assert_near(
                depolarize1_probability_from_independent_channel(independent),
                probability,
            );
            let disjoint = try_disjoint_to_independent_xyz_errors(
                prob(probability / 3.0),
                prob(probability / 3.0),
                prob(probability / 3.0),
            )
            .unwrap()
            .unwrap();
            assert_near(disjoint.x.get(), independent.get());
            assert_near(disjoint.y.get(), independent.get());
            assert_near(disjoint.z.get(), independent.get());
        }

        assert!(depolarize1_independent_channel_probability(prob(0.750001)).is_err());
    }

    #[test]
    fn error_decomp_depolarize2_conversion_matches_upstream_round_trips() {
        for probability in [0.0, 0.01, 0.125, 0.25, 15.0 / 16.0] {
            let independent =
                depolarize2_independent_channel_probability(prob(probability)).unwrap();
            assert_near(
                depolarize2_probability_from_independent_channel(independent),
                probability,
            );
        }

        assert!(depolarize2_independent_channel_probability(prob(15.0 / 16.0 + 0.000001)).is_err());
    }
}
