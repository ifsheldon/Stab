//! `stab sample_dem` output-role and replay behavior beyond the pinned-Stim manifest rows.

use stab_records::{RecordFormat, write_ptb64_records_checked};

use super::support::{CliRun, Scratch, ptb64_words, run};

const TWO_ERRORS: &[u8] = b"error(0.25) D0 L0\nerror(0.25) D1\n";

fn sample_dem(args: &[&str], dem: &[u8]) -> CliRun {
    let mut argv = vec!["stab", "sample_dem"];
    argv.extend_from_slice(args);
    run(&argv, dem)
}

#[test]
fn sample_dem_writes_observables_to_obs_out_like_upstream() {
    let scratch = Scratch::new();
    let obs = scratch.path("obs.01");
    sample_dem(
        &[
            "--obs_out",
            &obs,
            "--out_format",
            "01",
            "--obs_out_format",
            "01",
            "--shots",
            "5",
            "--seed",
            "0",
        ],
        b"error(0) D0\nerror(1) D1 L2\n",
    )
    .assert_ok(b"01\n01\n01\n01\n01\n");
    assert_eq!(scratch.read("obs.01"), b"001\n001\n001\n001\n001\n");
}

#[test]
fn sample_dem_dets_output_keeps_observables_separate_like_upstream() {
    let scratch = Scratch::new();
    let obs = scratch.path("obs.dets");
    sample_dem(
        &[
            "--out_format",
            "dets",
            "--obs_out",
            &obs,
            "--obs_out_format",
            "dets",
            "--shots",
            "2",
        ],
        b"error(1) D0 L0\n",
    )
    .assert_ok(b"shot D0\nshot D0\n");
    assert_eq!(scratch.read("obs.dets"), b"shot L0\nshot L0\n");
}

#[test]
fn sample_dem_writes_error_records_to_err_out_like_upstream() {
    let scratch = Scratch::new();
    let err = scratch.path("errors.01");
    let obs = scratch.path("obs.01");
    sample_dem(
        &[
            "--err_out",
            &err,
            "--err_out_format",
            "01",
            "--obs_out",
            &obs,
            "--obs_out_format",
            "01",
            "--shots",
            "2",
        ],
        b"error(1) D0 L0\nerror(0) D1\nerror(1) D1\n",
    )
    .assert_ok(b"11\n11\n");
    assert_eq!(scratch.read("errors.01"), b"101\n101\n");
    assert_eq!(scratch.read("obs.01"), b"1\n1\n");
}

#[test]
fn sample_dem_writes_ptb64_detector_observable_and_error_streams() {
    let scratch = Scratch::new();
    let obs = scratch.path("obs.ptb64");
    let err = scratch.path("errors.ptb64");
    sample_dem(
        &[
            "--out_format",
            "ptb64",
            "--obs_out",
            &obs,
            "--obs_out_format",
            "ptb64",
            "--err_out",
            &err,
            "--err_out_format",
            "ptb64",
            "--shots",
            "64",
        ],
        b"error(1) D0 L0\nerror(0) D1\nerror(1) D1\n",
    )
    .assert_ok(ptb64_words(&[u64::MAX, u64::MAX]));
    assert_eq!(scratch.read("obs.ptb64"), ptb64_words(&[u64::MAX]));
    assert_eq!(
        scratch.read("errors.ptb64"),
        ptb64_words(&[u64::MAX, 0, u64::MAX])
    );
}

#[test]
fn sample_dem_replays_error_records_into_detector_and_observable_streams() {
    let scratch = Scratch::new();
    let replay = scratch.file("errors.dets", "shot M0\nshot M1\nshot M0 M1\nshot M0\n");
    let err_copy = scratch.path("errors.hits");
    let obs = scratch.path("obs.01");
    sample_dem(
        &[
            "--replay_err_in",
            &replay,
            "--replay_err_in_format",
            "dets",
            "--err_out",
            &err_copy,
            "--err_out_format",
            "hits",
            "--obs_out",
            &obs,
            "--obs_out_format",
            "01",
            "--shots",
            "3",
            "--seed",
            "0",
        ],
        TWO_ERRORS,
    )
    .assert_ok(b"10\n01\n11\n");
    assert_eq!(scratch.read("obs.01"), b"1\n0\n1\n");
    assert_eq!(scratch.read("errors.hits"), b"0\n1\n0,1\n");
}

#[test]
fn sample_dem_hits_replay_sets_duplicate_error_indices() {
    let scratch = Scratch::new();
    let replay = scratch.file("errors.hits", "0,0\n1,1\n");
    sample_dem(
        &[
            "--replay_err_in",
            &replay,
            "--replay_err_in_format=hits",
            "--shots=2",
            "--seed=0",
        ],
        b"error(0.25) D0\nerror(0.25) D1\n",
    )
    .assert_ok(b"10\n01\n");
}

#[test]
fn sample_dem_replays_ptb64_error_records() {
    let scratch = Scratch::new();
    let mut replay_input =
        write_ptb64_records_checked(&vec![vec![true, false]; 64]).expect("ptb64 replay fixture");
    replay_input.push(0xA5);
    let replay = scratch.file("errors.ptb64", replay_input);
    let obs = scratch.path("obs.01");
    sample_dem(
        &[
            "--replay_err_in",
            &replay,
            "--replay_err_in_format",
            "ptb64",
            "--obs_out",
            &obs,
            "--shots",
            "64",
            "--seed",
            "0",
        ],
        TWO_ERRORS,
    )
    .assert_ok("10\n".repeat(64));
    assert_eq!(scratch.read("obs.01"), "1\n".repeat(64).as_bytes());
}

#[test]
fn sample_dem_round_trips_r8_detector_observable_error_and_replay_streams() {
    let scratch = Scratch::new();
    let replay = scratch.file("errors.r8", [0, 1, 1, 0]);
    let obs = scratch.path("obs.r8");
    let err = scratch.path("errors-copy.r8");
    sample_dem(
        &[
            "--out_format",
            "r8",
            "--replay_err_in",
            &replay,
            "--replay_err_in_format",
            "r8",
            "--obs_out",
            &obs,
            "--obs_out_format",
            "r8",
            "--err_out",
            &err,
            "--err_out_format",
            "r8",
            "--shots",
            "2",
        ],
        b"error(0.25) D0 L0\nerror(0.25) D1 L1\n",
    )
    .assert_ok([0, 1, 1, 0]);
    assert_eq!(scratch.read("obs.r8"), [0, 1, 1, 0]);
    assert_eq!(scratch.read("errors-copy.r8"), [0, 1, 1, 0]);
}

#[test]
fn sample_dem_replays_stim_compatible_crlf_text_records() {
    for (format, replay_input, dem_input, shots, expected_stdout) in [
        ("01", &b"1\r\n"[..], &b"error(0.25) D0\n"[..], "1", "1\n"),
        ("hits", b"0\r\n", TWO_ERRORS, "1", "10\n"),
        (
            "dets",
            b"shot M1\r\n\r\n\n   shot M0\r\n\n",
            TWO_ERRORS,
            "2",
            "01\n10\n",
        ),
    ] {
        let scratch = Scratch::new();
        let replay = scratch.file(&format!("errors.{format}"), replay_input);
        sample_dem(
            &[
                "--replay_err_in",
                &replay,
                "--replay_err_in_format",
                format,
                "--shots",
                shots,
            ],
            dem_input,
        )
        .assert_ok(expected_stdout);
    }
}

#[test]
fn sample_dem_rejects_malformed_or_short_replay_input() {
    let scratch = Scratch::new();
    let one_error_dem =
        stab_model::DetectorErrorModel::from_dem_str("error(1) D0\n").expect("parse DEM");
    let one_error_plan = stab_engine::DemSamplingCompiler::new()
        .compile(&one_error_dem)
        .expect("compile DEM");
    let record_cap = replay_text_record_cap(&one_error_plan, RecordFormat::Dets, 2 * 1024 * 1024);
    let mut blank_prefix = vec![b'\n'; record_cap + 1];
    blank_prefix.extend_from_slice(b"shot M0\n");
    let oversized_msg = format!(
        "sample_dem replay text record has more than {record_cap} bytes, exceeding the memory budget"
    );
    for (format, replay_bytes, shots, dem, expected) in [
        (
            "dets",
            blank_prefix,
            "1",
            &b"error(1) D0\n"[..],
            oversized_msg.as_str(),
        ),
        (
            "ptb64",
            ptb64_words(&[u64::MAX]),
            "64",
            TWO_ERRORS,
            "ptb64 input expected at least 16 bytes",
        ),
        (
            "01",
            b"1\n".to_vec(),
            "2",
            b"error(1) D0\n",
            "replay error input has 1 records but --shots requested 2",
        ),
    ] {
        let replay = scratch.file(&format!("errors.{format}"), replay_bytes);
        sample_dem(
            &[
                "--replay_err_in",
                &replay,
                "--replay_err_in_format",
                format,
                "--shots",
                shots,
                "--memory-budget=2MiB",
            ],
            dem,
        )
        .assert_err(&[expected]);
    }
}

/// The bytes the CLI admits beside its compiled plan before reading a replay input of `format`:
/// the reader's fixed storage for 64-record batches and the smallest replay session.
fn replay_reader_floor(plan: &stab_engine::DemSamplingPlan, format: RecordFormat) -> u64 {
    use stab_engine::DemSamplerLimits;
    use stab_model::MemoryBudget;
    use stab_records::RecordStreamStorage;

    let batch = if format == RecordFormat::Ptb64 { 1 } else { 64 };
    let fixed = u64::try_from(RecordStreamStorage::new(format, plan.error_count(), batch).bytes())
        .expect("u64 reader bound");
    let free = plan
        .admit_replay_input(
            &DemSamplerLimits::new(MemoryBudget::from_bytes(u64::MAX)),
            fixed,
        )
        .expect("admit under an unbounded budget");
    u64::MAX - free
}

/// The per-record cap of a text replay input of `format` under `--memory-budget=budget`: the
/// largest whose transport buffer fits a quarter of what the plan, the reader's fixed storage,
/// and the smallest replay session leave.
fn replay_text_record_cap(
    plan: &stab_engine::DemSamplingPlan,
    format: RecordFormat,
    budget: u64,
) -> usize {
    let free = budget - plan.retained_bytes() - replay_reader_floor(plan, format);
    stab_records::RecordStreamStorage::max_text_record_bytes(u128::from(free / 4))
}

/// A replay admits its reader's storage and the smallest replay session against the command
/// budget before reading: the exact requirement succeeds and one byte less is refused with the
/// shared resource-limit diagnostic, and a text reader's cap admits its longest line exactly.
#[test]
fn sample_dem_replay_admits_reader_storage_against_the_command_budget() {
    use stab_engine::DemSamplingCompiler;
    use stab_model::{DetectorErrorModel, ParseLimits};

    const DEM: &[u8] = b"repeat 4096 {\n    error(0.5) D0\n    shift_detectors 1\n}\n";
    let (parsed, _) =
        DetectorErrorModel::from_dem_bytes_admitted(DEM, ParseLimits::default()).expect("parse");
    let plan = DemSamplingCompiler::new()
        .compile(&parsed)
        .expect("compile");
    let held = plan.retained_bytes();
    let scratch = Scratch::new();

    let mut b8 = vec![0_u8; 3 * 512];
    for (shot, byte) in [(0, 0), (1, 100), (2, 511)] {
        *b8.get_mut(shot * 512 + byte).expect("record byte") = 0b0010_0001;
    }
    let hits = "0,5\n4095\n\n";
    let longest_line = 5;
    for (format, records, expected) in [
        (
            RecordFormat::B8,
            b8,
            &b"shot D0 D5\nshot D800 D805\nshot D4088 D4093\n"[..],
        ),
        (
            RecordFormat::Hits,
            hits.as_bytes().to_vec(),
            &b"shot D0 D5\nshot D4095\nshot\n"[..],
        ),
    ] {
        let replay = scratch.file(&format!("errors.{}", format.as_str()), &records);
        let run_at = |budget: u64| {
            sample_dem(
                &[
                    "--replay_err_in",
                    &replay,
                    "--replay_err_in_format",
                    format.as_str(),
                    "--out_format",
                    "dets",
                    "--shots",
                    "3",
                    &format!("--memory-budget={budget}"),
                ],
                DEM,
            )
        };
        run_at(1 << 30).assert_ok(expected);

        let floor = held + replay_reader_floor(&plan, format);
        let text_bytes = if format == RecordFormat::Hits {
            // A cap of `longest_line` bytes needs a transport buffer of one byte more, and the
            // buffer receives a quarter of what the floor leaves.
            4 * (longest_line + 1)
        } else {
            0
        };
        let exact = floor + text_bytes;
        run_at(exact).assert_ok(expected);
        if format == RecordFormat::Hits {
            run_at(exact - 1).assert_err(&[&format!(
                "sample_dem replay text record has more than {} bytes",
                longest_line - 1
            )]);
        }
        run_at(floor - 1).assert_err(&[&format!(
            "the sampling session would require at least {} bytes, exceeding the {}-byte memory budget",
            floor - held,
            floor - 1 - held
        )]);
    }
}

#[test]
fn sample_dem_replay_ignores_extra_records_after_requested_shots() {
    let scratch = Scratch::new();
    let text = scratch.file("errors.01", "1\nnot-a-record\n");
    sample_dem(
        &["--replay_err_in", &text, "--shots", "1"],
        b"error(1) D0\n",
    )
    .assert_ok(b"1\n");

    let b8 = scratch.file("errors.b8", [0b0000_0001, 0, 0b0000_0001]);
    let nine_errors = (0..9)
        .map(|index| format!("error(1) D{index}\n"))
        .collect::<String>();
    sample_dem(
        &[
            "--replay_err_in",
            &b8,
            "--replay_err_in_format",
            "b8",
            "--shots",
            "1",
        ],
        nine_errors.as_bytes(),
    )
    .assert_ok(b"100000000\n");
}

#[test]
fn sample_dem_record_format_options_reject_the_stim_circuit_format() {
    for option in [
        "--obs_out_format",
        "--err_out_format",
        "--replay_err_in_format",
    ] {
        let result = sample_dem(&[option, "stim"], b"");
        assert_ne!(result.status, 0, "{option}");
        assert_eq!(result.stdout, b"", "{option}");
        result.assert_stderr_contains(&["invalid value 'stim'"]);
    }
}

/// Replay retention under a generous budget, sparse retention under a minimal budget, upfront
/// release when the budget fits only the session, and mid-stream release when later sparse hits
/// outgrow the budget must emit identical outputs and reject a malformed trailing record before
/// creating any output file.
#[test]
fn sample_dem_replay_matches_across_retained_and_released_record_stores() {
    use stab_engine::{DemReplayRetention, DemSamplerLimits, DemSamplingCompiler, ShotCount};
    use stab_model::{DetectorErrorModel, MemoryBudget, ParseLimits};
    use stab_records::PackedShotBatch;

    const DEM: &[u8] = b"repeat 64 {\n    error(0.5) D0 L0\n    shift_detectors 1\n}\n";
    const SHOTS: usize = 130;

    let (parsed, _) =
        DetectorErrorModel::from_dem_bytes_admitted(DEM, ParseLimits::default()).expect("parse");
    let plan = DemSamplingCompiler::new()
        .compile(&parsed)
        .expect("compile");
    let fits_store = |bytes: u64| {
        plan.replay_record_store(
            &DemSamplerLimits::new(MemoryBudget::from_bytes(bytes)),
            ShotCount::new(SHOTS as u64),
        )
        .is_retaining()
    };
    let (mut refused, mut sparse_budget) = (0_u64, MemoryBudget::DEFAULT.bytes());
    while refused + 1 < sparse_budget {
        let mid = refused + (sparse_budget - refused) / 2;
        if fits_store(mid) {
            sparse_budget = mid;
        } else {
            refused = mid;
        }
    }
    let released_budget = sparse_budget - 1;

    let dense_vecs: Vec<Vec<bool>> = (0..SHOTS)
        .map(|shot| (0..64).map(|bit| (shot + bit) % 2 == 0).collect())
        .collect();
    let dense_batch = PackedShotBatch::from_records(&dense_vecs, 64).expect("pack dense");
    let fits_dense = |bytes: u64| {
        let mut store = plan.replay_record_store(
            &DemSamplerLimits::new(MemoryBudget::from_bytes(bytes)),
            ShotCount::new(SHOTS as u64),
        );
        store.try_retain(dense_batch.view()).expect("try dense") == DemReplayRetention::Retained
    };
    let (mut dense_refused, mut dense_budget) = (sparse_budget, MemoryBudget::DEFAULT.bytes());
    while dense_refused + 1 < dense_budget {
        let mid = dense_refused + (dense_budget - dense_refused) / 2;
        if fits_dense(mid) {
            dense_budget = mid;
        } else {
            dense_refused = mid;
        }
    }

    let dense_records: String = dense_vecs
        .iter()
        .map(|row| {
            let mut bits: String = row.iter().map(|&bit| if bit { '1' } else { '0' }).collect();
            bits.push('\n');
            bits
        })
        .collect();
    let scratch = Scratch::new();
    let dense_path = scratch.file("dense.01", &dense_records);

    let run_with_budget = |tag: &str, path: &str, budget: u64| {
        let out = scratch.path(&format!("{tag}.dets"));
        let obs = scratch.path(&format!("{tag}.obs"));
        let err = scratch.path(&format!("{tag}.err"));
        let budget_arg = format!("--memory-budget={budget}");
        let shots_arg = SHOTS.to_string();
        let cli = sample_dem(
            &[
                "--replay_err_in",
                path,
                "--out",
                &out,
                "--obs_out",
                &obs,
                "--err_out",
                &err,
                "--shots",
                &shots_arg,
                &budget_arg,
            ],
            DEM,
        );
        (cli, out, obs, err)
    };

    let (kept, kept_out, kept_obs, kept_err) =
        run_with_budget("kept_dense", &dense_path, dense_budget);
    kept.assert_ok(b"");
    let (rewound, rewound_out, rewound_obs, rewound_err) =
        run_with_budget("rewound_dense", &dense_path, released_budget);
    rewound.assert_ok(b"");
    assert_eq!(
        std::fs::read(&rewound_out).expect("out"),
        std::fs::read(&kept_out).expect("out")
    );
    assert_eq!(
        std::fs::read(&rewound_obs).expect("obs"),
        std::fs::read(&kept_obs).expect("obs")
    );
    assert_eq!(
        std::fs::read(&rewound_err).expect("err"),
        std::fs::read(&kept_err).expect("err")
    );

    let sparse_records: String = (0..SHOTS)
        .map(|shot| {
            let mut bits = vec![b'0'; 64];
            if shot % 3 == 0 {
                *bits.get_mut(shot % 64).expect("64 bits") = b'1';
            }
            bits.push(b'\n');
            String::from_utf8(bits).expect("ascii")
        })
        .collect();
    let sparse_path = scratch.file("sparse.01", &sparse_records);
    let (sparse_kept, sparse_kept_out, sparse_kept_obs, sparse_kept_err) =
        run_with_budget("kept_sparse", &sparse_path, MemoryBudget::DEFAULT.bytes());
    sparse_kept.assert_ok(b"");
    let (sparse_rewound, sparse_rewound_out, sparse_rewound_obs, sparse_rewound_err) =
        run_with_budget("rewound_sparse", &sparse_path, released_budget);
    sparse_rewound.assert_ok(b"");
    assert_eq!(
        std::fs::read(&sparse_rewound_out).expect("out"),
        std::fs::read(&sparse_kept_out).expect("out")
    );
    assert_eq!(
        std::fs::read(&sparse_rewound_obs).expect("obs"),
        std::fs::read(&sparse_kept_obs).expect("obs")
    );
    assert_eq!(
        std::fs::read(&sparse_rewound_err).expect("err"),
        std::fs::read(&sparse_kept_err).expect("err")
    );

    // At sparse_budget (zero-hit sparse retention only), an empty first batch selects Sparse, a
    // hit-heavy second batch releases mid-stream, and a malformed final record in the skipped
    // suffix must still abort before activating any output path.
    let mut midstream_then_bad = format!("{}\n", "0".repeat(64)).repeat(64);
    midstream_then_bad.push_str(&format!("{}\n", "1".repeat(64)).repeat(SHOTS - 65));
    midstream_then_bad.push_str("bad\n");
    let bad_path = scratch.file("sparse_bad.01", &midstream_then_bad);
    for (tag, budget) in [
        ("bad_retained_until_tail", MemoryBudget::DEFAULT.bytes()),
        ("bad_sparse_midstream", sparse_budget),
        ("bad_released_upfront", released_budget),
    ] {
        let (bad, out, obs, err) = run_with_budget(tag, &bad_path, budget);
        assert_ne!(bad.status, 0, "{tag}");
        for path in [out, obs, err] {
            assert!(
                !std::path::Path::new(&path).exists(),
                "{tag} created {path}"
            );
        }
    }
}
