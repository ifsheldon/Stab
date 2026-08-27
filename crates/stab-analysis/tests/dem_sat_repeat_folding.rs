#![allow(
    clippy::expect_used,
    clippy::panic_in_result_fn,
    reason = "Stim parity fixtures use direct assertions for compact diagnostics"
)]

use stab_analysis::{AnalysisResult, likeliest_error_sat_problem, shortest_error_sat_problem};
use stab_model::DetectorErrorModel;

const UNSAT: &str = "p wcnf 1 2 3\n3 -1 0\n3 1 0\n";
const SINGLE_OBSERVABLE_SHORTEST: &str = "p wcnf 1 2 3\n1 -1 0\n3 1 0\n";
const TWO_ERROR_SHORTEST: &str = "\
p wcnf 3 8 9
1 -1 0
9 1 2 -3 0
9 1 -2 3 0
9 -1 2 3 0
9 -1 -2 -3 0
1 -2 0
9 -3 0
9 1 0
";
const TWO_ERROR_LIKELIEST: &str = "\
p wcnf 3 8 81
10 -1 0
81 1 2 -3 0
81 1 -2 3 0
81 -1 2 3 0
81 -1 -2 -3 0
10 -2 0
81 -3 0
81 1 0
";
const HIGH_PROBABILITY_LIKELIEST: &str = "\
p wcnf 3 8 81
10 -1 0
81 1 2 -3 0
81 1 -2 3 0
81 -1 2 3 0
81 -1 -2 -3 0
10 2 0
81 -3 0
81 1 0
";
const HALF_PROBABILITY_LIKELIEST: &str = "\
p wcnf 3 7 71
10 -1 0
71 1 2 -3 0
71 1 -2 3 0
71 -1 2 3 0
71 -1 -2 -3 0
71 -3 0
71 1 0
";

fn dem(input: &str) -> AnalysisResult<DetectorErrorModel> {
    DetectorErrorModel::from_dem_str(input).map_err(Into::into)
}

#[test]
fn shortest_error_wcnf_common_matches_stim() -> AnalysisResult<()> {
    for source in ["", "error(0.1) D0\n", "error(0.1)\n"] {
        assert_eq!(
            shortest_error_sat_problem(&dem(source)?)?,
            UNSAT,
            "{source}"
        );
    }
    assert_eq!(
        shortest_error_sat_problem(&dem("error(0.1) L0\n")?)?,
        SINGLE_OBSERVABLE_SHORTEST
    );

    for source in [
        "error(0.1) D0 L0\nerror(0.1) D0\n",
        "error(1) D0 L0\nerror(0) D0\n",
        "error(0.001) D0 L0\nerror(0.999) D0\n",
        "error(0.1) D1000001 L1000001\nerror(0.2) D1000001\n",
        "repeat 100001 {\nerror(0.1) D0 L0\nerror(0.2) D0\n}\n",
        "repeat 100001 {\nrepeat 100001 {\nerror(0.1) D0 L0\nshift_detectors 0\nerror(0.2) D0\n}\n}\n",
    ] {
        assert_eq!(
            shortest_error_sat_problem(&dem(source)?)?,
            TWO_ERROR_SHORTEST,
            "{source}"
        );
    }

    assert_eq!(
        shortest_error_sat_problem(&dem("repeat 100001 {\nerror(0) L1000001\n}\n")?)?,
        SINGLE_OBSERVABLE_SHORTEST
    );
    assert_eq!(
        shortest_error_sat_problem(&dem(
            "repeat 2 {\nerror(0.1) D0\nshift_detectors 1\n}\nerror(0.1) D0 L0\n"
        )?)?,
        "p wcnf 3 7 8\n1 -1 0\n1 -2 0\n1 -3 0\n8 -1 0\n8 -2 0\n8 -3 0\n8 3 0\n"
    );
    Ok(())
}

#[test]
fn likeliest_error_wcnf_common_matches_stim() -> AnalysisResult<()> {
    assert_eq!(
        likeliest_error_sat_problem(&DetectorErrorModel::new(), 10)?,
        UNSAT
    );
    for (source, expected) in [
        ("error(0.1) D0 L0\nerror(0.1) D0\n", TWO_ERROR_LIKELIEST),
        (
            "error(0.1) D0 L0\nerror(0.9) D0\n",
            HIGH_PROBABILITY_LIKELIEST,
        ),
        (
            "error(0.1) D0 L0\nerror(0.5) D0\n",
            HALF_PROBABILITY_LIKELIEST,
        ),
    ] {
        assert_eq!(
            likeliest_error_sat_problem(&dem(source)?, 10)?,
            expected,
            "{source}"
        );
    }
    assert_eq!(
        likeliest_error_sat_problem(&dem("error(1) L0\n")?, 10)?,
        "p wcnf 1 2 21\n21 1 0\n21 1 0\n"
    );
    assert_eq!(
        likeliest_error_sat_problem(&dem("error(0.1) D0 L0\nerror(0.49) D0\n")?, 1,)?,
        "p wcnf 3 8 9\n1 -1 0\n9 1 2 -3 0\n9 1 -2 3 0\n9 -1 2 3 0\n9 -1 -2 -3 0\n9 -3 0\n9 1 0\n"
    );
    assert!(likeliest_error_sat_problem(&dem("error(0.1) L0\n")?, 0).is_err());

    for source in [
        "error(0) D9 L3\nerror(0.1) D0 L0\nerror(0.1) D0\n",
        "error(0.1) D1000001 L1000001\nerror(0.1) D1000001\n",
        "repeat 100001 {\nerror(0.1) D0 L0\n}\nerror(0.1) D0\n",
        "repeat 100001 {\nrepeat 100001 {\nerror(0.1) D0 L0\nshift_detectors 0\n}\n}\nerror(0.1) D0\n",
        "repeat 1000001 {\nerror(0) D0 L1000001\nshift_detectors 1\n}\nerror(0.1) D0 L0\nerror(0.1) D0\n",
    ] {
        assert_eq!(
            likeliest_error_sat_problem(&dem(source)?, 10)?,
            TWO_ERROR_LIKELIEST,
            "{source}"
        );
    }

    let even_high_probability =
        dem("repeat 100002 {\nerror(0.9) D0 L0\n}\nerror(0.1) D0\nerror(0.1) D0 L0\n")?;
    let reduced_even_high_probability = dem("error(0.1) D0 L0\nerror(0.1) D0\nerror(0.1) D0 L0\n")?;
    assert_eq!(
        likeliest_error_sat_problem(&even_high_probability, 100)?,
        likeliest_error_sat_problem(&reduced_even_high_probability, 100)?
    );

    let odd_high_probability =
        dem("repeat 100001 {\nerror(0.9) D0 L0\n}\nerror(0.1) D0\nerror(0.1) D0 L0\n")?;
    let reduced_odd_high_probability = dem("error(0.9) D0 L0\nerror(0.1) D0\nerror(0.1) D0 L0\n")?;
    assert_eq!(
        likeliest_error_sat_problem(&odd_high_probability, 100)?,
        likeliest_error_sat_problem(&reduced_odd_high_probability, 100)?
    );

    let even_deterministic =
        dem("repeat 100002 {\nerror(1) D0 L0\n}\nerror(0.1) D0\nerror(0.1) D0 L0\n")?;
    let reduced_even_deterministic = dem("error(0.1) D0\nerror(0.1) D0 L0\n")?;
    assert_eq!(
        likeliest_error_sat_problem(&even_deterministic, 100)?,
        likeliest_error_sat_problem(&reduced_even_deterministic, 100)?
    );

    let odd_deterministic =
        dem("repeat 100001 {\nrepeat 100001 {\nerror(1) D0 L0\nshift_detectors 0\n}\n}\n")?;
    assert_eq!(
        likeliest_error_sat_problem(&odd_deterministic, 100)?,
        likeliest_error_sat_problem(&dem("error(1) D0 L0\n")?, 100)?
    );
    Ok(())
}
