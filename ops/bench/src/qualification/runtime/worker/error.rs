use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum WorkerError {
    #[error("qualification workers require Linux RSS and process contracts")]
    UnsupportedHost,
    #[error(transparent)]
    Protocol(#[from] super::super::protocol::ProtocolError),
    #[error(transparent)]
    Circuit(#[from] stab_core::CircuitError),
    #[error(transparent)]
    Bits(#[from] stab_core::BitError),
    #[error("qualification workload {workload} requires measurement {expected}, got {actual}")]
    MeasurementMismatch {
        workload: &'static str,
        expected: &'static str,
        actual: String,
    },
    #[error("circuit-parse scale has {actual} instructions, maximum {maximum}")]
    CircuitScaleLimit { actual: u64, maximum: u64 },
    #[error("circuit-parse scale {0} cannot be represented on this host")]
    CircuitScaleRange(u64),
    #[error("circuit-parse fixture capacity overflows usize")]
    CircuitFixtureOverflow,
    #[error("qualification input byte count cannot be represented as u64")]
    InputSizeRange,
    #[error("gate-name-hash registry has {actual} names, expected {expected}")]
    GateHashNameCount { actual: usize, expected: u64 },
    #[error("gate-name-hash value {actual} for {name:?} cannot be represented as u16")]
    GateHashValueRange { name: String, actual: usize },
    #[error("gate-name-hash work count {actual} is not a complete sweep of {name_count} names")]
    GateHashPartialSweep { actual: u64, name_count: u64 },
    #[error("simd-word-popcount width {actual} bits is below the minimum {minimum}")]
    PopcountWidthMinimum { actual: u64, minimum: u64 },
    #[error("simd-word-popcount width {actual} bits exceeds the maximum {maximum}")]
    PopcountWidthLimit { actual: u64, maximum: u64 },
    #[error("simd-word-popcount width {actual} bits is not a multiple of {alignment}")]
    PopcountWidthAlignment { actual: u64, alignment: u64 },
    #[error("simd-word-popcount width {0} cannot be represented on this host")]
    PopcountWidthRange(u64),
    #[error("simd-word-popcount word index cannot be represented as u64")]
    PopcountWordIndexRange,
    #[error("simd-word-popcount fixture allocation failed: {0}")]
    PopcountFixtureAllocation(std::collections::TryReserveError),
    #[error("simd-word-popcount fixture does not contain its toggle bit")]
    MissingPopcountToggleBit,
    #[error("simd-word-popcount result cannot be represented as u64")]
    PopcountResultRange,
    #[error("simd-bits-xor width {actual} bits is below the minimum {minimum}")]
    DenseXorWidthMinimum { actual: u64, minimum: u64 },
    #[error("simd-bits-xor width {actual} bits exceeds the maximum {maximum}")]
    DenseXorWidthLimit { actual: u64, maximum: u64 },
    #[error("simd-bits-xor width {actual} bits is not a multiple of {alignment}")]
    DenseXorWidthAlignment { actual: u64, alignment: u64 },
    #[error("simd-bits-xor width {0} cannot be represented on this host")]
    DenseXorWidthRange(u64),
    #[error("simd-bits-xor word index cannot be represented as u64")]
    DenseXorWordIndexRange,
    #[error("simd-bits-xor fixture allocation failed: {0}")]
    DenseXorFixtureAllocation(std::collections::TryReserveError),
    #[error("simd-bits-not-zero width {actual} bits is below the minimum {minimum}")]
    NotZeroWidthMinimum { actual: u64, minimum: u64 },
    #[error("simd-bits-not-zero width {actual} bits exceeds the maximum {maximum}")]
    NotZeroWidthLimit { actual: u64, maximum: u64 },
    #[error("simd-bits-not-zero width {0} cannot be represented on this host")]
    NotZeroWidthRange(u64),
    #[error("simd-bits-not-zero fixture allocation failed: {0}")]
    NotZeroFixtureAllocation(std::collections::TryReserveError),
    #[error("simd-bits-not-zero hit index {index} is outside bit width {bit_count}")]
    NotZeroHitIndex { index: usize, bit_count: usize },
    #[error("{workload} work count {actual} is not a positive multiple of {base}")]
    SparseXorWorkShape {
        workload: &'static str,
        actual: u64,
        base: u64,
    },
    #[error("{workload} work count {actual} exceeds maximum {maximum}")]
    SparseXorWorkLimit {
        workload: &'static str,
        actual: u64,
        maximum: u64,
    },
    #[error("sparse XOR fixture value {0} cannot be represented on this host")]
    SparseXorFixtureRange(u64),
    #[error("sparse XOR fixture allocation failed: {0}")]
    SparseXorFixtureAllocation(std::collections::TryReserveError),
    #[error("sparse XOR canonical encoding size overflows usize")]
    SparseXorEncodingOverflow,
    #[error("sparse XOR canonical encoding allocation failed: {0}")]
    SparseXorEncodingAllocation(std::collections::TryReserveError),
    #[error("{0} capacity priming did not restore the canonical sparse XOR state")]
    SparseXorPrimingState(&'static str),
    #[error("bit-matrix transpose work count {0} is not a perfect square")]
    TransposeWorkNotSquare(u64),
    #[error("bit-matrix transpose dimension {actual} is below the minimum {minimum}")]
    TransposeDimensionMinimum { actual: u64, minimum: u64 },
    #[error("bit-matrix transpose dimension {actual} is not a multiple of {alignment}")]
    TransposeDimensionAlignment { actual: u64, alignment: u64 },
    #[error("bit-matrix transpose dimension {actual} exceeds maximum {maximum}")]
    TransposeDimensionLimit { actual: u64, maximum: u64 },
    #[error("bit-matrix transpose dimension {0} cannot be represented on this host")]
    TransposeDimensionRange(u64),
    #[error("bit-matrix transpose canonical byte count overflows u64")]
    TransposeByteCountOverflow,
    #[error("bit-matrix transpose fixture affine calculation overflows u64")]
    TransposeAffineOverflow,
    #[error("{0} warmup did not restore the canonical bit-matrix state")]
    TransposePrimingState(&'static str),
    #[error("allocating bit-matrix transpose produced no retained result")]
    MissingTransposeResult,
    #[error("allocating bit-matrix transpose modified its source matrix")]
    TransposeSourceChanged,
    #[error(transparent)]
    Stabilizer(#[from] stab_core::StabilizerError),
    #[error("Pauli multiplication width {actual} is below the minimum {minimum}")]
    PauliWidthMinimum { actual: u64, minimum: u64 },
    #[error("Pauli multiplication width {actual} exceeds maximum {maximum}")]
    PauliWidthLimit { actual: u64, maximum: u64 },
    #[error("Pauli multiplication width {0} cannot be represented on this host")]
    PauliWidthRange(u64),
    #[error("Pauli multiplication canonical byte count overflows u64")]
    PauliByteCountOverflow,
    #[error("Pauli multiplication warmup did not restore the canonical left operand")]
    PauliPrimingState,
    #[error("Pauli multiplication modified its right operand")]
    PauliRightChanged,
    #[error("{workload} work count {actual} is not a complete source-owned iterator traversal")]
    PauliIterWorkShape { workload: &'static str, actual: u64 },
    #[error("{workload} output count {actual} exceeds maximum {maximum}")]
    PauliIterOutputLimit {
        workload: &'static str,
        actual: u64,
        maximum: u64,
    },
    #[error("{workload} width {actual} exceeds maximum {maximum}")]
    PauliIterWidthLimit {
        workload: &'static str,
        actual: u64,
        maximum: u64,
    },
    #[error("Pauli iterator value {0} cannot be represented on this host")]
    PauliIterWidthRange(u64),
    #[error("Pauli iterator result width cannot be represented as u64")]
    PauliIterResultWidthRange,
    #[error("Pauli iterator combinatorial output count overflows u64")]
    PauliIterCountOverflow,
    #[error("Pauli iterator output-count times result-width checksum overflows u64")]
    PauliIterWidthChecksumOverflow,
    #[error("Pauli iterator validation produced no final yielded result")]
    PauliIterMissingFinalResult,
    #[error(
        "{workload} validation produced outputs {actual_outputs} and width checksum {actual_width_checksum}, expected {expected_outputs} and {expected_width_checksum}"
    )]
    PauliIterValidation {
        workload: &'static str,
        expected_outputs: u64,
        actual_outputs: u64,
        expected_width_checksum: u64,
        actual_width_checksum: u64,
    },
    #[error(
        "{workload} timing produced outputs {actual_outputs} and width checksum {actual_width_checksum}, expected {expected_outputs} and {expected_width_checksum}"
    )]
    PauliIterObserved {
        workload: &'static str,
        expected_outputs: u64,
        actual_outputs: u64,
        expected_width_checksum: u64,
        actual_width_checksum: u64,
    },
    #[error("Clifford-string workload requires --input-descriptor-hex")]
    MissingCliffordDescriptor,
    #[error("--input-descriptor-hex is only valid for Clifford-string workloads")]
    UnexpectedCliffordDescriptor,
    #[error("Clifford-string width must be positive")]
    CliffordWidthZero,
    #[error("Clifford-string width {actual} exceeds maximum {maximum}")]
    CliffordWidthLimit { actual: u64, maximum: u64 },
    #[error("Clifford-string width {0} cannot be represented on this host")]
    CliffordWidthRange(u64),
    #[error("Clifford-string descriptor width {width} differs from work-items {work_items}")]
    CliffordWidthWorkMismatch { width: u64, work_items: u64 },
    #[error("Clifford-string descriptor has unknown workload marker {0}")]
    CliffordUnknownMarker(u64),
    #[error("{workload} does not accept Clifford-string workload marker {marker}")]
    CliffordWorkloadMarkerMismatch { workload: &'static str, marker: u64 },
    #[error("Clifford-string descriptor {name} is {actual}, expected {expected}")]
    CliffordDescriptorField {
        name: &'static str,
        actual: u64,
        expected: u64,
    },
    #[error("qualification workload {0} has no matching prepared lifecycle")]
    PreparedWorkloadKind(&'static str),
    #[error("prepared qualification workload returned an incompatible output")]
    PreparedWorkloadOutput,
    #[error("Clifford-string execution state was not reset before the start barrier")]
    CliffordExecutionNotArmed,
    #[error("Clifford-string successful callback count overflowed u64")]
    CliffordCallbackOverflow,
    #[error("Clifford-string gate is missing at index {0}")]
    CliffordGateMissing(usize),
    #[error("Clifford-string gate code {0} is outside the canonical 24-gate table")]
    CliffordGateCodeRange(usize),
    #[error("independent Clifford product {left} by {right} was absent from the canonical table")]
    CliffordProductMissing { left: usize, right: usize },
    #[error("Clifford-string {name} sequence has length {actual}, expected {expected}")]
    CliffordSequenceLength {
        name: &'static str,
        actual: usize,
        expected: usize,
    },
    #[error(
        "Clifford-string {name} sequence differs at index {index}: code {actual}, expected {expected}"
    )]
    CliffordSequenceMismatch {
        name: &'static str,
        index: usize,
        actual: u8,
        expected: u8,
    },
    #[error("Clifford-string count cannot be represented as u64")]
    CliffordCountRange,
    #[error("Clifford-string callback count is {actual}, expected {expected}")]
    CliffordCallbackCount { actual: u64, expected: u64 },
    #[error("Clifford-string execution witness is {actual:#018x}, expected {expected:#018x}")]
    CliffordWitness { actual: u64, expected: u64 },
    #[error("qualification worker semantic work count overflows u64")]
    WorkOverflow,
    #[error("failed to read the qualification start barrier: {0}")]
    ReadStartBarrier(std::io::Error),
    #[error("qualification start barrier must contain exactly one newline")]
    InvalidStartBarrier,
    #[error("qualification CPU id cannot be represented on this host")]
    AffinityCpuRange,
    #[error("failed to read qualification worker CPU affinity: {0}")]
    ReadAffinity(rustix::io::Errno),
    #[error("qualification worker affinity is {actual:?}, expected only CPU {expected}")]
    AffinityMismatch { expected: usize, actual: Vec<usize> },
    #[error("qualification worker elapsed seconds are invalid: {0}")]
    InvalidElapsed(f64),
    #[error("failed to read /proc/self/status: {0}")]
    ReadStatus(std::io::Error),
    #[error("/proc/self/status is missing {0}")]
    MissingStatusField(&'static str),
    #[error("/proc/self/status has malformed {0}")]
    MalformedStatusField(&'static str),
    #[error("qualification worker resident memory overflows u64 bytes")]
    MemoryOverflow,
    #[error("qualification worker could not encode a SHA-256 digest")]
    DigestEncoding,
    #[error("failed to serialize qualification worker output: {0}")]
    Serialize(serde_json::Error),
    #[error("failed to write qualification worker output: {0}")]
    Write(std::io::Error),
}
