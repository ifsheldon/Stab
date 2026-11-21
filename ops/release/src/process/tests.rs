use std::assert_matches;

use super::*;
use std::io::Write as _;
use std::path::PathBuf;
use std::thread;

const HELPER_ENV: &str = "STAB_RELEASE_HELPER";
const STARTED_PATH_ENV: &str = "STAB_RELEASE_STARTED_PATH";
const PUBLISHED_PATH_ENV: &str = "STAB_RELEASE_PUBLISHED_PATH";

fn run_helper(
    working_directory: &Path,
    program: &OsStr,
    args: &[OsString],
    environment: &[(OsString, OsString)],
    timeout: Duration,
    output_limit: usize,
) -> Result<ProcessOutput, ReleaseError> {
    super::run(
        working_directory,
        &Invocation::new(Program::Resolve(program), args, timeout, output_limit)
            .environment(environment),
    )
}

fn captured(end: StreamEnd) -> CapturedStream {
    CapturedStream {
        bytes: b"partial".to_vec(),
        total_bytes: 7,
        exceeded: false,
        end,
    }
}

/// Regression: a reader stopped at its grace could return a truncated
/// capture as complete, so an empty `git status --porcelain` read as clean.
#[test]
fn truncated_capture_fails_while_a_drained_held_pipe_passes() {
    let complete = captured(StreamEnd::Eof);
    assert_matches!(
        require_complete_capture("git", &captured(StreamEnd::Truncated), &complete),
        Err(ReleaseError::CommandCapture { .. })
    );
    assert_matches!(
        require_complete_capture("git", &complete, &captured(StreamEnd::Truncated)),
        Err(ReleaseError::CommandCapture { .. })
    );
    require_complete_capture("git", &captured(StreamEnd::HeldOpen), &complete)
        .expect("a fully drained pipe held by an escaped descendant is complete");
}

#[test]
fn failed_command_diagnostics_are_bounded() {
    let error = require_success(
        "cargo package",
        std::os::unix::process::ExitStatusExt::from_raw(1 << 8),
        &vec![b'e'; 1 << 20],
    )
    .expect_err("nonzero status fails");
    let ReleaseError::CommandFailed {
        program, stderr, ..
    } = error
    else {
        unreachable!("expected a command failure");
    };
    assert_eq!(program, "cargo package");
    assert!(stderr.len() < 5 << 10);
    assert!(stderr.ends_with("[stderr truncated]"));
}

#[test]
fn retained_program_errors_name_the_source_path() {
    let executable = std::env::current_exe().expect("test executable");
    let file = safe_fs::open_regular_file(&executable).expect("open test executable");
    let retained = safe_fs::descriptor_program(&file, &executable).expect("retain");
    let invocation = Invocation::new(
        Program::Retained(&retained),
        ["--version"],
        Duration::from_secs(1),
        1,
    );
    assert_eq!(invocation.label, executable.display().to_string());
    assert!(!invocation.label.starts_with("/proc/self/fd"));
}

fn helper_request(mode: &str) -> (OsString, Vec<OsString>, Vec<(OsString, OsString)>) {
    let executable = std::env::current_exe()
        .expect("test executable")
        .into_os_string();
    let args = vec![
        OsString::from("--ignored"),
        OsString::from("--exact"),
        OsString::from("process::tests::subprocess_helper"),
        OsString::from("--nocapture"),
    ];
    let environment = vec![
        (OsString::from(HELPER_ENV), OsString::from(mode)),
        (
            OsString::from("GIT_DIR"),
            OsString::from("/tmp/attacker-controlled-git-dir"),
        ),
    ];
    (executable, args, environment)
}

#[test]
fn child_output_is_bounded_before_capture_completes() {
    let (program, args, environment) = helper_request("flood");
    assert_matches!(
        run_helper(
            Path::new(env!("CARGO_MANIFEST_DIR")),
            &program,
            &args,
            &environment,
            Duration::from_secs(5),
            4096,
        ),
        Err(ReleaseError::CommandOutputLimit { limit: 4096, .. })
    );
}

#[test]
fn child_timeout_terminates_the_process_group() {
    let (program, args, environment) = helper_request("sleep");
    assert_matches!(
        run_helper(
            Path::new(env!("CARGO_MANIFEST_DIR")),
            &program,
            &args,
            &environment,
            Duration::from_millis(100),
            4096,
        ),
        Err(ReleaseError::CommandTimeout { .. })
    );
}

#[test]
fn inherited_and_explicit_git_overrides_are_cleared() {
    let (program, args, environment) = helper_request("git-env");
    let output = run_helper(
        Path::new(env!("CARGO_MANIFEST_DIR")),
        &program,
        &args,
        &environment,
        Duration::from_secs(5),
        4096,
    )
    .expect("helper succeeds");
    assert!(String::from_utf8_lossy(&output.stdout).contains("git-env-absent"));
}

#[test]
fn ambient_release_credentials_do_not_cross_the_supervisor_boundary() {
    let (program, args, environment) = helper_request("secret-supervisor");
    let status = Command::new(program)
        .args(args)
        .envs(environment)
        .env("CARGO_REGISTRY_TOKEN", "must-not-reach-release-children")
        .env(
            "CARGO_REGISTRIES_CRATES_IO_TOKEN",
            "must-not-reach-release-children",
        )
        .env("GITHUB_TOKEN", "must-not-reach-release-children")
        .env("GH_TOKEN", "must-not-reach-release-children")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("spawn secret-scope supervisor");
    assert!(
        status.success(),
        "secret-scope supervisor failed with {status}"
    );
}

#[test]
fn successful_child_output_is_fully_drained() {
    let (program, args, environment) = helper_request("bounded-output");
    let output = run_helper(
        Path::new(env!("CARGO_MANIFEST_DIR")),
        &program,
        &args,
        &environment,
        Duration::from_secs(5),
        256 << 10,
    )
    .expect("bounded helper output succeeds");

    assert_eq!(
        output.stdout.iter().filter(|byte| **byte == 0xa5).count(),
        128 << 10
    );
}

#[cfg(unix)]
#[test]
fn explicit_program_name_survives_retained_execution() {
    let (program, args, environment) = helper_request("argv0");
    let output = super::run(
        Path::new(env!("CARGO_MANIFEST_DIR")),
        &Invocation::new(
            Program::Resolve(&program),
            &args,
            Duration::from_secs(5),
            4096,
        )
        .arg0(OsStr::new("rustup"))
        .environment(&environment),
    )
    .expect("named helper succeeds");

    assert!(String::from_utf8_lossy(&output.stdout).contains("argv0=rustup"));
}

#[test]
fn explicit_cancellation_prevents_mock_publication() {
    let directory = tempfile::tempdir().expect("temporary marker directory");
    let started = directory.path().join("started");
    let published = directory.path().join("published");
    let (program, args, mut environment) = helper_request("mock-publication");
    environment.extend(marker_environment(&started, &published));
    let cancellation = ReleaseCancellation::for_test();
    let trigger = cancellation.clone();
    let started_for_trigger = started.clone();
    let canceller = thread::spawn(move || {
        wait_for_pid(&started_for_trigger, Duration::from_secs(5));
        trigger.cancel();
    });

    let result = run_with_cancellation(
        Path::new(env!("CARGO_MANIFEST_DIR")),
        &Invocation::new(
            Program::Resolve(&program),
            &args,
            Duration::from_secs(10),
            4096,
        )
        .environment(&environment),
        &cancellation,
    );
    canceller.join().expect("cancellation thread");

    assert_matches!(result, Err(ReleaseError::CommandInterrupted { .. }));
    assert_process_gone(wait_for_pid(&started, Duration::from_secs(5)))
        .expect("mock publication process is gone");
    assert!(
        !published.exists(),
        "cancelled publication wrote its marker"
    );
}

#[cfg(unix)]
#[test]
fn sigint_and_sigterm_cancel_mock_publication() {
    for (name, signal) in [
        ("sigint", rustix::process::Signal::INT),
        ("sigterm", rustix::process::Signal::TERM),
    ] {
        let directory = tempfile::tempdir().expect("temporary marker directory");
        let started = directory.path().join(format!("{name}-started"));
        let published = directory.path().join(format!("{name}-published"));
        let (program, args, mut environment) = helper_request("signal-supervisor");
        environment.extend(marker_environment(&started, &published));
        let mut supervisor = Command::new(&program)
            .args(&args)
            .envs(environment.iter().map(|(key, value)| (key, value)))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn signal supervisor");
        let published_process = wait_for_pid(&started, Duration::from_secs(5));

        rustix::process::kill_process(process_id(supervisor.id()), signal)
            .expect("signal release supervisor");
        let status = wait_for_child(&mut supervisor, Duration::from_secs(5))
            .expect("signal supervisor completes");

        assert!(status.success(), "{name} supervisor failed with {status}");
        assert_process_gone(published_process).expect("mock publication process is gone");
        assert!(
            !published.exists(),
            "{name} cancellation allowed mock publication"
        );
    }
}

#[cfg(unix)]
#[test]
fn escaped_descendant_cannot_hold_reader_joins_open() {
    let (program, args, environment) = helper_request("leader-exit-with-descendant");
    let started = Instant::now();

    let output = run_helper(
        Path::new(env!("CARGO_MANIFEST_DIR")),
        &program,
        &args,
        &environment,
        Duration::from_secs(5),
        4096,
    )
    .expect("leader exit must complete despite escaped pipe holder");

    assert!(
        started.elapsed() < Duration::from_secs(2),
        "reader join waited for an escaped descendant"
    );
    let descendant = String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| line.strip_prefix("descendant-pid="))
        .expect("descendant pid marker")
        .parse::<u32>()
        .expect("numeric descendant pid");
    terminate_test_group(descendant);
    assert_process_gone(descendant).expect("escaped descendant is gone");
}

#[cfg(unix)]
#[test]
fn managed_child_drop_cancels_mock_publication() {
    let directory = tempfile::tempdir().expect("temporary marker directory");
    let started = directory.path().join("drop-started");
    let published = directory.path().join("drop-published");
    let (program, args, mut environment) = helper_request("mock-publication");
    environment.extend(marker_environment(&started, &published));
    let mut command = Command::new(&program);
    command
        .args(&args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .envs(environment.iter().map(|(key, value)| (key, value)));
    let leader = ProcessGroupLeader::spawn(&mut command).expect("spawn managed mock publication");
    let mut managed = ManagedChild::new(leader, program.to_string_lossy().into_owned());
    managed.start_readers(4096).expect("start bounded readers");
    let pid = wait_for_pid(&started, Duration::from_secs(5));

    let dropped = Instant::now();
    drop(managed);

    assert!(
        dropped.elapsed() < Duration::from_secs(2),
        "managed-child cleanup exceeded its reader bound"
    );
    assert_process_gone(pid).expect("managed child is gone");
    assert!(!published.exists(), "dropped guard allowed publication");
}

fn marker_environment(started: &Path, published: &Path) -> Vec<(OsString, OsString)> {
    vec![
        (
            OsString::from(STARTED_PATH_ENV),
            started.as_os_str().to_os_string(),
        ),
        (
            OsString::from(PUBLISHED_PATH_ENV),
            published.as_os_str().to_os_string(),
        ),
    ]
}

fn wait_for_pid(path: &Path, timeout: Duration) -> u32 {
    let deadline = Instant::now() + timeout;
    loop {
        if let Ok(marker) = std::fs::read_to_string(path)
            && let Ok(pid) = marker.parse::<u32>()
        {
            return pid;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for numeric process marker at {path:?}"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

fn wait_for_child(
    child: &mut std::process::Child,
    timeout: Duration,
) -> Result<ExitStatus, String> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait().expect("poll helper") {
            return Ok(status);
        }
        if Instant::now() >= deadline {
            drop(child.kill());
            let status = child.wait().expect("reap timed-out helper");
            return Err(format!("helper timed out and was killed with {status}"));
        }
        thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(unix)]
fn process_id(pid: u32) -> rustix::process::Pid {
    let raw = i32::try_from(pid).expect("test pid fits i32");
    rustix::process::Pid::from_raw(raw).expect("nonzero test pid")
}

#[cfg(unix)]
fn terminate_test_group(pid: u32) {
    if let Err(source) =
        rustix::process::kill_process_group(process_id(pid), rustix::process::Signal::KILL)
    {
        assert_eq!(source, rustix::io::Errno::SRCH);
    }
}

#[cfg(unix)]
fn assert_process_gone(pid: u32) -> Result<(), String> {
    let pid = process_id(pid);
    for _ in 0..200 {
        match rustix::process::test_kill_process(pid) {
            Err(rustix::io::Errno::SRCH) => return Ok(()),
            Ok(()) => thread::sleep(Duration::from_millis(10)),
            Err(source) => {
                return Err(format!("failed to inspect test process {pid:?}: {source}"));
            }
        }
    }
    Err(format!("test process {pid:?} survived supervisor cleanup"))
}

#[cfg(not(unix))]
fn assert_process_gone(_pid: u32) -> Result<(), String> {
    Ok(())
}

#[test]
#[ignore = "executed only as a subprocess by release process tests"]
fn subprocess_helper() {
    match std::env::var(HELPER_ENV).as_deref() {
        Ok("flood") => print!("{}", "x".repeat(1 << 20)),
        Ok("sleep") => thread::sleep(Duration::from_secs(5)),
        Ok("git-env") => {
            if std::env::var_os("GIT_DIR").is_none() {
                println!("git-env-absent");
            } else {
                std::process::exit(9);
            }
        }
        Ok("secret-env") => {
            if [
                "CARGO_REGISTRY_TOKEN",
                "CARGO_REGISTRIES_CRATES_IO_TOKEN",
                "GITHUB_TOKEN",
                "GH_TOKEN",
            ]
            .iter()
            .all(|name| std::env::var_os(name).is_none())
            {
                println!("release-credentials-absent");
            } else {
                std::process::exit(10);
            }
        }
        Ok("secret-supervisor") => {
            let (program, args, environment) = helper_request("secret-env");
            let output = run_helper(
                Path::new(env!("CARGO_MANIFEST_DIR")),
                &program,
                &args,
                &environment,
                Duration::from_secs(5),
                4096,
            )
            .expect("isolated secret probe");
            if !output
                .stdout
                .windows(b"release-credentials-absent".len())
                .any(|window| window == b"release-credentials-absent")
            {
                std::process::exit(11);
            }
        }
        Ok("bounded-output") => {
            std::io::stdout()
                .write_all(&vec![0xa5; 128 << 10])
                .expect("write bounded helper output");
        }
        Ok("argv0") => {
            let argument_zero = std::env::args_os().next().expect("argv0");
            println!("argv0={}", argument_zero.to_string_lossy());
        }
        Ok("mock-publication") => {
            let started = required_marker_path(STARTED_PATH_ENV);
            let published = required_marker_path(PUBLISHED_PATH_ENV);
            std::fs::write(started, std::process::id().to_string())
                .expect("write publication start marker");
            thread::sleep(Duration::from_secs(30));
            std::fs::write(published, b"published").expect("write mock publication marker");
        }
        Ok("signal-supervisor") => {
            let (program, args, mut environment) = helper_request("mock-publication");
            let started = required_marker_path(STARTED_PATH_ENV);
            let published = required_marker_path(PUBLISHED_PATH_ENV);
            environment.extend(marker_environment(&started, &published));
            match run_helper(
                Path::new(env!("CARGO_MANIFEST_DIR")),
                &program,
                &args,
                &environment,
                Duration::from_secs(30),
                4096,
            ) {
                Err(ReleaseError::CommandInterrupted { .. }) => {}
                Ok(_) => std::process::exit(11),
                Err(error) => {
                    eprintln!("unexpected supervisor error: {error}");
                    std::process::exit(12);
                }
            }
        }
        #[cfg(unix)]
        Ok("leader-exit-with-descendant") => {
            let (program, args, mut environment) = helper_request("pipe-holder");
            environment.retain(|(key, _)| key != "GIT_DIR");
            let mut command = Command::new(program);
            command
                .args(args)
                .envs(environment.iter().map(|(key, value)| (key, value)));
            use std::os::unix::process::CommandExt as _;
            command.process_group(0);
            let child = command.spawn().expect("spawn escaped pipe holder");
            println!("descendant-pid={}", child.id());
            std::io::stdout().flush().expect("flush descendant pid");
            drop(child);
        }
        Ok("pipe-holder") => thread::sleep(Duration::from_secs(30)),
        _ => std::process::exit(10),
    }
}

fn required_marker_path(key: &str) -> PathBuf {
    std::env::var_os(key)
        .map(PathBuf::from)
        .expect("required release marker path")
}
