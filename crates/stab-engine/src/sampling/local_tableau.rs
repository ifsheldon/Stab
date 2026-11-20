//! Local tableau actions that the resolved program attaches to multi-qubit unitary gates.

use stab_algebra::{PauliBasis, PauliSign, StabilizerError, Tableau};

use super::SamplingCompileError;
use super::reference_simulator::{LocalCliffordInverse, SignedLocalPauli};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LocalTableauTransform {
    target_count: usize,
    outputs: Vec<LocalTableauOutput>,
    inverse: LocalCliffordInverse,
}

impl LocalTableauTransform {
    pub(crate) fn from_tableau(tableau: &Tableau) -> Result<Self, SamplingCompileError> {
        let target_count = tableau.len();
        let output_count = local_basis_count(target_count)?;
        let mut outputs = Vec::with_capacity(output_count);
        for input_index in 0..output_count {
            let input_bases = bases_from_index(input_index, target_count);
            let input =
                stab_algebra::advanced::pauli_from_bases_unbudgeted(PauliSign::Plus, input_bases);
            let output = tableau.apply(&input).map_err(map_stabilizer_error)?;
            let mut output_bases = Vec::with_capacity(target_count);
            for target in 0..target_count {
                let Some(basis) = output.get(target) else {
                    return Err(SamplingCompileError::invalid_circuit(
                        "tableau output length changed while compiling sampler",
                    ));
                };
                output_bases.push(basis);
            }
            outputs.push(LocalTableauOutput {
                negative: output.sign().is_negative(),
                bases: output_bases,
            });
        }
        let inverse = inverse_images(target_count, &outputs)?;
        Ok(Self {
            target_count,
            outputs,
            inverse,
        })
    }

    pub(crate) fn target_count(&self) -> usize {
        self.target_count
    }

    /// The images `G^-1 P G` of each target's X and Z, as consumed by the reference simulator.
    pub(crate) fn inverse(&self) -> &LocalCliffordInverse {
        &self.inverse
    }

    #[cfg(test)]
    pub(super) fn output(&self, index: usize) -> Option<&LocalTableauOutput> {
        self.outputs.get(index)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct LocalTableauOutput {
    pub(super) negative: bool,
    pub(super) bases: Vec<PauliBasis>,
}

/// Inverts the forward conjugation table: if `G Q G^-1 = s P` then `G^-1 P G = s Q`.
fn inverse_images(
    target_count: usize,
    outputs: &[LocalTableauOutput],
) -> Result<LocalCliffordInverse, SamplingCompileError> {
    let mut images = Vec::with_capacity(target_count.saturating_mul(2));
    for target in 0..target_count {
        for wanted in [PauliBasis::X, PauliBasis::Z] {
            let (input_index, output) = outputs
                .iter()
                .enumerate()
                .find(|(_, output)| {
                    output.bases.iter().enumerate().all(|(index, basis)| {
                        *basis
                            == if index == target {
                                wanted
                            } else {
                                PauliBasis::I
                            }
                    })
                })
                .ok_or_else(|| {
                    SamplingCompileError::invalid_circuit(
                        "local tableau transform is not invertible",
                    )
                })?;
            images.push(SignedLocalPauli {
                negative: output.negative,
                bases: bases_from_index(input_index, target_count),
            });
        }
    }
    Ok(LocalCliffordInverse::new(images))
}

fn local_basis_count(target_count: usize) -> Result<usize, SamplingCompileError> {
    let mut count = 1usize;
    for _ in 0..target_count {
        count = count.checked_mul(4).ok_or_else(|| {
            SamplingCompileError::invalid_circuit(
                "local tableau transform has too many target basis states",
            )
        })?;
    }
    Ok(count)
}

fn bases_from_index(mut index: usize, target_count: usize) -> Vec<PauliBasis> {
    let mut bases = Vec::with_capacity(target_count);
    for _ in 0..target_count {
        bases.push(match index % 4 {
            0 => PauliBasis::I,
            1 => PauliBasis::X,
            2 => PauliBasis::Y,
            _ => PauliBasis::Z,
        });
        index /= 4;
    }
    bases
}

fn map_stabilizer_error(error: StabilizerError) -> SamplingCompileError {
    SamplingCompileError::invalid_circuit(error.to_string())
}
