use std::collections::BTreeMap;
use std::ffi::OsString;

use super::case::{PreparedFile, compare_exact_outputs, substitute_args};
use super::*;
use crate::e2e::data::{OutputKey, OutputWitness};
use crate::e2e::model::{FileRole, OutputContract};

#[test]
fn selection_rejects_unmatched_filters_and_keeps_source_order() {
    let root = RepoRoot::resolve(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .as_path(),
    )
    .expect("root");
    let loaded = load_suite(&root).expect("suite");
    let selected = select_cases(
        &loaded.suite,
        &["convert".to_string(), "generate.surface-large".to_string()],
    )
    .expect("selection");
    assert_eq!(
        selected
            .iter()
            .map(|case| case.id.as_str())
            .collect::<Vec<_>>(),
        [
            "generate.surface-large",
            "convert.sparse-r8-wide",
            "convert.dense-b8-to-01",
            "convert.typed-dets-wide"
        ]
    );
    assert!(select_cases(&loaded.suite, &["missing".to_string()]).is_err());
}

#[test]
fn placeholders_are_exact_and_unknown_names_fail() {
    let files = vec![PreparedFile {
        name: "obs".to_string(),
        role: FileRole::Output,
        path: PathBuf::from("/tmp/obs"),
        output: Some(OutputContract::Exact { minimum_bytes: 0 }),
    }];
    assert_eq!(
        substitute_args(&["--obs_out".to_string(), "{file:obs}".to_string()], &files)
            .expect("substitute"),
        [OsString::from("--obs_out"), OsString::from("/tmp/obs")]
    );
    assert!(substitute_args(&["{file:missing}".to_string()], &files).is_err());
}

fn witness(byte: u8) -> OutputWitness {
    OutputWitness {
        bytes: 1,
        sha256: byte.to_string(),
        one_bits: None,
    }
}

#[test]
fn exact_output_comparison_covers_exact_files_and_every_pipeline_step() {
    let files = vec![
        PreparedFile {
            name: "obs".to_string(),
            role: FileRole::Output,
            path: PathBuf::from("/tmp/obs"),
            output: Some(OutputContract::Exact { minimum_bytes: 0 }),
        },
        PreparedFile {
            name: "records".to_string(),
            role: FileRole::Output,
            path: PathBuf::from("/tmp/records"),
            output: Some(OutputContract::Records {
                format: crate::e2e::model::RecordFormat::B8,
                records: 1,
                bits: 1,
                minimum_one_bits: 0,
                maximum_one_fraction: 1.0,
            }),
        },
    ];
    let exact = OutputContract::Exact { minimum_bytes: 0 };
    let stim = BTreeMap::from([
        (OutputKey::Stdout, witness(1)),
        (OutputKey::File("obs".to_string()), witness(2)),
        (OutputKey::File("records".to_string()), witness(3)),
        (OutputKey::Step(0), witness(4)),
    ]);
    compare_exact_outputs(&exact, &files, &stim, &stim).expect("identical outputs");

    let mut stochastic = stim.clone();
    stochastic.insert(OutputKey::File("records".to_string()), witness(9));
    compare_exact_outputs(&exact, &files, &stim, &stochastic)
        .expect("record contracts are not compared byte for byte");

    for (key, value) in [
        (OutputKey::Stdout, Some(witness(9))),
        (OutputKey::File("obs".to_string()), Some(witness(9))),
        (OutputKey::Step(0), Some(witness(9))),
        (OutputKey::Step(1), Some(witness(4))),
        (OutputKey::File("obs".to_string()), None),
    ] {
        let mut stab = stim.clone();
        match value {
            Some(value) => stab.insert(key.clone(), value),
            None => stab.remove(&key),
        };
        let error =
            compare_exact_outputs(&exact, &files, &stim, &stab).expect_err("exact drift must fail");
        assert!(error.to_string().contains(&key.to_string()), "{error}");
    }
}

#[test]
fn cargo_messages_name_the_reported_release_stab_executable() {
    let artifact = r#"{"reason":"compiler-artifact","package_id":"path+file:///w/crates/stab-cli#0.2.0","manifest_path":"/w/crates/stab-cli/Cargo.toml","target":{"kind":["bin"],"crate_types":["bin"],"name":"stab","src_path":"/w/crates/stab-cli/src/main.rs","edition":"2024","doc":true,"doctest":false,"test":true},"profile":{"opt_level":"3","debuginfo":0,"debug_assertions":false,"overflow_checks":false,"test":false},"features":[],"filenames":["/elsewhere/release/stab"],"executable":"/elsewhere/release/stab","fresh":true}"#;
    let finished = r#"{"reason":"build-finished","success":true}"#;
    assert_eq!(
        stab_executable_from_cargo_messages(format!("{artifact}\n{finished}\n").as_bytes())
            .expect("executable"),
        PathBuf::from("/elsewhere/release/stab")
    );
    assert!(stab_executable_from_cargo_messages(finished.as_bytes()).is_err());
}
