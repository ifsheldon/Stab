use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use super::{MAX_PUBLIC_API_PATH_BYTES, PublicApiError};
use crate::qualification::model::PublicApiKind;

pub(super) fn rustc_host_target(root: &Path) -> Result<String, PublicApiError> {
    let output = crate::run_process(Path::new("rustc"), ["-vV"], &[], Some(root))
        .map_err(|source| PublicApiError::HostTarget(source.to_string().into_boxed_str()))?;
    if !output.success() {
        return Err(PublicApiError::HostTarget(
            format!(
                "rustc -vV exited with {}: {}",
                crate::process::display_status(output.status),
                output.stderr.render_for_diagnostics()
            )
            .into_boxed_str(),
        ));
    }
    let stdout = std::str::from_utf8(&output.stdout.bytes)
        .map_err(|_| PublicApiError::HostTarget("rustc -vV output is not UTF-8".into()))?;
    let host = stdout
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .ok_or_else(|| PublicApiError::HostTarget("rustc -vV omitted its host target".into()))?;
    if host.is_empty()
        || host.len() > 128
        || !host
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(PublicApiError::HostTarget(
            format!("rustc reported invalid host target {host:?}").into_boxed_str(),
        ));
    }
    Ok(host.to_string())
}

pub(super) fn item<'a>(
    index: &'a Map<String, Value>,
    id: &str,
) -> Result<&'a Value, PublicApiError> {
    index
        .get(id)
        .ok_or_else(|| PublicApiError::MissingItem(id.to_string()))
}

pub(super) fn inner(value: &Value) -> Result<&Map<String, Value>, PublicApiError> {
    value
        .get("inner")
        .and_then(Value::as_object)
        .ok_or(PublicApiError::InvalidField("item.inner"))
}

pub(super) fn first_key(inner: &Map<String, Value>) -> Result<&str, PublicApiError> {
    if inner.len() != 1 {
        return Err(PublicApiError::InvalidField("item.inner kind"));
    }
    inner
        .keys()
        .next()
        .map(String::as_str)
        .ok_or(PublicApiError::InvalidField("item.inner kind"))
}

pub(super) fn json_id(value: &Value) -> Result<String, PublicApiError> {
    match value {
        Value::Number(number) => Ok(number.to_string()),
        Value::String(value) => Ok(value.clone()),
        _ => Err(PublicApiError::InvalidField("item id")),
    }
}

pub(super) fn is_public(value: &Value) -> bool {
    value.get("visibility").and_then(Value::as_str) == Some("public")
}

pub(super) fn is_doc_hidden(value: &Value) -> bool {
    value
        .get("attrs")
        .and_then(Value::as_array)
        .is_some_and(|attrs| {
            attrs.iter().filter_map(attribute_text).any(|attr| {
                let compact = attr
                    .chars()
                    .filter(|ch| !ch.is_whitespace())
                    .collect::<String>();
                compact.contains("doc(hidden)")
            })
        })
}

fn attribute_text(value: &Value) -> Option<&str> {
    value
        .as_str()
        .or_else(|| value.get("other").and_then(Value::as_str))
}

pub(super) fn direct_kind(kind: &str) -> Option<PublicApiKind> {
    match kind {
        "constant" => Some(PublicApiKind::Constant),
        "enum" => Some(PublicApiKind::Enum),
        "function" => Some(PublicApiKind::Function),
        "macro" | "proc_macro" => Some(PublicApiKind::Macro),
        "module" => Some(PublicApiKind::Module),
        "static" => Some(PublicApiKind::Static),
        "struct" => Some(PublicApiKind::Struct),
        "trait" => Some(PublicApiKind::Trait),
        "type_alias" => Some(PublicApiKind::TypeAlias),
        "union" => Some(PublicApiKind::Union),
        _ => None,
    }
}

pub(super) fn source_span(value: &Value) -> Option<(PathBuf, u32)> {
    let span = value.get("span")?.as_object()?;
    let filename = span.get("filename")?.as_str()?;
    let begin = span.get("begin")?.as_array()?;
    let line = u32::try_from(begin.first()?.as_u64()?).ok()?;
    Some((PathBuf::from(filename), line))
}

pub(super) fn join_path(parent: &str, name: &str) -> Result<String, PublicApiError> {
    let path = format!("{parent}::{name}");
    validate_api_path(&path)?;
    Ok(path)
}

pub(super) fn rustdoc_path(
    paths: &Map<String, Value>,
    item_id: &str,
) -> Result<Option<String>, PublicApiError> {
    let Some(path_entry) = paths.get(item_id) else {
        return Ok(None);
    };
    let path = path_entry
        .get("path")
        .and_then(Value::as_array)
        .ok_or(PublicApiError::InvalidField("paths.path"))?
        .iter()
        .map(|component| {
            component
                .as_str()
                .ok_or(PublicApiError::InvalidField("paths.path component"))
        })
        .collect::<Result<Vec<_>, _>>()?
        .join("::");
    validate_api_path(&path)?;
    Ok(Some(path))
}

pub(super) fn validate_api_path(path: &str) -> Result<(), PublicApiError> {
    if path.is_empty()
        || path.len() > MAX_PUBLIC_API_PATH_BYTES
        || path.chars().any(char::is_control)
    {
        Err(PublicApiError::InvalidPath {
            path: path.to_string(),
        })
    } else {
        Ok(())
    }
}
