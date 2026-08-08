//! Exact text and opaque-byte storage for Stim model tags.

use std::io::{self, Write};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ModelTag {
    Text(Box<str>),
    Opaque(Box<OpaqueModelTag>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct OpaqueModelTag {
    bytes: Box<[u8]>,
    display: Box<str>,
}

impl ModelTag {
    pub(crate) fn from_string(tag: String) -> Option<Self> {
        if tag.is_empty() {
            None
        } else {
            Some(Self::Text(tag.into_boxed_str()))
        }
    }

    pub(crate) fn from_bytes(tag: Vec<u8>) -> Option<Self> {
        if tag.is_empty() {
            return None;
        }
        match String::from_utf8(tag) {
            Ok(tag) => Some(Self::Text(tag.into_boxed_str())),
            Err(error) => {
                let bytes = error.into_bytes().into_boxed_slice();
                let display = String::from_utf8_lossy(&bytes)
                    .into_owned()
                    .into_boxed_str();
                Some(Self::Opaque(Box::new(OpaqueModelTag { bytes, display })))
            }
        }
    }

    pub(crate) fn from_slice(tag: &[u8]) -> Option<Self> {
        Self::from_bytes(tag.to_vec())
    }

    pub(crate) fn as_str(&self) -> &str {
        match self {
            Self::Text(tag) => tag,
            Self::Opaque(tag) => &tag.display,
        }
    }

    pub(crate) fn as_bytes(&self) -> &[u8] {
        match self {
            Self::Text(tag) => tag.as_bytes(),
            Self::Opaque(tag) => &tag.bytes,
        }
    }

    pub(crate) fn write_escaped_text(&self, out: &mut String) {
        write_escaped_tag_text(self.as_str(), out);
    }

    pub(crate) fn write_escaped_bytes(&self, out: &mut impl Write) -> io::Result<()> {
        write_escaped_tag_bytes(self.as_bytes(), out)
    }
}

/// Writes tag text with Stim's tag escaping: `]` becomes `\C`, CR becomes `\r`, LF becomes
/// `\n`, and `\` becomes `\B`.
///
/// This is the single owner of the tag escape table for circuit and DEM tags; the byte-level
/// twin is [`write_escaped_tag_bytes`].
pub(crate) fn write_escaped_tag_text(tag: &str, out: &mut String) {
    for ch in tag.chars() {
        match ch {
            ']' => out.push_str("\\C"),
            '\r' => out.push_str("\\r"),
            '\n' => out.push_str("\\n"),
            '\\' => out.push_str("\\B"),
            _ => out.push(ch),
        }
    }
}

/// Byte-level twin of [`write_escaped_tag_text`] for exact opaque-tag round trips.
pub(crate) fn write_escaped_tag_bytes(tag: &[u8], out: &mut impl Write) -> io::Result<()> {
    for byte in tag {
        match byte {
            b']' => out.write_all(b"\\C")?,
            b'\r' => out.write_all(b"\\r")?,
            b'\n' => out.write_all(b"\\n")?,
            b'\\' => out.write_all(b"\\B")?,
            byte => out.write_all(&[*byte])?,
        }
    }
    Ok(())
}
