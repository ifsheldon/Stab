#![allow(
    clippy::expect_used,
    reason = "capability contract fixtures use direct setup assertions"
)]

use std::collections::BTreeSet;

use stab_core::advanced::compat::CompiledSampler;
use stab_core::{
    CapabilitySet, Circuit, CompilationOperation, CompilationRequestFingerprint,
    EncodedSizeEstimate, Estimate, Gate, ModelDialect, ParseLimits, RecordEncoding, RecordFormat,
    advanced::records::{read_records, write_records},
    estimate_sampling_request,
};

const RICH_CIRCUIT: &str = "X_ERROR[π](0.12345641) 0\n\
M !1\n\
CX sweep[7] 2\n\
MPP !X0*Y1*!Z2\n\
DETECTOR[coord](-0, 1.25) rec[-0] rec[-1]\n\
REPEAT[loop] 3 {\n\
    H 3\n\
}\n";

#[test]
fn capability_set_is_generated_from_current_product_descriptors() {
    let capabilities = CapabilitySet::current();

    assert_eq!(
        capabilities.dialects().collect::<Vec<_>>(),
        vec![ModelDialect::StimCircuit, ModelDialect::DetectorErrorModel]
    );
    assert_eq!(
        capabilities
            .gates()
            .map(Gate::canonical_name)
            .collect::<Vec<_>>(),
        Gate::all().map(Gate::canonical_name).collect::<Vec<_>>()
    );
    assert_eq!(
        capabilities.record_formats().collect::<Vec<_>>(),
        vec![
            RecordFormat::ZeroOne,
            RecordFormat::B8,
            RecordFormat::R8,
            RecordFormat::Hits,
            RecordFormat::Dets,
            RecordFormat::Ptb64,
        ]
    );
    assert_eq!(CapabilitySet::SCHEMA_VERSION, 1);
    assert_eq!(CapabilitySet::STIM_COMPATIBILITY_VERSION, "1.16.0");
    for codec in capabilities.codecs() {
        assert!(codec.can_decode());
        assert!(codec.can_encode());
        assert_eq!(
            codec.requires_typed_layout(),
            codec.format() == RecordFormat::Dets
        );
    }
    assert_eq!(
        capabilities.selectable_backend_ids().collect::<Vec<_>>(),
        vec!["scalar"]
    );

    let operations = capabilities
        .compilation_operations()
        .map(|compiler| {
            (
                compiler.operation(),
                compiler.input_dialect(),
                compiler.compiler_schema_version(),
                compiler.request_fingerprint_schema_version(),
                compiler.has_configurable_limits(),
                compiler.supports_backend_selection(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        operations,
        vec![
            (
                CompilationOperation::Sampling,
                ModelDialect::StimCircuit,
                CompilationRequestFingerprint::SAMPLING_COMPILER_SCHEMA_VERSION,
                Some(CompilationRequestFingerprint::SCHEMA_VERSION),
                false,
                true,
            ),
            (
                CompilationOperation::MeasurementToDetection,
                ModelDialect::StimCircuit,
                1,
                None,
                true,
                false,
            ),
            (
                CompilationOperation::DetectionSampling,
                ModelDialect::StimCircuit,
                1,
                None,
                true,
                false,
            ),
            (
                CompilationOperation::DemSampling,
                ModelDialect::DetectorErrorModel,
                1,
                None,
                false,
                false,
            ),
        ]
    );
    assert_eq!(
        capabilities.default_parse_limits(ModelDialect::StimCircuit),
        ParseLimits::default()
    );
}

#[test]
fn record_codec_descriptors_are_unique_and_structurally_truthful() {
    let formats = RecordFormat::all().collect::<Vec<_>>();
    let names = formats
        .iter()
        .map(|format| format.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(formats.len(), 6);
    assert_eq!(names.len(), formats.len());

    assert_eq!(RecordFormat::ZeroOne.encoding(), RecordEncoding::Text);
    assert_eq!(RecordFormat::B8.encoding(), RecordEncoding::BytePacked);
    assert_eq!(RecordFormat::R8.encoding(), RecordEncoding::RunLength);
    assert_eq!(RecordFormat::Hits.encoding(), RecordEncoding::Text);
    assert_eq!(RecordFormat::Dets.encoding(), RecordEncoding::Text);
    assert_eq!(RecordFormat::Ptb64.encoding(), RecordEncoding::BitPlane64);
    assert_eq!(RecordFormat::ZeroOne.records_per_group(), 1);
    assert_eq!(RecordFormat::Ptb64.records_per_group(), 64);
    assert_eq!(
        RecordFormat::ZeroOne.estimate_output_bytes(3, 9),
        EncodedSizeEstimate::Exact(30)
    );
    assert_eq!(
        RecordFormat::B8.estimate_output_bytes(3, 9),
        EncodedSizeEstimate::Exact(6)
    );
    assert_eq!(
        RecordFormat::Ptb64.estimate_output_bytes(64, 9),
        EncodedSizeEstimate::Exact(72)
    );
    assert_eq!(
        RecordFormat::Ptb64.estimate_output_bytes(63, 9),
        EncodedSizeEstimate::Unknown
    );
    assert_eq!(
        RecordFormat::Dets.estimate_output_bytes(3, 9),
        EncodedSizeEstimate::Unknown
    );

    let records = (0usize..64)
        .map(|shot| {
            (0usize..9)
                .map(|bit| (shot + bit).is_multiple_of(3))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    for codec in CapabilitySet::current().codecs() {
        let format = codec.format();
        let encoded = write_records(&records, format).expect("encode registered record format");
        let decoded = read_records(&encoded, format, 9).expect("decode registered record format");
        assert_eq!(decoded, records, "codec {:?}", codec.format());
    }
}

#[test]
fn sampling_compilation_fingerprint_normalizes_source_spelling() {
    let canonical = Circuit::from_stim_str("R 0\nREPEAT 3 {\n    CNOT 0 1\n    M 0 1\n}\n")
        .expect("canonical request circuit");
    let alternate = Circuit::from_stim_str("r 0\r\nrepeat 3 {\r\ncx 0 1\r\nm 0 1\r\n}\r\n")
        .expect("alternate request circuit");
    let changed = Circuit::from_stim_str("R 0\nREPEAT 4 {\nCX 0 1\nM 0 1\n}\n")
        .expect("changed request circuit");

    let fingerprint = CompilationRequestFingerprint::for_sampling(&canonical);
    assert_eq!(
        fingerprint,
        CompilationRequestFingerprint::for_sampling(&alternate)
    );
    assert_ne!(
        fingerprint,
        CompilationRequestFingerprint::for_sampling(&changed)
    );
    assert_eq!(fingerprint.schema_version(), 1);
    assert_eq!(fingerprint.compiler_schema_version(), 1);
    assert_eq!(fingerprint.operation(), CompilationOperation::Sampling);
    assert_eq!(fingerprint.model_fingerprint(), canonical.fingerprint());
    assert_eq!(fingerprint.digest().len(), 32);
    assert_eq!(fingerprint.digest_hex().len(), 64);

    let frozen = CompilationRequestFingerprint::for_sampling(
        &Circuit::from_stim_str(RICH_CIRCUIT).expect("frozen request circuit"),
    );
    assert_eq!(
        frozen.digest_hex(),
        "7d8879179ed9fe0f4cbc4717a228037951248185f668a84599d293776809dc33"
    );
    let allocations = allocation_counter::measure(|| {
        std::hint::black_box(CompilationRequestFingerprint::for_sampling(&canonical));
    });
    assert_eq!(allocations.count_total, 0, "{allocations:?}");
    assert_eq!(allocations.bytes_total, 0, "{allocations:?}");
}

#[test]
fn sampling_request_estimate_counts_folded_and_expanded_work_without_execution() {
    let circuit = Circuit::from_stim_str("R 0\nREPEAT 5 {\n    X_ERROR(0.25) 0\n    M 0\n}\n")
        .expect("sampling estimate circuit");

    let zero_one = estimate_sampling_request(&circuit, 64, RecordFormat::ZeroOne);
    assert_eq!(zero_one.input_items(), Estimate::Exact(4));
    assert_eq!(zero_one.folded_traversal(), Estimate::Exact(4));
    assert_eq!(zero_one.expanded_operations(), Estimate::Exact(11));
    assert_eq!(zero_one.output_bytes(), Estimate::Exact(384));
    assert_eq!(zero_one.scratch_bytes(), Estimate::Unknown);
    assert_eq!(zero_one.resident_bytes(), Estimate::Unknown);
    assert_eq!(zero_one.work_units(), Estimate::Unknown);

    let b8 = estimate_sampling_request(&circuit, 64, RecordFormat::B8);
    assert_eq!(b8.output_bytes(), Estimate::Exact(64));

    let ptb64 = estimate_sampling_request(&circuit, 64, RecordFormat::Ptb64);
    assert_eq!(ptb64.output_bytes(), Estimate::Exact(40));
    let incomplete_ptb64 = estimate_sampling_request(&circuit, 65, RecordFormat::Ptb64);
    assert_eq!(incomplete_ptb64.output_bytes(), Estimate::Unknown);

    for sparse in [RecordFormat::R8, RecordFormat::Hits, RecordFormat::Dets] {
        assert_eq!(
            estimate_sampling_request(&circuit, 64, sparse).output_bytes(),
            Estimate::Unknown
        );
    }

    let expanded_overflow = Circuit::from_stim_str(
        "REPEAT 4294967296 {\n\
             REPEAT 4294967296 {\n\
                 M 0\n\
             }\n\
         }\n",
    )
    .expect("nested repeat overflow circuit");
    let overflow_estimate = estimate_sampling_request(&expanded_overflow, 1, RecordFormat::ZeroOne);
    assert_eq!(overflow_estimate.input_items(), Estimate::Exact(3));
    assert_eq!(overflow_estimate.folded_traversal(), Estimate::Exact(3));
    assert_eq!(overflow_estimate.expanded_operations(), Estimate::Unknown);
    assert_eq!(overflow_estimate.output_bytes(), Estimate::Unknown);

    let output_overflow = Circuit::from_stim_str("M 0\n").expect("output overflow circuit");
    assert_eq!(
        estimate_sampling_request(&output_overflow, usize::MAX, RecordFormat::ZeroOne)
            .output_bytes(),
        Estimate::Unknown
    );

    let allocations = allocation_counter::measure(|| {
        std::hint::black_box(estimate_sampling_request(
            &circuit,
            64,
            RecordFormat::ZeroOne,
        ));
    });
    assert_eq!(allocations.count_total, 0, "{allocations:?}");
    assert_eq!(allocations.bytes_total, 0, "{allocations:?}");
}

#[test]
fn sampling_compilation_is_deterministic_and_preserves_circuit_semantics() {
    let circuit = Circuit::from_stim_str(
        "X 0\n\
         M 0\n\
         R 0\n\
         M 0\n\
         REPEAT 2 {\n\
             X 0\n\
             M 0\n\
         }\n",
    )
    .expect("sampling compilation circuit");
    let model_before = circuit.fingerprint();

    let first = CompiledSampler::compile(&circuit).expect("first compilation");
    let second = CompiledSampler::compile(&circuit).expect("second compilation");

    assert_eq!(first, second);
    assert_eq!(circuit.fingerprint(), model_before);
    assert_eq!(
        first.sample_zero_one_with_seed(3, Some(5)),
        vec![
            vec![true, false, true, false],
            vec![true, false, true, false],
            vec![true, false, true, false],
        ]
    );
}
