use super::tests::test_receipt;
use super::*;

#[test]
fn receipt_comparator_sources_are_complete_unique_and_ordered() {
    let source = "a".repeat(64);
    let library = "b".repeat(64);
    let binary = "c".repeat(64);
    let receipt = test_receipt(&source, &library, &binary);

    let assert_invalid = |mutated: AdapterBuildReceipt| {
        assert!(!mutated.validates_report_identity(&source, &mutated.build_fingerprint, &binary,));
    };

    let mut missing = receipt.clone();
    missing.comparator_sources.pop();
    assert_invalid(missing);

    let mut extra = receipt.clone();
    extra.comparator_sources.push(AdapterComparatorSource {
        path: "benchmarks/stim_adapter/extra_contract.h".to_string(),
        sha256: source.clone(),
    });
    assert_invalid(extra);

    let mut duplicate = receipt.clone();
    let first = duplicate
        .comparator_sources
        .first()
        .expect("popcount comparator")
        .clone();
    *duplicate
        .comparator_sources
        .get_mut(1)
        .expect("XOR comparator") = first;
    assert_invalid(duplicate);

    let mut reordered = receipt.clone();
    reordered.comparator_sources.swap(0, 1);
    assert_invalid(reordered);

    let mut path_altered = receipt.clone();
    path_altered
        .comparator_sources
        .first_mut()
        .expect("popcount comparator")
        .path = "benchmarks/stim_adapter/renamed_contract.h".to_string();
    assert_invalid(path_altered);

    let mut content_altered = receipt;
    content_altered
        .comparator_sources
        .get_mut(1)
        .expect("XOR comparator")
        .sha256 = "d".repeat(64);
    assert_invalid(content_altered);
}

#[test]
fn group_comparator_sources_reject_cross_receipt_transplants() {
    let source = "a".repeat(64);
    let receipt = test_receipt(&source, &"b".repeat(64), &"c".repeat(64));
    let transplanted: Vec<super::super::group::ComparatorSourceContract> =
        serde_json::from_value(serde_json::json!([
            {"path": ADAPTER_SOURCE, "sha256": source},
            {
                "path": SIMD_BITS_XOR_COMPARATOR_SOURCE,
                "sha256": "d".repeat(64)
            }
        ]))
        .expect("cross-receipt comparator source contracts");

    assert!(!receipt.validates_comparator_sources(&transplanted));
}
