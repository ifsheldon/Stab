use super::run_from;

#[test]
fn detect_deprecation_warning_precedes_routing_errors_in_both_modes() {
    // Pinned Stim emits the deprecation warning while reading flags, before
    // the observable-routing combination error (WS3 item 9).
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        [
            "stab",
            "detect",
            "--prepend_observables",
            "--append_observables",
            "--shots",
            "1",
        ],
        b"M 0\nDETECTOR rec[-1]\n".as_slice(),
        &mut stdout,
        &mut stderr,
    );
    assert_eq!(status, 1);
    assert_eq!(stdout, b"");
    assert_eq!(
        String::from_utf8(stderr).unwrap(),
        "[DEPRECATION] Avoid using `--prepend_observables`. Data readers assume observables are appended, not prepended.\n\
         error: cannot combine --prepend_observables, --append_observables, or --obs_out\n"
    );

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        [
            "stab",
            "--error-format",
            "json",
            "detect",
            "--prepend_observables",
            "--append_observables",
            "--shots",
            "1",
        ],
        b"M 0\nDETECTOR rec[-1]\n".as_slice(),
        &mut stdout,
        &mut stderr,
    );
    assert_eq!(status, 1);
    assert_eq!(stdout, b"");
    assert_eq!(
        String::from_utf8(stderr).unwrap(),
        concat!(
            "{\"schema_version\":1,\"code\":\"deprecated-prepend-observables\",",
            "\"severity\":\"warning\",\"message\":\"`--prepend_observables` is deprecated\",",
            "\"span\":null,\"labels\":[],",
            "\"help\":\"Use appended observables or `--obs_out` instead.\",",
            "\"context\":{\"flag\":\"--prepend_observables\",\"replacement\":\"--obs_out\"}}\n",
            "{\"schema_version\":1,\"code\":\"conflicting-observable-routing\",",
            "\"severity\":\"error\",",
            "\"message\":\"cannot combine --prepend_observables, --append_observables, or --obs_out\",",
            "\"span\":null,\"labels\":[],\"help\":null,\"context\":{}}\n",
        )
    );
}
