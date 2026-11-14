//! CPU-affinity and file-size limits applied to supervised processes.

use std::path::PathBuf;

const MAX_AFFINITY_PASSES: usize = 8;
const MAX_CHILD_TASKS: usize = 4_096;

/// Pins the calling thread to one CPU so processes it spawns inherit the mask.
pub(super) fn pin_current_thread(cpu: usize) -> Result<(), std::io::Error> {
    let set = singleton_cpu_set(cpu)?;
    rustix::thread::sched_setaffinity(None, &set)?;
    if affinity_is_singleton(&rustix::thread::sched_getaffinity(None)?, cpu) {
        Ok(())
    } else {
        Err(std::io::Error::other(format!(
            "the launcher did not converge to CPU {cpu} affinity"
        )))
    }
}

fn singleton_cpu_set(cpu: usize) -> Result<rustix::thread::CpuSet, std::io::Error> {
    if cpu >= rustix::thread::CpuSet::MAX_CPU {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("CPU {cpu} exceeds the supported affinity mask"),
        ));
    }
    let mut set = rustix::thread::CpuSet::new();
    set.set(cpu);
    Ok(set)
}

fn bounded_file_limit(requested_maximum: u64) -> Result<u64, std::io::Error> {
    let inherited = rustix::process::getrlimit(rustix::process::Resource::Fsize);
    [
        Some(requested_maximum),
        inherited.current,
        inherited.maximum,
    ]
    .into_iter()
    .flatten()
    .min()
    .ok_or_else(|| std::io::Error::other("file-size limit has no finite bound"))
}

pub(super) fn set_child_affinity(pid: u32, cpu: usize) -> Result<(), std::io::Error> {
    let set = singleton_cpu_set(cpu)?;
    let leader = process_id(pid)?;

    // Pin the leader first so newly created threads inherit the requested mask,
    // then close the post-spawn race by pinning threads that already exist.
    set_task_affinity(leader, &set)?;
    for _ in 0..MAX_AFFINITY_PASSES {
        let task_ids = child_task_ids(pid)?;
        for task_id in task_ids {
            if let Err(source) = rustix::thread::sched_setaffinity(Some(task_id), &set)
                && source != rustix::io::Errno::SRCH
            {
                return Err(source.into());
            }
        }

        let mut complete = true;
        for task_id in child_task_ids(pid)? {
            match rustix::thread::sched_getaffinity(Some(task_id)) {
                Ok(actual) if affinity_is_singleton(&actual, cpu) => {}
                Ok(_) | Err(rustix::io::Errno::SRCH) => complete = false,
                Err(source) => return Err(source.into()),
            }
        }
        if complete {
            return Ok(());
        }
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::WouldBlock,
        format!("child process {pid} did not converge to CPU {cpu} affinity"),
    ))
}

fn process_id(pid: u32) -> Result<rustix::process::Pid, std::io::Error> {
    let raw_pid = i32::try_from(pid).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("child process id {pid} exceeds the supported Linux pid range"),
        )
    })?;
    rustix::process::Pid::from_raw(raw_pid).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("child process id {pid} is invalid"),
        )
    })
}

fn set_task_affinity(
    task_id: rustix::process::Pid,
    set: &rustix::thread::CpuSet,
) -> Result<(), std::io::Error> {
    rustix::thread::sched_setaffinity(Some(task_id), set).map_err(Into::into)
}

fn child_task_ids(pid: u32) -> Result<Vec<rustix::process::Pid>, std::io::Error> {
    let path = PathBuf::from(format!("/proc/{pid}/task"));
    let entries = std::fs::read_dir(&path)?;
    let mut task_ids = Vec::new();
    for entry in entries {
        if task_ids.len() == MAX_CHILD_TASKS {
            return Err(std::io::Error::other(format!(
                "child process {pid} exceeds the {MAX_CHILD_TASKS}-task affinity limit"
            )));
        }
        let name = entry?.file_name().into_string().map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("child task directory {path:?} contains a non-UTF-8 entry"),
            )
        })?;
        let raw_task_id = name.parse::<i32>().map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("child task directory {path:?} contains nonnumeric entry {name:?}"),
            )
        })?;
        let task_id = rustix::process::Pid::from_raw(raw_task_id).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("child task directory {path:?} contains invalid task id {name:?}"),
            )
        })?;
        task_ids.push(task_id);
    }
    if task_ids.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("child process {pid} has no visible tasks"),
        ));
    }
    Ok(task_ids)
}

fn affinity_is_singleton(set: &rustix::thread::CpuSet, cpu: usize) -> bool {
    (0..rustix::thread::CpuSet::MAX_CPU)
        .all(|candidate| set.is_set(candidate) == (candidate == cpu))
}

pub(super) fn set_child_file_limit(pid: u32, requested_maximum: u64) -> Result<(), std::io::Error> {
    let raw_pid = i32::try_from(pid).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("child process id {pid} exceeds the supported Linux pid range"),
        )
    })?;
    let pid = rustix::process::Pid::from_raw(raw_pid).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("child process id {pid} is invalid"),
        )
    })?;
    let maximum = bounded_file_limit(requested_maximum)?;
    rustix::process::prlimit(
        Some(pid),
        rustix::process::Resource::Fsize,
        rustix::process::Rlimit {
            current: Some(maximum),
            maximum: Some(maximum),
        },
    )
    .map(|_| ())
    .map_err(Into::into)
}
