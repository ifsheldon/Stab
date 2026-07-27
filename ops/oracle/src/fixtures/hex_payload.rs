use crate::RepoRoot;

use super::{FIXTURE_ROOT, FixtureError, paths};

pub(super) fn read(root: &RepoRoot, relative: &str) -> Result<Vec<u8>, FixtureError> {
    let bytes = paths::read_fixture_file(root, relative)?;
    if !relative.ends_with(".hex") {
        return Ok(bytes);
    }
    decode(&bytes).map_err(|reason| FixtureError::InvalidHexPayload {
        path: root.path.join(FIXTURE_ROOT).join(relative),
        reason,
    })
}

pub(super) fn write(root: &RepoRoot, relative: &str, bytes: &[u8]) -> Result<(), FixtureError> {
    if !relative.ends_with(".hex") {
        return paths::write_fixture_file(root, relative, bytes);
    }
    let mut encoded = Vec::with_capacity(bytes.len().saturating_mul(2).saturating_add(1));
    for byte in bytes {
        encoded.push(encode_digit(byte >> 4));
        encoded.push(encode_digit(byte & 0x0f));
    }
    encoded.push(b'\n');
    paths::write_fixture_file(root, relative, &encoded)
}

pub(super) fn decode(bytes: &[u8]) -> Result<Vec<u8>, Box<str>> {
    let digits = bytes
        .iter()
        .copied()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect::<Vec<_>>();
    if digits.len() % 2 != 0 {
        return Err("an even number of hexadecimal digits is required".into());
    }
    let mut decoded = Vec::with_capacity(digits.len() / 2);
    for pair in digits.chunks_exact(2) {
        let [high, low] = pair else {
            return Err("internal hex pair width was not two".into());
        };
        let high = decode_digit(*high)?;
        let low = decode_digit(*low)?;
        decoded.push((high << 4) | low);
    }
    Ok(decoded)
}

fn encode_digit(nibble: u8) -> u8 {
    if nibble < 10 {
        b'0' + nibble
    } else {
        b'a' + (nibble - 10)
    }
}

fn decode_digit(byte: u8) -> Result<u8, Box<str>> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(format!("byte 0x{byte:02x} is not a hexadecimal digit").into_boxed_str()),
    }
}
