#![feature(portable_simd)]

//! Core circuit, detector error model, and simulator primitives for Stab.

pub mod bits;
mod circuit;
mod circuit_flow;
mod circuit_generation;
mod circuit_inverse;
mod circuit_simplify;
mod circuit_tableau;
mod error;
mod gate;
mod ids;
mod mbqc_decomposition;
pub mod stabilizers;
mod target;

pub use bits::{BitBlock, BitError, BitLen, BitMatrix, BitResult, BitSlice, BitVec, SparseXorVec};
pub use circuit::{Circuit, CircuitInstruction, CircuitItem, RepeatBlock};
pub use circuit_flow::{check_if_circuit_has_unsigned_stabilizer_flows, circuit_flow_generators};
pub use circuit_generation::{
    CodeDistance, ColorCodeParams, ColorCodeTask, GeneratedCircuit, RepetitionCodeParams,
    RepetitionCodeTask, RoundCount, SurfaceCodeParams, SurfaceCodeTask,
    generate_color_code_circuit, generate_repetition_code_circuit, generate_surface_code_circuit,
};
pub use circuit_inverse::{circuit_inverse_qec, circuit_inverse_unitary};
pub use circuit_simplify::simplified_circuit;
pub use circuit_tableau::circuit_to_tableau;
pub use error::{CircuitError, CircuitResult};
pub use gate::{Gate, GateCategory};
pub use ids::{MeasureRecordOffset, ObservableId, Probability, QubitId, RepeatCount};
pub use mbqc_decomposition::mbqc_decomposition;
pub use stabilizers::{
    CliffordString, CommutingPauliStringIterator, FlexPauliString, Flow, PauliBasis, PauliPhase,
    PauliSign, PauliString, PauliStringIterator, SingleQubitClifford, StabilizerError,
    StabilizerResult, Tableau, TableauIterator, stabilizers_to_tableau,
};
pub use target::{Pauli, Target};
