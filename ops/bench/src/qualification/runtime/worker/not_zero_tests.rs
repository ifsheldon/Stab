use super::not_zero::{
    NOT_ZERO_MAX_BITS, NOT_ZERO_MIN_BITS, NotZeroPattern, not_zero_fixture, not_zero_output_digest,
    simd_bits_not_zero,
};
use super::{WorkerError, semantic_digest};

#[test]
fn not_zero_fixture_binds_exact_scales_patterns_and_outputs() {
    let expected = [
        (
            NotZeroPattern::Early,
            10_000,
            1_256,
            "652aebf153201450c8fe9d3707aed8cb0ee9fee8f5332d88e2001c56cfd0838f",
            "13f255827af928e2e3cf98e7379be0b49c9ab0f5c1281014016fb945d9a99ce8",
        ),
        (
            NotZeroPattern::Early,
            640_000,
            80_000,
            "f2af8de388713368d12e7bf4188e96c030bf1c3e2906250672e2f2eee9370aa8",
            "d707ab0fd88e4dd51532cf41f85313698414a086866fbbea2bc8a1546878b56c",
        ),
        (
            NotZeroPattern::Early,
            40_960_000,
            5_120_000,
            "84118644943bed7c2aa82daafc7e8b8f2358d0e38ab07fd140c8aba466fb3ba4",
            "f1a2b6c7412dff1558bf67f1d81e314c32a97c1a810a6af8f2a75f52e278730d",
        ),
        (
            NotZeroPattern::Zero,
            10_000,
            1_256,
            "b6286dfe1dca80e14e17bbc6a371565900665697e8f4f2b22d30a303f804b537",
            "25ba7441093c190b2c669e6a68d2c190a9ea7bff8b092c5c0dfe39efd8ce1b2a",
        ),
        (
            NotZeroPattern::Zero,
            640_000,
            80_000,
            "60aace21d864e2176a3f43edcd21a970c401e36a0223c24d09a8d482e075aae0",
            "a034e07354d5546ecf746672938991820f607a5c1cb4aa3e7dffdf4bef154ec8",
        ),
        (
            NotZeroPattern::Zero,
            40_960_000,
            5_120_000,
            "080543f5fd6fe5ca816fbfc568988f74eb08c7477f433ccbdecbc16d62790ec8",
            "d6e69f348ef1df632ee8f7e8a31e092fac6835009a789066133595b2e78f6988",
        ),
        (
            NotZeroPattern::Late,
            10_000,
            1_256,
            "76618d8f234d913b3b6f99be0c83fca1e8a6eb3c5cdb6f622c06dccc7aaa2cc0",
            "8dd09f03893d3ea3e24e3f1e4ec3b002706f7d9cefdbfafc9b82ba51bcbb5263",
        ),
        (
            NotZeroPattern::Late,
            640_000,
            80_000,
            "61aace21da17e2176a3f445b0d21a9b0c41d536a0223c24deda8d482e075aae6",
            "b0fde3065145adda03aefd418d2e95500d365352476869b18f4b25ebf09f7f97",
        ),
        (
            NotZeroPattern::Late,
            40_960_000,
            5_120_000,
            "0b0543f60288e5ca816fc551a8988eb4e96d37477f433ccbe2cbc16d62790f06",
            "f016c905ce7505349916829d8c2ec5c9126a8254e5833c0a07bb34092d85b330",
        ),
    ];
    for (pattern, bit_count, input_bytes, input_digest, output_digest) in expected {
        let fixture = not_zero_fixture(bit_count, pattern).expect("source-owned fixture");
        assert_eq!(fixture.input_bytes, input_bytes);
        assert_eq!(semantic_digest(fixture.input_digest), input_digest);
        let checksum = simd_bits_not_zero(2, &fixture);
        assert_eq!(checksum, u64::from(pattern != NotZeroPattern::Zero) * 2);
        assert_eq!(
            semantic_digest(not_zero_output_digest(checksum, 2, bit_count, &fixture)),
            output_digest,
        );
    }
}

#[test]
fn not_zero_fixture_constructs_and_executes_the_accepted_maximum() {
    let maximum = not_zero_fixture(NOT_ZERO_MAX_BITS, NotZeroPattern::Late)
        .expect("maximum source-owned fixture");
    assert_eq!(maximum.input_bytes, 33_554_432);
    assert_eq!(
        semantic_digest(maximum.input_digest),
        "6ce3c25931cb0f6aee9c2dbe7f534bfba0b5722656ef9b35d9086091d9c60472",
    );
    let checksum = simd_bits_not_zero(1, &maximum);
    assert_eq!(checksum, 1);
    assert_eq!(
        semantic_digest(not_zero_output_digest(
            checksum,
            1,
            NOT_ZERO_MAX_BITS,
            &maximum,
        )),
        "526b1acd58d6aaa5d2dd53a5edacdc0b05f37ef65076587d43739e8eb4c979bd",
    );
}

#[cfg(feature = "count-allocations")]
#[test]
fn not_zero_timed_scans_allocate_nothing() {
    for pattern in [
        NotZeroPattern::Early,
        NotZeroPattern::Zero,
        NotZeroPattern::Late,
    ] {
        for bit_count in [10_000, 640_000, 40_960_000] {
            let fixture = not_zero_fixture(bit_count, pattern).expect("source-owned fixture");
            let allocations = allocation_counter::measure(|| {
                std::hint::black_box(simd_bits_not_zero(2, &fixture));
            });
            assert_eq!(
                allocations.count_total, 0,
                "pattern={pattern:?} bit_count={bit_count} {allocations:?}",
            );
            assert_eq!(
                allocations.bytes_total, 0,
                "pattern={pattern:?} bit_count={bit_count} {allocations:?}",
            );
        }
    }
}

#[test]
fn not_zero_fixture_rejects_widths_outside_the_source_owned_range() {
    assert!(matches!(
        not_zero_fixture(NOT_ZERO_MIN_BITS - 1, NotZeroPattern::Zero),
        Err(WorkerError::NotZeroWidthMinimum { .. })
    ));
    assert!(matches!(
        not_zero_fixture(NOT_ZERO_MAX_BITS + 1, NotZeroPattern::Late),
        Err(WorkerError::NotZeroWidthLimit { .. })
    ));
}
