use std::path::PathBuf;

use crate::report::CompareRowResult;

pub(crate) const HOT_PATH_PROFILER_NOTE_RATIO: f64 = 1.5;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct ProfilerNoteFindings {
    pub(super) blockers: Vec<String>,
}

pub(super) fn apply_profiler_notes(
    rows: &mut [CompareRowResult],
    note_dirs: &[(PathBuf, PathBuf)],
) -> ProfilerNoteFindings {
    let mut findings = ProfilerNoteFindings::default();
    for row in rows {
        let file_name = format!("{}.md", row.id);
        let matches = match find_profiler_note_matches(note_dirs, &file_name) {
            Ok(matches) => matches,
            Err(error) => {
                record_profiler_note_error(row, &mut findings, error, None);
                continue;
            }
        };
        if matches.len() > 1 {
            let paths = matches
                .iter()
                .map(|(read_path, _)| read_path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            record_profiler_note_error(
                row,
                &mut findings,
                ProfilerNoteError::Invalid(format!(
                    "profiler note exists in multiple configured directories: {paths}"
                )),
                matches.first().map(|(_, report_path)| report_path.clone()),
            );
            continue;
        }
        if !row
            .relative_ratio
            .is_some_and(|ratio| ratio > HOT_PATH_PROFILER_NOTE_RATIO)
        {
            row.profiler_note_status = "not-required".to_string();
            continue;
        }
        match read_and_validate_profiler_note(matches.first()) {
            Ok(relative_path) => {
                row.profiler_note_path = Some(relative_path.display().to_string());
                row.profiler_note_status = "present".to_string();
            }
            Err(error) => {
                let report_path = matches
                    .first()
                    .map(|(_, report_path)| report_path.clone())
                    .or_else(|| {
                        note_dirs
                            .first()
                            .map(|(_, report_dir)| report_dir.join(&file_name))
                    });
                record_profiler_note_error(row, &mut findings, error, report_path);
            }
        }
    }
    findings
}

pub(super) fn profiler_note_report_metadata(paths: &[PathBuf]) -> (Option<String>, Vec<String>) {
    let paths = paths
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>();
    (paths.first().cloned(), paths)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ProfilerNoteError {
    Missing,
    Invalid(String),
}

impl ProfilerNoteError {
    fn status(&self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Invalid(_) => "invalid",
        }
    }

    pub(crate) fn message(&self) -> &str {
        match self {
            Self::Missing => "profiler note is missing",
            Self::Invalid(message) => message,
        }
    }
}

fn record_profiler_note_error(
    row: &mut CompareRowResult,
    findings: &mut ProfilerNoteFindings,
    error: ProfilerNoteError,
    report_path: Option<PathBuf>,
) {
    row.profiler_note_path = report_path.map(|path| path.display().to_string());
    row.profiler_note_status = error.status().to_string();
    row.profiler_note_error = Some(error.message().to_string());
    findings
        .blockers
        .push(format!("{}: {}", row.id, error.message()));
}

fn find_profiler_note_matches(
    note_dirs: &[(PathBuf, PathBuf)],
    file_name: &str,
) -> Result<Vec<(PathBuf, PathBuf)>, ProfilerNoteError> {
    let mut found = Vec::new();
    for (read_dir, report_dir) in note_dirs {
        let path = read_dir.join(file_name);
        match std::fs::metadata(&path) {
            Ok(_) => found.push((path, report_dir.join(file_name))),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(ProfilerNoteError::Invalid(format!(
                    "failed to inspect profiler note {}: {error}",
                    path.display()
                )));
            }
        }
    }
    Ok(found)
}

fn read_and_validate_profiler_note(
    note: Option<&(PathBuf, PathBuf)>,
) -> Result<PathBuf, ProfilerNoteError> {
    let (read_path, report_path) = note.ok_or(ProfilerNoteError::Missing)?;
    let content = std::fs::read_to_string(read_path).map_err(|error| {
        ProfilerNoteError::Invalid(format!(
            "failed to read profiler note {}: {error}",
            read_path.display()
        ))
    })?;
    validate_profiler_note_content(&content).map_err(|error| {
        ProfilerNoteError::Invalid(format!("{}: {}", read_path.display(), error.message()))
    })?;
    Ok(report_path.clone())
}

pub(crate) fn validate_profiler_note_content(content: &str) -> Result<(), ProfilerNoteError> {
    if content.trim().is_empty() {
        return Err(ProfilerNoteError::Invalid(
            "profiler note is empty".to_string(),
        ));
    }
    if !has_named_nonempty_field(content, "Dominant cost:") {
        return Err(ProfilerNoteError::Invalid(
            "profiler note must include `Dominant cost:`".to_string(),
        ));
    }
    if !has_named_nonempty_field(content, "Next owner action:") {
        return Err(ProfilerNoteError::Invalid(
            "profiler note must include `Next owner action:`".to_string(),
        ));
    }
    Ok(())
}

fn has_named_nonempty_field(content: &str, field: &str) -> bool {
    content.lines().any(|line| {
        line.trim_start()
            .strip_prefix(field)
            .is_some_and(|value| !value.trim().is_empty())
    })
}
