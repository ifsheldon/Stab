use super::*;

#[test]
fn portable_simd_kernel_apis_follow_their_semantic_domains() {
    assert_eq!(
        classify_public_api_source(
            "stab_kernels_simd",
            Path::new("crates/stab-kernels-simd/src/lib.rs"),
            "stab_kernels_simd::xor_assign_block",
        ),
        Some(FeatureId::BitKernels)
    );
    assert_eq!(
        classify_public_api_source(
            "stab_kernels_simd",
            Path::new("crates/stab-kernels-simd/src/lib.rs"),
            "stab_kernels_simd::clifford_right_multiply_block",
        ),
        Some(FeatureId::Algebra)
    );
}
