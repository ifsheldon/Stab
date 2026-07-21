use crate::error::BenchError;

const MAX_JSON_OBJECTS: usize = 50_000;
const MAX_JSON_ARRAYS: usize = 50_000;
const MAX_JSON_DEPTH: usize = 64;
const MAX_JSON_STRING_TOKEN_BYTES: usize = 16_384;

pub(super) fn preflight_json_shape(bytes: &[u8]) -> Result<(), BenchError> {
    let mut objects = 0_usize;
    let mut arrays = 0_usize;
    let mut depth = 0_usize;
    let mut in_string = false;
    let mut escaped = false;
    let mut string_bytes = 0_usize;
    for byte in bytes {
        if in_string {
            string_bytes = string_bytes.saturating_add(1);
            if string_bytes > MAX_JSON_STRING_TOKEN_BYTES {
                return Err(BenchError::Qualification(format!(
                    "qualification JSON string token exceeds {MAX_JSON_STRING_TOKEN_BYTES} bytes"
                )));
            }
            if escaped {
                escaped = false;
            } else if *byte == b'\\' {
                escaped = true;
            } else if *byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match *byte {
            b'"' => {
                in_string = true;
                string_bytes = 0;
            }
            b'{' => {
                objects = objects.saturating_add(1);
                depth = depth.saturating_add(1);
            }
            b'[' => {
                arrays = arrays.saturating_add(1);
                depth = depth.saturating_add(1);
            }
            b'}' | b']' => {
                depth = depth.checked_sub(1).ok_or_else(|| {
                    BenchError::Qualification(
                        "qualification JSON closes an unopened container".to_string(),
                    )
                })?;
            }
            _ => {}
        }
        if objects > MAX_JSON_OBJECTS || arrays > MAX_JSON_ARRAYS || depth > MAX_JSON_DEPTH {
            return Err(BenchError::Qualification(format!(
                "qualification JSON exceeds shape limits: objects={objects}/{MAX_JSON_OBJECTS} arrays={arrays}/{MAX_JSON_ARRAYS} depth={depth}/{MAX_JSON_DEPTH}"
            )));
        }
    }
    if in_string || depth != 0 {
        return Err(BenchError::Qualification(
            "qualification JSON has an unterminated string or container".to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_preflight_rejects_deep_and_oversized_string_shapes() {
        let deep = format!("{}{}", "[".repeat(65), "]".repeat(65));
        let long_string = format!("\"{}\"", "x".repeat(MAX_JSON_STRING_TOKEN_BYTES + 1));

        assert!(preflight_json_shape(deep.as_bytes()).is_err());
        assert!(preflight_json_shape(long_string.as_bytes()).is_err());
        assert!(preflight_json_shape(br#"{"safe":[1,2,3]}"#).is_ok());
    }
}
