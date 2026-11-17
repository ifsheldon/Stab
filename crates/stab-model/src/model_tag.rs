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

/// A circuit item's optional tag, boxed so an untagged item spends one pointer on it.
pub(crate) type ItemTag = Option<Box<ModelTag>>;

impl ModelTag {
    pub(crate) fn from_string(tag: String) -> ItemTag {
        if tag.is_empty() {
            None
        } else {
            Some(Box::new(Self::Text(tag.into_boxed_str())))
        }
    }

    pub(crate) fn from_bytes(tag: Vec<u8>) -> ItemTag {
        if tag.is_empty() {
            return None;
        }
        match String::from_utf8(tag) {
            Ok(tag) => Some(Box::new(Self::Text(tag.into_boxed_str()))),
            Err(error) => {
                let bytes = error.into_bytes().into_boxed_slice();
                let display = lossy_display(&bytes);
                Some(Box::new(Self::Opaque(Box::new(OpaqueModelTag {
                    bytes,
                    display,
                }))))
            }
        }
    }

    /// Bytes an opaque tag of `bytes` retains beyond the tag charge every tag pays: its display
    /// text and the record holding both; zero for valid UTF-8.
    ///
    /// The display replaces each invalid sequence with U+FFFD, so it can take three bytes per
    /// source byte.
    pub(crate) fn opaque_overhead_bytes_of(bytes: &[u8]) -> usize {
        if std::str::from_utf8(bytes).is_ok() {
            return 0;
        }
        lossy_display_len(bytes).saturating_add(size_of::<OpaqueModelTag>())
    }

    /// [`Self::opaque_overhead_bytes_of`] for this tag.
    pub(crate) fn opaque_overhead_bytes(&self) -> usize {
        match self {
            Self::Text(_) => 0,
            Self::Opaque(tag) => tag
                .display
                .len()
                .saturating_add(size_of::<OpaqueModelTag>()),
        }
    }

    pub(crate) fn from_slice(tag: &[u8]) -> ItemTag {
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

/// Length of [`String::from_utf8_lossy`]'s text for `bytes`, which replaces each invalid chunk
/// with one U+FFFD.
fn lossy_display_len(bytes: &[u8]) -> usize {
    bytes.utf8_chunks().fold(0_usize, |len, chunk| {
        let replacement = if chunk.invalid().is_empty() {
            0
        } else {
            char::REPLACEMENT_CHARACTER.len_utf8()
        };
        len.saturating_add(chunk.valid().len())
            .saturating_add(replacement)
    })
}

/// [`String::from_utf8_lossy`]'s text for `bytes`, built in one exact-size allocation.
fn lossy_display(bytes: &[u8]) -> Box<str> {
    let mut display = String::with_capacity(lossy_display_len(bytes));
    for chunk in bytes.utf8_chunks() {
        display.push_str(chunk.valid());
        if !chunk.invalid().is_empty() {
            display.push(char::REPLACEMENT_CHARACTER);
        }
    }
    display.into_boxed_str()
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

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::{lossy_display, lossy_display_len};

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(4096))]

        /// Opaque tags display exactly as `String::from_utf8_lossy`, built in one exact-size
        /// allocation whose length the parser charges beforehand.
        #[test]
        fn exact_lossy_display_matches_the_standard_library(
            bytes in proptest::collection::vec(
                prop_oneof![any::<u8>(), Just(0xe2_u8), Just(0x82_u8), Just(0xac_u8)],
                0..48,
            )
        ) {
            let expected = String::from_utf8_lossy(&bytes);
            let display = lossy_display(&bytes);
            prop_assert_eq!(&*display, &*expected);
            prop_assert_eq!(lossy_display_len(&bytes), expected.len());
        }
    }
}
