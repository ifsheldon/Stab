#![cfg(unix)]

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use tempfile::tempdir;

use super::run_from;

#[derive(Clone, Copy, Debug)]
enum AliasKind {
    Direct,
    NormalizedRelative,
    Symlink,
    Hardlink,
}

#[derive(Clone, Copy, Debug)]
enum CommandFixture {
    Sample,
    Detect,
    M2d,
    AnalyzeErrors,
    SampleDem,
    Convert,
}

#[derive(Clone, Copy, Debug)]
struct RolePair {
    command: CommandFixture,
    first: &'static str,
    second: &'static str,
}

const ROLE_PAIRS: &[RolePair] = &[
    RolePair {
        command: CommandFixture::Sample,
        first: "--in",
        second: "--out",
    },
    RolePair {
        command: CommandFixture::Detect,
        first: "--in",
        second: "--out",
    },
    RolePair {
        command: CommandFixture::Detect,
        first: "--in",
        second: "--obs_out",
    },
    RolePair {
        command: CommandFixture::Detect,
        first: "--out",
        second: "--obs_out",
    },
    RolePair {
        command: CommandFixture::M2d,
        first: "--circuit",
        second: "--out",
    },
    RolePair {
        command: CommandFixture::M2d,
        first: "--circuit",
        second: "--obs_out",
    },
    RolePair {
        command: CommandFixture::M2d,
        first: "--in",
        second: "--out",
    },
    RolePair {
        command: CommandFixture::M2d,
        first: "--in",
        second: "--obs_out",
    },
    RolePair {
        command: CommandFixture::M2d,
        first: "--sweep",
        second: "--out",
    },
    RolePair {
        command: CommandFixture::M2d,
        first: "--sweep",
        second: "--obs_out",
    },
    RolePair {
        command: CommandFixture::M2d,
        first: "--out",
        second: "--obs_out",
    },
    RolePair {
        command: CommandFixture::AnalyzeErrors,
        first: "--in",
        second: "--out",
    },
    RolePair {
        command: CommandFixture::SampleDem,
        first: "--in",
        second: "--out",
    },
    RolePair {
        command: CommandFixture::SampleDem,
        first: "--in",
        second: "--obs_out",
    },
    RolePair {
        command: CommandFixture::SampleDem,
        first: "--in",
        second: "--err_out",
    },
    RolePair {
        command: CommandFixture::SampleDem,
        first: "--replay_err_in",
        second: "--out",
    },
    RolePair {
        command: CommandFixture::SampleDem,
        first: "--replay_err_in",
        second: "--obs_out",
    },
    RolePair {
        command: CommandFixture::SampleDem,
        first: "--replay_err_in",
        second: "--err_out",
    },
    RolePair {
        command: CommandFixture::SampleDem,
        first: "--out",
        second: "--obs_out",
    },
    RolePair {
        command: CommandFixture::SampleDem,
        first: "--out",
        second: "--err_out",
    },
    RolePair {
        command: CommandFixture::SampleDem,
        first: "--obs_out",
        second: "--err_out",
    },
    RolePair {
        command: CommandFixture::Convert,
        first: "--in",
        second: "--out",
    },
    RolePair {
        command: CommandFixture::Convert,
        first: "--in",
        second: "--obs_out",
    },
    RolePair {
        command: CommandFixture::Convert,
        first: "--circuit",
        second: "--out",
    },
    RolePair {
        command: CommandFixture::Convert,
        first: "--circuit",
        second: "--obs_out",
    },
    RolePair {
        command: CommandFixture::Convert,
        first: "--dem",
        second: "--out",
    },
    RolePair {
        command: CommandFixture::Convert,
        first: "--dem",
        second: "--obs_out",
    },
    RolePair {
        command: CommandFixture::Convert,
        first: "--out",
        second: "--obs_out",
    },
];

#[test]
fn explicit_direct_file_role_aliases_fail_without_truncation() {
    assert_alias_matrix(AliasKind::Direct);
}

#[test]
fn normalized_relative_file_role_aliases_fail_without_truncation() {
    assert_alias_matrix(AliasKind::NormalizedRelative);
}

#[test]
fn symlink_file_role_aliases_fail_without_truncation() {
    assert_alias_matrix(AliasKind::Symlink);
}

#[test]
fn hardlink_file_role_aliases_fail_without_truncation() {
    assert_alias_matrix(AliasKind::Hardlink);
}

#[test]
fn zero_shot_commands_still_reject_explicit_input_output_aliases() {
    for command in [
        CommandFixture::Sample,
        CommandFixture::Detect,
        CommandFixture::SampleDem,
    ] {
        let directory = tempdir().expect("temporary directory");
        let mut fixture = Fixture::new(command, directory.path());
        fixture.alias("--out", "--in", AliasKind::Direct);
        if let Some(shots) = fixture
            .args
            .iter_mut()
            .find(|argument| argument.to_string_lossy().starts_with("--shots="))
        {
            *shots = OsString::from("--shots=0");
        } else {
            fixture.args.push(OsString::from("--shots=0"));
        }
        fixture.assert_rejected("--in", "--out");
    }
}

#[test]
fn m2d_allows_measurement_and_sweep_inputs_to_share_one_file() {
    let directory = tempdir().expect("temporary directory");
    let circuit = directory.path().join("circuit.stim");
    let shared_input = directory.path().join("shared.01");
    std::fs::write(&circuit, "CX sweep[0] 0\nM 0\nDETECTOR rec[-1]\n").expect("write circuit");
    std::fs::write(&shared_input, "0\n").expect("write shared input");
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        [
            OsString::from("stab"),
            OsString::from("m2d"),
            OsString::from("--circuit"),
            circuit.into_os_string(),
            OsString::from("--in"),
            shared_input.clone().into_os_string(),
            OsString::from("--in_format=01"),
            OsString::from("--sweep"),
            shared_input.into_os_string(),
            OsString::from("--sweep_format=01"),
            OsString::from("--out_format=01"),
        ],
        std::io::empty(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0, "{}", String::from_utf8_lossy(&stderr));
    assert_eq!(stdout, b"0\n");
}

#[test]
fn special_output_files_are_streamed_without_regular_file_truncation() {
    let directory = tempdir().expect("temporary directory");
    let circuit = directory.path().join("circuit.stim");
    let primary = directory.path().join("primary.dets");
    std::fs::write(
        &circuit,
        "M 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
    )
    .expect("write circuit");
    std::fs::write(&primary, "replace-me\n").expect("seed primary output");
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        [
            OsString::from("stab"),
            OsString::from("detect"),
            OsString::from("--shots=1"),
            OsString::from("--in"),
            circuit.into_os_string(),
            OsString::from("--out"),
            primary.clone().into_os_string(),
            OsString::from("--obs_out=/dev/null"),
        ],
        std::io::empty(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0, "{}", String::from_utf8_lossy(&stderr));
    assert_eq!(stdout, Vec::<u8>::new());
    assert_ne!(
        std::fs::read(&primary).expect("read primary output"),
        b"replace-me\n"
    );
}

fn assert_alias_matrix(kind: AliasKind) {
    for pair in ROLE_PAIRS {
        let directory = tempdir().expect("temporary directory");
        let mut fixture = Fixture::new(pair.command, directory.path());
        fixture.alias(pair.second, pair.first, kind);
        fixture.assert_rejected(pair.first, pair.second);
    }
}

struct Fixture {
    args: Vec<OsString>,
    role_paths: BTreeMap<&'static str, PathBuf>,
}

impl Fixture {
    fn new(command: CommandFixture, root: &Path) -> Self {
        let mut role_paths = BTreeMap::new();
        for role in roles(command) {
            let name = role.trim_start_matches('-').replace('_', "-");
            role_paths.insert(*role, root.join(name));
        }
        seed_paths(command, &role_paths);
        let args = command_args(command, &role_paths);
        Self { args, role_paths }
    }

    fn alias(&mut self, alias_role: &'static str, target_role: &'static str, kind: AliasKind) {
        let target = self
            .role_paths
            .get(target_role)
            .expect("target role path")
            .clone();
        let alias = self
            .role_paths
            .get(alias_role)
            .expect("alias role path")
            .clone();
        let replacement = match kind {
            AliasKind::Direct => target,
            AliasKind::NormalizedRelative => {
                let parent = target.parent().expect("target parent");
                std::fs::create_dir(parent.join("alias-components"))
                    .expect("create alias component directory");
                parent
                    .join("alias-components")
                    .join("..")
                    .join(target.file_name().expect("target file name"))
            }
            AliasKind::Symlink => {
                std::fs::remove_file(&alias).expect("remove alias role file");
                symlink(&target, &alias).expect("create symlink alias");
                alias
            }
            AliasKind::Hardlink => {
                std::fs::remove_file(&alias).expect("remove alias role file");
                std::fs::hard_link(&target, &alias).expect("create hardlink alias");
                alias
            }
        };
        self.role_paths.insert(alias_role, replacement);
        let replacement = self
            .role_paths
            .get(alias_role)
            .expect("replacement role path");
        replace_flag_value(&mut self.args, alias_role, replacement);
    }

    fn assert_rejected(&self, first_role: &str, second_role: &str) {
        let snapshots = snapshot_paths(self.role_paths.values());
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = run_from(
            self.args.iter().cloned(),
            std::io::empty(),
            &mut stdout,
            &mut stderr,
        );
        let stderr = String::from_utf8(stderr).expect("utf-8 stderr");

        assert_ne!(
            status, 0,
            "{:?} unexpectedly accepted {first_role}/{second_role}",
            self.args
        );
        assert!(
            stderr.contains(first_role),
            "stderr omitted {first_role}: {stderr}"
        );
        assert!(
            stderr.contains(second_role),
            "stderr omitted {second_role}: {stderr}"
        );
        for (path, expected) in snapshots {
            assert_eq!(
                std::fs::read(&path).expect("read path after rejection"),
                expected,
                "{} changed after rejecting {first_role}/{second_role}: {stderr}",
                path.display()
            );
        }
    }
}

fn roles(command: CommandFixture) -> &'static [&'static str] {
    match command {
        CommandFixture::Sample => &["--in", "--out"],
        CommandFixture::Detect => &["--in", "--out", "--obs_out"],
        CommandFixture::M2d => &["--circuit", "--in", "--sweep", "--out", "--obs_out"],
        CommandFixture::AnalyzeErrors => &["--in", "--out"],
        CommandFixture::SampleDem => {
            &["--in", "--replay_err_in", "--out", "--obs_out", "--err_out"]
        }
        CommandFixture::Convert => &["--in", "--circuit", "--dem", "--out", "--obs_out"],
    }
}

fn seed_paths(command: CommandFixture, paths: &BTreeMap<&str, PathBuf>) {
    for (role, path) in paths {
        let content: &[u8] = match (command, *role) {
            (CommandFixture::Sample, "--in") => Some(b"M 0\n".as_slice()),
            (CommandFixture::Detect, "--in") => {
                Some(b"M 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n".as_slice())
            }
            (CommandFixture::M2d, "--circuit") => Some(
                b"M 0\nCX sweep[0] 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n"
                    .as_slice(),
            ),
            (CommandFixture::M2d, "--in") => Some(b"00\n".as_slice()),
            (CommandFixture::M2d, "--sweep") => Some(b"0\n".as_slice()),
            (CommandFixture::AnalyzeErrors, "--in") => {
                Some(b"X_ERROR(0.25) 0\nM 0\nDETECTOR rec[-1]\n".as_slice())
            }
            (CommandFixture::SampleDem, "--in") => Some(b"error(0.5) D0 L0\n".as_slice()),
            (CommandFixture::SampleDem, "--replay_err_in") => Some(b"0\n".as_slice()),
            (CommandFixture::Convert, "--in") => Some(b"00\n".as_slice()),
            (CommandFixture::Convert, "--circuit") => {
                Some(b"M 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n".as_slice())
            }
            (CommandFixture::Convert, "--dem") => Some(b"error(0.5) D0 L0\n".as_slice()),
            (_, "--out") => Some(b"primary-output-sentinel\n".as_slice()),
            (_, "--obs_out") => Some(b"observable-output-sentinel\n".as_slice()),
            (_, "--err_out") => Some(b"error-output-sentinel\n".as_slice()),
            _ => None,
        }
        .expect("every command role has fixture bytes");
        std::fs::write(path, content).expect("seed role path");
    }
}

fn command_args(command: CommandFixture, paths: &BTreeMap<&str, PathBuf>) -> Vec<OsString> {
    let mut args = vec![OsString::from("stab")];
    match command {
        CommandFixture::Sample => {
            args.push(OsString::from("sample"));
        }
        CommandFixture::Detect => {
            args.extend([OsString::from("detect"), OsString::from("--shots=1")]);
        }
        CommandFixture::M2d => {
            args.extend([
                OsString::from("m2d"),
                OsString::from("--in_format=01"),
                OsString::from("--sweep_format=01"),
            ]);
        }
        CommandFixture::AnalyzeErrors => {
            args.push(OsString::from("analyze_errors"));
        }
        CommandFixture::SampleDem => {
            args.extend([
                OsString::from("sample_dem"),
                OsString::from("--shots=1"),
                OsString::from("--replay_err_in_format=01"),
            ]);
        }
        CommandFixture::Convert => {
            args.extend([
                OsString::from("convert"),
                OsString::from("--in_format=01"),
                OsString::from("--out_format=01"),
                OsString::from("--types=DL"),
            ]);
        }
    }
    for role in roles(command) {
        args.push(OsString::from(role));
        args.push(paths[*role].clone().into_os_string());
    }
    args
}

fn replace_flag_value(args: &mut [OsString], flag: &str, path: &Path) {
    let index = args
        .iter()
        .position(|arg| arg == flag)
        .expect("flag in command arguments");
    let value = args.get_mut(index + 1).expect("flag value");
    *value = path.as_os_str().to_owned();
}

fn snapshot_paths<'a>(paths: impl IntoIterator<Item = &'a PathBuf>) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut snapshots = BTreeMap::new();
    for path in paths {
        snapshots
            .entry(path.clone())
            .or_insert_with(|| std::fs::read(path).expect("snapshot role path"));
    }
    snapshots
}
