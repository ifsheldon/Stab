//! The per-bit append that preceded the table-driven kernels, kept as the test reference.
//!
//! Appending a local Clifford to selected output qubits of every generator row.
//!
//! Appending `G` on targets `t` replaces each row's local Pauli on `t` by its image under `G`
//! and multiplies the row sign by the image sign, because Paulis on disjoint qubits commute and
//! the row's other qubits are unchanged. One- and two-qubit gates use a precomputed table of at
//! most sixteen local images and allocate nothing; wider gates conjugate each gathered local
//! Pauli through the gate's own rows. Either way a gate costs `O(n)` row visits.

use super::action::{apply_words, generator_phases};
use super::{PauliRows, Tableau};
use crate::budget::{StabilizerResource, try_zeroed_words};
use crate::{PauliBasis, PauliPhase, StabilizerError, StabilizerResult};

const WORD_BITS: usize = u64::BITS as usize;

pub(super) fn append_per_bit(
    rows: &mut PauliRows,
    gate: &Tableau,
    targets: &[usize],
) -> StabilizerResult<()> {
    if gate.len() <= 2 {
        append_small(rows, gate, targets)
    } else {
        append_general(rows, gate, targets)
    }
}

#[derive(Clone, Copy)]
struct LocalPauliImage {
    phase: PauliPhase,
    bases: [PauliBasis; 2],
}

impl LocalPauliImage {
    const IDENTITY: Self = Self {
        phase: PauliPhase::Plus,
        bases: [PauliBasis::I; 2],
    };
}

/// The local image table index of `basis`: 0 to 3 for I, X, Y, Z.
fn pauli_xyz(basis: PauliBasis) -> u8 {
    match basis {
        PauliBasis::I => 0,
        PauliBasis::X => 1,
        PauliBasis::Y => 2,
        PauliBasis::Z => 3,
    }
}

fn append_small(rows: &mut PauliRows, gate: &Tableau, targets: &[usize]) -> StabilizerResult<()> {
    let images = local_pauli_images(gate)?;
    let local_count = gate.len();
    for row in 0..rows.row_count() {
        let mut code = 0_usize;
        for (local_index, target) in targets.iter().copied().enumerate() {
            let (x, z) = rows.bits(row, target)?;
            code |= usize::from(pauli_xyz(PauliBasis::from_xz(x, z))) << (2 * local_index);
        }
        let image = images
            .get(code)
            .ok_or(StabilizerError::TableauIndexOutOfRange {
                index: code,
                len: images.len(),
            })?;
        if image.phase.is_negative() {
            rows.flip_sign(row)?;
        }
        for (basis, target) in image.bases.iter().zip(targets).take(local_count) {
            rows.set_bits(row, *target, basis.x_bit(), basis.z_bit())?;
        }
    }
    Ok(())
}

fn local_pauli_images(gate: &Tableau) -> StabilizerResult<[LocalPauliImage; 16]> {
    let image_count = 1_usize << (2 * gate.len());
    let mut images = [LocalPauliImage::IDENTITY; 16];
    for (code, image) in images.iter_mut().enumerate().take(image_count) {
        for index in 0..gate.len() {
            let basis = basis_from_xyz((code >> (2 * index)) & 3);
            let factor = local_generator_image(gate, index, basis)?;
            multiply_local_images(image, factor);
        }
        if image.phase.is_imaginary() {
            return Err(StabilizerError::ImaginaryProduct { phase: image.phase });
        }
    }
    Ok(images)
}

fn local_generator_image(
    gate: &Tableau,
    index: usize,
    basis: PauliBasis,
) -> StabilizerResult<LocalPauliImage> {
    let num_qubits = gate.len();
    match basis {
        PauliBasis::I => Ok(LocalPauliImage::IDENTITY),
        PauliBasis::X => local_image_from_row(gate, index),
        PauliBasis::Z => local_image_from_row(gate, num_qubits + index),
        PauliBasis::Y => {
            let mut image = local_image_from_row(gate, index)?;
            multiply_local_images(&mut image, local_image_from_row(gate, num_qubits + index)?);
            image.phase = image.phase.multiply(PauliPhase::PlusI);
            Ok(image)
        }
    }
}

fn local_image_from_row(gate: &Tableau, row: usize) -> StabilizerResult<LocalPauliImage> {
    let rows = gate.rows();
    let mut image = LocalPauliImage {
        phase: if rows.sign(row)? {
            PauliPhase::Minus
        } else {
            PauliPhase::Plus
        },
        ..LocalPauliImage::IDENTITY
    };
    for (qubit, basis) in image.bases.iter_mut().enumerate().take(gate.len()) {
        let (x, z) = rows.bits(row, qubit)?;
        *basis = PauliBasis::from_xz(x, z);
    }
    Ok(image)
}

fn multiply_local_images(left: &mut LocalPauliImage, right: LocalPauliImage) {
    left.phase = left.phase.multiply(right.phase);
    for (left_basis, right_basis) in left.bases.iter_mut().zip(right.bases) {
        let (basis, phase) = left_basis.multiply(right_basis);
        *left_basis = basis;
        left.phase = left.phase.multiply(phase);
    }
}

fn basis_from_xyz(code: usize) -> PauliBasis {
    match code {
        0 => PauliBasis::I,
        1 => PauliBasis::X,
        2 => PauliBasis::Y,
        _ => PauliBasis::Z,
    }
}

fn append_general(rows: &mut PauliRows, gate: &Tableau, targets: &[usize]) -> StabilizerResult<()> {
    let local_rows = gate.rows();
    let local_half = local_rows.half_words();
    let phases = generator_phases(local_rows)?;
    let mut input = try_zeroed_words(StabilizerResource::Tableau, 2 * local_half)?;
    let mut output = try_zeroed_words(StabilizerResource::Tableau, 2 * local_half)?;
    for row in 0..rows.row_count() {
        input.fill(0);
        {
            let (in_x, in_z) = input.split_at_mut(local_half);
            for (local_index, target) in targets.iter().copied().enumerate() {
                let (x, z) = rows.bits(row, target)?;
                set_bit(in_x, local_index, x)?;
                set_bit(in_z, local_index, z)?;
            }
        }
        let (in_x, in_z) = input.split_at(local_half);
        let negative = apply_words(local_rows, Some(&phases), in_x, in_z, false, &mut output)?;
        let (out_x, out_z) = output.split_at(local_half);
        for (local_index, target) in targets.iter().copied().enumerate() {
            rows.set_bits(
                row,
                target,
                get_bit(out_x, local_index)?,
                get_bit(out_z, local_index)?,
            )?;
        }
        if negative {
            rows.flip_sign(row)?;
        }
    }
    Ok(())
}

fn get_bit(words: &[u64], index: usize) -> StabilizerResult<bool> {
    let word = words
        .get(index / WORD_BITS)
        .ok_or(StabilizerError::TableauIndexOutOfRange {
            index,
            len: words.len() * WORD_BITS,
        })?;
    Ok(word >> (index % WORD_BITS) & 1 != 0)
}

fn set_bit(words: &mut [u64], index: usize, value: bool) -> StabilizerResult<()> {
    let len = words.len() * WORD_BITS;
    let word = words
        .get_mut(index / WORD_BITS)
        .ok_or(StabilizerError::TableauIndexOutOfRange { index, len })?;
    if value {
        *word |= 1_u64 << (index % WORD_BITS);
    }
    Ok(())
}
