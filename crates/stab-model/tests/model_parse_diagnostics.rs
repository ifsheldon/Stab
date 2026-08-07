#![allow(
    clippy::expect_used,
    reason = "diagnostic contract tests extract required failure payloads directly"
)]

use std::hint::black_box;

use stab_model::{
    ByteSpan, Circuit, DetectorErrorModel, DiagnosticSeverity, ModelDialect, ModelError,
    ParseError, ParseErrorCode, ParseErrorContext, ParseLimits, RepeatNestingLimit,
    SourceLineLimit,
};

#[test]
fn parse_error_codes_are_stable() {
    for (code, expected) in [
        (ParseErrorCode::InvalidUtf8Input, "invalid-utf8-input"),
        (ParseErrorCode::InvalidSyntax, "invalid-syntax"),
        (
            ParseErrorCode::MissingInstructionName,
            "missing-instruction-name",
        ),
        (ParseErrorCode::UnknownInstruction, "unknown-instruction"),
        (ParseErrorCode::InvalidTagEscape, "invalid-tag-escape"),
        (ParseErrorCode::UnterminatedTag, "unterminated-tag"),
        (
            ParseErrorCode::UnterminatedTagEscape,
            "unterminated-tag-escape",
        ),
        (
            ParseErrorCode::UnterminatedArgumentList,
            "unterminated-argument-list",
        ),
        (ParseErrorCode::InvalidNumber, "invalid-number"),
        (ParseErrorCode::InvalidArgument, "invalid-argument"),
        (
            ParseErrorCode::InvalidArgumentCount,
            "invalid-argument-count",
        ),
        (ParseErrorCode::InvalidTargetSyntax, "invalid-target-syntax"),
        (ParseErrorCode::InvalidTarget, "invalid-target"),
        (ParseErrorCode::InvalidTargetCount, "invalid-target-count"),
        (ParseErrorCode::InvalidRepeatBlock, "invalid-repeat-block"),
        (ParseErrorCode::MissingRepeatCount, "missing-repeat-count"),
        (ParseErrorCode::InvalidRepeatCount, "invalid-repeat-count"),
        (ParseErrorCode::IntegerOutOfRange, "integer-out-of-range"),
        (
            ParseErrorCode::UnexpectedRepeatTerminator,
            "unexpected-repeat-terminator",
        ),
        (
            ParseErrorCode::UnterminatedRepeatBlock,
            "unterminated-repeat-block",
        ),
    ] {
        assert_eq!(code.as_str(), expected);
    }
}

#[test]
fn circuit_parse_diagnostics_locate_crlf_utf8_arguments_targets_and_repeats() {
    assert_parse_error(
        Circuit::from_stim_str("H 0\r\nUNKNOWN 1\r\n").expect_err("unknown gate"),
        ParseErrorCode::UnknownInstruction,
        span(5, 7),
        "unknown gate UNKNOWN",
        "failed to parse line 2: unknown gate UNKNOWN",
    );
    assert_parse_error(
        Circuit::from_stim_str("H[é\\Q] 0\n").expect_err("invalid UTF-8-adjacent tag escape"),
        ParseErrorCode::InvalidTagEscape,
        span(4, 2),
        "invalid tag escape \\Q",
        "failed to parse line 1: invalid tag escape \\Q",
    );
    assert_parse_error(
        Circuit::from_stim_str("H[é").expect_err("unterminated UTF-8 tag"),
        ParseErrorCode::UnterminatedTag,
        span(4, 0),
        "unterminated tag",
        "failed to parse line 1: unterminated tag",
    );
    assert_parse_error(
        Circuit::from_stim_str("X_ERROR(abc) 0\n").expect_err("invalid argument"),
        ParseErrorCode::InvalidNumber,
        span(8, 3),
        "invalid argument abc",
        "failed to parse line 1: invalid argument abc",
    );
    assert_parse_error(
        Circuit::from_stim_str("H rec[-1]\n").expect_err("invalid target"),
        ParseErrorCode::InvalidTarget,
        span(2, 7),
        "gate H received invalid target rec[-1]",
        "failed to parse line 1: gate H received invalid target rec[-1]",
    );
    assert_parse_error(
        Circuit::from_stim_str("REPEAT nope {\n}\n").expect_err("invalid repeat count"),
        ParseErrorCode::InvalidRepeatCount,
        span(7, 1),
        "invalid repeat count",
        "failed to parse line 1: invalid repeat count",
    );
    assert_parse_error(
        Circuit::from_stim_str("\r\n  }\r\n").expect_err("unexpected terminator"),
        ParseErrorCode::UnexpectedRepeatTerminator,
        span(4, 1),
        "unexpected repeat block terminator",
        "unexpected repeat block terminator",
    );

    let unterminated = "REPEAT 2 {\r\n H 0";
    assert_parse_error(
        Circuit::from_stim_str(unterminated).expect_err("unterminated repeat"),
        ParseErrorCode::UnterminatedRepeatBlock,
        span(unterminated.len(), 0),
        "unterminated repeat block",
        "unterminated repeat block",
    );
}

#[test]
fn dem_parse_diagnostics_locate_crlf_utf8_numeric_and_eof_failures() {
    assert_parse_error(
        DetectorErrorModel::from_dem_str("error(0.1) D0\r\nwat 0\r\n")
            .expect_err("unknown DEM instruction"),
        ParseErrorCode::UnknownInstruction,
        span(15, 3),
        "unknown DEM instruction wat",
        "failed to parse line 2: invalid detector error model: unknown DEM instruction wat",
    );
    assert_parse_error(
        DetectorErrorModel::from_dem_str("error[é\\Q](0.1) D0\n")
            .expect_err("invalid UTF-8-adjacent DEM tag escape"),
        ParseErrorCode::InvalidTagEscape,
        span(8, 2),
        "invalid tag escape \\Q",
        "failed to parse line 1: invalid tag escape \\Q",
    );
    assert_parse_error(
        DetectorErrorModel::from_dem_str("error(nope) D0\n").expect_err("invalid DEM argument"),
        ParseErrorCode::InvalidNumber,
        span(6, 4),
        "invalid argument nope",
        "failed to parse line 1: invalid argument nope",
    );
    assert_parse_error(
        DetectorErrorModel::from_dem_str("error(0.1) D1152921504606846976\n")
            .expect_err("out-of-range DEM target"),
        ParseErrorCode::IntegerOutOfRange,
        span(11, 20),
        "relative detector target 1152921504606846976 exceeds 1152921504606846975",
        "failed to parse line 1: invalid detector error model: relative detector target 1152921504606846976 exceeds 1152921504606846975",
    );

    let unterminated = "repeat 2 {\r\n error(0.1) D0";
    let error =
        DetectorErrorModel::from_dem_str(unterminated).expect_err("unterminated DEM repeat");
    let diagnostic = error.parse_error().expect("typed DEM parse error");
    assert_eq!(diagnostic.code(), ParseErrorCode::UnterminatedRepeatBlock);
    assert_eq!(diagnostic.span(), span(unterminated.len(), 0));
    assert_eq!(
        diagnostic.context().dialect(),
        ModelDialect::DetectorErrorModel
    );
}

#[test]
fn parse_error_context_variants_preserve_typed_fields_and_dialect() {
    let cases = [
        (
            Circuit::from_stim_str("}\n").expect_err("unexpected circuit terminator"),
            ParseErrorContext::Model {
                dialect: ModelDialect::StimCircuit,
            },
        ),
        (
            ParseError::decode_utf8(ModelDialect::StimCircuit, b"H 0\n\xff")
                .expect_err("invalid circuit UTF-8")
                .into(),
            ParseErrorContext::Utf8 {
                dialect: ModelDialect::StimCircuit,
                valid_up_to: 4,
                error_length: Some(1),
            },
        ),
        (
            Circuit::from_stim_str("UNKNOWN 0\n").expect_err("unknown circuit instruction"),
            ParseErrorContext::Instruction {
                dialect: ModelDialect::StimCircuit,
                instruction: "UNKNOWN".to_string(),
            },
        ),
        (
            Circuit::from_stim_str("H 16777216\n").expect_err("out-of-range qubit target"),
            ParseErrorContext::DomainValue {
                dialect: ModelDialect::StimCircuit,
                kind: "qubit target",
                value: "16777216".to_string(),
            },
        ),
        (
            Circuit::from_stim_str("H(0.1) 0\n").expect_err("unexpected gate argument"),
            ParseErrorContext::ArgumentCount {
                dialect: ModelDialect::StimCircuit,
                instruction: "H".to_string(),
                expected: "0",
                actual: 1,
            },
        ),
        (
            Circuit::from_stim_str("X_ERROR(2) 0\n").expect_err("invalid gate argument"),
            ParseErrorContext::Argument {
                dialect: ModelDialect::StimCircuit,
                instruction: "X_ERROR".to_string(),
                argument: "2".to_string(),
            },
        ),
        (
            Circuit::from_stim_str("H rec[-1]\n").expect_err("invalid gate target"),
            ParseErrorContext::Target {
                dialect: ModelDialect::StimCircuit,
                instruction: "H".to_string(),
                target: "rec[-1]".to_string(),
            },
        ),
        (
            Circuit::from_stim_str("CX 0\n").expect_err("incomplete target pair"),
            ParseErrorContext::TargetCount {
                dialect: ModelDialect::StimCircuit,
                instruction: "CX".to_string(),
                actual: 1,
            },
        ),
        (
            DetectorErrorModel::from_dem_str("repeat 2 {\n").expect_err("unterminated DEM repeat"),
            ParseErrorContext::Model {
                dialect: ModelDialect::DetectorErrorModel,
            },
        ),
    ];

    for (error, expected_context) in cases {
        let context = error
            .parse_error()
            .expect("fixture should expose a typed parse context")
            .context();
        assert_eq!(context, &expected_context);
        assert_eq!(context.dialect(), expected_context.dialect());
    }
}

#[test]
fn parse_error_decode_utf8_accepts_valid_and_locates_invalid_sequences() {
    assert_eq!(
        ParseError::decode_utf8(ModelDialect::StimCircuit, "H é\n".as_bytes())
            .expect("valid UTF-8"),
        "H é\n"
    );

    for (dialect, input, expected_span, expected_length) in [
        (
            ModelDialect::StimCircuit,
            b"H 0\n\xffX".as_slice(),
            span(4, 1),
            Some(1),
        ),
        (
            ModelDialect::DetectorErrorModel,
            b"error(0.1) D0\n\xc3".as_slice(),
            span(14, 1),
            None,
        ),
    ] {
        let error = ParseError::decode_utf8(dialect, input).expect_err("invalid UTF-8");
        assert_eq!(error.code(), ParseErrorCode::InvalidUtf8Input);
        assert_eq!(error.span(), expected_span);
        assert_eq!(
            error.context(),
            &ParseErrorContext::Utf8 {
                dialect,
                valid_up_to: expected_span.byte_start(),
                error_length: expected_length,
            }
        );
        assert_eq!(error.context().dialect(), dialect);
    }
}

#[test]
fn circuit_byte_entrypoint_reports_invalid_utf8_at_the_first_invalid_sequence() {
    let circuit = b"H 0\n\xff";
    let circuit_error = Circuit::from_stim_bytes(circuit).expect_err("invalid circuit UTF-8");
    assert_parse_error(
        circuit_error,
        ParseErrorCode::InvalidUtf8Input,
        span(4, 1),
        "input is not valid UTF-8 text",
        "input is not valid UTF-8 text",
    );
}

#[test]
fn dem_byte_entrypoint_reports_invalid_utf8_at_the_first_invalid_sequence() {
    let dem = b"error(0.1) D0\n\xc3";
    let dem_error = DetectorErrorModel::from_dem_bytes(dem).expect_err("incomplete DEM UTF-8");
    let diagnostic = dem_error.parse_error().expect("typed UTF-8 error");
    assert_eq!(diagnostic.code(), ParseErrorCode::InvalidUtf8Input);
    assert_eq!(diagnostic.span(), span(dem.len() - 1, 1));
    assert_eq!(
        diagnostic.context(),
        &ParseErrorContext::Utf8 {
            dialect: ModelDialect::DetectorErrorModel,
            valid_up_to: dem.len() - 1,
            error_length: None,
        }
    );
}

#[test]
fn byte_parse_limit_entrypoints_apply_custom_limits_after_utf8_decode() {
    let line_limits = ParseLimits::default().with_source_line_limit(SourceLineLimit::new(2));
    Circuit::from_stim_bytes_with_limits(b"H 0\nM 0\n", line_limits)
        .expect("two circuit byte lines are admitted");
    DetectorErrorModel::from_dem_bytes_with_limits(
        b"error(0.1) D0\nshift_detectors 1\n",
        line_limits,
    )
    .expect("two DEM byte lines are admitted");

    for error in [
        Circuit::from_stim_bytes_with_limits(b"H 0\nM 0\nTICK\n", line_limits)
            .expect_err("third circuit byte line exceeds the policy"),
        DetectorErrorModel::from_dem_bytes_with_limits(
            b"error(0.1) D0\nshift_detectors 1\nlogical_observable L0\n",
            line_limits,
        )
        .expect_err("third DEM byte line exceeds the policy"),
    ] {
        let resource = error
            .resource_limit_error()
            .expect("byte parser line rejection should be structured");
        assert_eq!(resource.resource(), stab_model::ResourceKind::SourceLines);
        assert_eq!(resource.actual(), 3);
        assert_eq!(resource.limit(), 2);
    }

    let repeat_limits = ParseLimits::default().with_repeat_nesting_limit(
        RepeatNestingLimit::try_new(1).expect("one repeat level is safe"),
    );
    Circuit::from_stim_bytes_with_limits(b"REPEAT 1 {\nH 0\n}\n", repeat_limits)
        .expect("one circuit byte repeat level is admitted");
    DetectorErrorModel::from_dem_bytes_with_limits(
        b"repeat 1 {\nerror(0.1) D0\n}\n",
        repeat_limits,
    )
    .expect("one DEM byte repeat level is admitted");

    for error in [
        Circuit::from_stim_bytes_with_limits(b"REPEAT 1 {\nREPEAT 1 {\nH 0\n}\n}\n", repeat_limits)
            .expect_err("second circuit byte repeat level exceeds the policy"),
        DetectorErrorModel::from_dem_bytes_with_limits(
            b"repeat 1 {\nrepeat 1 {\nerror(0.1) D0\n}\n}\n",
            repeat_limits,
        )
        .expect_err("second DEM byte repeat level exceeds the policy"),
    ] {
        let resource = error
            .resource_limit_error()
            .expect("byte parser nesting rejection should be structured");
        assert_eq!(resource.resource(), stab_model::ResourceKind::RepeatNesting);
        assert_eq!(resource.actual(), 2);
        assert_eq!(resource.limit(), 1);
    }
}

#[test]
fn byte_parsers_report_earlier_repeat_limits_before_later_invalid_utf8() {
    let limits = ParseLimits::default()
        .with_repeat_nesting_limit(RepeatNestingLimit::try_new(1).expect("valid test limit"));
    for error in [
        Circuit::from_stim_bytes_with_limits(b"REPEAT 1 {\nREPEAT 1 {\n}\n}\nH \xff\n", limits)
            .expect_err("earlier circuit nesting rejection"),
        DetectorErrorModel::from_dem_bytes_with_limits(
            b"repeat 1 {\nrepeat 1 {\n}\n}\nerror(0.1) D\xff\n",
            limits,
        )
        .expect_err("earlier DEM nesting rejection"),
    ] {
        let resource = error
            .resource_limit_error()
            .expect("repeat limit should precede later invalid UTF-8");
        assert_eq!(resource.resource(), stab_model::ResourceKind::RepeatNesting);
        assert_eq!(resource.actual(), 2);
        assert_eq!(resource.limit(), 1);
        assert_eq!(resource.span(), span(11, 10));
    }
}

#[test]
fn parse_resource_failures_carry_the_rejected_source_span() {
    let limits = ParseLimits::default()
        .with_repeat_nesting_limit(RepeatNestingLimit::try_new(1).expect("valid test limit"));
    let circuit = "REPEAT 1 {\nREPEAT 1 {\n}\n}\n";
    let circuit_error =
        Circuit::from_stim_str_with_limits(circuit, limits).expect_err("circuit nesting limit");
    let circuit_resource = circuit_error
        .resource_limit_error()
        .expect("typed circuit resource failure");
    assert_eq!(circuit_resource.span(), span(11, 10));
    assert_eq!(
        circuit_error.to_string(),
        "failed to parse line 2: repeat nesting exceeds current limit 1"
    );

    let dem = "repeat 1 {\nrepeat 1 {\n}\n}\n";
    let dem_error =
        DetectorErrorModel::from_dem_str_with_limits(dem, limits).expect_err("DEM nesting limit");
    let dem_resource = dem_error
        .resource_limit_error()
        .expect("typed DEM resource failure");
    assert_eq!(dem_resource.span(), span(11, 10));
    assert_eq!(
        dem_error.to_string(),
        "invalid detector error model: DEM repeat nesting exceeds current limit 1"
    );
}

#[test]
fn circuit_diagnostics_keep_raw_token_regions_after_semantic_normalization() {
    let invalid_escape =
        Circuit::from_stim_str("H[\\Q\\C] 0\n").expect_err("first invalid escape wins");
    assert_code_and_span(
        &invalid_escape,
        ParseErrorCode::InvalidTagEscape,
        span(2, 2),
    );

    let zero_count = Circuit::from_stim_str("REPEAT 00 {\n}\n").expect_err("zero repeat count");
    assert_code_and_span(&zero_count, ParseErrorCode::InvalidRepeatCount, span(7, 2));

    let duplicate_pair =
        Circuit::from_stim_str("CX 01 01\n").expect_err("duplicate normalized pair");
    assert_code_and_span(&duplicate_pair, ParseErrorCode::InvalidTarget, span(3, 5));

    let non_finite = Circuit::from_stim_str("X_ERROR(1e309) 0\n").expect_err("non-finite argument");
    assert_code_and_span(&non_finite, ParseErrorCode::InvalidNumber, span(8, 5));
}

#[test]
fn typed_parser_diagnostics_preserve_established_human_errors_when_behavior_is_unchanged() {
    assert_eq!(
        Circuit::from_stim_str("1GATE 0\n")
            .expect_err("invalid instruction name")
            .to_string(),
        "failed to parse line 1: missing instruction name"
    );
    assert_eq!(
        Circuit::from_stim_str("X_ERROR(1e309) 0\n")
            .expect_err("non-finite argument")
            .to_string(),
        "failed to parse line 1: gate X_ERROR received invalid argument inf"
    );
    assert_eq!(
        DetectorErrorModel::from_dem_str("error(0.1) Q0\n")
            .expect_err("invalid DEM target")
            .to_string(),
        "failed to parse line 1: invalid detector error model: invalid DEM target \"Q0\""
    );
    assert_eq!(
        DetectorErrorModel::from_dem_str("repeat 2\n")
            .expect_err("repeat without block")
            .to_string(),
        "failed to parse line 1: invalid detector error model: unknown DEM instruction repeat"
    );
}

#[test]
fn dem_repeat_count_diagnostics_preserve_established_human_messages() {
    let malformed =
        DetectorErrorModel::from_dem_str("repeat nope {\n}\n").expect_err("malformed count");
    assert_eq!(
        malformed.to_string(),
        "failed to parse line 1: invalid detector error model: invalid repeat count \"nope\""
    );

    let out_of_range = DetectorErrorModel::from_dem_str("repeat 1152921504606846976 {\n}\n")
        .expect_err("count above the DEM text range");
    assert_eq!(
        out_of_range.to_string(),
        "failed to parse line 1: invalid detector error model: repeat count 1152921504606846976 exceeds 1152921504606846975"
    );
}

#[test]
fn missing_repeat_counts_keep_established_spans_after_structural_shortcuts() {
    for terminator in ["", "\n", "\r\n"] {
        for prefix in ["REPEAT ", "REPEAT[tag] "] {
            let input = format!("{prefix}{{{terminator}");
            let error = Circuit::from_stim_str(&input).expect_err("missing circuit repeat count");
            let diagnostic = error.parse_error().expect("typed circuit diagnostic");
            assert_eq!(
                (diagnostic.code(), diagnostic.span()),
                (ParseErrorCode::MissingRepeatCount, span(prefix.len(), 1)),
                "{input:?}",
            );
        }
        for prefix in ["repeat ", "repeat[tag] "] {
            let input = format!("{prefix}{{{terminator}");
            let error = DetectorErrorModel::from_dem_str(&input)
                .expect_err("missing detector-error-model repeat count");
            let diagnostic = error.parse_error().expect("typed DEM diagnostic");
            assert_eq!(
                (diagnostic.code(), diagnostic.span()),
                (
                    ParseErrorCode::MissingRepeatCount,
                    span(prefix.len() - 1, 0),
                ),
                "{input:?}",
            );
        }
    }
}

#[test]
fn byte_entrypoints_follow_source_order_and_accept_opaque_metadata_bytes() {
    let circuit_error =
        Circuit::from_stim_bytes(b"UNKNOWN 0\n\xff").expect_err("the earlier gate error must win");
    assert_eq!(
        circuit_error.to_string(),
        "failed to parse line 1: unknown gate UNKNOWN"
    );

    let dem_error = DetectorErrorModel::from_dem_bytes(b"unknown D0\n\xff")
        .expect_err("the earlier DEM instruction error must win");
    assert_eq!(
        dem_error.to_string(),
        "failed to parse line 1: invalid detector error model: unknown DEM instruction unknown"
    );

    Circuit::from_stim_bytes(b"H 0 # \xff\nM 0\n")
        .expect("Stim treats comment payload as opaque bytes");
    DetectorErrorModel::from_dem_bytes(b"error(0.1) D0 # \xff\n")
        .expect("Stim treats DEM comment payload as opaque bytes");

    let fused_text =
        Circuit::from_stim_bytes(b"H[tag] 0\nH[tag] 1\n").expect("equal text tags fuse");
    assert_eq!(fused_text.to_stim_bytes(), b"H[tag] 0 1\n");

    let fused_opaque =
        Circuit::from_stim_bytes(b"H[\xff] 0\nH[\xff] 1\n").expect("equal opaque tags fuse");
    assert_eq!(fused_opaque.to_stim_bytes(), b"H[\xff] 0 1\n");

    let distinct_opaque = Circuit::from_stim_bytes(b"H[\xff] 0\nH[\xfe] 1\n")
        .expect("distinct opaque tags remain distinct");
    assert_eq!(distinct_opaque.to_stim_bytes(), b"H[\xff] 0\nH[\xfe] 1\n");
}

#[test]
fn opaque_comments_do_not_shift_later_parser_diagnostics() {
    let circuit_input = b"H 0 # \xff\r\nUNKNOWN 1\r\n";
    let circuit_error = Circuit::from_stim_bytes(circuit_input).expect_err("later circuit error");
    assert_code_and_span(
        &circuit_error,
        ParseErrorCode::UnknownInstruction,
        span(9, 7),
    );

    let dem_input = b"error(0.1) D0 # \xff\r\nwat 0\r\n";
    let dem_error = DetectorErrorModel::from_dem_bytes(dem_input).expect_err("later DEM error");
    assert_code_and_span(&dem_error, ParseErrorCode::UnknownInstruction, span(19, 3));
}

#[test]
fn opaque_unterminated_tags_report_the_exact_original_eof() {
    let circuit_input = b"H[\xff";
    let circuit_error =
        Circuit::from_stim_bytes(circuit_input).expect_err("unterminated circuit tag");
    assert_code_and_span(
        &circuit_error,
        ParseErrorCode::UnterminatedTag,
        span(circuit_input.len(), 0),
    );

    let dem_input = b"error[\xff";
    let dem_error =
        DetectorErrorModel::from_dem_bytes(dem_input).expect_err("unterminated DEM tag");
    assert_code_and_span(
        &dem_error,
        ParseErrorCode::UnterminatedTag,
        span(dem_input.len(), 0),
    );
}

#[test]
fn attacker_controlled_parser_text_is_stored_as_bounded_utf8_excerpts() {
    let gate = "A".repeat(16_384);
    let gate_input = format!("{gate} 0\n");
    let gate_error = Circuit::from_stim_str(&gate_input).expect_err("unknown oversized gate");
    let gate_diagnostic = gate_error.parse_error().expect("typed gate diagnostic");
    assert_eq!(gate_diagnostic.span(), span(0, gate.len()));
    assert!(gate_diagnostic.message().len() <= 256);
    assert!(gate_error.to_string().len() <= 256);
    let instruction = match gate_diagnostic.context() {
        ParseErrorContext::Instruction { instruction, .. } => Some(instruction),
        _ => None,
    }
    .expect("unknown gate should retain instruction context");
    assert!(instruction.len() <= 256);
    assert!(instruction.ends_with(" [truncated; original length: 16384 bytes]"));

    let argument = "é".repeat(8_192);
    let argument_input = format!("X_ERROR({argument}) 0\n");
    let argument_error =
        Circuit::from_stim_str(&argument_input).expect_err("invalid oversized argument");
    let argument_diagnostic = argument_error
        .parse_error()
        .expect("typed argument diagnostic");
    assert_eq!(argument_diagnostic.span(), span(8, argument.len()));
    assert!(argument_diagnostic.message().len() <= 256);
    assert!(argument_error.to_string().len() <= 256);
    let excerpt = match argument_diagnostic.context() {
        ParseErrorContext::Argument { argument, .. } => Some(argument),
        _ => None,
    }
    .expect("invalid number should retain argument context");
    assert!(excerpt.len() <= 256);
    assert!(excerpt.ends_with(" [truncated; original length: 16384 bytes]"));
    assert!(std::str::from_utf8(excerpt.as_bytes()).is_ok());
}

#[test]
fn rejected_parser_diagnostic_allocation_is_independent_of_token_length() {
    let allocations = |length: usize| {
        let gate_input = format!("{} 0\n", "A".repeat(length));
        allocation_counter::measure(|| {
            let error =
                Circuit::from_stim_str(&gate_input).expect_err("oversized unknown gate rejects");
            black_box(error);
        })
    };
    let short = allocations(1_024);
    let long = allocations(16_384);
    assert!(
        long.count_total <= short.count_total + 2,
        "diagnostic allocation count scaled with rejected token: short={short:?}, long={long:?}"
    );
    assert!(
        long.bytes_total <= short.bytes_total + 512,
        "diagnostic allocation bytes scaled with rejected token: short={short:?}, long={long:?}"
    );
}

#[test]
fn circuit_byte_tags_expose_exact_opaque_bytes() {
    let circuit = Circuit::from_stim_bytes(b"H[\xff\\C\\B\\r\\n] 0\n").expect("opaque circuit tag");
    let instruction = circuit
        .items()
        .first()
        .and_then(stab_model::CircuitItem::as_instruction)
        .expect("fixture instruction");
    assert_eq!(instruction.tag_bytes(), Some(b"\xff]\\\r\n".as_slice()));

    let circuit_repeat =
        Circuit::from_stim_bytes(b"REPEAT[\xff] 1 {\n}\n").expect("opaque circuit repeat tag");
    let repeat = circuit_repeat
        .items()
        .first()
        .and_then(stab_model::CircuitItem::as_repeat_block)
        .expect("fixture repeat");
    assert_eq!(repeat.tag_bytes(), Some(b"\xff".as_slice()));
}

#[test]
fn circuit_byte_tags_serialize_without_loss() {
    let circuit = Circuit::from_stim_bytes(b"H[\xff\\C\\B\\r\\n] 0\n").expect("opaque circuit tag");
    assert_eq!(circuit.to_stim_bytes(), b"H[\xff\\C\\B\\r\\n] 0\n");

    let circuit_repeat =
        Circuit::from_stim_bytes(b"REPEAT[\xff] 1 {\n}\n").expect("opaque circuit repeat tag");
    assert_eq!(circuit_repeat.to_stim_bytes(), b"REPEAT[\xff] 1 {\n\n}\n");
}

#[test]
fn dem_byte_tags_expose_and_serialize_exact_opaque_bytes() {
    let dem = DetectorErrorModel::from_dem_bytes(b"error[\xff\\C\\B\\r\\n](0.1) D0\n")
        .expect("opaque DEM tag");
    let instruction = dem
        .items()
        .iter()
        .find_map(|item| match item {
            stab_model::DemItem::Instruction(instruction) => Some(instruction),
            stab_model::DemItem::RepeatBlock(_) => None,
        })
        .expect("fixture instruction");
    assert_eq!(instruction.tag_bytes(), Some(b"\xff]\\\r\n".as_slice()));
    assert_eq!(
        dem.to_dem_bytes(),
        b"error[\xff\\C\\B\\r\\n](0.1000000000000000055511151231257827) D0\n"
    );

    let dem_repeat = DetectorErrorModel::from_dem_bytes(b"repeat[\xff] 1 {\n}\n")
        .expect("opaque DEM repeat tag");
    let repeat = dem_repeat
        .items()
        .iter()
        .find_map(|item| match item {
            stab_model::DemItem::Instruction(_) => None,
            stab_model::DemItem::RepeatBlock(repeat) => Some(repeat),
        })
        .expect("fixture repeat");
    assert_eq!(repeat.tag_bytes(), Some(b"\xff".as_slice()));
    assert_eq!(dem_repeat.to_dem_bytes(), b"repeat[\xff] 1 {\n\n}\n");
}

#[test]
fn model_parse_end_spans_distinguish_lf_crlf_and_eof() {
    for (input, expected_span) in [
        ("H[tag\n", span(5, 1)),
        ("H[tag\r\n", span(5, 1)),
        ("H[tag", span(5, 0)),
    ] {
        let error = Circuit::from_stim_str(input).expect_err("unterminated circuit tag");
        assert_code_and_span(&error, ParseErrorCode::UnterminatedTag, expected_span);
    }
    for (input, expected_span) in [
        ("X_ERROR(0.1\n", span(11, 1)),
        ("X_ERROR(0.1\r\n", span(11, 1)),
        ("X_ERROR(0.1", span(11, 0)),
    ] {
        let error = Circuit::from_stim_str(input).expect_err("unterminated circuit arguments");
        assert_code_and_span(
            &error,
            ParseErrorCode::UnterminatedArgumentList,
            expected_span,
        );
    }
    for (input, expected_span) in [
        ("error[tag\n", span(9, 1)),
        ("error[tag\r\n", span(9, 1)),
        ("error[tag", span(9, 0)),
    ] {
        let error = DetectorErrorModel::from_dem_str(input).expect_err("unterminated DEM tag");
        assert_code_and_span(&error, ParseErrorCode::UnterminatedTag, expected_span);
    }
    for (input, expected_span) in [
        ("error(0.1\n", span(9, 1)),
        ("error(0.1\r\n", span(9, 1)),
        ("error(0.1", span(9, 0)),
    ] {
        let error =
            DetectorErrorModel::from_dem_str(input).expect_err("unterminated DEM arguments");
        assert_code_and_span(
            &error,
            ParseErrorCode::UnterminatedArgumentList,
            expected_span,
        );
    }
}

#[test]
fn circuit_and_dem_keep_stim_spacing_and_empty_argument_grammar() {
    Circuit::from_stim_str("X_ERROR() 0\n").expect("empty argument field means zero");
    DetectorErrorModel::from_dem_str("error() D0\n").expect("empty DEM argument field means zero");

    for input in [
        "H() 0\n",
        "H [tag] 0\n",
        "X_ERROR (0.1) 0\n",
        "H\u{a0}0\n",
        "DETECTOR rec[-1]\u{a0}rec[-2]\n",
    ] {
        Circuit::from_stim_str(input).expect_err("reject syntax outside the Stim grammar");
    }
    for input in [
        "error [tag](0.1) D0\n",
        "error (0.1) D0\n",
        "error(0.1)\u{a0}D0\n",
    ] {
        DetectorErrorModel::from_dem_str(input)
            .expect_err("reject DEM syntax outside the Stim grammar");
    }
}

#[test]
fn circuit_and_dem_accept_stim_inline_block_boundaries() {
    let circuit = Circuit::from_stim_str("REPEAT 2 { H 0\n} M 0\n").expect("inline circuit block");
    assert_eq!(circuit.to_stim_string(), "REPEAT 2 {\n    H 0\n}\nM 0\n");

    let dem = DetectorErrorModel::from_dem_str("repeat 2 { error(0.1) D0\n} detector(1) D0\n")
        .expect("inline DEM block");
    assert_eq!(
        dem.to_dem_string(),
        "repeat 2 {\n    error(0.1000000000000000055511151231257827) D0\n}\ndetector(1) D0\n"
    );
}

#[test]
fn circuit_and_dem_enforce_stim_numeric_token_and_repeat_ranges() {
    let sixty_three_zeroes = "0".repeat(63);
    let sixty_four_zeroes = "0".repeat(64);
    Circuit::from_stim_str(&format!("X_ERROR({sixty_three_zeroes}) 0\n"))
        .expect("Stim accepts a 63-byte number token");
    DetectorErrorModel::from_dem_str(&format!("error({sixty_three_zeroes}) D0\n"))
        .expect("Stim accepts a 63-byte DEM number token");

    let circuit_number = Circuit::from_stim_str(&format!("X_ERROR({sixty_four_zeroes}) 0\n"))
        .expect_err("Stim rejects a 64-byte number token");
    assert_code_and_span(&circuit_number, ParseErrorCode::InvalidNumber, span(8, 64));
    let dem_number = DetectorErrorModel::from_dem_str(&format!("error({sixty_four_zeroes}) D0\n"))
        .expect_err("Stim rejects a 64-byte DEM number token");
    assert_code_and_span(&dem_number, ParseErrorCode::InvalidNumber, span(6, 64));

    Circuit::from_stim_str("REPEAT 9223372036854775807 {\n}\n")
        .expect("Stim accepts the largest uint63 repeat count");
    let repeat =
        Circuit::from_stim_str("REPEAT 9223372036854775808 {\n}\n").expect_err("Stim rejects 2^63");
    assert_code_and_span(&repeat, ParseErrorCode::IntegerOutOfRange, span(7, 19));
}

#[test]
fn circuit_diagnostics_use_character_exact_spans_and_consistent_range_codes() {
    let name = Circuit::from_stim_str("é 0\n").expect_err("non-ASCII instruction start");
    assert_code_and_span(&name, ParseErrorCode::MissingInstructionName, span(0, 2));

    let repeat =
        Circuit::from_stim_str("REPEAT 1é {\n}\n").expect_err("non-ASCII repeat-count suffix");
    assert_code_and_span(&repeat, ParseErrorCode::InvalidRepeatCount, span(8, 2));

    let target = Circuit::from_stim_str("H 16777216\n").expect_err("out-of-range qubit target");
    let diagnostic = target.parse_error().expect("typed target range diagnostic");
    assert_eq!(diagnostic.code(), ParseErrorCode::IntegerOutOfRange);
    assert_eq!(diagnostic.span(), span(2, 8));
    assert_eq!(
        diagnostic.context(),
        &ParseErrorContext::DomainValue {
            dialect: ModelDialect::StimCircuit,
            kind: "qubit target",
            value: "16777216".to_string(),
        }
    );
}

#[test]
fn malformed_nested_headers_precede_repeat_depth_admission() {
    let circuit_prefix = "REPEAT 1 {\n".repeat(RepeatNestingLimit::HARD_MAX);
    let circuit = format!("{circuit_prefix}REPEAT nope {{\n");
    let circuit_error = Circuit::from_stim_str(&circuit).expect_err("invalid nested count");
    assert_eq!(
        circuit_error
            .parse_error()
            .expect("circuit syntax diagnostic")
            .code(),
        ParseErrorCode::InvalidRepeatCount
    );

    let dem_prefix = "repeat 1 {\n".repeat(RepeatNestingLimit::HARD_MAX);
    let dem = format!("{dem_prefix}repeat nope {{\n");
    let dem_error = DetectorErrorModel::from_dem_str(&dem).expect_err("invalid nested DEM count");
    assert_eq!(
        dem_error
            .parse_error()
            .expect("DEM syntax diagnostic")
            .code(),
        ParseErrorCode::InvalidRepeatCount
    );
}

#[test]
fn dem_lexing_precedes_semantic_probability_validation() {
    let prefix_error =
        DetectorErrorModel::from_dem_str("error(2) Q5\n").expect_err("target prefix is invalid");
    assert_code_and_span(
        &prefix_error,
        ParseErrorCode::InvalidTargetSyntax,
        span(9, 1),
    );

    let probability_error =
        DetectorErrorModel::from_dem_str("error(2) D0\n").expect_err("probability is invalid");
    assert_code_and_span(
        &probability_error,
        ParseErrorCode::InvalidArgument,
        span(6, 1),
    );

    let non_finite =
        DetectorErrorModel::from_dem_str("error(1e309) D0\n").expect_err("non-finite argument");
    assert_code_and_span(&non_finite, ParseErrorCode::InvalidNumber, span(6, 5));
}

#[test]
fn default_repeat_depth_rejects_the_first_unsafe_header_without_stack_overflow() {
    let circuit = nested_model("REPEAT 1 {\n", RepeatNestingLimit::HARD_MAX + 1, "H 0\n");
    let circuit_error = Circuit::from_stim_str(&circuit).expect_err("reject circuit depth 257");
    let circuit_resource = circuit_error
        .resource_limit_error()
        .expect("typed circuit repeat limit");
    assert_eq!(
        circuit_resource.actual(),
        (RepeatNestingLimit::HARD_MAX + 1) as u64
    );
    assert_eq!(
        circuit_resource.limit(),
        RepeatNestingLimit::HARD_MAX as u64
    );
    assert_eq!(
        circuit_resource.span(),
        span(
            RepeatNestingLimit::HARD_MAX * "REPEAT 1 {\n".len(),
            "REPEAT 1 {".len(),
        )
    );

    let dem = nested_model(
        "repeat 1 {\n",
        RepeatNestingLimit::HARD_MAX + 1,
        "error(1) D0\n",
    );
    let dem_error = DetectorErrorModel::from_dem_str(&dem).expect_err("reject DEM depth 257");
    let dem_resource = dem_error
        .resource_limit_error()
        .expect("typed DEM repeat limit");
    assert_eq!(
        dem_resource.actual(),
        (RepeatNestingLimit::HARD_MAX + 1) as u64
    );
    assert_eq!(dem_resource.limit(), RepeatNestingLimit::HARD_MAX as u64);
    assert_eq!(
        dem_resource.span(),
        span(
            RepeatNestingLimit::HARD_MAX * "repeat 1 {\n".len(),
            "repeat 1 {".len(),
        )
    );
}

fn assert_parse_error(
    error: ModelError,
    expected_code: ParseErrorCode,
    expected_span: ByteSpan,
    expected_message: &str,
    expected_human: &str,
) {
    let diagnostic = error.parse_error().expect("typed parse diagnostic");
    assert_eq!(diagnostic.code(), expected_code);
    assert_eq!(diagnostic.severity(), DiagnosticSeverity::Error);
    assert_eq!(diagnostic.span(), expected_span);
    assert_eq!(diagnostic.message(), expected_message);
    assert_eq!(error.to_string(), expected_human);
    assert_eq!(
        std::error::Error::source(&error).and_then(|source| source.downcast_ref::<ParseError>()),
        Some(diagnostic)
    );
}

fn assert_code_and_span(
    error: &ModelError,
    expected_code: ParseErrorCode,
    expected_span: ByteSpan,
) {
    let diagnostic = error.parse_error().expect("typed parse diagnostic");
    assert_eq!(diagnostic.code(), expected_code);
    assert_eq!(diagnostic.span(), expected_span);
}

fn nested_model(open: &str, depth: usize, leaf: &str) -> String {
    let mut model = String::with_capacity(
        open.len()
            .saturating_mul(depth)
            .saturating_add(leaf.len())
            .saturating_add(2_usize.saturating_mul(depth)),
    );
    for _ in 0..depth {
        model.push_str(open);
    }
    model.push_str(leaf);
    for _ in 0..depth {
        model.push_str("}\n");
    }
    model
}

fn span(byte_start: usize, byte_length: usize) -> ByteSpan {
    ByteSpan::try_new(byte_start, byte_length).expect("valid fixture span")
}
