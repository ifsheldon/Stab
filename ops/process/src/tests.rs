use std::assert_matches;

use super::*;
use std::io::{BufRead as _, Read as _, Write as _};

const HELPER_TEST: &str = "tests::process_helper";
const HELPER_ENV: &str = "STAB_BENCH_PROCESS_HELPER";
const EXPECTED_CPU_ENV: &str = "STAB_BENCH_EXPECTED_CPU";
const OUTPUT_PATH_ENV: &str = "STAB_BENCH_OUTPUT_PATH";

fn request(mode: &str) -> ProcessRequest {
    ProcessRequest {
        launch: ProcessLaunch::Direct,
        program: std::env::current_exe().expect("test executable"),
        args: vec![
            OsString::from(HELPER_TEST),
            OsString::from("--exact"),
            OsString::from("--ignored"),
            OsString::from("--nocapture"),
        ],
        stdin: Vec::new(),
        unread_stdin: UnreadStdin::Reject,
        working_directory: std::env::current_dir().expect("working directory"),
        environment: vec![(OsString::from(HELPER_ENV), OsString::from(mode))].into(),
        affinity_cpu: None,
        limits: ProcessLimits {
            stdin_bytes: 8 << 20,
            stdout: (4096).into(),
            stderr: (4096).into(),
            regular_file_bytes: None,
            timeout: Duration::from_secs(2),
        },
    }
}

#[test]
fn captures_success_nonzero_and_signal_status() {
    let success = run_bounded_process(&request("success")).expect("successful helper");
    assert_eq!(success.status, ProcessExit::Exited(0));
    assert!(String::from_utf8_lossy(&success.stdout).contains("helper-success"));

    let nonzero = run_bounded_process(&request("nonzero")).expect("nonzero is captured");
    assert_eq!(nonzero.status, ProcessExit::Exited(7));

    let signalled = run_bounded_process(&request("signal")).expect("signal is captured");
    assert_eq!(
        signalled.status,
        ProcessExit::Signaled(signal_hook::consts::signal::SIGTERM)
    );
}

#[test]
fn captures_peak_rss_from_a_short_lived_child() {
    let output = run_bounded_process(&request("peak-rss")).expect("short allocation succeeds");
    assert_eq!(output.status, ProcessExit::Exited(0));
    assert!(
        output
            .parent_observed_peak_rss_bytes
            .is_some_and(|bytes| bytes >= 16 << 20),
        "kernel peak RSS was {:?}",
        output.parent_observed_peak_rss_bytes
    );
}

#[test]
fn measured_wall_time_stops_at_child_completion() {
    let started_at = Instant::now();
    let completed_at = started_at + Duration::from_millis(2);

    assert_eq!(
        measured_wall_elapsed(started_at, completed_at, Path::new("helper"))
            .expect("ordered completion clock"),
        Duration::from_millis(2)
    );
}

#[test]
fn rejects_missing_binary_and_all_stream_limits() {
    let mut missing = request("success");
    missing.program = PathBuf::from("/definitely/missing/stab-bench-worker");
    assert_matches!(
        run_bounded_process(&missing),
        Err(ProcessError::Spawn { .. })
    );

    let mut stdin = request("success");
    stdin.stdin = vec![0_u8; 2];
    stdin.limits.stdin_bytes = 1;
    assert_matches!(
        run_bounded_process(&stdin),
        Err(ProcessError::StdinLimit { .. })
    );

    let mut stdout = request("stdout-overflow");
    stdout.limits.stdout = 32.into();
    assert_matches!(
        run_bounded_process(&stdout),
        Err(ProcessError::OutputLimit {
            stream: "stdout",
            ..
        })
    );

    let mut stderr = request("stderr-overflow");
    stderr.limits.stderr = 32.into();
    assert_matches!(
        run_bounded_process(&stderr),
        Err(ProcessError::OutputLimit {
            stream: "stderr",
            ..
        })
    );
}

#[test]
fn unread_stdin_policy_decides_whether_a_broken_pipe_fails() {
    let mut request = request("close-stdin");
    request.stdin = vec![0_u8; 8 << 20];
    assert_matches!(
        run_bounded_process(&request),
        Err(ProcessError::WriteStdin { .. })
    );

    request.unread_stdin = UnreadStdin::Allow;
    let output = run_bounded_process(&request).expect("an early exit stays observable");
    assert_eq!(output.status, ProcessExit::Exited(3));
    assert_eq!(output.stderr, b"rejected");
}

#[test]
fn timeout_includes_blocked_stdin_writes() {
    let mut request = request("sleep");
    request.stdin = vec![0_u8; 8 << 20];
    request.limits.timeout = Duration::from_millis(200);
    let started = Instant::now();

    assert_matches!(
        run_bounded_process(&request),
        Err(ProcessError::TimedOut { .. })
    );
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[test]
fn sticky_cancellation_prevents_a_later_process_from_starting() {
    let directory = tempfile::tempdir().expect("temporary marker directory");
    let marker = directory.path().join("must-not-start");
    let mut request = request("success");
    request.program = PathBuf::from("/bin/sh");
    request.args = vec![
        OsString::from("-c"),
        OsString::from("printf started > \"$1\""),
        OsString::from("sticky-cancellation"),
        marker.as_os_str().to_owned(),
    ];
    let cancellation = Cancellation::manual();
    cancellation.cancel();

    assert_matches!(
        run_bounded_process_with_cancellation(&request, &cancellation),
        Err(ProcessError::Interrupted(_))
    );
    assert!(!marker.exists());
}

#[test]
fn drains_child_output_while_writing_stdin() {
    let mut request = request("output-before-stdin");
    request.stdin = vec![b'i'; 1 << 20];
    request.limits.stdout = (2 << 20).into();

    let output = run_bounded_process(&request).expect("concurrent pipe transfer succeeds");

    assert_eq!(output.status, ProcessExit::Exited(0));
    assert!(
        output
            .stdout
            .windows(b"stdin-drained\n".len())
            .any(|window| window == b"stdin-drained\n")
    );
}

#[test]
fn cancellation_terminates_blocked_io() {
    let mut request = request("sleep");
    request.stdin = vec![0_u8; 8 << 20];
    let cancellation = Cancellation::manual();
    let trigger = cancellation.clone();
    let canceller = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        trigger.cancel();
    });

    let result = run_bounded_process_with_cancellation(&request, &cancellation);
    canceller.join().expect("cancellation thread");

    assert_matches!(result, Err(ProcessError::Interrupted(_)));
}

#[test]
fn discard_policy_drains_without_capturing_or_limiting_output() {
    let mut request = request("stdout-overflow");
    request.limits.stdout = OutputPolicy::Discard;

    let output = run_bounded_process(&request).expect("discarded output succeeds");

    assert_eq!(output.status, ProcessExit::Exited(0));
    assert!(output.stdout.is_empty());
}

#[test]
fn inherited_and_cleared_environments_are_distinct() {
    let mut request = request("success");
    request.program = PathBuf::from("/usr/bin/env");
    request.args.clear();
    request.environment = ProcessEnvironment::Inherit;
    request.limits.stdout = (1 << 20).into();
    let inherited = run_bounded_process(&request).expect("inherited environment");
    assert!(inherited.stdout.windows(5).any(|window| window == b"PATH="));

    request.environment = ProcessEnvironment::ClearAndSet(Vec::new());
    let cleared = run_bounded_process(&request).expect("cleared environment");
    assert!(cleared.stdout.is_empty());
}

#[test]
fn failures_render_bounded_lossy_diagnostic_prefixes() {
    let mut request = request("invalid-utf8-flood");
    request.limits.stdout = (16 << 10).into();
    let error = run_bounded_process(&request).expect_err("output must exceed its limit");
    let rendered = error.to_string();
    let ProcessError::OutputLimit { failure: error, .. } = error else {
        unreachable!("expected output-limit error, got {rendered}");
    };

    assert_eq!(error.stdout.len(), 16 << 10);
    assert!(error.stdout_diagnostic.contains('\u{fffd}'));
    assert!(error.stdout_diagnostic.ends_with("[diagnostic truncated]"));
    assert!(error.stdout_diagnostic.len() < 5 << 10);
    assert!(rendered.contains('\u{fffd}'));
    assert!(rendered.len() < 6 << 10);
}

#[test]
fn pins_the_child_to_the_requested_singleton_cpu() {
    let allowed = rustix::thread::sched_getaffinity(None).expect("read parent affinity");
    let cpu = (0..rustix::thread::CpuSet::MAX_CPU)
        .find(|cpu| allowed.is_set(*cpu))
        .expect("at least one allowed CPU");
    let mut request = request("affinity");
    request.stdin = vec![b'\n'];
    request.affinity_cpu = Some(cpu);
    request.environment.push((
        OsString::from(EXPECTED_CPU_ENV),
        OsString::from(cpu.to_string()),
    ));

    let result = run_bounded_process(&request).expect("affinity helper succeeds");

    assert_eq!(
        result.status,
        ProcessExit::Exited(0),
        "affinity helper stderr: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(String::from_utf8_lossy(&result.stdout).contains("affinity-ok"));
}

#[test]
fn pins_threads_that_the_child_created_before_the_affinity_request() {
    let allowed = rustix::thread::sched_getaffinity(None).expect("read parent affinity");
    let cpu = (0..rustix::thread::CpuSet::MAX_CPU)
        .find(|cpu| allowed.is_set(*cpu))
        .expect("at least one allowed CPU");
    let mut child = std::process::Command::new(std::env::current_exe().expect("test executable"))
        .args([HELPER_TEST, "--exact", "--ignored", "--nocapture"])
        .env_clear()
        .env(HELPER_ENV, "affinity-existing-threads")
        .env(EXPECTED_CPU_ENV, cpu.to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn affinity helper");
    let mut stdout = std::io::BufReader::new(child.stdout.take().expect("helper stdout"));
    let mut captured_stdout = String::new();
    let mut ready = false;
    for _ in 0..16 {
        let mut line = String::new();
        if stdout.read_line(&mut line).expect("read ready marker") == 0 {
            break;
        }
        captured_stdout.push_str(&line);
        if line == "threads-ready\n" {
            ready = true;
            break;
        }
    }
    if !ready {
        drop(child.kill());
        drop(child.wait());
    }
    assert!(ready, "helper omitted ready marker: {captured_stdout:?}");

    set_child_affinity(child.id(), cpu).expect("pin every existing child task");
    child
        .stdin
        .take()
        .expect("helper stdin")
        .write_all(b"\n")
        .expect("release helper");

    stdout
        .read_to_string(&mut captured_stdout)
        .expect("read helper stdout");
    let mut captured_stderr = String::new();
    child
        .stderr
        .take()
        .expect("helper stderr")
        .read_to_string(&mut captured_stderr)
        .expect("read helper stderr");
    let status = child.wait().expect("wait for affinity helper");
    assert!(
        status.success(),
        "affinity helper failed: {captured_stderr}"
    );
    assert!(captured_stdout.contains("affinity-all-tasks-ok"));
}

#[test]
fn rejects_invalid_affinity_and_direct_regular_file_limits() {
    let mut invalid_affinity = request("success");
    invalid_affinity.affinity_cpu = Some(rustix::thread::CpuSet::MAX_CPU);
    assert_matches!(
        run_bounded_process(&invalid_affinity),
        Err(ProcessError::SetAffinity { .. })
    );

    // A direct spawn could bound the program only after it already started,
    // so a file limit requires the launcher and the request never starts.
    let directory = tempfile::tempdir().expect("temporary output directory");
    let output = directory.path().join("unbounded-output");
    let mut direct_file = request("file-overflow-immediate");
    direct_file.limits.regular_file_bytes = Some(8 << 10);
    direct_file.environment.push((
        OsString::from(OUTPUT_PATH_ENV),
        output.as_os_str().to_os_string(),
    ));
    assert_matches!(
        run_bounded_process(&direct_file),
        Err(ProcessError::FileLimitRequiresLauncher { .. })
    );
    assert!(!output.exists(), "a rejected request must not start");
}

/// Regression: the file limit was applied with `prlimit` after `spawn`
/// returned, so a program that writes at startup escaped it.
#[test]
fn file_limit_binds_a_program_that_writes_immediately() {
    let directory = tempfile::tempdir().expect("temporary output directory");
    let output = directory.path().join("bounded-output");
    let mut immediate = measured("file-overflow-immediate");
    immediate.limits.regular_file_bytes = Some(8 << 10);
    immediate.environment.push((
        OsString::from(OUTPUT_PATH_ENV),
        output.as_os_str().to_os_string(),
    ));
    for _ in 0..8 {
        assert_matches!(
            run_bounded_process(&immediate),
            Err(ProcessError::FileLimit { maximum, .. }) if maximum == 8 << 10
        );
        assert!(
            std::fs::metadata(&output)
                .expect("bounded output exists")
                .len()
                <= 8 << 10
        );
        std::fs::remove_file(&output).expect("reset bounded output");
    }
}

/// Regression: a descendant that left the process group kept the inherited
/// stdout open, so joining the reader blocked forever after the leader exited.
#[test]
fn escaped_pipe_holder_is_reported_after_leader_exit() {
    let started = Instant::now();
    let error = run_bounded_process(&request("escaped-holder-exit"))
        .expect_err("an escaped pipe holder must be reported");
    assert!(started.elapsed() < Duration::from_secs(5));
    let ProcessError::PipesHeld(failure) = error else {
        unreachable!("expected a held-pipe error, got {error}");
    };
    assert_eq!(failure.held_streams, ["stdout"]);
    reap_escaped_holder(&failure.stdout);
}

/// Regression: after a timeout killed the group, an escaped pipe holder kept
/// the supervisor blocked past its own deadline.
#[test]
fn escaped_pipe_holder_cannot_extend_a_timeout() {
    let mut request = request("escaped-holder-sleep");
    request.limits.timeout = Duration::from_millis(500);
    let started = Instant::now();
    let error = run_bounded_process(&request).expect_err("helper must time out");
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "timeout waited {:?} for an escaped pipe holder",
        started.elapsed()
    );
    let ProcessError::TimedOut { failure, .. } = error else {
        unreachable!("expected a timeout, got {error}");
    };
    assert!(failure.held_streams.contains(&"stdout"));
    assert!(failure.to_string().contains("still held its"));
    reap_escaped_holder(&failure.stdout);
}

fn reap_escaped_holder(stdout: &[u8]) {
    let pid = String::from_utf8_lossy(stdout)
        .lines()
        .find_map(|line| line.strip_prefix("grandchild-pid="))
        .expect("grandchild pid")
        .parse::<u32>()
        .expect("numeric grandchild pid");
    let pid = group::process_id(pid).expect("grandchild pid");
    let killed = rustix::process::kill_process(pid, rustix::process::Signal::KILL);
    assert!(matches!(killed, Ok(()) | Err(rustix::io::Errno::SRCH)));
}

#[test]
fn captures_worker_panic_as_a_failed_process() {
    let output = run_bounded_process(&request("panic")).expect("panic is captured");
    assert_eq!(output.status, ProcessExit::Exited(101));
    assert!(String::from_utf8_lossy(&output.stderr).contains("process helper panic"));
}

#[test]
fn timeout_kills_the_entire_process_group() {
    let mut request = request("child-tree");
    request.limits.timeout = Duration::from_millis(100);
    let error = run_bounded_process(&request).expect_err("helper must time out");
    assert_matches!(error, ProcessError::TimedOut { .. });
    let ProcessError::TimedOut { failure: error, .. } = error else {
        unreachable!("timeout shape checked above");
    };
    let output = String::from_utf8_lossy(&error.stdout);
    let pid = output
        .lines()
        .find_map(|line| line.strip_prefix("grandchild-pid="))
        .expect("grandchild pid")
        .parse::<u32>()
        .expect("numeric grandchild pid");
    for _ in 0..100 {
        if !PathBuf::from(format!("/proc/{pid}")).exists() {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(
        !PathBuf::from(format!("/proc/{pid}")).exists(),
        "grandchild process {pid} survived process-group timeout"
    );
}

#[test]
fn leader_exit_closes_descendants_that_retain_pipe_handles() {
    let output = run_bounded_process(&request("leader-exit-with-descendant"))
        .expect("leader exit with inherited descendant pipes completes");
    assert_eq!(output.status, ProcessExit::Exited(0));
    let pid = String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| line.strip_prefix("grandchild-pid="))
        .expect("grandchild pid")
        .parse::<u32>()
        .expect("numeric grandchild pid");
    for _ in 0..100 {
        if !PathBuf::from(format!("/proc/{pid}")).exists() {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(
        !PathBuf::from(format!("/proc/{pid}")).exists(),
        "grandchild process {pid} survived normal leader exit"
    );
}

#[test]
fn managed_child_drop_kills_and_reaps_after_io_start() {
    let request = request("sleep");
    let mut command = std::process::Command::new(&request.program);
    command
        .args(&request.args)
        .current_dir(&request.working_directory)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let ProcessEnvironment::ClearAndSet(environment) = &request.environment {
        command
            .env_clear()
            .envs(environment.iter().map(|(key, value)| (key, value)));
    }
    let mut managed =
        SupervisedChild::spawn_command(&mut command, request.program.clone()).expect("spawn");
    let pid = managed.leader.id();
    managed.start_io(&request).expect("start managed IO");
    let dropped = Instant::now();
    drop(managed);
    assert!(dropped.elapsed() < Duration::from_secs(2));

    for _ in 0..100 {
        if !PathBuf::from(format!("/proc/{pid}")).exists() {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(
        !PathBuf::from(format!("/proc/{pid}")).exists(),
        "managed child {pid} survived guard cleanup"
    );
}

const LAUNCHER_TEST: &str = "tests::launcher_entry";

/// Runs `mode` through the measurement launcher, played by this test binary.
fn measured(mode: &str) -> ProcessRequest {
    let mut request = request(mode);
    request.launch = ProcessLaunch::Measured(Launcher {
        program: std::env::current_exe().expect("test executable"),
        args: [LAUNCHER_TEST, "--exact", "--ignored", "--nocapture"]
            .into_iter()
            .map(OsString::from)
            .collect(),
    });
    request
}

#[test]
#[ignore = "executed only as the measurement launcher by bounded-process tests"]
fn launcher_entry() {
    std::process::exit(i32::from(launcher::launcher_exit_code()));
}

/// Regression: a directly spawned child inherited the supervisor's high-water
/// RSS, so every benchmark reported the runner's footprint instead of its own.
#[test]
fn measured_peak_rss_excludes_the_supervisor_footprint() {
    let mut supervisor = vec![0_u8; 128 << 20];
    for byte in supervisor.iter_mut().step_by(4096) {
        *byte = 1;
    }
    std::hint::black_box(&supervisor);

    let idle = run_bounded_process(&measured("success")).expect("measured helper");
    assert_eq!(idle.status, ProcessExit::Exited(0));
    assert!(String::from_utf8_lossy(&idle.stdout).contains("helper-success"));
    let idle_peak = idle.parent_observed_peak_rss_bytes.expect("measured peak");
    assert!(
        idle_peak < 64 << 20,
        "an idle child reported {idle_peak} bytes after the supervisor touched 128 MiB"
    );

    let allocating = run_bounded_process(&measured("peak-rss")).expect("measured allocation");
    let allocating_peak = allocating
        .parent_observed_peak_rss_bytes
        .expect("measured peak");
    assert!(
        allocating_peak >= idle_peak + (24 << 20),
        "a 32 MiB allocation reported {allocating_peak} bytes over an idle {idle_peak}"
    );
    drop(supervisor);
}

#[test]
fn measured_runs_report_the_program_status_affinity_and_file_limit() {
    assert_eq!(
        run_bounded_process(&measured("nonzero"))
            .expect("nonzero")
            .status,
        ProcessExit::Exited(7)
    );
    assert_eq!(
        run_bounded_process(&measured("signal"))
            .expect("signal")
            .status,
        ProcessExit::Signaled(signal_hook::consts::signal::SIGTERM)
    );

    let allowed = rustix::thread::sched_getaffinity(None).expect("read parent affinity");
    let cpu = (0..rustix::thread::CpuSet::MAX_CPU)
        .find(|cpu| allowed.is_set(*cpu))
        .expect("at least one allowed CPU");
    let mut pinned = measured("affinity");
    pinned.stdin = vec![b'\n'];
    pinned.affinity_cpu = Some(cpu);
    pinned.environment.push((
        OsString::from(EXPECTED_CPU_ENV),
        OsString::from(cpu.to_string()),
    ));
    let result = run_bounded_process(&pinned).expect("affinity helper succeeds");
    assert!(String::from_utf8_lossy(&result.stdout).contains("affinity-ok"));

    let directory = tempfile::tempdir().expect("temporary output directory");
    let output = directory.path().join("bounded-output");
    let mut bounded_file = measured("file-overflow");
    bounded_file.stdin = vec![b'\n'];
    bounded_file.limits.stdin_bytes = 1;
    bounded_file.limits.regular_file_bytes = Some(8 << 10);
    bounded_file.environment.push((
        OsString::from(OUTPUT_PATH_ENV),
        output.as_os_str().to_os_string(),
    ));
    assert_matches!(
        run_bounded_process(&bounded_file),
        Err(ProcessError::FileLimit { maximum, .. }) if maximum == 8 << 10
    );

    // The launcher writes its own report under the program's limit.
    let mut below_report = measured("success");
    below_report.limits.regular_file_bytes = Some(64);
    assert_matches!(
        run_bounded_process(&below_report),
        Err(ProcessError::FileLimitBelowLauncherReport { minimum: 4096, .. })
    );

    let mut invalid_affinity = measured("success");
    invalid_affinity.affinity_cpu = Some(rustix::thread::CpuSet::MAX_CPU);
    assert_matches!(
        run_bounded_process(&invalid_affinity),
        Err(ProcessError::Launch { .. })
    );
}

#[test]
#[ignore = "executed only as a subprocess by bounded-process tests"]
fn process_helper() {
    let mode = std::env::var(HELPER_ENV).expect("helper mode");
    match mode.as_str() {
        "success" => println!("helper-success"),
        "nonzero" => std::process::exit(7),
        "signal" => {
            let result = rustix::process::kill_process(
                rustix::process::getpid(),
                rustix::process::Signal::TERM,
            );
            assert!(result.is_ok());
            std::thread::sleep(Duration::from_secs(30));
        }
        "stdout-overflow" => println!("{}", "x".repeat(1024)),
        "stderr-overflow" => eprintln!("{}", "x".repeat(1024)),
        "invalid-utf8-flood" => {
            let mut stdout = std::io::stdout().lock();
            stdout
                .write_all(&[0xff])
                .expect("write invalid UTF-8 prefix");
            stdout
                .write_all(&vec![b'x'; 32 << 10])
                .expect("write diagnostic flood");
        }
        "output-before-stdin" => {
            let mut stdout = std::io::stdout().lock();
            stdout
                .write_all(&vec![b'o'; 1 << 20])
                .expect("write output before stdin");
            stdout.flush().expect("flush output before stdin");
            let mut stdin = Vec::new();
            std::io::stdin()
                .read_to_end(&mut stdin)
                .expect("drain concurrent stdin");
            assert_eq!(stdin.len(), 1 << 20);
            stdout
                .write_all(b"stdin-drained\n")
                .expect("write completion marker");
        }
        "close-stdin" => {
            eprint!("rejected");
            std::process::exit(3);
        }
        "file-overflow" => {
            let mut barrier = [0_u8; 1];
            std::io::stdin()
                .read_exact(&mut barrier)
                .expect("read file limit barrier");
            assert_eq!(barrier, *b"\n");
            let path = std::env::var_os(OUTPUT_PATH_ENV).expect("output path");
            std::fs::write(path, vec![0_u8; 64 << 10]).expect("file limit terminates write");
        }
        "file-overflow-immediate" => {
            let path = std::env::var_os(OUTPUT_PATH_ENV).expect("output path");
            std::fs::write(path, vec![0_u8; 64 << 10]).expect("file limit terminates write");
        }
        "escaped-holder-exit" | "escaped-holder-sleep" => {
            let mut holder =
                std::process::Command::new(std::env::current_exe().expect("helper executable"))
                    .args([HELPER_TEST, "--exact", "--ignored", "--nocapture"])
                    .env_clear()
                    .env(HELPER_ENV, "setsid-grandchild")
                    .stderr(Stdio::piped())
                    .spawn()
                    .expect("spawn escaping grandchild");
            let mut escaped = String::new();
            std::io::BufReader::new(holder.stderr.take().expect("grandchild stderr"))
                .read_line(&mut escaped)
                .expect("read escape marker");
            assert!(escaped.ends_with("escaped\n"), "{escaped:?}");
            println!("grandchild-pid={}", holder.id());
            std::io::stdout().flush().expect("flush grandchild pid");
            drop(holder);
            if mode == "escaped-holder-sleep" {
                std::thread::sleep(Duration::from_secs(30));
            }
        }
        "setsid-grandchild" => {
            // Leave the supervised session and process group, keeping the
            // inherited stdout, then tell the leader through stderr.
            rustix::process::setsid().expect("leave the supervised group");
            eprintln!("escaped");
            std::thread::sleep(Duration::from_secs(30));
        }
        "panic" => {
            std::env::var_os("STAB_BENCH_INTENTIONALLY_MISSING").expect("process helper panic");
        }
        "peak-rss" => {
            let mut allocation = vec![0_u8; 32 << 20];
            for byte in allocation.iter_mut().step_by(4096) {
                *byte = 1;
            }
            std::hint::black_box(&allocation);
        }
        "sleep" => std::thread::sleep(Duration::from_secs(30)),
        "child-tree" => {
            let child =
                std::process::Command::new(std::env::current_exe().expect("helper executable"))
                    .args([HELPER_TEST, "--exact", "--ignored", "--nocapture"])
                    .env_clear()
                    .env(HELPER_ENV, "grandchild")
                    .spawn()
                    .expect("spawn grandchild");
            println!("grandchild-pid={}", child.id());
            std::io::stdout().flush().expect("flush grandchild pid");
            drop(child);
            std::thread::sleep(Duration::from_secs(30));
        }
        "leader-exit-with-descendant" => {
            let child =
                std::process::Command::new(std::env::current_exe().expect("helper executable"))
                    .args([HELPER_TEST, "--exact", "--ignored", "--nocapture"])
                    .env_clear()
                    .env(HELPER_ENV, "grandchild")
                    .spawn()
                    .expect("spawn grandchild");
            println!("grandchild-pid={}", child.id());
            std::io::stdout().flush().expect("flush grandchild pid");
            drop(child);
        }
        "affinity" => {
            let mut barrier = [0_u8; 1];
            std::io::stdin()
                .read_exact(&mut barrier)
                .expect("read affinity barrier");
            assert_eq!(barrier, *b"\n");
            let expected = std::env::var(EXPECTED_CPU_ENV)
                .expect("expected CPU")
                .parse::<usize>()
                .expect("numeric CPU");
            let set = rustix::thread::sched_getaffinity(None).expect("read child affinity");
            let actual = (0..rustix::thread::CpuSet::MAX_CPU)
                .filter(|cpu| set.is_set(*cpu))
                .collect::<Vec<_>>();
            assert_eq!(actual, [expected]);
            println!("affinity-ok");
        }
        "affinity-existing-threads" => {
            let expected = std::env::var(EXPECTED_CPU_ENV)
                .expect("expected CPU")
                .parse::<usize>()
                .expect("numeric CPU");
            let (release, wait) = std::sync::mpsc::channel();
            let worker = std::thread::spawn(move || {
                wait.recv().expect("worker affinity barrier");
                assert_current_affinity(expected);
            });
            println!("threads-ready");
            std::io::stdout().flush().expect("flush ready marker");
            let mut barrier = [0_u8; 1];
            std::io::stdin()
                .read_exact(&mut barrier)
                .expect("read affinity barrier");
            assert_eq!(barrier, *b"\n");
            release.send(()).expect("release affinity worker");
            assert_current_affinity(expected);
            worker.join().expect("affinity worker");
            println!("affinity-all-tasks-ok");
        }
        "grandchild" => std::thread::sleep(Duration::from_secs(30)),
        other => {
            eprintln!("unknown helper mode {other}");
            std::process::exit(125);
        }
    }
}

fn assert_current_affinity(expected: usize) {
    let set = rustix::thread::sched_getaffinity(None).expect("read child affinity");
    let actual = (0..rustix::thread::CpuSet::MAX_CPU)
        .filter(|cpu| set.is_set(*cpu))
        .collect::<Vec<_>>();
    assert_eq!(actual, [expected]);
}
