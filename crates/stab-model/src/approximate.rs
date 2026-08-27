use crate::AbsoluteTolerance;

pub(crate) fn arguments_approx_equal(
    left: &[f64],
    right: &[f64],
    tolerance: AbsoluteTolerance,
) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| (left - right).abs() <= tolerance.get())
}
