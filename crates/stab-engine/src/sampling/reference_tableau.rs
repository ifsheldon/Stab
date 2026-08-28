use stab_algebra::PauliBasis;
use stab_model::advanced::ClassicalControl;

use super::SamplingExecutionError;
use super::execute::record_lookback;
use super::operation::{SampleOperation, SampleProgram};
use super::stabilizer_frame::{FrameStorageError, StabilizerGenerator};

const MAX_FAST_REFERENCE_OPERATIONS: u128 = 100_000;

pub(super) fn try_compute_reference_sample(
    program: &SampleProgram,
    qubit_count: usize,
    measurement_count: usize,
    expanded_operation_count: u128,
    record: &mut Vec<bool>,
    output: &mut Vec<bool>,
) -> Result<bool, SamplingExecutionError> {
    if expanded_operation_count > MAX_FAST_REFERENCE_OPERATIONS {
        return Ok(false);
    }

    let mut frame = CssReferenceTableau::try_new(qubit_count).map_err(|error| {
        SamplingExecutionError::SessionStorageAllocation {
            message: error.to_string(),
        }
    })?;
    record.clear();
    output.clear();

    let mut cursor = program.cursor();
    while let Some(operation) = cursor.next_operation()? {
        let supported = match operation {
            SampleOperation::ApplyHadamard { qubit } => frame.prepend_hadamard(*qubit),
            SampleOperation::ApplyControlledX { control, target } => {
                frame.prepend_controlled_x(*control, *target)
            }
            SampleOperation::Reset { qubit, basis } => {
                *basis == PauliBasis::Z && frame.reset_z(*qubit)
            }
            SampleOperation::Measure {
                qubit,
                basis,
                inverted,
                flip_probability: _,
                reset,
            } => {
                if *basis != PauliBasis::Z {
                    false
                } else if let Some(measured) = frame.measure_z(*qubit) {
                    let result = measured ^ *inverted;
                    record.push(result);
                    output.push(result);
                    if *reset {
                        frame.reset_collapsed_z(*qubit)
                    } else {
                        true
                    }
                } else {
                    false
                }
            }
            SampleOperation::Pad {
                value,
                flip_probability: _,
            } => {
                record.push(*value);
                output.push(*value);
                true
            }
            SampleOperation::SingleQubitPauliChannel { .. }
            | SampleOperation::TwoQubitPauliChannel { .. }
            | SampleOperation::CorrelatedError { .. } => true,
            SampleOperation::HeraldedPauliChannel { .. } => {
                record.push(false);
                output.push(false);
                true
            }
            SampleOperation::ClassicallyControlledPauli {
                control,
                qubit,
                basis,
            } => {
                let active = match control {
                    ClassicalControl::Record(offset) => record_lookback(record, *offset),
                    ClassicalControl::Sweep(_) => false,
                };
                !active || frame.prepend_pauli(*qubit, *basis)
            }
            SampleOperation::ApplyTableau { .. } | SampleOperation::MeasureProduct { .. } => false,
        };
        if !supported {
            return Ok(false);
        }
    }

    if output.len() != measurement_count || record.len() != measurement_count {
        return Err(SamplingExecutionError::InternalInvariant {
            message: format!(
                "CSS reference tableau produced {} measurements but {measurement_count} were compiled",
                output.len()
            ),
        });
    }
    Ok(true)
}

struct CssReferenceTableau {
    // Rows are the X and Z outputs of the inverse state-preparation tableau. CSS gates can
    // therefore prepend in O(n) row work instead of conjugating every stabilizer generator.
    xs: Vec<StabilizerGenerator>,
    zs: Vec<StabilizerGenerator>,
}

impl CssReferenceTableau {
    fn try_new(qubit_count: usize) -> Result<Self, FrameStorageError> {
        Ok(Self {
            xs: try_identity_rows(qubit_count, PauliBasis::X)?,
            zs: try_identity_rows(qubit_count, PauliBasis::Z)?,
        })
    }

    fn prepend_hadamard(&mut self, qubit: usize) -> bool {
        let Some(x) = self.xs.get_mut(qubit) else {
            return false;
        };
        let Some(z) = self.zs.get_mut(qubit) else {
            return false;
        };
        std::mem::swap(x, z);
        true
    }

    fn prepend_controlled_x(&mut self, control: usize, target: usize) -> bool {
        control != target
            && multiply_distinct(&mut self.zs, target, control)
            && multiply_distinct(&mut self.xs, control, target)
    }

    fn prepend_pauli(&mut self, qubit: usize, basis: PauliBasis) -> bool {
        let Some(x) = self.xs.get_mut(qubit) else {
            return false;
        };
        let Some(z) = self.zs.get_mut(qubit) else {
            return false;
        };
        if basis.z_bit() {
            x.flip_sign();
        }
        if basis.x_bit() {
            z.flip_sign();
        }
        true
    }

    fn measure_z(&mut self, qubit: usize) -> Option<bool> {
        self.collapse_z(qubit).then(|| {
            self.zs
                .get(qubit)
                .is_some_and(StabilizerGenerator::is_negative)
        })
    }

    fn reset_z(&mut self, qubit: usize) -> bool {
        self.collapse_z(qubit) && self.reset_collapsed_z(qubit)
    }

    fn reset_collapsed_z(&mut self, qubit: usize) -> bool {
        let Some(x) = self.xs.get_mut(qubit) else {
            return false;
        };
        let Some(z) = self.zs.get_mut(qubit) else {
            return false;
        };
        x.set_negative(false);
        z.set_negative(false);
        true
    }

    fn collapse_z(&mut self, target: usize) -> bool {
        let Some(target_output) = self.zs.get(target) else {
            return false;
        };
        if !target_output.has_x_terms() {
            return true;
        }
        let Some(pivot) = (0..self.zs.len()).find(|qubit| target_output.basis(*qubit).x_bit())
        else {
            return false;
        };

        // Insert initially inert CNOTs to isolate one anti-commuting inverse output, then
        // rotate it onto Z and choose Stim's deterministic-false reference outcome.
        for qubit in pivot.saturating_add(1)..self.zs.len() {
            let eliminate = self
                .zs
                .get(target)
                .is_some_and(|row| row.basis(qubit).x_bit());
            if eliminate && !self.append_controlled_x(pivot, qubit) {
                return false;
            }
        }
        if !self
            .zs
            .get(target)
            .is_some_and(|row| row.basis(pivot) == PauliBasis::X)
            || !self.append_hadamard(pivot)
        {
            return false;
        }
        if self
            .zs
            .get(target)
            .is_some_and(StabilizerGenerator::is_negative)
        {
            return self.append_pauli(pivot, PauliBasis::X);
        }
        true
    }

    fn append_hadamard(&mut self, qubit: usize) -> bool {
        if qubit >= self.xs.len() || qubit >= self.zs.len() {
            return false;
        }
        for row in self.xs.iter_mut().chain(&mut self.zs) {
            row.apply_hadamard(qubit);
        }
        true
    }

    fn append_controlled_x(&mut self, control: usize, target: usize) -> bool {
        if control == target
            || control >= self.xs.len()
            || target >= self.xs.len()
            || control >= self.zs.len()
            || target >= self.zs.len()
        {
            return false;
        }
        for row in self.xs.iter_mut().chain(&mut self.zs) {
            row.apply_controlled_x(control, target);
        }
        true
    }

    fn append_pauli(&mut self, qubit: usize, basis: PauliBasis) -> bool {
        if qubit >= self.xs.len() || qubit >= self.zs.len() {
            return false;
        }
        for row in self.xs.iter_mut().chain(&mut self.zs) {
            row.apply_pauli(qubit, basis);
        }
        true
    }
}

fn try_identity_rows(
    qubit_count: usize,
    basis: PauliBasis,
) -> Result<Vec<StabilizerGenerator>, FrameStorageError> {
    let mut rows = Vec::new();
    rows.try_reserve_exact(qubit_count)
        .map_err(|_| FrameStorageError::new("CSS reference tableau rows", qubit_count))?;
    for qubit in 0..qubit_count {
        rows.push(StabilizerGenerator::try_single(
            qubit_count,
            qubit,
            basis,
            false,
        )?);
    }
    Ok(rows)
}

fn multiply_distinct(rows: &mut [StabilizerGenerator], left: usize, right: usize) -> bool {
    if left == right || left >= rows.len() || right >= rows.len() {
        return false;
    }
    if left < right {
        let (before, from_right) = rows.split_at_mut(right);
        let Some(left_row) = before.get_mut(left) else {
            return false;
        };
        let Some(right_row) = from_right.first() else {
            return false;
        };
        left_row.multiply_assign(right_row);
    } else {
        let (through_right, from_left) = rows.split_at_mut(left);
        let Some(right_row) = through_right.get(right) else {
            return false;
        };
        let Some(left_row) = from_left.first_mut() else {
            return false;
        };
        left_row.multiply_assign(right_row);
    }
    true
}
