use super::*;

const DENSE_XOR_SMALL_INPUT_DIGEST: &str =
    "d7fbfcc618ad7e3fd8a616be64f8b41949214afbbca6b58514d40fa5ea7ac498";
const DENSE_XOR_MEDIUM_INPUT_DIGEST: &str =
    "7f2b0610db451711e538c7bea04e7cdbead09cc41c088ebfeb3da0788d53ca46";
const DENSE_XOR_LARGE_INPUT_DIGEST: &str =
    "43fe5c79be45a459124be3bd00a45b65dbc886a6915fe19b3a173d37abc088ee";
const DENSE_XOR_MAX_INPUT_DIGEST: &str =
    "b3a240e29cde0478904e22b3d6d60e31f4e8c7b457d8992bab1d4d0596cc2ae0";
const DENSE_XOR_ODD_OUTPUT_DIGEST: &str =
    "0a654f5fe059e663b6f2f6ddea1ab61b4fb0b85927dde926da88de95caff58d4";
const DENSE_XOR_EVEN_OUTPUT_DIGEST: &str =
    "b6623d77b32fe22daee0e7c30fcacdf3bc332854e7dcdf7d561a0da0325a3aa3";
const DENSE_XOR_MAX_OUTPUT_DIGEST: &str =
    "451ffe13a031a8f9656ff3e3a89c1bd224e0f1cb94193456e32ff2cd854395b8";

#[test]
fn dense_xor_fixture_binds_exact_scales() {
    let small = dense_xor_fixture(4_096).expect("small fixture");
    let medium = dense_xor_fixture(262_144).expect("medium fixture");
    let large = dense_xor_fixture(16_777_216).expect("large fixture");

    assert_eq!(small.input_bytes, 1_024);
    assert_eq!(medium.input_bytes, 65_536);
    assert_eq!(large.input_bytes, 4_194_304);
    assert_eq!(
        semantic_digest(small.input_digest),
        DENSE_XOR_SMALL_INPUT_DIGEST
    );
    assert_eq!(
        semantic_digest(medium.input_digest),
        DENSE_XOR_MEDIUM_INPUT_DIGEST
    );
    assert_eq!(
        semantic_digest(large.input_digest),
        DENSE_XOR_LARGE_INPUT_DIGEST
    );
}

#[test]
fn dense_xor_binds_odd_and_even_final_states() {
    let mut odd = dense_xor_fixture(4_096).expect("odd fixture");
    let initial_destination = odd.destination.words().to_vec();
    let initial_source = odd.source.words().to_vec();
    dense_xor(1, &mut odd).expect("odd XOR workload");

    assert_eq!(odd.source.words(), initial_source);
    for ((&actual, &destination), &source) in odd
        .destination
        .words()
        .iter()
        .zip(&initial_destination)
        .zip(&initial_source)
    {
        assert_eq!(actual, destination ^ source);
    }
    assert_eq!(
        semantic_digest(dense_xor_output_digest(&odd, 1, 4_096)),
        DENSE_XOR_ODD_OUTPUT_DIGEST
    );

    let mut even = dense_xor_fixture(4_096).expect("even fixture");
    dense_xor(2, &mut even).expect("even XOR workload");
    assert_eq!(even.destination.words(), initial_destination);
    assert_eq!(even.source.words(), initial_source);
    assert_eq!(
        semantic_digest(dense_xor_output_digest(&even, 2, 4_096)),
        DENSE_XOR_EVEN_OUTPUT_DIGEST
    );
}

#[test]
fn dense_xor_constructs_and_executes_the_accepted_maximum() {
    let mut maximum = dense_xor_fixture(DENSE_XOR_MAX_BITS).expect("maximum fixture");
    assert_eq!(maximum.input_bytes, DENSE_XOR_MAX_BITS / 4);
    assert_eq!(
        semantic_digest(maximum.input_digest),
        DENSE_XOR_MAX_INPUT_DIGEST
    );
    let source_before = byte_digest_words(maximum.source.words());
    dense_xor(1, &mut maximum).expect("maximum XOR workload");
    assert_eq!(byte_digest_words(maximum.source.words()), source_before);
    assert_eq!(
        semantic_digest(dense_xor_output_digest(&maximum, 1, DENSE_XOR_MAX_BITS,)),
        DENSE_XOR_MAX_OUTPUT_DIGEST
    );
}

#[cfg(feature = "count-allocations")]
#[test]
fn dense_xor_timed_mutation_allocates_nothing() {
    for bit_count in [4_096, 262_144, 16_777_216, DENSE_XOR_MAX_BITS] {
        let mut fixture = dense_xor_fixture(bit_count).expect("source-owned fixture");
        let allocations = allocation_counter::measure(|| {
            dense_xor(2, &mut fixture).expect("dense XOR workload");
        });

        assert_eq!(
            allocations.count_total, 0,
            "bit_count={bit_count} {allocations:?}"
        );
        assert_eq!(
            allocations.bytes_total, 0,
            "bit_count={bit_count} {allocations:?}"
        );
    }
}

#[test]
fn dense_xor_fixture_rejects_invalid_widths_before_allocation() {
    assert!(matches!(
        dense_xor_fixture(128),
        Err(WorkerError::DenseXorWidthMinimum { .. })
    ));
    assert!(matches!(
        dense_xor_fixture(257),
        Err(WorkerError::DenseXorWidthAlignment { .. })
    ));
    assert!(matches!(
        dense_xor_fixture(DENSE_XOR_MAX_BITS + DENSE_XOR_ALIGNMENT_BITS),
        Err(WorkerError::DenseXorWidthLimit { .. })
    ));
}
