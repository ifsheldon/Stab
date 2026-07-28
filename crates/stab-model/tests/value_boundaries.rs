use std::str::FromStr;

use stab_model::{MeasureRecordOffset, ModelError, Probability, QubitId, RepeatCount, Target};

#[test]
fn typed_model_values_preserve_stim_boundaries() {
    assert_eq!(
        QubitId::new((1 << 24) - 1).map(QubitId::get),
        Ok((1 << 24) - 1)
    );
    assert_eq!(
        QubitId::new(1 << 24),
        Err(ModelError::InvalidDomainValue {
            kind: "qubit id",
            value: (1 << 24).to_string(),
        })
    );

    assert_eq!(
        RepeatCount::try_new(0),
        Err(ModelError::InvalidDomainValue {
            kind: "repeat count",
            value: "0".to_owned(),
        })
    );
    assert!(Probability::try_new(f64::NAN).is_err());
    assert!(Probability::try_new(-0.0).is_ok());
    assert!(Probability::try_new(1.0).is_ok());
    assert!(Probability::try_new(f64::INFINITY).is_err());
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
        Err(ModelError::InvalidDomainValue {
            kind: "measurement record offset",
            value: "0".to_owned(),
        })
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
