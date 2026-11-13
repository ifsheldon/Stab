//! Every command opens its inputs, validates its layout, and admits its work before it creates or
//! truncates an output role.

use super::super::support::{Scratch, ptb64_words, run};
use crate::MAX_CIRCUIT_INPUT_BYTES;

/// One rejected invocation. `@name` arguments become paths in a fresh scratch directory; every
/// seeded file must be byte-identical afterwards and no `absent` path may exist.
struct Preflight<'a> {
    args: &'a [&'a str],
    files: &'a [(&'a str, &'a [u8])],
    stdin: &'a [u8],
    stderr: &'a [&'a str],
    not_in_stderr: &'a [&'a str],
    absent: &'a [&'a str],
}

const DEFAULT: Preflight<'static> = Preflight {
    args: &[],
    files: &[],
    stdin: b"not-used",
    stderr: &[],
    not_in_stderr: &[],
    absent: &[],
};

impl Preflight<'_> {
    #[track_caller]
    fn assert_rejected_without_output_side_effects(&self) {
        let scratch = Scratch::new();
        for (name, bytes) in self.files {
            scratch.file(name, bytes);
        }
        let args = std::iter::once("stab".to_owned())
            .chain(self.args.iter().map(|arg| match arg.strip_prefix('@') {
                Some(name) => scratch.path(name),
                None => (*arg).to_owned(),
            }))
            .collect::<Vec<_>>();
        let result = run(
            &args.iter().map(String::as_str).collect::<Vec<_>>(),
            self.stdin,
        );

        result.assert_err(self.stderr);
        for part in self.not_in_stderr {
            assert!(
                !result.stderr_text().contains(part),
                "{args:?}: unexpected {part:?} in {}",
                result.stderr_text()
            );
        }
        for (name, bytes) in self.files {
            assert_eq!(scratch.read(name), *bytes, "{args:?} changed {name}");
        }
        for name in self.absent {
            assert!(!scratch.exists(name), "{args:?} created {name}");
        }
    }
}

const M2D_SWEEP_CIRCUIT: &[u8] =
    b"CX sweep[0] 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n";
const M2D_01: [&str; 3] = ["m2d", "--in_format=01", "--out_format=01"];

/// Inputs are opened, layouts validated, and work admitted before any output role is created or
/// truncated, for every command with file outputs.
#[test]
fn inputs_and_admission_precede_output_creation_and_truncation() {
    let m2d = |rest: &[&'static str]| [&M2D_01[..], rest].concat();
    let cases = [
        Preflight {
            args: &m2d(&["--out", "@output.01", "--circuit", "@missing.stim"]),
            stdin: b"0\n",
            stderr: &["failed to read", "missing.stim"],
            absent: &["output.01"],
            ..DEFAULT
        },
        Preflight {
            args: &m2d(&[
                "--in",
                "@missing.01",
                "--out",
                "@no-dir/output.01",
                "--circuit",
                "@c.stim",
            ]),
            files: &[("c.stim", b"M 0\nDETECTOR rec[-1]\n")],
            stderr: &["failed to read", "missing.01"],
            not_in_stderr: &["failed to write"],
            ..DEFAULT
        },
        // Paths open before the converter is built from an invalid circuit.
        Preflight {
            args: &m2d(&[
                "--in",
                "@missing.01",
                "--out",
                "@no-dir/output.01",
                "--circuit",
                "@c.stim",
            ]),
            files: &[("c.stim", b"DETECTOR rec[-1]\n")],
            stderr: &["failed to read", "missing.01"],
            not_in_stderr: &["rec[-1]"],
            ..DEFAULT
        },
        Preflight {
            args: &m2d(&[
                "--in",
                "@m.01",
                "--out",
                "@no-dir/output.01",
                "--circuit",
                "@c.stim",
            ]),
            files: &[("c.stim", b"DETECTOR rec[-1]\n"), ("m.01", b"")],
            stderr: &["failed to write", "output.01"],
            not_in_stderr: &["rec[-1]"],
            ..DEFAULT
        },
        // Every input opens before any output.
        Preflight {
            args: &m2d(&[
                "--in",
                "@m.01",
                "--out",
                "@no-dir/output.01",
                "--sweep",
                "@missing-sweep.01",
                "--circuit",
                "@c.stim",
            ]),
            files: &[("c.stim", M2D_SWEEP_CIRCUIT), ("m.01", b"0\n")],
            stderr: &["failed to read", "missing-sweep"],
            not_in_stderr: &["output.01"],
            ..DEFAULT
        },
        // Output roles are preflighted as one batch: a bad side output keeps the primary intact.
        Preflight {
            args: &m2d(&[
                "--in",
                "@m.01",
                "--out",
                "@output.01",
                "--sweep",
                "@sweep.01",
                "--obs_out",
                "@no-dir/obs.01",
                "--obs_out_format=01",
                "--circuit",
                "@c.stim",
            ]),
            files: &[
                ("c.stim", M2D_SWEEP_CIRCUIT),
                ("m.01", b"0\n"),
                ("sweep.01", b"0\n"),
                ("output.01", b"keep\n"),
            ],
            stderr: &["failed to write", "obs.01"],
            ..DEFAULT
        },
        Preflight {
            args: &m2d(&[
                "--in",
                "@m.01",
                "--out",
                "@output.01",
                "--obs_out",
                "@obs.01",
                "--obs_out_format=01",
                "--circuit",
                "@c.stim",
            ]),
            files: &[
                ("c.stim", b"REPEAT 1000001 {\nTICK\n}\n"),
                ("m.01", b""),
                ("output.01", b"primary sentinel\n"),
                ("obs.01", b"observable sentinel\n"),
            ],
            stderr: &["1000001 expanded instructions; current limit is 1000000"],
            ..DEFAULT
        },
        Preflight {
            args: &m2d(&[
                "--skip_reference_sample",
                "--in",
                "@m.01",
                "--out",
                "@output.01",
                "--obs_out",
                "@obs.01",
                "--obs_out_format=01",
                "--circuit",
                "@c.stim",
            ]),
            files: &[
                ("c.stim", b"CX 0 sweep[0]\nM 0\nDETECTOR rec[-1]\n"),
                ("m.01", b"0\n"),
                ("output.01", b"primary sentinel\n"),
                ("obs.01", b"observable sentinel\n"),
            ],
            stderr: &["CX target shape"],
            ..DEFAULT
        },
        Preflight {
            args: &["detect", "--shots=3", "--out", "@detectors.01"],
            files: &[("detectors.01", b"keep\n")],
            stdin: b"RX 0\nCX 0 sweep[0]\nOBSERVABLE_INCLUDE(0) X0\n",
            stderr: &["CX target shape"],
            ..DEFAULT
        },
        Preflight {
            args: &["analyze_errors", "--in", "@missing.stim"],
            stdin: b"",
            stderr: &["failed to read", "missing.stim"],
            ..DEFAULT
        },
        Preflight {
            args: &["analyze_errors", "--out", "@no-dir/output.dem"],
            stdin: b"M 0\nDETECTOR rec[-1]\n",
            stderr: &["failed to write", "output.dem"],
            ..DEFAULT
        },
        // The output path is checked before malformed input is parsed...
        Preflight {
            args: &["analyze_errors", "--out", "@no-dir/output.dem"],
            stdin: b"NOT_A_GATE\n",
            stderr: &["failed to write", "output.dem"],
            ..DEFAULT
        },
        // ...but an existing output is not truncated until the input is admitted.
        Preflight {
            args: &["analyze_errors", "--out", "@out.dem"],
            files: &[("out.dem", b"old output\n")],
            stdin: b"NOT_A_GATE\n",
            ..DEFAULT
        },
        Preflight {
            args: &[
                "sample_dem",
                "--replay_err_in",
                "@errors.01",
                "--out",
                "@output.01",
                "--shots",
                "32000001",
            ],
            files: &[("errors.01", b"0\n"), ("output.01", b"sentinel\n")],
            stdin: b"error(0) D0\n",
            stderr: &["64000002 replay work units; current limit is 64000000"],
            // Work admission happens before replay-prefix scanning.
            not_in_stderr: &["ended before shot"],
            ..DEFAULT
        },
        // Like Stim, zero shots still validate declared input paths.
        Preflight {
            args: &["sample_dem", "--shots", "0", "--in", "@missing.dem"],
            stdin: b"",
            stderr: &["failed to read", "missing.dem"],
            ..DEFAULT
        },
        Preflight {
            args: &[
                "sample_dem",
                "--shots",
                "0",
                "--replay_err_in",
                "@missing.01",
            ],
            stdin: b"",
            stderr: &["failed to read", "missing.01"],
            ..DEFAULT
        },
        Preflight {
            args: &[
                "sample_dem",
                "--replay_err_in",
                "@missing.01",
                "--shots",
                "64000001",
            ],
            stdin: b"",
            stderr: &["failed to read", "missing.01"],
            ..DEFAULT
        },
        Preflight {
            args: &[
                "convert",
                "--in_format=01",
                "--bits_per_shot=1",
                "--in",
                "@missing.01",
            ],
            stdin: b"1\n",
            stderr: &["failed to read", "missing.01"],
            ..DEFAULT
        },
        Preflight {
            args: &[
                "convert",
                "--in_format=01",
                "--types=M",
                "--circuit",
                "@missing.stim",
            ],
            stdin: b"1\n",
            stderr: &["failed to read", "missing.stim"],
            ..DEFAULT
        },
        Preflight {
            args: &["convert", "--in_format=01", "--dem", "@missing.dem"],
            stdin: b"1\n",
            stderr: &["failed to read", "missing.dem"],
            ..DEFAULT
        },
        Preflight {
            args: &[
                "convert",
                "--in_format=01",
                "--bits_per_shot=1",
                "--out",
                "@no-dir/out.01",
            ],
            stdin: b"1\n",
            stderr: &["failed to write", "out.01"],
            ..DEFAULT
        },
        Preflight {
            args: &[
                "convert",
                "--in_format=01",
                "--num_measurements=1",
                "--num_observables=1",
                "--obs_out",
                "@no-dir/obs.01",
            ],
            stdin: b"11\n",
            stderr: &["failed to write", "obs.01"],
            ..DEFAULT
        },
        // Circuit-to-circuit conversion rejects record-only roles without opening them.
        Preflight {
            args: &[
                "convert",
                "--in_format=stim",
                "--out_format=stim",
                "--obs_out",
                "@obs.01",
            ],
            files: &[("obs.01", b"keep\n")],
            stdin: b"H 0\n",
            stderr: &["--obs_out"],
            ..DEFAULT
        },
        Preflight {
            args: &[
                "convert",
                "--in_format=stim",
                "--out_format=stim",
                "--circuit",
                "@missing.stim",
            ],
            stdin: b"H 0\n",
            stderr: &["--circuit"],
            not_in_stderr: &["failed to read"],
            ..DEFAULT
        },
    ];
    for case in &cases {
        case.assert_rejected_without_output_side_effects();
    }
}

#[test]
fn malformed_replay_prefix_does_not_create_or_truncate_outputs() {
    let ptb64_prefix = ptb64_words(&[u64::MAX]);
    for (format, shots, replay) in [
        ("--replay_err_in_format=01", "--shots=2", &b"1\nx\n"[..]),
        ("--replay_err_in_format=b8", "--shots=2", &[1]),
        ("--replay_err_in_format=ptb64", "--shots=64", &ptb64_prefix),
    ] {
        let args = [
            "sample_dem",
            "--replay_err_in",
            "@errors",
            format,
            "--out",
            "@out",
            "--obs_out",
            "@obs",
            "--err_out",
            "@err",
            shots,
        ];
        let outputs = ["out", "obs", "err"];
        Preflight {
            args: &args,
            files: &[("errors", replay)],
            stdin: b"error(0.25) D0 L0\nerror(0.25) D1\n",
            absent: &outputs,
            ..DEFAULT
        }
        .assert_rejected_without_output_side_effects();
        Preflight {
            args: &args,
            files: &[
                ("errors", replay),
                ("out", b"sentinel\n"),
                ("obs", b"sentinel\n"),
                ("err", b"sentinel\n"),
            ],
            stdin: b"error(0.25) D0 L0\nerror(0.25) D1\n",
            ..DEFAULT
        }
        .assert_rejected_without_output_side_effects();
    }
}

#[test]
fn ptb64_output_requires_whole_64_shot_groups_before_creating_output() {
    for (args, stdin) in [
        (
            &["sample", "--shots=65", "--out_format", "ptb64"][..],
            &b"M 0\n"[..],
        ),
        (
            &["detect", "--shots=63", "--out_format=ptb64"],
            b"M 0\nDETECTOR rec[-1]\n",
        ),
        (
            &["sample_dem", "--shots=63", "--out_format=ptb64"],
            b"error(1) D0\n",
        ),
    ] {
        Preflight {
            args: &[args, &["--out", "@out.ptb64"]].concat(),
            stdin,
            stderr: &["shots must be a multiple of 64 to use ptb64 format"],
            absent: &["out.ptb64"],
            ..DEFAULT
        }
        .assert_rejected_without_output_side_effects();
    }
    Preflight {
        args: &[
            "convert",
            "--in_format",
            "01",
            "--out_format",
            "ptb64",
            "--bits_per_shot",
            "2",
        ],
        stdin: b"10\n",
        stderr: &["groups of 64"],
        ..DEFAULT
    }
    .assert_rejected_without_output_side_effects();
}

#[test]
fn oversized_model_inputs_are_rejected_before_reading() {
    let scratch = Scratch::new();
    let oversized = scratch.sparse_file("oversized", MAX_CIRCUIT_INPUT_BYTES + 1);
    for (args, label) in [
        (&["sample", "--in"][..], "sample circuit input"),
        (&["detect", "--in"], "detect circuit input"),
        (&["m2d", "--in_format=01", "--circuit"], "m2d circuit input"),
        (&["analyze_errors", "--in"], "analyze_errors input"),
        (&["sample_dem", "--in"], "sample_dem input"),
    ] {
        let argv = [&["stab"][..], args, &[oversized.as_str()]].concat();
        run(&argv, b"").assert_err(&[&format!("{label} is too large; limit is 67108864 bytes")]);
    }
}

#[test]
fn read_limited_input_rejects_special_file_after_limit() {
    let path = std::path::PathBuf::from("/dev/zero");
    if !path.exists() {
        return;
    }
    let mut stdin = std::io::empty();
    let error = crate::input::read_limited_input(Some(&path), &mut stdin, 1024, "test input")
        .expect_err("special file should be limited while reading");

    assert!(
        error
            .to_string()
            .contains("test input is too large; limit is 1024 bytes")
    );
}
