use super::CargoTestSelector;

#[test]
fn accepts_the_extracted_algebra_package() {
    let selector = [
        "cargo",
        "test",
        "-p",
        "stab-algebra",
        "algebra_contract",
        "--quiet",
        "--exact",
    ]
    .map(String::from);

    let parsed = CargoTestSelector::parse(&selector).expect("algebra selector");

    assert!(parsed.is_exact());
    assert_eq!(
        parsed.run_args(),
        [
            "test",
            "-p",
            "stab-algebra",
            "--quiet",
            "--",
            "algebra_contract",
            "--exact",
        ]
    );
}

#[test]
fn accepts_the_extracted_model_package() {
    let selector = [
        "cargo",
        "test",
        "-p",
        "stab-model",
        "--test",
        "value_boundaries",
        "typed_model_values_preserve_stim_boundaries",
        "--quiet",
        "--exact",
    ]
    .map(String::from);

    let parsed = CargoTestSelector::parse(&selector).expect("model selector");

    assert!(parsed.is_exact());
    assert_eq!(
        parsed.run_args(),
        [
            "test",
            "-p",
            "stab-model",
            "--test",
            "value_boundaries",
            "--quiet",
            "--",
            "typed_model_values_preserve_stim_boundaries",
            "--exact",
        ]
    );
}
