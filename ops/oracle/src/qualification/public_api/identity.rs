use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use super::{PublicApiError, json_id};

pub(super) fn resolved_path_name(
    value: &Value,
    paths: &Map<String, Value>,
) -> Result<String, PublicApiError> {
    let resolved = value.get("resolved_path").unwrap_or(value);
    let base = value
        .get("resolved_path")
        .and_then(|path| path.get("path"))
        .or_else(|| value.get("path"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or(PublicApiError::InvalidField("impl.trait.path"))?;
    let Some(args) = resolved.get("args").filter(|args| !args.is_null()) else {
        return Ok(base);
    };
    let suffix = canonical_value_digest_with_paths(args, paths);
    Ok(format!("{base}@{suffix}"))
}

pub(super) fn resolved_path_id(value: &Value) -> Result<Option<String>, PublicApiError> {
    let Some(path) = value.get("resolved_path") else {
        return Ok(None);
    };
    path.get("id")
        .map(json_id)
        .transpose()
        .map_err(|_| PublicApiError::InvalidField("resolved_path.id"))
}

pub(super) fn canonical_value_digest(value: &Value) -> String {
    canonical_value_digest_with_paths(value, &Map::new())
}

fn canonical_value_digest_with_paths(value: &Value, paths: &Map<String, Value>) -> String {
    let canonical = canonicalize_rustdoc_value(value, paths);
    let digest = Sha256::digest(canonical.to_string().as_bytes());
    digest
        .iter()
        .take(6)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn canonicalize_rustdoc_value(value: &Value, paths: &Map<String, Value>) -> Value {
    match value {
        Value::Array(values) => Value::Array(
            values
                .iter()
                .map(|value| canonicalize_rustdoc_value(value, paths))
                .collect(),
        ),
        Value::Object(values) => {
            let qualified_path = values
                .get("id")
                .and_then(|id| json_id(id).ok())
                .and_then(|id| paths.get(&id))
                .and_then(|summary| summary.get("path"))
                .and_then(Value::as_array)
                .map(|segments| {
                    segments
                        .iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join("::")
                })
                .filter(|path| !path.is_empty());
            let mut canonical = values
                .iter()
                .filter(|(key, _)| !matches!(key.as_str(), "id" | "crate_id"))
                .map(|(key, value)| (key.clone(), canonicalize_rustdoc_value(value, paths)))
                .collect::<Map<_, _>>();
            if let Some(path) = qualified_path {
                canonical.insert("path".to_string(), Value::String(path));
            }
            Value::Object(canonical)
        }
        _ => value.clone(),
    }
}
