use std::fmt::{Display, Formatter};
use std::hint::black_box;
use std::str::FromStr;
use std::sync::atomic::{Ordering, compiler_fence};

use sha2::{Digest as _, Sha256};
use stab_core::{
    CliffordString, Gate, SingleQubitClifford, StabilizerResource, Tableau, unitary_to_tableau,
};

use super::WorkerError;

pub(in crate::qualification::runtime) const CLIFFORD_DESCRIPTOR_BYTES: u64 = 64;
pub(in crate::qualification::runtime) const CLIFFORD_FIXTURE_SCHEMA: u64 = 1;
pub(in crate::qualification::runtime) const CLIFFORD_GATE_COUNT: u64 = 24;
pub(in crate::qualification::runtime) const CLIFFORD_NON_IDENTITY_CYCLE: u64 = 23;
pub(in crate::qualification::runtime) const CLIFFORD_COMPLETE_SPAN: u64 = 552;
pub(in crate::qualification::runtime) const CLIFFORD_PUBLIC_CAP: u64 =
    StabilizerResource::CliffordQubits.limit() as u64;
pub(in crate::qualification::runtime) const CLIFFORD_IDENTITY_MARKER: u64 =
    u64::from_le_bytes(*b"CLIF_ID1");
pub(in crate::qualification::runtime) const CLIFFORD_NON_IDENTITY_MARKER: u64 =
    u64::from_le_bytes(*b"CLIF_NI1");

const GATE_DIGEST_DOMAIN: &[u8] = b"stab.clifford-string.gates.v1";
const WITNESS_INCREMENT: u64 = 0x9e37_79b9_7f4a_7c15;

pub(in crate::qualification::runtime) const STIM_GATE_ORDER: [SingleQubitClifford; 24] = [
    SingleQubitClifford::I,
    SingleQubitClifford::X,
    SingleQubitClifford::Y,
    SingleQubitClifford::Z,
    SingleQubitClifford::Hxy,
    SingleQubitClifford::S,
    SingleQubitClifford::SDag,
    SingleQubitClifford::Hnxy,
    SingleQubitClifford::H,
    SingleQubitClifford::SqrtYDag,
    SingleQubitClifford::Hnxz,
    SingleQubitClifford::SqrtY,
    SingleQubitClifford::Hyz,
    SingleQubitClifford::Hnyz,
    SingleQubitClifford::SqrtX,
    SingleQubitClifford::SqrtXDag,
    SingleQubitClifford::Cxyz,
    SingleQubitClifford::Cxynz,
    SingleQubitClifford::Cnxyz,
    SingleQubitClifford::Cxnyz,
    SingleQubitClifford::Czyx,
    SingleQubitClifford::Cznyx,
    SingleQubitClifford::Cnzyx,
    SingleQubitClifford::Czynx,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::qualification::runtime) enum CliffordWorkloadKind {
    Identity,
    NonIdentity,
}

impl CliffordWorkloadKind {
    pub(in crate::qualification::runtime) const fn workload(self) -> &'static str {
        match self {
            Self::Identity => "clifford-string-right-multiply-identity",
            Self::NonIdentity => "clifford-string-right-multiply-non-identity",
        }
    }

    pub(in crate::qualification::runtime) const fn measurement(self) -> &'static str {
        match self {
            Self::Identity => "right-multiply-identity",
            Self::NonIdentity => "right-multiply-non-identity",
        }
    }

    pub(in crate::qualification::runtime) const fn marker(self) -> u64 {
        match self {
            Self::Identity => CLIFFORD_IDENTITY_MARKER,
            Self::NonIdentity => CLIFFORD_NON_IDENTITY_MARKER,
        }
    }

    const fn cycle_count(self) -> u64 {
        match self {
            Self::Identity => 0,
            Self::NonIdentity => CLIFFORD_NON_IDENTITY_CYCLE,
        }
    }

    const fn complete_span(self) -> u64 {
        match self {
            Self::Identity => 0,
            Self::NonIdentity => CLIFFORD_COMPLETE_SPAN,
        }
    }

    fn from_marker(marker: u64) -> Result<Self, WorkerError> {
        match marker {
            CLIFFORD_IDENTITY_MARKER => Ok(Self::Identity),
            CLIFFORD_NON_IDENTITY_MARKER => Ok(Self::NonIdentity),
            _ => Err(WorkerError::CliffordUnknownMarker(marker)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::qualification::runtime) struct CliffordDescriptor {
    fields: [u64; 8],
}

impl CliffordDescriptor {
    pub(in crate::qualification::runtime) const fn canonical(
        kind: CliffordWorkloadKind,
        width: u64,
    ) -> Self {
        Self {
            fields: [
                width,
                kind.marker(),
                CLIFFORD_FIXTURE_SCHEMA,
                CLIFFORD_GATE_COUNT,
                kind.cycle_count(),
                kind.complete_span(),
                CLIFFORD_PUBLIC_CAP,
                0,
            ],
        }
    }

    pub(in crate::qualification::runtime) const fn from_fields(fields: [u64; 8]) -> Self {
        Self { fields }
    }

    pub(in crate::qualification::runtime) const fn fields(self) -> [u64; 8] {
        self.fields
    }

    pub(in crate::qualification::runtime) const fn width(self) -> u64 {
        self.fields[0]
    }

    pub(in crate::qualification::runtime) fn bytes(self) -> [u8; 64] {
        let mut bytes = [0_u8; 64];
        for (target, field) in bytes.chunks_exact_mut(8).zip(self.fields) {
            target.copy_from_slice(&field.to_le_bytes());
        }
        bytes
    }

    pub(in crate::qualification::runtime) fn input_digest(self) -> Result<String, WorkerError> {
        super::hex_bytes(&Sha256::digest(self.bytes()))
    }

    pub(super) fn validate(
        self,
        requested_kind: CliffordWorkloadKind,
        work_items: u64,
    ) -> Result<(), WorkerError> {
        let width = self.fields[0];
        if width == 0 {
            return Err(WorkerError::CliffordWidthZero);
        }
        if width > CLIFFORD_PUBLIC_CAP {
            return Err(WorkerError::CliffordWidthLimit {
                actual: width,
                maximum: CLIFFORD_PUBLIC_CAP,
            });
        }
        let actual_kind = CliffordWorkloadKind::from_marker(self.fields[1])?;
        if actual_kind != requested_kind {
            return Err(WorkerError::CliffordWorkloadMarkerMismatch {
                workload: requested_kind.workload(),
                marker: self.fields[1],
            });
        }
        validate_field("fixture schema", self.fields[2], CLIFFORD_FIXTURE_SCHEMA)?;
        validate_field("canonical gate count", self.fields[3], CLIFFORD_GATE_COUNT)?;
        validate_field(
            "right-cycle count",
            self.fields[4],
            requested_kind.cycle_count(),
        )?;
        validate_field(
            "complete cross-product span",
            self.fields[5],
            requested_kind.complete_span(),
        )?;
        validate_field(
            "public Clifford-qubit cap",
            self.fields[6],
            CLIFFORD_PUBLIC_CAP,
        )?;
        validate_field("reserved field", self.fields[7], 0)?;
        if width != work_items {
            return Err(WorkerError::CliffordWidthWorkMismatch { width, work_items });
        }
        Ok(())
    }
}

impl Display for CliffordDescriptor {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        for byte in self.bytes() {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::qualification::runtime) struct CliffordDescriptorParseError(&'static str);

impl Display for CliffordDescriptorParseError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.0)
    }
}

impl std::error::Error for CliffordDescriptorParseError {}

impl FromStr for CliffordDescriptor {
    type Err = CliffordDescriptorParseError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if text.len() != 128 {
            return Err(CliffordDescriptorParseError(
                "Clifford descriptor must contain exactly 128 hexadecimal characters",
            ));
        }
        let mut raw = [0_u8; 64];
        for (target, pair) in raw.iter_mut().zip(text.as_bytes().chunks_exact(2)) {
            let [high, low] = pair else {
                return Err(CliffordDescriptorParseError(
                    "Clifford descriptor contains an incomplete byte",
                ));
            };
            *target = (hex_nibble(*high)? << 4) | hex_nibble(*low)?;
        }
        let mut fields = [0_u64; 8];
        for (field, bytes) in fields.iter_mut().zip(raw.chunks_exact(8)) {
            let mut word = [0_u8; 8];
            word.copy_from_slice(bytes);
            *field = u64::from_le_bytes(word);
        }
        Ok(Self { fields })
    }
}

fn hex_nibble(value: u8) -> Result<u8, CliffordDescriptorParseError> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        b'A'..=b'F' => Ok(value - b'A' + 10),
        _ => Err(CliffordDescriptorParseError(
            "Clifford descriptor contains a non-hexadecimal character",
        )),
    }
}

fn validate_field(name: &'static str, actual: u64, expected: u64) -> Result<(), WorkerError> {
    if actual == expected {
        Ok(())
    } else {
        Err(WorkerError::CliffordDescriptorField {
            name,
            actual,
            expected,
        })
    }
}

struct ScalarExpected {
    final_left_codes: Vec<u8>,
    right_codes: Vec<u8>,
    execution_witness: u64,
}

pub(in crate::qualification::runtime) struct CliffordStringFixture {
    kind: CliffordWorkloadKind,
    descriptor: CliffordDescriptor,
    left: CliffordString,
    right: CliffordString,
    expected: ScalarExpected,
    callback_count: u64,
    execution_witness: u64,
    armed: bool,
    pub(super) input_bytes: u64,
    pub(super) input_digest: String,
}

impl CliffordStringFixture {
    pub(in crate::qualification::runtime) fn prepare(
        kind: CliffordWorkloadKind,
        descriptor: CliffordDescriptor,
        work_items: u64,
        iterations: u64,
    ) -> Result<Self, WorkerError> {
        descriptor.validate(kind, work_items)?;
        let width = usize::try_from(descriptor.width())
            .map_err(|_| WorkerError::CliffordWidthRange(descriptor.width()))?;
        let table = scalar_multiplication_table()?;
        let expected = scalar_expected(kind, width, iterations, &table)?;
        let left = codes_to_clifford(&initial_left_codes(kind, width)?)?;
        let right = codes_to_clifford(&expected.right_codes)?;
        Ok(Self {
            kind,
            descriptor,
            left,
            right,
            expected,
            callback_count: 0,
            execution_witness: 0,
            armed: false,
            input_bytes: CLIFFORD_DESCRIPTOR_BYTES,
            input_digest: descriptor.input_digest()?,
        })
    }

    pub(in crate::qualification::runtime) fn reset_execution_state(&mut self) {
        self.callback_count = 0;
        self.execution_witness = 0;
        self.armed = true;
    }

    pub(in crate::qualification::runtime) fn execute(
        &mut self,
        iterations: u64,
    ) -> Result<(), WorkerError> {
        if !self.armed {
            return Err(WorkerError::CliffordExecutionNotArmed);
        }
        let width = self.left.len();
        for _ in 0..iterations {
            compiler_fence(Ordering::SeqCst);
            let left = black_box(&mut self.left);
            let right = black_box(&self.right);
            left.right_multiply_in_place(right)?;
            compiler_fence(Ordering::SeqCst);
            self.callback_count = self
                .callback_count
                .checked_add(1)
                .ok_or(WorkerError::CliffordCallbackOverflow)?;
            let width_u64 = u64::try_from(width).map_err(|_| WorkerError::CliffordCountRange)?;
            let callback_index = usize::try_from((self.callback_count - 1) % width_u64)
                .map_err(|_| WorkerError::CliffordWidthRange(width_u64))?;
            let observed_code = gate_code(
                self.left
                    .gate_at(black_box(callback_index))
                    .ok_or(WorkerError::CliffordGateMissing(callback_index))?,
            );
            self.execution_witness = (self.execution_witness ^ u64::from(observed_code))
                .rotate_left(13)
                .wrapping_add(WITNESS_INCREMENT)
                .wrapping_add(self.callback_count);
            black_box(observed_code);
            black_box(self.execution_witness);
        }
        black_box(&self.left);
        Ok(())
    }

    pub(in crate::qualification::runtime) fn output_fields(
        &self,
        iterations: u64,
        work_count: u64,
    ) -> Result<[u64; 16], WorkerError> {
        if self.callback_count != iterations {
            return Err(WorkerError::CliffordCallbackCount {
                actual: self.callback_count,
                expected: iterations,
            });
        }
        if self.execution_witness != self.expected.execution_witness {
            return Err(WorkerError::CliffordWitness {
                actual: self.execution_witness,
                expected: self.expected.execution_witness,
            });
        }
        let actual_left = clifford_codes(&self.left)?;
        let actual_right = clifford_codes(&self.right)?;
        compare_codes("left", &actual_left, &self.expected.final_left_codes)?;
        compare_codes("right", &actual_right, &self.expected.right_codes)?;
        let left_non_identity = non_identity_count(&actual_left)?;
        let right_non_identity = non_identity_count(&actual_right)?;
        let left_digest = gate_sequence_digest_lanes(&actual_left)?;
        let right_digest = gate_sequence_digest_lanes(&actual_right)?;
        let mut fields = [0_u64; 16];
        fields[..8].copy_from_slice(&[
            iterations,
            work_count,
            self.descriptor.width(),
            self.kind.marker(),
            left_non_identity,
            right_non_identity,
            self.callback_count,
            self.execution_witness,
        ]);
        fields[8..12].copy_from_slice(&left_digest);
        fields[12..16].copy_from_slice(&right_digest);
        Ok(fields)
    }

    pub(in crate::qualification::runtime) fn output_digest(
        &self,
        iterations: u64,
        work_count: u64,
    ) -> Result<String, WorkerError> {
        let fields = self.output_fields(iterations, work_count)?;
        let mut digest = Sha256::new();
        for field in fields {
            digest.update(field.to_le_bytes());
        }
        super::hex_bytes(&digest.finalize())
    }
}

fn initial_left_codes(kind: CliffordWorkloadKind, width: usize) -> Result<Vec<u8>, WorkerError> {
    (0..width)
        .map(|index| match kind {
            CliffordWorkloadKind::Identity => Ok(0),
            CliffordWorkloadKind::NonIdentity => {
                u8::try_from(index % 24).map_err(|_| WorkerError::CliffordGateCodeRange(index % 24))
            }
        })
        .collect()
}

fn right_codes(kind: CliffordWorkloadKind, width: usize) -> Result<Vec<u8>, WorkerError> {
    (0..width)
        .map(|index| match kind {
            CliffordWorkloadKind::Identity => Ok(0),
            CliffordWorkloadKind::NonIdentity => u8::try_from(1 + (index / 24) % 23)
                .map_err(|_| WorkerError::CliffordGateCodeRange(1 + (index / 24) % 23)),
        })
        .collect()
}

fn scalar_expected(
    kind: CliffordWorkloadKind,
    width: usize,
    iterations: u64,
    table: &[[u8; 24]; 24],
) -> Result<ScalarExpected, WorkerError> {
    let initial_left = initial_left_codes(kind, width)?;
    let right = right_codes(kind, width)?;
    let final_left_codes = match kind {
        CliffordWorkloadKind::Identity => initial_left.clone(),
        CliffordWorkloadKind::NonIdentity => initial_left
            .iter()
            .copied()
            .zip(right.iter().copied())
            .map(|(left, right)| scalar_right_power(left, right, iterations, table))
            .collect::<Result<Vec<_>, _>>()?,
    };
    let width_u64 = u64::try_from(width).map_err(|_| WorkerError::CliffordCountRange)?;
    let execution_witness = match kind {
        CliffordWorkloadKind::Identity => {
            let mut witness = 0_u64;
            for callback_count in 1..=iterations {
                witness = witness
                    .rotate_left(13)
                    .wrapping_add(WITNESS_INCREMENT)
                    .wrapping_add(callback_count);
            }
            witness
        }
        CliffordWorkloadKind::NonIdentity => {
            let mut observed_codes = initial_left
                .iter()
                .copied()
                .zip(right.iter().copied())
                .enumerate()
                .map(|(index, (left, right))| {
                    let first_exponent = u64::try_from(index)
                        .map_err(|_| WorkerError::CliffordCountRange)?
                        .checked_add(1)
                        .ok_or(WorkerError::CliffordCallbackOverflow)?;
                    scalar_right_power(left, right, first_exponent, table)
                })
                .collect::<Result<Vec<_>, _>>()?;
            let stride_codes = right
                .iter()
                .copied()
                .map(|right| scalar_right_power(0, right, width_u64, table))
                .collect::<Result<Vec<_>, _>>()?;
            let mut witness = 0_u64;
            for callback_count in 1..=iterations {
                let index = usize::try_from((callback_count - 1) % width_u64)
                    .map_err(|_| WorkerError::CliffordWidthRange(width_u64))?;
                let code = *observed_codes
                    .get(index)
                    .ok_or(WorkerError::CliffordGateMissing(index))?;
                let stride = *stride_codes
                    .get(index)
                    .ok_or(WorkerError::CliffordGateMissing(index))?;
                let next_code = scalar_product_code(code, stride, table)?;
                *observed_codes
                    .get_mut(index)
                    .ok_or(WorkerError::CliffordGateMissing(index))? = next_code;
                witness = (witness ^ u64::from(code))
                    .rotate_left(13)
                    .wrapping_add(WITNESS_INCREMENT)
                    .wrapping_add(callback_count);
            }
            witness
        }
    };
    Ok(ScalarExpected {
        final_left_codes,
        right_codes: right,
        execution_witness,
    })
}

fn scalar_right_power(
    initial: u8,
    right: u8,
    mut exponent: u64,
    table: &[[u8; 24]; 24],
) -> Result<u8, WorkerError> {
    let mut accumulated = 0_u8;
    let mut factor = right;
    while exponent != 0 {
        if exponent & 1 != 0 {
            accumulated = scalar_product_code(accumulated, factor, table)?;
        }
        exponent >>= 1;
        if exponent != 0 {
            factor = scalar_product_code(factor, factor, table)?;
        }
    }
    scalar_product_code(initial, accumulated, table)
}

fn scalar_product_code(left: u8, right: u8, table: &[[u8; 24]; 24]) -> Result<u8, WorkerError> {
    table
        .get(usize::from(left))
        .and_then(|row| row.get(usize::from(right)))
        .copied()
        .ok_or(WorkerError::CliffordProductMissing {
            left: usize::from(left),
            right: usize::from(right),
        })
}

fn scalar_multiplication_table() -> Result<[[u8; 24]; 24], WorkerError> {
    let tableaus = STIM_GATE_ORDER
        .iter()
        .map(|gate| {
            let matrix = Gate::from_name(gate.canonical_name())?
                .unitary_matrix()?
                .to_vecs();
            unitary_to_tableau(&matrix, true).map_err(WorkerError::from)
        })
        .collect::<Result<Vec<Tableau>, WorkerError>>()?;
    let mut products = [[0_u8; 24]; 24];
    for (left_index, left) in tableaus.iter().enumerate() {
        for (right_index, right) in tableaus.iter().enumerate() {
            let product = right.then(left)?;
            let code = tableaus
                .iter()
                .position(|candidate| *candidate == product)
                .ok_or(WorkerError::CliffordProductMissing {
                    left: left_index,
                    right: right_index,
                })?;
            let target = products
                .get_mut(left_index)
                .and_then(|row| row.get_mut(right_index))
                .ok_or(WorkerError::CliffordProductMissing {
                    left: left_index,
                    right: right_index,
                })?;
            *target = u8::try_from(code).map_err(|_| WorkerError::CliffordGateCodeRange(code))?;
        }
    }
    Ok(products)
}

fn clifford_codes(value: &CliffordString) -> Result<Vec<u8>, WorkerError> {
    (0..value.len())
        .map(|index| {
            value
                .gate_at(index)
                .map(gate_code)
                .ok_or(WorkerError::CliffordGateMissing(index))
        })
        .collect()
}

fn compare_codes(name: &'static str, actual: &[u8], expected: &[u8]) -> Result<(), WorkerError> {
    if actual.len() != expected.len() {
        return Err(WorkerError::CliffordSequenceLength {
            name,
            actual: actual.len(),
            expected: expected.len(),
        });
    }
    for (index, (actual_code, expected_code)) in actual.iter().zip(expected).enumerate() {
        if actual_code != expected_code {
            return Err(WorkerError::CliffordSequenceMismatch {
                name,
                index,
                actual: *actual_code,
                expected: *expected_code,
            });
        }
    }
    Ok(())
}

fn non_identity_count(codes: &[u8]) -> Result<u64, WorkerError> {
    u64::try_from(codes.iter().filter(|code| **code != 0).count())
        .map_err(|_| WorkerError::CliffordCountRange)
}

fn gate_sequence_digest_lanes(codes: &[u8]) -> Result<[u64; 4], WorkerError> {
    let width = u64::try_from(codes.len()).map_err(|_| WorkerError::CliffordCountRange)?;
    let mut digest = Sha256::new();
    digest.update(GATE_DIGEST_DOMAIN);
    digest.update([0]);
    digest.update(width.to_le_bytes());
    digest.update(codes);
    let digest = digest.finalize();
    let mut lanes = [0_u64; 4];
    for (lane, bytes) in lanes.iter_mut().zip(digest.chunks_exact(8)) {
        let mut word = [0_u8; 8];
        word.copy_from_slice(bytes);
        *lane = u64::from_le_bytes(word);
    }
    Ok(lanes)
}

fn gate_from_code(code: u8) -> Result<SingleQubitClifford, WorkerError> {
    STIM_GATE_ORDER
        .get(usize::from(code))
        .copied()
        .ok_or(WorkerError::CliffordGateCodeRange(usize::from(code)))
}

fn codes_to_clifford(codes: &[u8]) -> Result<CliffordString, WorkerError> {
    let gates = codes
        .iter()
        .copied()
        .map(gate_from_code)
        .collect::<Result<Vec<_>, _>>()?;
    CliffordString::from_gates(gates).map_err(WorkerError::from)
}

fn gate_code(gate: SingleQubitClifford) -> u8 {
    match gate {
        SingleQubitClifford::I => 0,
        SingleQubitClifford::X => 1,
        SingleQubitClifford::Y => 2,
        SingleQubitClifford::Z => 3,
        SingleQubitClifford::Hxy => 4,
        SingleQubitClifford::S => 5,
        SingleQubitClifford::SDag => 6,
        SingleQubitClifford::Hnxy => 7,
        SingleQubitClifford::H => 8,
        SingleQubitClifford::SqrtYDag => 9,
        SingleQubitClifford::Hnxz => 10,
        SingleQubitClifford::SqrtY => 11,
        SingleQubitClifford::Hyz => 12,
        SingleQubitClifford::Hnyz => 13,
        SingleQubitClifford::SqrtX => 14,
        SingleQubitClifford::SqrtXDag => 15,
        SingleQubitClifford::Cxyz => 16,
        SingleQubitClifford::Cxynz => 17,
        SingleQubitClifford::Cnxyz => 18,
        SingleQubitClifford::Cxnyz => 19,
        SingleQubitClifford::Czyx => 20,
        SingleQubitClifford::Cznyx => 21,
        SingleQubitClifford::Cnzyx => 22,
        SingleQubitClifford::Czynx => 23,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_round_trips_and_binds_the_exact_markers() {
        for kind in [
            CliffordWorkloadKind::Identity,
            CliffordWorkloadKind::NonIdentity,
        ] {
            let descriptor = CliffordDescriptor::canonical(kind, 10_000);
            assert_eq!(descriptor.to_string().parse(), Ok(descriptor));
            assert_eq!(descriptor.to_string().len(), 128);
            let [_, marker, ..] = descriptor.fields();
            assert_eq!(marker, kind.marker());
        }
        assert_eq!(CLIFFORD_IDENTITY_MARKER.to_le_bytes(), *b"CLIF_ID1");
        assert_eq!(CLIFFORD_NON_IDENTITY_MARKER.to_le_bytes(), *b"CLIF_NI1");
    }

    #[test]
    fn scalar_reference_covers_the_complete_non_identity_cycle_and_tails() {
        let table = scalar_multiplication_table().expect("independent Clifford table");
        let complete_span = usize::try_from(CLIFFORD_COMPLETE_SPAN).expect("span fits usize");
        let complete = scalar_expected(CliffordWorkloadKind::NonIdentity, complete_span, 1, &table)
            .expect("complete cycle");
        assert_eq!(complete.final_left_codes.len(), complete_span);
        for (width, expected_tail, expected_pair) in [
            (10_000_usize, 64, (15, 3)),
            (100_000, 88, (15, 4)),
            (1_000_000, 328, (15, 14)),
            (1_048_576, 328, (15, 14)),
        ] {
            let tail = width % complete_span;
            let final_index = width - 1;
            assert_eq!(tail, expected_tail);
            assert_eq!(
                (
                    u8::try_from(final_index % 24).expect("left code fits u8"),
                    u8::try_from(1 + (final_index / 24) % 23).expect("right code fits u8")
                ),
                expected_pair
            );
        }
    }

    #[test]
    fn scalar_power_matches_literal_composition_and_handles_maximum_exponent() {
        let table = scalar_multiplication_table().expect("independent Clifford table");
        for initial in 0_u8..24 {
            for right in 0_u8..24 {
                let mut literal = initial;
                for exponent in 0_u64..=24 {
                    assert_eq!(
                        scalar_right_power(initial, right, exponent, &table)
                            .expect("powered product"),
                        literal,
                        "initial={initial} right={right} exponent={exponent}"
                    );
                    literal = scalar_product_code(literal, right, &table).expect("literal product");
                }

                let before_max = scalar_right_power(initial, right, u64::MAX - 1, &table)
                    .expect("pre-maximum power");
                let at_max =
                    scalar_right_power(initial, right, u64::MAX, &table).expect("maximum power");
                assert_eq!(
                    at_max,
                    scalar_product_code(before_max, right, &table).expect("maximum successor")
                );
            }
        }
    }

    #[test]
    fn scalar_reference_matches_literal_callback_evolution() {
        let table = scalar_multiplication_table().expect("independent Clifford table");
        for kind in [
            CliffordWorkloadKind::Identity,
            CliffordWorkloadKind::NonIdentity,
        ] {
            for width in [1_usize, 2, 23, 24, 25, 552, 553] {
                for iterations in [0_u64, 1, 2, 25, 553, 1_105] {
                    let powered = scalar_expected(kind, width, iterations, &table)
                        .expect("powered reference");
                    let mut literal_left = initial_left_codes(kind, width).expect("literal left");
                    let literal_right = right_codes(kind, width).expect("literal right");
                    let mut literal_witness = 0_u64;
                    for callback_count in 1..=iterations {
                        for (left, &right) in literal_left.iter_mut().zip(&literal_right) {
                            *left =
                                scalar_product_code(*left, right, &table).expect("literal product");
                        }
                        let index = usize::try_from((callback_count - 1) % width as u64)
                            .expect("literal index");
                        let code = *literal_left.get(index).expect("literal code");
                        literal_witness = (literal_witness ^ u64::from(code))
                            .rotate_left(13)
                            .wrapping_add(WITNESS_INCREMENT)
                            .wrapping_add(callback_count);
                    }
                    assert_eq!(powered.final_left_codes, literal_left);
                    assert_eq!(powered.right_codes, literal_right);
                    assert_eq!(powered.execution_witness, literal_witness);
                }
            }
        }
    }

    #[test]
    fn identity_scalar_reference_handles_large_iteration_counts_without_width_work() {
        let table = scalar_multiplication_table().expect("independent Clifford table");
        let expected = scalar_expected(CliffordWorkloadKind::Identity, 10_000, 100_000, &table)
            .expect("large identity reference");
        assert!(expected.final_left_codes.iter().all(|&code| code == 0));
        assert!(expected.right_codes.iter().all(|&code| code == 0));
        assert_ne!(expected.execution_witness, 0);
    }

    #[test]
    fn callback_witness_is_result_derived_and_resets_between_fixtures() {
        let descriptor = CliffordDescriptor::canonical(CliffordWorkloadKind::Identity, 10_000);
        let mut odd =
            CliffordStringFixture::prepare(CliffordWorkloadKind::Identity, descriptor, 10_000, 1)
                .expect("odd fixture");
        odd.reset_execution_state();
        odd.execute(1).expect("odd execution");
        assert_eq!(odd.execution_witness, 0x9e37_79b9_7f4a_7c16);

        let mut even =
            CliffordStringFixture::prepare(CliffordWorkloadKind::Identity, descriptor, 10_000, 2)
                .expect("even fixture");
        even.reset_execution_state();
        even.execute(2).expect("even execution");
        assert_eq!(even.execution_witness, 0x8d6e_a9a2_cecd_4fdd);
        assert_eq!(even.callback_count, 2);
    }

    #[test]
    fn fixtures_match_the_independent_scalar_reference() {
        for kind in [
            CliffordWorkloadKind::Identity,
            CliffordWorkloadKind::NonIdentity,
        ] {
            for iterations in [1, 2] {
                let descriptor = CliffordDescriptor::canonical(kind, 10_000);
                let mut fixture =
                    CliffordStringFixture::prepare(kind, descriptor, 10_000, iterations)
                        .expect("fixture");
                fixture.reset_execution_state();
                fixture.execute(iterations).expect("execute");
                let fields = fixture
                    .output_fields(iterations, iterations * 10_000)
                    .expect("validated output");
                assert_eq!(fields[6], iterations);
                assert_eq!(fields[7], fixture.expected.execution_witness);
            }
        }
    }

    #[test]
    fn source_shape_freezes_symmetric_public_timing_and_result_witnesses() {
        let rust_callback = include_str!("clifford_string.rs");
        let rust_worker = include_str!("../worker.rs");
        let rust_prepared = include_str!("prepared.rs");
        let cpp_callback = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../benchmarks/stim_adapter/clifford_string_contract.h"
        ));
        let cpp_worker = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../benchmarks/stim_adapter/main.cc"
        ));

        let rust_callback = rust_callback
            .split_once("#[cfg(test)]")
            .map_or(rust_callback, |(production, _)| production);
        assert_eq!(
            rust_callback
                .matches("compiler_fence(Ordering::SeqCst);")
                .count(),
            2
        );
        assert!(rust_callback.contains("left.right_multiply_in_place(right)?;"));
        assert!(rust_callback.contains("scalar_right_power("));
        assert!(rust_callback.contains("self.execution_witness ="));
        assert!(rust_callback.contains("black_box(self.execution_witness);"));
        assert!(
            rust_prepared
                .find("fixture.reset_execution_state();")
                .is_some()
        );
        let rust_arm = rust_worker
            .find("prepared.arm();")
            .expect("Rust arm source");
        let rust_barrier = rust_worker
            .find("if args.start_barrier {")
            .expect("Rust barrier source");
        assert!(rust_arm < rust_barrier);

        assert_eq!(
            cpp_callback
                .matches("std::atomic_signal_fence(std::memory_order_seq_cst);")
                .count(),
            2
        );
        assert!(cpp_callback.contains("clifford_optimizer_opaque_mutable(fixture.left) *="));
        assert!(cpp_callback.contains("clifford_scalar_right_power("));
        assert!(cpp_callback.contains("fixture.execution_witness ="));
        assert!(
            cpp_callback.contains("clifford_optimizer_opaque_const(fixture.execution_witness);")
        );
        let cpp_reset = cpp_worker
            .find("clifford_reset_execution_state(clifford_string.value());")
            .expect("Stim reset source");
        let cpp_barrier = cpp_worker
            .find("if (arguments.start_barrier) {")
            .expect("Stim barrier source");
        assert!(cpp_reset < cpp_barrier);
        assert!(cpp_worker.contains("clifford_output_digest("));
    }

    #[cfg(feature = "count-allocations")]
    #[test]
    fn equal_width_callbacks_allocate_nothing_at_every_contract_scale() {
        for kind in [
            CliffordWorkloadKind::Identity,
            CliffordWorkloadKind::NonIdentity,
        ] {
            for width in [10_000_u64, 100_000, 1_000_000, CLIFFORD_PUBLIC_CAP] {
                let descriptor = CliffordDescriptor::canonical(kind, width);
                let mut fixture =
                    CliffordStringFixture::prepare(kind, descriptor, width, 1).expect("fixture");
                fixture.reset_execution_state();
                let mut execution = None;
                let allocations = allocation_counter::measure(|| {
                    execution = Some(fixture.execute(1));
                });
                execution.expect("execution result").expect("execution");
                assert_eq!(allocations.count_total, 0, "kind={kind:?} width={width}");
                assert_eq!(allocations.bytes_total, 0, "kind={kind:?} width={width}");
            }
        }
    }
}
