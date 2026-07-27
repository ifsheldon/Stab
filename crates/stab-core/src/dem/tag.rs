use std::io::{self, Write};

use arrayvec::ArrayString;

const INLINE_DEM_TAG_BYTES: usize = 16;

#[derive(Clone, Debug, PartialEq)]
pub(super) enum DemTag {
    Inline(ArrayString<INLINE_DEM_TAG_BYTES>),
    Heap(Box<str>),
    Opaque(Box<OpaqueDemTag>),
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct OpaqueDemTag {
    bytes: Box<[u8]>,
    display: Box<str>,
}

impl DemTag {
    pub(super) fn from_text(tag: &str) -> Option<Self> {
        if tag.is_empty() {
            return None;
        }
        Some(match ArrayString::from(tag) {
            Ok(tag) => Self::Inline(tag),
            Err(_) => Self::Heap(tag.into()),
        })
    }

    pub(super) fn from_string(tag: String) -> Option<Self> {
        if tag.is_empty() {
            return None;
        }
        Some(match ArrayString::from(tag.as_str()) {
            Ok(inline) => Self::Inline(inline),
            Err(_) => Self::Heap(tag.into_boxed_str()),
        })
    }

    pub(super) fn from_bytes(tag: Vec<u8>) -> Option<Self> {
        if tag.is_empty() {
            return None;
        }
        match String::from_utf8(tag) {
            Ok(tag) => Self::from_string(tag),
            Err(error) => {
                let bytes = error.into_bytes().into_boxed_slice();
                let display = String::from_utf8_lossy(&bytes)
                    .into_owned()
                    .into_boxed_str();
                Some(Self::Opaque(Box::new(OpaqueDemTag { bytes, display })))
            }
        }
    }

    pub(super) fn from_slice(tag: &[u8]) -> Option<Self> {
        Self::from_bytes(tag.to_vec())
    }

    pub(super) fn as_str(&self) -> &str {
        match self {
            Self::Inline(tag) => tag.as_str(),
            Self::Heap(tag) => tag,
            Self::Opaque(tag) => &tag.display,
        }
    }

    pub(super) fn as_bytes(&self) -> &[u8] {
        match self {
            Self::Inline(tag) => tag.as_bytes(),
            Self::Heap(tag) => tag.as_bytes(),
            Self::Opaque(tag) => &tag.bytes,
        }
    }

    pub(super) fn write_escaped_text(&self, out: &mut String) {
        for ch in self.as_str().chars() {
            match ch {
                ']' => out.push_str("\\C"),
                '\r' => out.push_str("\\r"),
                '\n' => out.push_str("\\n"),
                '\\' => out.push_str("\\B"),
                _ => out.push(ch),
            }
        }
    }

    pub(super) fn write_escaped_bytes(&self, out: &mut impl Write) -> io::Result<()> {
        for byte in self.as_bytes() {
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
}
