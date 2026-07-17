use std::hint::black_box;
use std::sync::atomic::{Ordering, compiler_fence};

use stab_core::{PauliBasis, PauliSign, PauliString, StabilizerResource};

use super::{WorkerError, byte_digest_iter, byte_digest_word_pair, byte_digest_words};

pub(super) const PAULI_MIN_QUBITS: u64 = 1;
pub(super) const PAULI_MAX_QUBITS: u64 = StabilizerResource::PauliQubits.limit() as u64;

const LEFT_SEED: u64 = 0x243f_6a88_85a3_08d3;
const LEFT_STRIDE: u64 = 0x9e37_79b9_7f4a_7c15;
const RIGHT_SEED: u64 = 0x1319_8a2e_0370_7344;
const RIGHT_STRIDE: u64 = 0xbf58_476d_1ce4_e5b9;
const WORKLOAD_MARKER: u64 = 5;
const LEFT_SIGN_ENCODING: u64 = 0;
const RIGHT_SIGN_ENCODING: u64 = 1;

pub(super) struct PauliMultiplyFixture {
    width: u64,
    left: PauliString,
    right: PauliString,
    initial_left_digest: [u64; 4],
    initial_right_digest: [u64; 4],
    phase_checksum: u64,
    pub(super) input_bytes: u64,
    pub(super) input_digest: [u64; 4],
}

impl PauliMultiplyFixture {
    pub(super) fn prepare(work_items: u64) -> Result<Self, WorkerError> {
        let (width, input_bytes) = checked_pauli_shape(work_items)?;
        let host_width = usize::try_from(width).map_err(|_| WorkerError::PauliWidthRange(width))?;
        let left = PauliString::from_bases(
            PauliSign::Plus,
            qualified_bases(LEFT_SEED, LEFT_STRIDE, host_width),
        )?;
        let right = PauliString::from_bases(
            PauliSign::Minus,
            qualified_bases(RIGHT_SEED, RIGHT_STRIDE, host_width),
        )?;
        let initial_left_digest = pauli_planes_digest(&left);
        let initial_right_digest = pauli_planes_digest(&right);
        let input_digest = pauli_input_digest(width, &left, &right);
        let mut fixture = Self {
            width,
            left,
            right,
            initial_left_digest,
            initial_right_digest,
            phase_checksum: 0,
            input_bytes,
            input_digest,
        };

        for _ in 0..2 {
            black_box(&mut fixture.left)
                .right_multiply_in_place_returning_log_i_scalar(black_box(&fixture.right))?;
        }
        if pauli_planes_digest(&fixture.left) != fixture.initial_left_digest {
            return Err(WorkerError::PauliPrimingState);
        }
        if pauli_planes_digest(&fixture.right) != fixture.initial_right_digest {
            return Err(WorkerError::PauliRightChanged);
        }
        Ok(fixture)
    }

    pub(super) fn execute(&mut self, iterations: u64) -> Result<(), WorkerError> {
        for _ in 0..iterations {
            compiler_fence(Ordering::SeqCst);
            let phase = black_box(&mut self.left)
                .right_multiply_in_place_returning_log_i_scalar(black_box(&self.right))?;
            self.phase_checksum = self.phase_checksum.wrapping_add(u64::from(phase));
        }
        black_box(&self.left);
        Ok(())
    }

    pub(super) fn output_digest(
        &self,
        iterations: u64,
        semantic_work: u64,
    ) -> Result<[u64; 4], WorkerError> {
        let final_left_digest = pauli_planes_digest(&self.left);
        let final_right_digest = pauli_planes_digest(&self.right);
        if final_right_digest != self.initial_right_digest {
            return Err(WorkerError::PauliRightChanged);
        }
        Ok(byte_digest_words(&[
            iterations,
            semantic_work,
            self.width,
            WORKLOAD_MARKER,
            self.phase_checksum,
            self.input_digest[0],
            self.input_digest[1],
            self.input_digest[2],
            self.input_digest[3],
            final_left_digest[0],
            final_left_digest[1],
            final_left_digest[2],
            final_left_digest[3],
            final_right_digest[0],
            final_right_digest[1],
            final_right_digest[2],
            final_right_digest[3],
        ]))
    }

    #[cfg(test)]
    pub(super) const fn width(&self) -> u64 {
        self.width
    }

    #[cfg(test)]
    pub(super) const fn left(&self) -> &PauliString {
        &self.left
    }

    #[cfg(test)]
    pub(super) const fn right(&self) -> &PauliString {
        &self.right
    }

    #[cfg(test)]
    pub(super) const fn phase_checksum(&self) -> u64 {
        self.phase_checksum
    }
}

pub(super) fn checked_pauli_shape(work_items: u64) -> Result<(u64, u64), WorkerError> {
    if work_items < PAULI_MIN_QUBITS {
        return Err(WorkerError::PauliWidthMinimum {
            actual: work_items,
            minimum: PAULI_MIN_QUBITS,
        });
    }
    if work_items > PAULI_MAX_QUBITS {
        return Err(WorkerError::PauliWidthLimit {
            actual: work_items,
            maximum: PAULI_MAX_QUBITS,
        });
    }
    usize::try_from(work_items).map_err(|_| WorkerError::PauliWidthRange(work_items))?;
    let input_bytes = work_items
        .div_ceil(64)
        .checked_mul(4 * u64::from(u64::BITS / 8))
        .and_then(|bytes| bytes.checked_add(4 * u64::from(u64::BITS / 8)))
        .ok_or(WorkerError::PauliByteCountOverflow)?;
    Ok((work_items, input_bytes))
}

fn qualified_bases(seed: u64, stride: u64, width: usize) -> impl Iterator<Item = PauliBasis> {
    (0..width).map(move |index| {
        let value = splitmix64(seed.wrapping_add((index as u64).wrapping_mul(stride)));
        PauliBasis::from_xz(value & 1 != 0, value & 2 != 0)
    })
}

fn splitmix64(input: u64) -> u64 {
    let mut value = input.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn pauli_input_digest(width: u64, left: &PauliString, right: &PauliString) -> [u64; 4] {
    let header = [
        width,
        WORKLOAD_MARKER,
        LEFT_SIGN_ENCODING,
        RIGHT_SIGN_ENCODING,
    ];
    byte_digest_iter(
        header
            .iter()
            .chain(left.x_bits())
            .chain(left.z_bits())
            .chain(right.x_bits())
            .chain(right.z_bits())
            .flat_map(|word| word.to_le_bytes()),
    )
}

fn pauli_planes_digest(pauli: &PauliString) -> [u64; 4] {
    byte_digest_word_pair(pauli.x_bits(), pauli.z_bits())
}
