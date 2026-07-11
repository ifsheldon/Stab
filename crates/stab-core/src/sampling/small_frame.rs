use rand::{Rng, RngExt as _};

use crate::{MeasureRecordOffset, PauliBasis, SampleFormat, result_formats::MeasureRecordWriter};

use super::measurement_flip;
use super::operation::{
    SINGLE_QUBIT_PAULI_CHANNEL_BASES, SampleOperation, TWO_QUBIT_PAULI_CHANNEL_BASES,
};
use super::stabilizer_frame::reset_correction;
use super::{estimated_sample_bytes_capacity, execute::record_lookback};

pub(super) fn sample_bytes<R>(
    qubit_count: usize,
    measurement_count: usize,
    operations: &[SampleOperation],
    shots: usize,
    format: SampleFormat,
    reference: Option<&[bool]>,
    rng: &mut R,
) -> Option<Vec<u8>>
where
    R: Rng,
{
    if qubit_count > SmallStabilizerFrame::MAX_QUBITS
        || !matches!(format, SampleFormat::ZeroOne | SampleFormat::B8)
        || !supports_operations(operations)
    {
        return None;
    }

    let mut writer = MeasureRecordWriter::with_capacity(
        format,
        estimated_sample_bytes_capacity(format, shots, measurement_count),
    );
    let mut frame = SmallStabilizerFrame::new(qubit_count);
    let mut record = Vec::with_capacity(measurement_count);
    let mut output = Vec::with_capacity(measurement_count);
    for _ in 0..shots {
        frame.reset_to_z_basis();
        record.clear();
        output.clear();
        let mut correlated_error_occurred = false;
        execute_operations(
            operations,
            &mut frame,
            &mut record,
            &mut output,
            &mut correlated_error_occurred,
            rng,
        );
        if let Some(reference) = reference {
            for (bit, reference_bit) in output.iter_mut().zip(reference) {
                *bit ^= *reference_bit;
            }
        }
        writer.write_bits(&output);
        writer.write_end();
    }
    Some(writer.into_bytes())
}

fn supports_operations(operations: &[SampleOperation]) -> bool {
    operations.iter().all(|operation| match operation {
        SampleOperation::ApplyHadamard { .. }
        | SampleOperation::ApplyControlledX { .. }
        | SampleOperation::Reset { .. }
        | SampleOperation::Measure { .. }
        | SampleOperation::MeasureProduct { .. }
        | SampleOperation::Pad { .. }
        | SampleOperation::SingleQubitPauliChannel { .. }
        | SampleOperation::TwoQubitPauliChannel { .. }
        | SampleOperation::CorrelatedError { .. }
        | SampleOperation::HeraldedPauliChannel { .. }
        | SampleOperation::FeedbackPauli { .. } => true,
        SampleOperation::Repeat { body, .. } => supports_operations(body),
        SampleOperation::ApplyTableau { .. } | SampleOperation::SweepPauli { .. } => false,
    })
}

fn execute_operations<R>(
    operations: &[SampleOperation],
    frame: &mut SmallStabilizerFrame,
    record: &mut Vec<bool>,
    output: &mut Vec<bool>,
    correlated_error_occurred: &mut bool,
    rng: &mut R,
) where
    R: Rng,
{
    for operation in operations {
        match operation {
            SampleOperation::ApplyHadamard { qubit } => {
                frame.apply_hadamard(*qubit);
            }
            SampleOperation::ApplyControlledX { control, target } => {
                frame.apply_controlled_x(*control, *target);
            }
            SampleOperation::ApplyTableau { .. } => {
                return;
            }
            SampleOperation::Reset { qubit, basis } => {
                frame.reset(*qubit, *basis, rng);
            }
            SampleOperation::Measure {
                qubit,
                basis,
                inverted,
                flip_probability,
                reset,
            } => {
                let noisy_flip =
                    measurement_flip::sample(*flip_probability, rng, super::ExecutionMode::Sample);
                let physical_result = frame.measure(*qubit, *basis, false, rng);
                let reported_result = physical_result ^ *inverted ^ noisy_flip;
                record.push(reported_result);
                output.push(reported_result);
                if *reset && physical_result {
                    frame.apply_pauli(*qubit, reset_correction(*basis));
                }
            }
            SampleOperation::MeasureProduct {
                terms,
                inverted,
                flip_probability,
            } => {
                let noisy_flip =
                    measurement_flip::sample(*flip_probability, rng, super::ExecutionMode::Sample);
                let result = frame.measure_pauli_product(terms, *inverted ^ noisy_flip, rng);
                record.push(result);
                output.push(result);
            }
            SampleOperation::Pad {
                value,
                flip_probability,
            } => {
                let result = *value
                    ^ measurement_flip::sample(
                        *flip_probability,
                        rng,
                        super::ExecutionMode::Sample,
                    );
                record.push(result);
                output.push(result);
            }
            SampleOperation::SingleQubitPauliChannel {
                qubit,
                probabilities,
                total_probability,
            } => {
                apply_single_qubit_pauli_channel(
                    frame,
                    *qubit,
                    probabilities,
                    *total_probability,
                    rng,
                );
            }
            SampleOperation::TwoQubitPauliChannel {
                left,
                right,
                probabilities,
                total_probability,
            } => {
                apply_two_qubit_pauli_channel(
                    frame,
                    *left,
                    *right,
                    probabilities,
                    *total_probability,
                    rng,
                );
            }
            SampleOperation::CorrelatedError {
                else_branch,
                probability,
                terms,
            } => {
                apply_correlated_error(
                    frame,
                    terms,
                    *probability,
                    *else_branch,
                    correlated_error_occurred,
                    rng,
                );
            }
            SampleOperation::HeraldedPauliChannel {
                qubit,
                probabilities,
            } => {
                let herald = apply_heralded_pauli_channel(frame, *qubit, probabilities, rng);
                record.push(herald);
                output.push(herald);
            }
            SampleOperation::FeedbackPauli {
                offset,
                qubit,
                basis,
            } => {
                if measurement_record_bit(record, *offset) {
                    frame.apply_pauli(*qubit, *basis);
                }
            }
            SampleOperation::SweepPauli { .. } => return,
            SampleOperation::Repeat { count, body } => {
                for _ in 0..*count {
                    execute_operations(body, frame, record, output, correlated_error_occurred, rng);
                }
            }
        }
    }
}

fn measurement_record_bit(record: &[bool], offset: MeasureRecordOffset) -> bool {
    record_lookback(record, offset)
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SmallStabilizerFrame {
    qubit_count: usize,
    row_count: usize,
    xs_by_qubit: Vec<u64>,
    zs_by_qubit: Vec<u64>,
    xs_by_row: Vec<u64>,
    zs_by_row: Vec<u64>,
    rows_valid: bool,
    signs: u64,
}

impl SmallStabilizerFrame {
    const MAX_QUBITS: usize = u64::BITS as usize;

    fn new(qubit_count: usize) -> Self {
        let mut frame = Self {
            qubit_count,
            row_count: qubit_count,
            xs_by_qubit: vec![0; qubit_count],
            zs_by_qubit: vec![0; qubit_count],
            xs_by_row: vec![0; Self::MAX_QUBITS],
            zs_by_row: vec![0; Self::MAX_QUBITS],
            rows_valid: false,
            signs: 0,
        };
        frame.reset_to_z_basis();
        frame
    }

    fn reset_to_z_basis(&mut self) {
        self.row_count = self.qubit_count;
        self.signs = 0;
        self.xs_by_qubit.fill(0);
        self.zs_by_qubit.fill(0);
        self.xs_by_row.fill(0);
        self.zs_by_row.fill(0);
        self.rows_valid = false;
        for (qubit, z_rows) in self.zs_by_qubit.iter_mut().enumerate() {
            *z_rows = row_bit(qubit);
        }
    }

    fn row_mask(&self) -> u64 {
        low_bits(self.row_count)
    }

    fn apply_hadamard(&mut self, qubit: usize) {
        let row_mask = self.row_mask();
        let qubit_bit = qubit_bit(qubit);
        let Some(x_rows) = self.xs_by_qubit.get_mut(qubit) else {
            return;
        };
        let Some(z_rows) = self.zs_by_qubit.get_mut(qubit) else {
            return;
        };
        let old_x_rows = *x_rows;
        let old_z_rows = *z_rows;
        self.signs ^= (*x_rows & *z_rows) & row_mask;
        std::mem::swap(x_rows, z_rows);
        if !self.rows_valid {
            return;
        }
        let mut changed_rows = (old_x_rows ^ old_z_rows) & row_mask;
        while changed_rows != 0 {
            let row = changed_rows.trailing_zeros() as usize;
            self.xor_row_x(row, qubit_bit);
            self.xor_row_z(row, qubit_bit);
            changed_rows &= changed_rows - 1;
        }
    }

    fn apply_controlled_x(&mut self, control: usize, target: usize) {
        let Some(control_x) = self.xs_by_qubit.get(control).copied() else {
            return;
        };
        let Some(control_z) = self.zs_by_qubit.get(control).copied() else {
            return;
        };
        let Some(target_x) = self.xs_by_qubit.get(target).copied() else {
            return;
        };
        let Some(target_z) = self.zs_by_qubit.get(target).copied() else {
            return;
        };
        self.signs ^= (control_x & target_z & !(target_x ^ control_z)) & self.row_mask();
        if let Some(control_z_slot) = self.zs_by_qubit.get_mut(control) {
            *control_z_slot ^= target_z;
        }
        if let Some(target_x_slot) = self.xs_by_qubit.get_mut(target) {
            *target_x_slot ^= control_x;
        }
        if !self.rows_valid {
            return;
        }
        let control_bit = qubit_bit(control);
        let target_bit = qubit_bit(target);
        let mut z_changed_rows = target_z & self.row_mask();
        while z_changed_rows != 0 {
            let row = z_changed_rows.trailing_zeros() as usize;
            self.xor_row_z(row, control_bit);
            z_changed_rows &= z_changed_rows - 1;
        }
        let mut x_changed_rows = control_x & self.row_mask();
        while x_changed_rows != 0 {
            let row = x_changed_rows.trailing_zeros() as usize;
            self.xor_row_x(row, target_bit);
            x_changed_rows &= x_changed_rows - 1;
        }
    }

    fn apply_pauli(&mut self, qubit: usize, basis: PauliBasis) {
        let Some(x_rows) = self.xs_by_qubit.get(qubit).copied() else {
            return;
        };
        let Some(z_rows) = self.zs_by_qubit.get(qubit).copied() else {
            return;
        };
        let anti_rows = match basis {
            PauliBasis::I => 0,
            PauliBasis::X => z_rows,
            PauliBasis::Y => x_rows ^ z_rows,
            PauliBasis::Z => x_rows,
        };
        self.signs ^= anti_rows & self.row_mask();
    }

    fn reset<R>(&mut self, qubit: usize, basis: PauliBasis, rng: &mut R)
    where
        R: Rng,
    {
        let measured = self.measure(qubit, basis, false, rng);
        if measured {
            self.apply_pauli(qubit, reset_correction(basis));
        }
    }

    fn measure<R>(&mut self, qubit: usize, basis: PauliBasis, inverted: bool, rng: &mut R) -> bool
    where
        R: Rng,
    {
        let observable = SmallObservable::single(qubit, basis);
        self.measure_observable(&observable, rng) ^ inverted
    }

    fn measure_pauli_product<R>(
        &mut self,
        terms: &[(usize, PauliBasis)],
        inverted: bool,
        rng: &mut R,
    ) -> bool
    where
        R: Rng,
    {
        let observable = SmallObservable::from_terms(terms);
        self.measure_observable(&observable, rng) ^ inverted
    }

    fn measure_observable<R>(&mut self, observable: &SmallObservable, rng: &mut R) -> bool
    where
        R: Rng,
    {
        if let Some(row) = self.find_same_row(observable) {
            return self.row_negative(row) ^ observable.negative;
        }

        let anti_rows = self.anti_rows(observable);
        if anti_rows == 0
            && let Some(bit) = self.deterministic_span_bit(observable)
        {
            return bit;
        }

        let sampled = rng.random_bool(0.5);
        let collapsed = SmallObservable {
            x: observable.x,
            z: observable.z,
            negative: observable.negative ^ sampled,
        };
        if anti_rows == 0 {
            self.append_row(collapsed);
            return sampled;
        }

        let pivot = anti_rows.trailing_zeros() as usize;
        let pivot_x = self.row_x(pivot);
        let pivot_z = self.row_z(pivot);
        let pivot_negative = self.row_negative(pivot);
        let mut rows = anti_rows & !row_bit(pivot);
        while rows != 0 {
            let row = rows.trailing_zeros() as usize;
            self.multiply_row_assign(row, pivot_x, pivot_z, pivot_negative);
            rows &= rows - 1;
        }
        self.set_row(pivot, collapsed.x, collapsed.z, collapsed.negative);
        sampled
    }

    fn find_same_row(&self, observable: &SmallObservable) -> Option<usize> {
        let mut candidates = self.row_mask();
        for (qubit, (x_rows, z_rows)) in self
            .xs_by_qubit
            .iter()
            .copied()
            .zip(self.zs_by_qubit.iter().copied())
            .enumerate()
        {
            let qubit_bit = qubit_bit(qubit);
            candidates &= if observable.x & qubit_bit == 0 {
                !x_rows
            } else {
                x_rows
            };
            candidates &= if observable.z & qubit_bit == 0 {
                !z_rows
            } else {
                z_rows
            };
            candidates &= self.row_mask();
            if candidates == 0 {
                return None;
            }
        }
        Some(candidates.trailing_zeros() as usize)
    }

    fn anti_rows(&self, observable: &SmallObservable) -> u64 {
        let mut anti_rows = 0;
        let mut z_terms = observable.z;
        while z_terms != 0 {
            let qubit = z_terms.trailing_zeros() as usize;
            anti_rows ^= self.xs_by_qubit.get(qubit).copied().unwrap_or(0);
            z_terms &= z_terms - 1;
        }
        let mut x_terms = observable.x;
        while x_terms != 0 {
            let qubit = x_terms.trailing_zeros() as usize;
            anti_rows ^= self.zs_by_qubit.get(qubit).copied().unwrap_or(0);
            x_terms &= x_terms - 1;
        }
        anti_rows & self.row_mask()
    }

    fn deterministic_span_bit(&mut self, observable: &SmallObservable) -> Option<bool> {
        self.ensure_rows();
        let coefficients = self.solve_span(observable)?;
        let mut product_x = 0;
        let mut product_z = 0;
        let mut product_negative = false;
        let mut rows = coefficients;
        while rows != 0 {
            let row = rows.trailing_zeros() as usize;
            let row_x = self.row_x(row);
            let row_z = self.row_z(row);
            let row_negative = self.row_negative(row);
            let log_i = sign_log_i(product_negative)
                .wrapping_add(sign_log_i(row_negative))
                .wrapping_add(pauli_product_log_i(product_x, product_z, row_x, row_z));
            product_negative = (log_i & 2) != 0;
            product_x ^= row_x;
            product_z ^= row_z;
            rows &= rows - 1;
        }
        Some(product_negative ^ observable.negative)
    }

    fn solve_span(&self, observable: &SmallObservable) -> Option<u64> {
        let width = self.qubit_count.checked_mul(2)?;
        let mut basis = vec![None; width];
        for row in 0..self.row_count {
            let mut span_row = SpanRow::from_frame_row(self, row);
            reduce_span_row(&mut span_row, &basis);
            if let Some(pivot) = span_row.first_one()
                && let Some(slot) = basis.get_mut(pivot)
            {
                *slot = Some(span_row);
            }
        }

        let mut target = SpanRow {
            bits: observable.symplectic_bits(self.qubit_count),
            coefficients: 0,
        };
        for column in 0..width {
            if !target.bit(column) {
                continue;
            }
            let pivot = basis.get(column).and_then(Option::as_ref)?;
            target.xor_assign(pivot);
        }
        if target.bits == 0 {
            Some(target.coefficients)
        } else {
            None
        }
    }

    fn multiply_row_assign(&mut self, row: usize, rhs_x: u64, rhs_z: u64, rhs_negative: bool) {
        let lhs_x = self.row_x(row);
        let lhs_z = self.row_z(row);
        let lhs_negative = self.row_negative(row);
        let log_i = sign_log_i(lhs_negative)
            .wrapping_add(sign_log_i(rhs_negative))
            .wrapping_add(pauli_product_log_i(lhs_x, lhs_z, rhs_x, rhs_z));
        self.set_row(row, lhs_x ^ rhs_x, lhs_z ^ rhs_z, (log_i & 2) != 0);
    }

    fn append_row(&mut self, observable: SmallObservable) {
        if self.row_count >= Self::MAX_QUBITS {
            return;
        }
        let row = self.row_count;
        self.row_count += 1;
        self.set_row(row, observable.x, observable.z, observable.negative);
    }

    fn set_row(&mut self, row: usize, x: u64, z: u64, negative: bool) {
        let row_bit = row_bit(row);
        if self.rows_valid {
            if let Some(x_bits) = self.xs_by_row.get_mut(row) {
                *x_bits = x;
            }
            if let Some(z_bits) = self.zs_by_row.get_mut(row) {
                *z_bits = z;
            }
        }
        for (qubit, (x_rows, z_rows)) in self
            .xs_by_qubit
            .iter_mut()
            .zip(self.zs_by_qubit.iter_mut())
            .enumerate()
        {
            let qubit_bit = qubit_bit(qubit);
            if x & qubit_bit == 0 {
                *x_rows &= !row_bit;
            } else {
                *x_rows |= row_bit;
            }
            if z & qubit_bit == 0 {
                *z_rows &= !row_bit;
            } else {
                *z_rows |= row_bit;
            }
        }
        if negative {
            self.signs |= row_bit;
        } else {
            self.signs &= !row_bit;
        }
    }

    fn row_x(&self, row: usize) -> u64 {
        if self.rows_valid {
            self.xs_by_row.get(row).copied().unwrap_or(0)
        } else {
            self.row_bits_from_columns(row, &self.xs_by_qubit)
        }
    }

    fn row_z(&self, row: usize) -> u64 {
        if self.rows_valid {
            self.zs_by_row.get(row).copied().unwrap_or(0)
        } else {
            self.row_bits_from_columns(row, &self.zs_by_qubit)
        }
    }

    fn ensure_rows(&mut self) {
        if self.rows_valid {
            return;
        }
        self.xs_by_row.fill(0);
        self.zs_by_row.fill(0);
        for row in 0..self.row_count {
            let x_bits = self.row_bits_from_columns(row, &self.xs_by_qubit);
            let z_bits = self.row_bits_from_columns(row, &self.zs_by_qubit);
            if let Some(slot) = self.xs_by_row.get_mut(row) {
                *slot = x_bits;
            }
            if let Some(slot) = self.zs_by_row.get_mut(row) {
                *slot = z_bits;
            }
        }
        self.rows_valid = true;
    }

    fn row_bits_from_columns(&self, row: usize, columns: &[u64]) -> u64 {
        let row_bit = row_bit(row);
        let mut bits = 0;
        for (qubit, column) in columns.iter().copied().enumerate() {
            if column & row_bit != 0 {
                bits |= qubit_bit(qubit);
            }
        }
        bits
    }

    fn xor_row_x(&mut self, row: usize, qubit_bit: u64) {
        if let Some(bits) = self.xs_by_row.get_mut(row) {
            *bits ^= qubit_bit;
        }
    }

    fn xor_row_z(&mut self, row: usize, qubit_bit: u64) {
        if let Some(bits) = self.zs_by_row.get_mut(row) {
            *bits ^= qubit_bit;
        }
    }

    fn row_negative(&self, row: usize) -> bool {
        self.signs & row_bit(row) != 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SmallObservable {
    x: u64,
    z: u64,
    negative: bool,
}

impl SmallObservable {
    fn single(qubit: usize, basis: PauliBasis) -> Self {
        let qubit_bit = qubit_bit(qubit);
        Self {
            x: if basis.x_bit() { qubit_bit } else { 0 },
            z: if basis.z_bit() { qubit_bit } else { 0 },
            negative: false,
        }
    }

    fn from_terms(terms: &[(usize, PauliBasis)]) -> Self {
        let mut observable = Self {
            x: 0,
            z: 0,
            negative: false,
        };
        for (qubit, basis) in terms {
            let qubit_bit = qubit_bit(*qubit);
            let rhs_x = if basis.x_bit() { qubit_bit } else { 0 };
            let rhs_z = if basis.z_bit() { qubit_bit } else { 0 };
            let log_i = sign_log_i(observable.negative).wrapping_add(pauli_product_log_i(
                observable.x,
                observable.z,
                rhs_x,
                rhs_z,
            ));
            observable.negative = (log_i & 2) != 0;
            observable.x ^= rhs_x;
            observable.z ^= rhs_z;
        }
        observable
    }

    fn symplectic_bits(self, qubit_count: usize) -> u128 {
        u128::from(self.x) | (u128::from(self.z) << qubit_count)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SpanRow {
    bits: u128,
    coefficients: u64,
}

impl SpanRow {
    fn from_frame_row(frame: &SmallStabilizerFrame, row: usize) -> Self {
        Self {
            bits: u128::from(frame.row_x(row))
                | (u128::from(frame.row_z(row)) << frame.qubit_count),
            coefficients: row_bit(row),
        }
    }

    fn bit(self, column: usize) -> bool {
        self.bits & (1u128 << column) != 0
    }

    fn first_one(self) -> Option<usize> {
        if self.bits == 0 {
            None
        } else {
            Some(self.bits.trailing_zeros() as usize)
        }
    }

    fn xor_assign(&mut self, rhs: &Self) {
        self.bits ^= rhs.bits;
        self.coefficients ^= rhs.coefficients;
    }
}

fn reduce_span_row(row: &mut SpanRow, basis: &[Option<SpanRow>]) {
    for column in 0..basis.len() {
        if !row.bit(column) {
            continue;
        }
        let Some(pivot) = basis.get(column).and_then(Option::as_ref) else {
            return;
        };
        row.xor_assign(pivot);
    }
}

fn apply_heralded_pauli_channel<R>(
    frame: &mut SmallStabilizerFrame,
    qubit: usize,
    probabilities: &[f64; 4],
    rng: &mut R,
) -> bool
where
    R: Rng,
{
    let [i_probability, x_probability, y_probability, z_probability] = *probabilities;
    let mut sampled_probability = rng.random::<f64>();
    if sampled_probability < i_probability {
        return true;
    }
    sampled_probability -= i_probability;
    for (basis, probability) in [
        (PauliBasis::X, x_probability),
        (PauliBasis::Y, y_probability),
        (PauliBasis::Z, z_probability),
    ] {
        if sampled_probability < probability {
            frame.apply_pauli(qubit, basis);
            return true;
        }
        sampled_probability -= probability;
    }
    false
}

fn apply_single_qubit_pauli_channel<R>(
    frame: &mut SmallStabilizerFrame,
    qubit: usize,
    probabilities: &[f64; 3],
    total_probability: f64,
    rng: &mut R,
) where
    R: Rng,
{
    let mut sampled_probability = rng.random::<f64>();
    if sampled_probability >= total_probability {
        return;
    }
    for (basis, probability) in SINGLE_QUBIT_PAULI_CHANNEL_BASES
        .into_iter()
        .zip(probabilities.iter().copied())
    {
        if sampled_probability < probability {
            frame.apply_pauli(qubit, basis);
            return;
        }
        sampled_probability -= probability;
    }
}

fn apply_two_qubit_pauli_channel<R>(
    frame: &mut SmallStabilizerFrame,
    left: usize,
    right: usize,
    probabilities: &[f64; 15],
    total_probability: f64,
    rng: &mut R,
) where
    R: Rng,
{
    let mut sampled_probability = rng.random::<f64>();
    if sampled_probability >= total_probability {
        return;
    }
    for ((left_basis, right_basis), probability) in TWO_QUBIT_PAULI_CHANNEL_BASES
        .into_iter()
        .zip(probabilities.iter().copied())
    {
        if sampled_probability < probability {
            if let Some(basis) = left_basis {
                frame.apply_pauli(left, basis);
            }
            if let Some(basis) = right_basis {
                frame.apply_pauli(right, basis);
            }
            return;
        }
        sampled_probability -= probability;
    }
}

fn apply_correlated_error<R>(
    frame: &mut SmallStabilizerFrame,
    terms: &[(usize, PauliBasis)],
    probability: f64,
    else_branch: bool,
    correlated_error_occurred: &mut bool,
    rng: &mut R,
) where
    R: Rng,
{
    if else_branch && *correlated_error_occurred {
        return;
    }
    if rng.random::<f64>() < probability {
        for (qubit, basis) in terms {
            frame.apply_pauli(*qubit, *basis);
        }
        *correlated_error_occurred = true;
    } else if !else_branch {
        *correlated_error_occurred = false;
    }
}

fn pauli_product_log_i(left_x: u64, left_z: u64, right_x: u64, right_z: u64) -> u32 {
    let left_x_only = left_x & !left_z;
    let left_y = left_x & left_z;
    let left_z_only = left_z & !left_x;
    let right_x_only = right_x & !right_z;
    let right_y = right_x & right_z;
    let right_z_only = right_z & !right_x;
    let plus_i_pairs =
        (left_x_only & right_y) | (left_y & right_z_only) | (left_z_only & right_x_only);
    let minus_i_pairs =
        (left_x_only & right_z_only) | (left_y & right_x_only) | (left_z_only & right_y);
    let plus_i_count = plus_i_pairs.count_ones();
    let minus_i_count = minus_i_pairs.count_ones();
    plus_i_count + 3 * minus_i_count
}

fn sign_log_i(negative: bool) -> u32 {
    if negative { 2 } else { 0 }
}

fn row_bit(row: usize) -> u64 {
    1u64 << row
}

fn qubit_bit(qubit: usize) -> u64 {
    1u64 << qubit
}

fn low_bits(count: usize) -> u64 {
    if count >= u64::BITS as usize {
        u64::MAX
    } else {
        (1u64 << count) - 1
    }
}
