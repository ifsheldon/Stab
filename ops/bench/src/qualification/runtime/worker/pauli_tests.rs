use stab_core::{PauliBasis, PauliSign};

use super::pauli::{PAULI_MAX_QUBITS, PauliMultiplyFixture, checked_pauli_shape};
use super::{WorkerError, semantic_digest};

const SMALL_INPUT_DIGEST: &str = "401b897ceb9c02fec1c57b15f76cdc45045fd551354c3dc5ae499e791aef22f4";
const MEDIUM_INPUT_DIGEST: &str =
    "51b8460e6069c3590ce2e25ee912a0ef92dfe1000a28aa4a1aa3b644ba0d402f";
const LARGE_INPUT_DIGEST: &str = "5babb5f0de800c6ed6c644d103b7a0d01ab22fa7696a363e9120c7cac8157fd9";
const MAX_INPUT_DIGEST: &str = "404403b9507220987eff4ee0fea6d6794029fd9bbda3c8b3ea5b4379cfb2d009";
const SMALL_ODD_OUTPUT_DIGEST: &str =
    "295e7945d9961ad35f77e614b5c3c9ae84f419db221f53f0e609eb77fe773269";
const SMALL_EVEN_OUTPUT_DIGEST: &str =
    "89e436e86731c707ad1baa48ca83f1d69d21fe61aed075ca60485642d0c4b0bd";
const MAX_OUTPUT_DIGEST: &str = "b3fce0417dc4a2c5c91c2d79fe36f7d67759056ad86261ed20f1a9ba4e9e1848";

#[test]
fn pauli_fixture_uses_the_frozen_splitmix_basis_and_sign_contract() {
    let fixture = PauliMultiplyFixture::prepare(65).expect("prepare fixture");
    assert_eq!(fixture.width(), 65);
    assert_eq!(fixture.left().sign(), PauliSign::Plus);
    assert_eq!(fixture.right().sign(), PauliSign::Minus);
    assert_eq!(fixture.left().get(0), Some(PauliBasis::X));
    assert_eq!(fixture.right().get(0), Some(PauliBasis::Z));
    assert_eq!(fixture.left().get(64), Some(PauliBasis::I));
    assert_eq!(fixture.right().get(64), Some(PauliBasis::I));
    assert_eq!(fixture.phase_checksum(), 0);
}

#[test]
fn pauli_fixture_restores_after_even_calls_and_changes_after_odd_calls() {
    let mut fixture = PauliMultiplyFixture::prepare(65).expect("prepare fixture");
    let initial = fixture.output_digest(0, 65).expect("initial output digest");

    fixture.execute(1).expect("odd execution");
    let odd = fixture.output_digest(1, 65).expect("odd output digest");
    assert_ne!(odd, initial);
    assert_ne!(fixture.phase_checksum(), 0);

    fixture.execute(1).expect("even execution");
    let even = fixture.output_digest(2, 130).expect("even output digest");
    assert_ne!(even, odd);
    assert_eq!(fixture.left().sign(), PauliSign::Plus);
    assert_eq!(fixture.right().sign(), PauliSign::Minus);
}

#[test]
fn pauli_shape_freezes_runtime_and_maximum_byte_counts() {
    assert_eq!(
        checked_pauli_shape(10_000).expect("small shape"),
        (10_000, 5_056)
    );
    assert_eq!(
        checked_pauli_shape(100_000).expect("medium shape"),
        (100_000, 50_048)
    );
    assert_eq!(
        checked_pauli_shape(1_000_000).expect("large shape"),
        (1_000_000, 500_032)
    );
    assert_eq!(
        checked_pauli_shape(PAULI_MAX_QUBITS).expect("maximum shape"),
        (PAULI_MAX_QUBITS, 524_320)
    );
}

#[test]
fn pauli_shape_rejects_zero_and_first_over_limit() {
    assert!(matches!(
        checked_pauli_shape(0),
        Err(WorkerError::PauliWidthMinimum { .. })
    ));
    assert!(matches!(
        checked_pauli_shape(PAULI_MAX_QUBITS + 1),
        Err(WorkerError::PauliWidthLimit { .. })
    ));
}

#[test]
fn pauli_fixture_matches_frozen_input_and_odd_even_output_receipts() {
    for (width, expected) in [
        (10_000, SMALL_INPUT_DIGEST),
        (100_000, MEDIUM_INPUT_DIGEST),
        (1_000_000, LARGE_INPUT_DIGEST),
        (PAULI_MAX_QUBITS, MAX_INPUT_DIGEST),
    ] {
        let fixture = PauliMultiplyFixture::prepare(width).expect("prepare input fixture");
        assert_eq!(semantic_digest(fixture.input_digest), expected);
    }

    let mut odd = PauliMultiplyFixture::prepare(10_000).expect("prepare odd fixture");
    odd.execute(1).expect("execute odd fixture");
    assert_eq!(
        semantic_digest(odd.output_digest(1, 10_000).expect("odd output")),
        SMALL_ODD_OUTPUT_DIGEST
    );

    let mut even = PauliMultiplyFixture::prepare(10_000).expect("prepare even fixture");
    even.execute(2).expect("execute even fixture");
    assert_eq!(
        semantic_digest(even.output_digest(2, 20_000).expect("even output")),
        SMALL_EVEN_OUTPUT_DIGEST
    );

    let mut maximum =
        PauliMultiplyFixture::prepare(PAULI_MAX_QUBITS).expect("prepare maximum fixture");
    maximum.execute(1).expect("execute maximum fixture");
    assert_eq!(
        semantic_digest(
            maximum
                .output_digest(1, PAULI_MAX_QUBITS)
                .expect("maximum output")
        ),
        MAX_OUTPUT_DIGEST
    );
}

#[cfg(feature = "count-allocations")]
#[test]
fn pauli_timed_public_calls_allocate_nothing_at_every_qualified_width() {
    for width in [10_000, 100_000, 1_000_000, PAULI_MAX_QUBITS] {
        let mut fixture = PauliMultiplyFixture::prepare(width).expect("prepare fixture");
        let mut execution = None;
        let allocations = allocation_counter::measure(|| {
            execution = Some(fixture.execute(1));
        });
        execution
            .expect("execution result")
            .expect("execute fixture");
        assert_eq!(allocations.count_total, 0, "width={width} {allocations:?}");
        assert_eq!(allocations.bytes_total, 0, "width={width} {allocations:?}");
    }
}
