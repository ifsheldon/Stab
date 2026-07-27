use serde_json::Value;
use sha2::{Digest, Sha256};

use super::{PublicApiError, json_id};

pub(super) fn resolved_path_name(value: &Value) -> Result<String, PublicApiError> {
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
    let suffix = canonical_value_digest(args);
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
    let canonical = canonicalize_rustdoc_value(value);
    let digest = Sha256::digest(canonical.to_string().as_bytes());
    digest
        .iter()
        .take(6)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn canonicalize_rustdoc_value(value: &Value) -> Value {
    match value {
        Value::Array(values) => {
            Value::Array(values.iter().map(canonicalize_rustdoc_value).collect())
        }
        Value::Object(values) => Value::Object(
            values
                .iter()
                .filter(|(key, _)| !matches!(key.as_str(), "id" | "crate_id"))
                .map(|(key, value)| (key.clone(), canonicalize_rustdoc_value(value)))
                .collect(),
        ),
        _ => value.clone(),
    }
}
