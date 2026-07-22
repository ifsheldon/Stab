use clap::ValueEnum;

use super::clifford_string::CliffordWorkloadKind;
use super::not_zero::NotZeroPattern;
use super::pauli_iter::PauliIterKind;
use super::sparse_xor::SparseXorKind;
use super::transpose::TransposeKind;

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum WorkerWorkload {
    ProtocolSmoke,
    CircuitParse,
    CircuitCanonicalPrint,
    DemParse,
    DemCanonicalPrint,
    GateNameHash,
    SimdWordPopcount,
    SimdBitsXor,
    SimdBitsNotZeroEarly,
    SimdBitsNotZeroZero,
    SimdBitsNotZeroLate,
    SparseXorRow,
    SparseXorItem,
    BitMatrixTransposeInPlace,
    BitMatrixTransposeAllocating,
    PauliStringRightMultiply,
    PauliStringIterRange,
    PauliStringIterSingleton,
    CliffordStringRightMultiplyIdentity,
    CliffordStringRightMultiplyNonIdentity,
}

impl WorkerWorkload {
    pub(super) fn id(self) -> &'static str {
        match self {
            Self::ProtocolSmoke => "protocol-smoke",
            Self::CircuitParse => "circuit-parse",
            Self::CircuitCanonicalPrint => "circuit-canonical-print",
            Self::DemParse => "dem-parse",
            Self::DemCanonicalPrint => "dem-canonical-print",
            Self::GateNameHash => "gate-name-hash",
            Self::SimdWordPopcount => "simd-word-popcount",
            Self::SimdBitsXor => "simd-bits-xor",
            Self::SimdBitsNotZeroEarly => "simd-bits-not-zero-early",
            Self::SimdBitsNotZeroZero => "simd-bits-not-zero-zero",
            Self::SimdBitsNotZeroLate => "simd-bits-not-zero-late",
            Self::SparseXorRow => "sparse-xor-row",
            Self::SparseXorItem => "sparse-xor-item",
            Self::BitMatrixTransposeInPlace => "bit-matrix-transpose-in-place",
            Self::BitMatrixTransposeAllocating => "bit-matrix-transpose-allocating",
            Self::PauliStringRightMultiply => "pauli-string-right-multiply",
            Self::PauliStringIterRange => "pauli-string-iter-range",
            Self::PauliStringIterSingleton => "pauli-string-iter-singleton",
            Self::CliffordStringRightMultiplyIdentity => CliffordWorkloadKind::Identity.workload(),
            Self::CliffordStringRightMultiplyNonIdentity => {
                CliffordWorkloadKind::NonIdentity.workload()
            }
        }
    }

    pub(super) fn measurement_id(self) -> &'static str {
        match self {
            Self::ProtocolSmoke => "main",
            Self::CircuitParse => "parse",
            Self::CircuitCanonicalPrint => "serialize",
            Self::DemParse => "parse",
            Self::DemCanonicalPrint => "serialize",
            Self::GateNameHash => "hash-all-names",
            Self::SimdWordPopcount => "toggle-popcount",
            Self::SimdBitsXor => "xor-complete-vector",
            Self::SimdBitsNotZeroEarly | Self::SimdBitsNotZeroZero | Self::SimdBitsNotZeroLate => {
                "not-zero"
            }
            Self::SparseXorRow => "row-xor",
            Self::SparseXorItem => "xor-item",
            Self::BitMatrixTransposeInPlace => TransposeKind::InPlace.measurement(),
            Self::BitMatrixTransposeAllocating => TransposeKind::Allocating.measurement(),
            Self::PauliStringRightMultiply => "right-multiply-in-place",
            Self::PauliStringIterRange | Self::PauliStringIterSingleton => {
                "construct-and-iterate-borrowed"
            }
            Self::CliffordStringRightMultiplyIdentity => {
                CliffordWorkloadKind::Identity.measurement()
            }
            Self::CliffordStringRightMultiplyNonIdentity => {
                CliffordWorkloadKind::NonIdentity.measurement()
            }
        }
    }

    pub(super) const fn not_zero_pattern(self) -> Option<NotZeroPattern> {
        match self {
            Self::SimdBitsNotZeroEarly => Some(NotZeroPattern::Early),
            Self::SimdBitsNotZeroZero => Some(NotZeroPattern::Zero),
            Self::SimdBitsNotZeroLate => Some(NotZeroPattern::Late),
            _ => None,
        }
    }

    pub(super) const fn sparse_xor_kind(self) -> Option<SparseXorKind> {
        match self {
            Self::SparseXorRow => Some(SparseXorKind::Row),
            Self::SparseXorItem => Some(SparseXorKind::Item),
            _ => None,
        }
    }

    pub(super) const fn transpose_kind(self) -> Option<TransposeKind> {
        match self {
            Self::BitMatrixTransposeInPlace => Some(TransposeKind::InPlace),
            Self::BitMatrixTransposeAllocating => Some(TransposeKind::Allocating),
            _ => None,
        }
    }

    pub(super) const fn pauli_iter_kind(self) -> Option<PauliIterKind> {
        match self {
            Self::PauliStringIterRange => Some(PauliIterKind::Range),
            Self::PauliStringIterSingleton => Some(PauliIterKind::Singleton),
            _ => None,
        }
    }

    pub(super) const fn clifford_kind(self) -> Option<CliffordWorkloadKind> {
        match self {
            Self::CliffordStringRightMultiplyIdentity => Some(CliffordWorkloadKind::Identity),
            Self::CliffordStringRightMultiplyNonIdentity => Some(CliffordWorkloadKind::NonIdentity),
            _ => None,
        }
    }
}
