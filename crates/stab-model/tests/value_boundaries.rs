use std::str::FromStr;

use stab_model::{
    DiagnosticSeverity, MeasureRecordOffset, ModelError, Probability, QubitId, RepeatCount, Target,
    ValidationError, ValidationErrorCode,
};

#[test]
fn typed_model_values_preserve_stim_boundaries() {
    assert_eq!(
        QubitId::new((1 << 24) - 1).map(QubitId::get),
        Ok((1 << 24) - 1)
    );
    assert_eq!(
        QubitId::new(1 << 24),
        Err(ModelError::Validation(
            ValidationError::InvalidDomainValue {
                kind: "qubit id",
                value: (1 << 24).to_string(),
            }
        ))
    );

    assert_eq!(
        RepeatCount::try_new(0),
        Err(ModelError::Validation(
            ValidationError::InvalidDomainValue {
                kind: "repeat count",
                value: "0".to_owned(),
            }
        ))
    );
}

#[test]
fn probability_typed_boundary_matches_stim_argument_validation() {
    assert_eq!(Probability::try_new(0.0).map(Probability::get), Ok(0.0));
    assert_eq!(Probability::try_new(-0.0).map(Probability::get), Ok(-0.0));
    assert_eq!(Probability::try_new(1.0).map(Probability::get), Ok(1.0));

    for rejected in [-0.1, 1.1, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(
            Probability::try_new(rejected).is_err(),
            "expected {rejected:?} to be rejected"
        );
    }
}

#[test]
#[allow(
    clippy::expect_used,
    reason = "the test must inspect the typed error returned by exact rejected model fixtures"
)]
fn model_errors_expose_validation_from_real_model_failures() {
    let domain_error = QubitId::new(1 << 24).expect_err("reject oversized Stim qubit id");
    assert!(matches!(
        domain_error.validation_error(),
        Some(ValidationError::InvalidDomainValue {
            kind: "qubit id",
            ..
        })
    ));
    let domain_validation = domain_error
        .validation_error()
        .expect("oversized id is a validation failure");
    assert_eq!(
        domain_validation.code(),
        ValidationErrorCode::InvalidDomainValue
    );
    assert_eq!(domain_validation.severity(), DiagnosticSeverity::Error);
    assert!(domain_error.parse_error().is_none());
    assert!(domain_error.resource_limit_error().is_none());

    let gate_error = stab_model::Gate::from_name("missing")
        .expect_err("reject a name outside the closed Stim gate table");
    assert!(matches!(
        gate_error.validation_error(),
        Some(ValidationError::UnknownGate(name)) if name == "missing"
    ));
    assert_eq!(
        gate_error
            .validation_error()
            .expect("unknown gate is a validation failure")
            .code()
            .as_str(),
        "unknown-gate"
    );
}

#[test]
#[allow(
    clippy::expect_used,
    reason = "the exact accepted target spelling is the test fixture"
)]
fn target_parser_preserves_stim_negative_zero() {
    let target = Target::from_str("rec[-0]").expect("rec[-0] must parse");
    let offset = target
        .measurement_record_offset()
        .expect("rec[-0] must produce a measurement-record target");

    assert!(offset.is_negative_zero());
    assert_eq!(offset.get(), 0);
    assert_eq!(offset.stim_text().to_string(), "-0");
    assert_eq!(
        MeasureRecordOffset::try_new(0),
        Err(ModelError::Validation(
            ValidationError::InvalidDomainValue {
                kind: "measurement record offset",
                value: "0".to_owned(),
            }
        ))
    );
}

#[test]
#[allow(
    clippy::expect_used,
    reason = "every exact accepted target spelling is a test fixture"
)]
fn target_parser_distinguishes_typed_target_families() {
    let cases = [
        ("!23", "!23"),
        ("rec[-7]", "rec[-7]"),
        ("sweep[13]", "sweep[13]"),
        ("Y42", "Y42"),
        ("!Z9", "!Z9"),
        ("*", "*"),
    ];

    for (source, expected) in cases {
        let target = Target::from_str(source).expect("fixture target must parse");
        assert_eq!(target.to_string(), expected);
    }
}
