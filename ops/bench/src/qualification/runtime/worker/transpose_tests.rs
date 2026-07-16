use super::transpose::{
    TRANSPOSE_DIMENSION_ALIGNMENT, TRANSPOSE_MAX_DIMENSION, TransposeFixture, TransposeKind,
    checked_transpose_shape, independently_computed_digest, independently_encoded_matrix,
};
use super::{WorkerError, semantic_digest};

const SMALL_WORK_ITEMS: u64 = 256 * 256;
const MEDIUM_WORK_ITEMS: u64 = 2_048 * 2_048;
const MAX_WORK_ITEMS: u64 = TRANSPOSE_MAX_DIMENSION * TRANSPOSE_MAX_DIMENSION;

const SMALL_INPUT_DIGEST: &str = "2a2a5f587d3c9fdb6fea43274c06ad453fcc76bbbcf6bcd9563991076cdf79da";
const MEDIUM_INPUT_DIGEST: &str =
    "15e610ea94b541a52446f7ff48ff9ca9560f8dbef5f96232806d0bcbff95f054";
const MAX_INPUT_DIGEST: &str = "d68c253c0ca01452ce0624f0fdeb67dd92c85b442034b4b0e574286f3c9f636e";
const IN_PLACE_ODD_OUTPUT_DIGEST: &str =
    "ff51fae4355733df7b3982f6daa235aba38d942553ee115340cc736c728421df";
const IN_PLACE_EVEN_OUTPUT_DIGEST: &str =
    "de2f8204bcc441d6b20f738e6574c3f5020f2ea07adaf9c23e0a59d183477a23";
const IN_PLACE_MAX_OUTPUT_DIGEST: &str =
    "d81fc0d732588e992e3f99542618f8cfa6affb401d5505b0c74efaab8c7f156a";
const ALLOCATING_ODD_OUTPUT_DIGEST: &str =
    "47f71e7254cf47c483f4574713cb1c8bee018181e19c218f7c19a4474a8c6373";
const ALLOCATING_EVEN_OUTPUT_DIGEST: &str =
    "6f0c4bdf0e761601a1545f63299a06bc37a7e59dcff098ac8ddc5619b9511641";
const ALLOCATING_MAX_OUTPUT_DIGEST: &str =
    "4b0e6174ee44ad29107bbe4e60df501c8d64c16d7e464e4d85063f2732391133";

#[test]
fn transpose_fixtures_bind_exact_inputs_and_independent_encoding() {
    for (work_items, expected_dimension, expected_bytes, expected_digest) in [
        (SMALL_WORK_ITEMS, 256, 8_208, SMALL_INPUT_DIGEST),
        (MEDIUM_WORK_ITEMS, 2_048, 524_304, MEDIUM_INPUT_DIGEST),
        (MAX_WORK_ITEMS, 16_384, 33_554_448, MAX_INPUT_DIGEST),
    ] {
        let fixture = TransposeFixture::prepare(TransposeKind::Allocating, work_items)
            .expect("source-owned transpose fixture");
        assert_eq!(fixture.dimension(), expected_dimension);
        assert_eq!(fixture.input_bytes, expected_bytes);
        assert_eq!(
            u64::try_from(
                independently_encoded_matrix(fixture.matrix())
                    .expect("independent canonical encoding")
                    .len()
            )
            .expect("bounded encoding length"),
            expected_bytes,
        );
        assert_eq!(
            independently_computed_digest(fixture.matrix())
                .expect("independently recomputed digest"),
            fixture.input_digest,
        );
        assert_eq!(semantic_digest(fixture.input_digest), expected_digest);
    }
}

#[test]
fn transpose_fixtures_bind_odd_even_and_maximum_outputs() {
    for (kind, iterations, work_items, expected) in [
        (
            TransposeKind::InPlace,
            1,
            SMALL_WORK_ITEMS,
            IN_PLACE_ODD_OUTPUT_DIGEST,
        ),
        (
            TransposeKind::InPlace,
            2,
            SMALL_WORK_ITEMS,
            IN_PLACE_EVEN_OUTPUT_DIGEST,
        ),
        (
            TransposeKind::InPlace,
            1,
            MAX_WORK_ITEMS,
            IN_PLACE_MAX_OUTPUT_DIGEST,
        ),
        (
            TransposeKind::Allocating,
            1,
            SMALL_WORK_ITEMS,
            ALLOCATING_ODD_OUTPUT_DIGEST,
        ),
        (
            TransposeKind::Allocating,
            2,
            SMALL_WORK_ITEMS,
            ALLOCATING_EVEN_OUTPUT_DIGEST,
        ),
        (
            TransposeKind::Allocating,
            1,
            MAX_WORK_ITEMS,
            ALLOCATING_MAX_OUTPUT_DIGEST,
        ),
    ] {
        let mut fixture =
            TransposeFixture::prepare(kind, work_items).expect("source-owned transpose fixture");
        fixture.execute(iterations).expect("transpose workload");
        assert_eq!(
            semantic_digest(
                fixture
                    .output_digest(iterations, work_items)
                    .expect("transpose output digest"),
            ),
            expected,
            "kind={kind:?} iterations={iterations} work={work_items}",
        );
    }
}

#[test]
fn transpose_fixture_is_nonsymmetric_and_preserves_allocating_source() {
    let mut allocating = TransposeFixture::prepare(TransposeKind::Allocating, SMALL_WORK_ITEMS)
        .expect("allocating fixture");
    let source = allocating.matrix().clone();
    let expected = source.transpose().expect("reference transpose");
    assert_ne!(source, expected);

    allocating.execute(1).expect("allocating transpose");
    assert_eq!(allocating.matrix(), &source);
    assert_eq!(allocating.result(), Some(&expected));

    let mut in_place = TransposeFixture::prepare(TransposeKind::InPlace, SMALL_WORK_ITEMS)
        .expect("in-place fixture");
    in_place.execute(1).expect("in-place transpose");
    assert_eq!(in_place.matrix(), &expected);
    in_place.execute(1).expect("second in-place transpose");
    assert_eq!(in_place.matrix(), &source);
}

#[test]
fn transpose_shape_rejects_every_frozen_invalid_class_before_allocation() {
    assert!(matches!(
        checked_transpose_shape(65_025),
        Err(WorkerError::TransposeDimensionMinimum { actual: 255, .. })
    ));
    assert!(matches!(
        checked_transpose_shape(65_537),
        Err(WorkerError::TransposeWorkNotSquare(65_537))
    ));
    assert!(matches!(
        checked_transpose_shape(66_049),
        Err(WorkerError::TransposeDimensionAlignment {
            actual: 257,
            alignment: TRANSPOSE_DIMENSION_ALIGNMENT,
        })
    ));
    assert!(matches!(
        checked_transpose_shape(276_889_600),
        Err(WorkerError::TransposeDimensionLimit {
            actual: 16_640,
            maximum: TRANSPOSE_MAX_DIMENSION,
        })
    ));
}

#[cfg(feature = "count-allocations")]
#[test]
fn transpose_timed_public_calls_preserve_exact_allocation_contracts() {
    for work_items in [SMALL_WORK_ITEMS, MEDIUM_WORK_ITEMS, MAX_WORK_ITEMS] {
        let mut in_place = TransposeFixture::prepare(TransposeKind::InPlace, work_items)
            .expect("in-place fixture");
        let in_place_allocations = allocation_counter::measure(|| {
            in_place.execute(1).expect("in-place transpose");
        });
        assert_eq!(
            in_place_allocations.count_total, 0,
            "in-place work={work_items} {in_place_allocations:?}",
        );
        assert_eq!(
            in_place_allocations.bytes_total, 0,
            "in-place work={work_items} {in_place_allocations:?}",
        );

        let mut allocating = TransposeFixture::prepare(TransposeKind::Allocating, work_items)
            .expect("allocating fixture");
        let allocating_allocations = allocation_counter::measure(|| {
            allocating.execute(1).expect("allocating transpose");
        });
        assert_eq!(
            allocating_allocations.count_total, 1,
            "allocating work={work_items} {allocating_allocations:?}",
        );
        assert_eq!(
            allocating_allocations.bytes_total,
            work_items / 8,
            "allocating work={work_items} {allocating_allocations:?}",
        );
    }
}
