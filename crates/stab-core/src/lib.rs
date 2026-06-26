#![feature(portable_simd)]

//! Core circuit, detector error model, and simulator primitives for Stab.

pub mod bits;
mod circuit;
mod error;
mod gate;
mod ids;
pub mod stabilizers;
mod target;

pub use bits::{BitBlock, BitError, BitLen, BitMatrix, BitResult, BitSlice, BitVec, SparseXorVec};
pub use circuit::{Circuit, CircuitInstruction, CircuitItem, RepeatBlock};
pub use error::{CircuitError, CircuitResult};
pub use gate::{Gate, GateCategory};
pub use ids::{MeasureRecordOffset, ObservableId, Probability, QubitId, RepeatCount};
pub use stabilizers::{
    CliffordString, FlexPauliString, PauliBasis, PauliPhase, PauliSign, PauliString,
    SingleQubitClifford, StabilizerError, StabilizerResult, Tableau,
};
pub use target::{Pauli, Target};
