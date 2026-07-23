use super::*;
use std::io::BufRead as _;

const HELPER_TEST: &str = "process::tests::process_helper";
const HELPER_ENV: &str = "STAB_BENCH_PROCESS_HELPER";
const EXPECTED_CPU_ENV: &str = "STAB_BENCH_EXPECTED_CPU";
const OUTPUT_PATH_ENV: &str = "STAB_BENCH_OUTPUT_PATH";

fn request(mode: &str) -> ProcessRequest {
    ProcessRequest {
        program: std::env::current_exe().expect("test executable"),
        args: vec![
            OsString::from(HELPER_TEST),
            OsString::from("--exact"),
            OsString::from("--ignored"),
            OsString::from("--nocapture"),
        ],
        stdin: Vec::new(),
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
    assert_eq!(success.status, Some(0));
    assert!(String::from_utf8_lossy(&success.stdout).contains("helper-success"));

    let nonzero = run_bounded_process(&request("nonzero")).expect("nonzero is captured");
    assert_eq!(nonzero.status, Some(7));

    let signalled = run_bounded_process(&request("signal")).expect("signal is captured");
    assert_eq!(signalled.status, None);
}

#[test]
fn rejects_missing_binary_and_all_stream_limits() {
    let mut missing = request("success");
    missing.program = PathBuf::from("/definitely/missing/stab-bench-worker");
    assert!(matches!(
        run_bounded_process(&missing),
        Err(ProcessError::Spawn { .. })
    ));

    let mut stdin = request("success");
    stdin.stdin = vec![0_u8; 2];
    stdin.limits.stdin_bytes = 1;
    assert!(matches!(
        run_bounded_process(&stdin),
        Err(ProcessError::StdinLimit { .. })
    ));

    let mut stdout = request("stdout-overflow");
    stdout.limits.stdout = 32.into();
    assert!(matches!(
        run_bounded_process(&stdout),
        Err(ProcessError::OutputLimit(error)) if error.stream == "stdout"
    ));

    let mut stderr = request("stderr-overflow");
    stderr.limits.stderr = 32.into();
    assert!(matches!(
        run_bounded_process(&stderr),
        Err(ProcessError::OutputLimit(error)) if error.stream == "stderr"
    ));
}

#[test]
fn propagates_writer_failure() {
    let mut request = request("close-stdin");
    request.stdin = vec![0_u8; 8 << 20];
    let result = run_bounded_process(&request);
    assert!(matches!(result, Err(ProcessError::WriteStdin { .. })));
}

#[test]
fn drains_child_output_while_writing_stdin() {
    let mut request = request("output-before-stdin");
    request.stdin = vec![b'i'; 1 << 20];
    request.limits.stdout = (2 << 20).into();

    let output = run_bounded_process(&request).expect("concurrent pipe transfer succeeds");

    assert_eq!(output.status, Some(0));
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
    let cancellation = ProcessCancellation::for_test();
    let trigger = cancellation.clone();
    let canceller = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        trigger.cancel();
    });

    let result = run_bounded_process_with_cancellation(&request, &cancellation);
    canceller.join().expect("cancellation thread");

    assert!(matches!(result, Err(ProcessError::Interrupted(_))));
}

#[test]
fn discard_policy_drains_without_capturing_or_limiting_output() {
    let mut request = request("stdout-overflow");
    request.limits.stdout = OutputPolicy::Discard;

    let output = run_bounded_process(&request).expect("discarded output succeeds");

    assert_eq!(output.status, Some(0));
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
    let ProcessError::OutputLimit(error) = error else {
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
        Some(0),
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
fn rejects_invalid_affinity_and_bounds_regular_files() {
    let mut invalid_affinity = request("success");
    invalid_affinity.affinity_cpu = Some(rustix::thread::CpuSet::MAX_CPU);
    assert!(matches!(
        run_bounded_process(&invalid_affinity),
        Err(ProcessError::SetAffinity { .. })
    ));

    let directory = tempfile::tempdir().expect("temporary output directory");
    let output = directory.path().join("bounded-output");
    let mut bounded_file = request("file-overflow");
    bounded_file.stdin = vec![b'\n'];
    bounded_file.limits.stdin_bytes = 1;
    bounded_file.limits.regular_file_bytes = Some(64);
    bounded_file.environment.push((
        OsString::from(OUTPUT_PATH_ENV),
        output.as_os_str().to_os_string(),
    ));
    assert!(matches!(
        run_bounded_process(&bounded_file),
        Err(ProcessError::FileLimit(error)) if error.maximum == 64
    ));
    assert!(
        std::fs::metadata(output)
            .expect("bounded output exists")
            .len()
            <= 64
    );
}

#[test]
fn captures_worker_panic_as_a_failed_process() {
    let output = run_bounded_process(&request("panic")).expect("panic is captured");
    assert_eq!(output.status, Some(101));
    assert!(String::from_utf8_lossy(&output.stderr).contains("process helper panic"));
}

#[test]
fn timeout_kills_the_entire_process_group() {
    let mut request = request("child-tree");
    request.limits.timeout = Duration::from_millis(100);
    let error = run_bounded_process(&request).expect_err("helper must time out");
    assert!(matches!(error, ProcessError::TimedOut(_)));
    let ProcessError::TimedOut(error) = error else {
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
    assert_eq!(output.status, Some(0));
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
    use std::os::unix::process::CommandExt as _;
    command.process_group(0);
    let child = command.spawn().expect("spawn managed helper");
    let pid = child.id();
    let mut managed = ManagedChild::new(child, request.program.clone());
    managed.start_io(&request).expect("start managed IO");
    drop(managed);

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
        "close-stdin" => std::process::exit(0),
        "file-overflow" => {
            let mut barrier = [0_u8; 1];
            std::io::stdin()
                .read_exact(&mut barrier)
                .expect("read file limit barrier");
            assert_eq!(barrier, *b"\n");
            let path = std::env::var_os(OUTPUT_PATH_ENV).expect("output path");
            std::fs::write(path, vec![0_u8; 1024]).expect("file limit terminates write");
        }
        "panic" => {
            std::env::var_os("STAB_BENCH_INTENTIONALLY_MISSING").expect("process helper panic");
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
