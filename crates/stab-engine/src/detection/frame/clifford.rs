//! Compile-time Pauli-frame actions of unitary Clifford gates and their word-parallel kernels.
//!
//! A Pauli frame ignores signs, so a unitary Clifford acts on it as an invertible GF(2) linear
//! map: a 2x2 matrix on `(x, z)` for one qubit and a 4x4 symplectic matrix on
//! `(x1, z1, x2, z2)` for two qubits. The map is derived once per gate by composing the pinned
//! Stim `H`/`S`/`CX` decomposition of the gate, which needs no allocation and so keeps program
//! admission allocation-free, and is executed as whole-word XORs that update every lane at once.
//! The tableau-based differential tests check each derived map against the gate's tableau.

use arrayvec::ArrayVec;
use stab_model::{Gate, GateTargetGroupKind};

use super::BitPlaneDetectionFrame;
use super::helpers::frame_qubit_out_of_range;
use super::word::FrameWord;
use crate::detection::error::{DetectionError, DetectionResult};

/// The frame action of a single-qubit Clifford other than the identity.
///
/// The invertible 2x2 GF(2) matrices form a group of six elements, so these five variants and
/// the identity cover every single-qubit Clifford exactly.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SingleQubitFrameMap {
    /// `X <-> Z`, as for `H`: `x' = z`, `z' = x`.
    SwapXz,
    /// `X -> Y`, `Z -> Z`, as for `S`: `z' = z ^ x`.
    XorXIntoZ,
    /// `X -> X`, `Z -> Y`, as for `SQRT_X`: `x' = x ^ z`.
    XorZIntoX,
    /// `X -> Y -> Z -> X`, as for `C_XYZ`: `x' = x ^ z`, `z' = x`.
    CycleXyz,
    /// `X -> Z -> Y -> X`, as for `C_ZYX`: `x' = z`, `z' = x ^ z`.
    CycleXzy,
}

impl SingleQubitFrameMap {
    /// Classifies the map whose input planes `x, z` have the output-plane images `x_image` and
    /// `z_image`, with bit 0 for `x` and bit 1 for `z`.
    ///
    /// Returns `Ok(None)` for the identity.
    fn from_columns(x_image: u8, z_image: u8) -> DetectionResult<Option<Self>> {
        const X: u8 = 0b01;
        const Z: u8 = 0b10;
        const Y: u8 = 0b11;
        match (x_image, z_image) {
            (X, Z) => Ok(None),
            (Z, X) => Ok(Some(Self::SwapXz)),
            (Y, Z) => Ok(Some(Self::XorXIntoZ)),
            (X, Y) => Ok(Some(Self::XorZIntoX)),
            (Y, X) => Ok(Some(Self::CycleXyz)),
            (Z, Y) => Ok(Some(Self::CycleXzy)),
            _ => Err(DetectionError::invalid_sampler_compilation(format!(
                "single-qubit frame map with columns {x_image:#04b}, {z_image:#04b} is not invertible"
            ))),
        }
    }
}

/// Inputs and outputs of a two-qubit frame map, in the order `x1, z1, x2, z2`.
const TWO_QUBIT_PLANES: usize = 4;

/// A 4x4 GF(2) matrix stored by column: bit `k` of `columns[i]` is set when input plane `i`
/// contributes to output plane `k`, with planes ordered `x1, z1, x2, z2`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TwoQubitFrameMatrix {
    columns: [u8; TWO_QUBIT_PLANES],
}

impl TwoQubitFrameMatrix {
    const IDENTITY: Self = Self::from_columns([0b0001, 0b0010, 0b0100, 0b1000]);
    const CONTROLLED_X: Self = Self::from_columns([0b0101, 0b0010, 0b0100, 0b1010]);
    const CONTROLLED_Z: Self = Self::from_columns([0b1001, 0b0010, 0b0110, 0b1000]);
    const SWAP: Self = Self::from_columns([0b0100, 0b1000, 0b0001, 0b0010]);

    const fn from_columns(columns: [u8; TWO_QUBIT_PLANES]) -> Self {
        Self { columns }
    }

    #[inline(always)]
    fn masks<W: FrameWord>(self) -> [[W; TWO_QUBIT_PLANES]; TWO_QUBIT_PLANES] {
        self.columns.map(|column| {
            std::array::from_fn(|output| {
                if (column >> output) & 1 == 0 {
                    W::default()
                } else {
                    !W::default()
                }
            })
        })
    }
}

/// The frame action of a two-qubit Clifford other than the identity.
///
/// The common gates get dedicated kernels; every other symplectic matrix runs through the
/// branch-free general kernel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TwoQubitFrameMap {
    ControlledX,
    ControlledZ,
    Swap,
    General(TwoQubitFrameMatrix),
}

impl TwoQubitFrameMap {
    fn from_matrix(matrix: TwoQubitFrameMatrix) -> Option<Self> {
        match matrix {
            TwoQubitFrameMatrix::IDENTITY => None,
            TwoQubitFrameMatrix::CONTROLLED_X => Some(Self::ControlledX),
            TwoQubitFrameMatrix::CONTROLLED_Z => Some(Self::ControlledZ),
            TwoQubitFrameMatrix::SWAP => Some(Self::Swap),
            general => Some(Self::General(general)),
        }
    }
}

/// The frame action of a unitary gate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum FrameCliffordAction {
    /// The gate cannot change a Pauli frame, as for Pauli gates and identities.
    Identity,
    Single(SingleQubitFrameMap),
    Two(TwoQubitFrameMap),
}

impl FrameCliffordAction {
    /// Derives the frame action of a fixed-arity unitary gate, or `Ok(None)` for any other gate.
    pub(super) fn for_gate(gate: Gate) -> DetectionResult<Option<Self>> {
        let arity = match gate.target_group_kind() {
            GateTargetGroupKind::Singles => 1,
            GateTargetGroupKind::Pairs => 2,
            _ => return Ok(None),
        };
        if !gate.is_unitary() {
            return Ok(None);
        }
        let Some(decomposition) = stab_model::advanced::gate_decomposition(gate) else {
            return Ok(None);
        };
        let mut columns = TwoQubitFrameMatrix::IDENTITY.columns;
        for line in decomposition.as_stim_str().lines() {
            let mut tokens = line
                .split('#')
                .next()
                .unwrap_or_default()
                .split_whitespace();
            let Some(name) = tokens.next() else {
                continue;
            };
            let mut qubits = ArrayVec::<usize, 2>::new();
            for token in tokens {
                let qubit = token
                    .parse::<usize>()
                    .ok()
                    .filter(|qubit| *qubit < arity)
                    .ok_or_else(|| decomposition_error(gate))?;
                qubits
                    .try_push(qubit)
                    .map_err(|_| decomposition_error(gate))?;
            }
            let step: fn(u8, &[usize]) -> Option<u8> = match name {
                "H" => |column, qubits| {
                    qubits.iter().try_fold(column, |column, &qubit| {
                        let x = (column >> (2 * qubit)) & 1;
                        let z = (column >> (2 * qubit + 1)) & 1;
                        Some(column ^ ((x ^ z) * (0b11 << (2 * qubit))))
                    })
                },
                "S" => |column, qubits| {
                    qubits.iter().try_fold(column, |column, &qubit| {
                        Some(column ^ (((column >> (2 * qubit)) & 1) << (2 * qubit + 1)))
                    })
                },
                "CX" | "CNOT" => |column, qubits| {
                    let (pairs, unpaired) = qubits.as_chunks::<2>();
                    if !unpaired.is_empty() {
                        return None;
                    }
                    pairs.iter().try_fold(column, |column, &[control, target]| {
                        let control_x = (column >> (2 * control)) & 1;
                        let target_z = (column >> (2 * target + 1)) & 1;
                        Some(column ^ (control_x << (2 * target)) ^ (target_z << (2 * control + 1)))
                    })
                },
                _ => return Err(decomposition_error(gate)),
            };
            for column in &mut columns {
                *column = step(*column, &qubits).ok_or_else(|| decomposition_error(gate))?;
            }
        }
        if arity == 1 {
            let [x_image, z_image, ..] = columns;
            return Ok(Some(
                SingleQubitFrameMap::from_columns(x_image, z_image)?
                    .map_or(Self::Identity, Self::Single),
            ));
        }
        Ok(Some(
            TwoQubitFrameMap::from_matrix(TwoQubitFrameMatrix::from_columns(columns))
                .map_or(Self::Identity, Self::Two),
        ))
    }
}

fn decomposition_error(gate: Gate) -> DetectionError {
    DetectionError::invalid_sampler_compilation(format!(
        "gate {} has no H/S/CX decomposition usable for detector-frame lowering",
        gate.canonical_name()
    ))
}

/// Memoizes [`FrameCliffordAction::for_gate`] across one program compilation, so a circuit
/// derives each gate's frame map once rather than once per instruction.
#[derive(Default)]
pub(super) struct FrameCliffordActionCache {
    entries: ArrayVec<(Gate, Option<FrameCliffordAction>), 64>,
}

impl FrameCliffordActionCache {
    pub(super) fn action(&mut self, gate: Gate) -> DetectionResult<Option<FrameCliffordAction>> {
        if let Some((_, action)) = self.entries.iter().find(|(cached, _)| *cached == gate) {
            return Ok(*action);
        }
        let action = FrameCliffordAction::for_gate(gate)?;
        // A full cache only costs a recomputation; it never changes the derived action.
        let _ = self.entries.try_push((gate, action));
        Ok(action)
    }
}

impl<W: FrameWord> BitPlaneDetectionFrame<W> {
    pub(super) fn apply_single_qubit_map(
        &mut self,
        map: SingleQubitFrameMap,
        targets: &[u32],
    ) -> DetectionResult<()> {
        match map {
            SingleQubitFrameMap::SwapXz => self.for_each_qubit(targets, |x, z| (z, x)),
            SingleQubitFrameMap::XorXIntoZ => self.for_each_qubit(targets, |x, z| (x, z ^ x)),
            SingleQubitFrameMap::XorZIntoX => self.for_each_qubit(targets, |x, z| (x ^ z, z)),
            SingleQubitFrameMap::CycleXyz => self.for_each_qubit(targets, |x, z| (x ^ z, x)),
            SingleQubitFrameMap::CycleXzy => self.for_each_qubit(targets, |x, z| (z, x ^ z)),
        }
    }

    #[inline(always)]
    fn for_each_qubit(
        &mut self,
        targets: &[u32],
        kernel: impl Fn(W, W) -> (W, W),
    ) -> DetectionResult<()> {
        for &qubit in targets {
            let qubit = qubit as usize;
            let (Some(x), Some(z)) = (self.xs.get_mut(qubit), self.zs.get_mut(qubit)) else {
                return Err(frame_qubit_out_of_range(qubit));
            };
            (*x, *z) = kernel(*x, *z);
        }
        Ok(())
    }

    pub(super) fn apply_two_qubit_map(
        &mut self,
        map: TwoQubitFrameMap,
        targets: &[u32],
    ) -> DetectionResult<()> {
        let (pairs, unpaired) = targets.as_chunks::<2>();
        if !unpaired.is_empty() {
            return Err(DetectionError::invalid_sampler_compilation(
                "compiled two-qubit Clifford target list was not paired",
            ));
        }
        match map {
            TwoQubitFrameMap::ControlledX => {
                for &[control, target] in pairs {
                    let (control, target) = (control as usize, target as usize);
                    let control_x = self.x_word(control)?;
                    let target_z = self.z_word(target)?;
                    self.xor_x_word(target, control_x)?;
                    self.xor_z_word(control, target_z)?;
                }
                Ok(())
            }
            TwoQubitFrameMap::ControlledZ => {
                for &[left, right] in pairs {
                    let (left, right) = (left as usize, right as usize);
                    let left_x = self.x_word(left)?;
                    let right_x = self.x_word(right)?;
                    self.xor_z_word(left, right_x)?;
                    self.xor_z_word(right, left_x)?;
                }
                Ok(())
            }
            TwoQubitFrameMap::Swap => {
                self.for_each_pair(pairs, |[x1, z1, x2, z2]| [x2, z2, x1, z1])
            }
            TwoQubitFrameMap::General(matrix) => {
                let masks = matrix.masks::<W>();
                self.for_each_pair(pairs, |inputs| {
                    let mut outputs = [W::default(); TWO_QUBIT_PLANES];
                    for (input, column) in inputs.into_iter().zip(masks) {
                        for (output, mask) in outputs.iter_mut().zip(column) {
                            *output ^= input & mask;
                        }
                    }
                    outputs
                })
            }
        }
    }

    /// Reads both qubits' planes before writing any output, then writes the first qubit's
    /// planes before the second's.
    #[inline(always)]
    fn for_each_pair(
        &mut self,
        pairs: &[[u32; 2]],
        kernel: impl Fn([W; TWO_QUBIT_PLANES]) -> [W; TWO_QUBIT_PLANES],
    ) -> DetectionResult<()> {
        for &[left, right] in pairs {
            let (left, right) = (left as usize, right as usize);
            let inputs = [
                self.x_word(left)?,
                self.z_word(left)?,
                self.x_word(right)?,
                self.z_word(right)?,
            ];
            let [left_x, left_z, right_x, right_z] = kernel(inputs);
            self.set_x_word(left, left_x)?;
            self.set_z_word(left, left_z)?;
            self.set_x_word(right, right_x)?;
            self.set_z_word(right, right_z)?;
        }
        Ok(())
    }
}
