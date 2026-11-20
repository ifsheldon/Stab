//! Exact text and opaque-byte storage for Stim model tags.

use std::io::{self, Write};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ModelTag {
    Text(Box<str>),
    Opaque(Box<OpaqueTag>),
}

/// The record a tag restored from non-UTF-8 bytes keeps: the exact bytes, which printing writes
/// back, and their lossy display text, which text accessors return.
///
/// Circuit and DEM tags share this one record, so the overhead the byte parsers charge for an
/// opaque tag is defined once for both dialects.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct OpaqueTag {
    bytes: Box<[u8]>,
    display: Box<str>,
}

impl OpaqueTag {
    /// Builds the record for bytes [`RecoveredTag::new`] classified as invalid UTF-8, building
    /// the display in one exact-size allocation.
    pub(crate) fn new(bytes: Vec<u8>) -> Self {
        let bytes = bytes.into_boxed_slice();
        let display = lossy_display(&bytes);
        Self { bytes, display }
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub(crate) fn display(&self) -> &str {
        &self.display
    }

    /// Bytes this tag retains beyond the tag charge every tag pays: its display text and the
    /// record holding both.
    pub(crate) fn overhead_bytes(&self) -> usize {
        self.display.len().saturating_add(size_of::<Self>())
    }
}

/// A circuit item's optional tag, boxed so an untagged item spends one pointer on it.
pub(crate) type ItemTag = Option<Box<ModelTag>>;

/// Recovered tag bytes after their single UTF-8 validation, before an opaque tag's display text
/// is built, so the display can be admitted first.
pub(crate) enum RecoveredTag {
    Text(String),
    Opaque(Vec<u8>),
}

impl RecoveredTag {
    pub(crate) fn new(bytes: Vec<u8>) -> Self {
        match String::from_utf8(bytes) {
            Ok(text) => Self::Text(text),
            Err(error) => Self::Opaque(error.into_bytes()),
        }
    }

    /// Bytes the tag will retain beyond the tag charge every tag pays, which is
    /// [`OpaqueTag::overhead_bytes`] of the record it builds; zero for valid UTF-8.
    ///
    /// The display replaces each invalid sequence with U+FFFD, so it can take three bytes per
    /// source byte.
    pub(crate) fn opaque_overhead_bytes(&self) -> usize {
        match self {
            Self::Text(_) => 0,
            Self::Opaque(bytes) => lossy_display_len(bytes).saturating_add(size_of::<OpaqueTag>()),
        }
    }
}

/// An item's tag slot as the shared model-tree walks see it: present or absent, its exact bytes,
/// what it retains beyond its tag charge, and its replacement by recovered source bytes.
pub(crate) trait TagSlot {
    /// The tag's exact bytes, or `None` for an untagged item.
    fn bytes(&self) -> Option<&[u8]>;

    /// [`OpaqueTag::overhead_bytes`] for an opaque tag; zero otherwise.
    fn opaque_overhead_bytes(&self) -> usize;

    /// Replaces the tag with one built from `tag`.
    fn set_recovered(&mut self, tag: RecoveredTag);
}

impl TagSlot for ItemTag {
    fn bytes(&self) -> Option<&[u8]> {
        self.as_deref().map(ModelTag::as_bytes)
    }

    fn opaque_overhead_bytes(&self) -> usize {
        match self.as_deref() {
            Some(ModelTag::Opaque(tag)) => tag.overhead_bytes(),
            Some(ModelTag::Text(_)) | None => 0,
        }
    }

    fn set_recovered(&mut self, tag: RecoveredTag) {
        *self = ModelTag::from_recovered(tag);
    }
}

impl ModelTag {
    pub(crate) fn from_string(tag: String) -> ItemTag {
        if tag.is_empty() {
            None
        } else {
            Some(Box::new(Self::Text(tag.into_boxed_str())))
        }
    }

    pub(crate) fn from_bytes(tag: Vec<u8>) -> ItemTag {
        Self::from_recovered(RecoveredTag::new(tag))
    }

    /// Builds the tag from bytes validated once by [`RecoveredTag::new`].
    pub(crate) fn from_recovered(tag: RecoveredTag) -> ItemTag {
        match tag {
            RecoveredTag::Text(text) => Self::from_string(text),
            RecoveredTag::Opaque(bytes) => {
                Some(Box::new(Self::Opaque(Box::new(OpaqueTag::new(bytes)))))
            }
        }
    }

    pub(crate) fn from_slice(tag: &[u8]) -> ItemTag {
        Self::from_bytes(tag.to_vec())
    }

    pub(crate) fn as_str(&self) -> &str {
        match self {
            Self::Text(tag) => tag,
            Self::Opaque(tag) => tag.display(),
        }
    }

    pub(crate) fn as_bytes(&self) -> &[u8] {
        match self {
            Self::Text(tag) => tag.as_bytes(),
            Self::Opaque(tag) => tag.bytes(),
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
pub(crate) fn lossy_display_len(bytes: &[u8]) -> usize {
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
pub(crate) fn lossy_display(bytes: &[u8]) -> Box<str> {
    let mut display = String::with_capacity(lossy_display_len(bytes));
    for chunk in bytes.utf8_chunks() {
        display.push_str(chunk.valid());
        if !chunk.invalid().is_empty() {
            display.push(char::REPLACEMENT_CHARACTER);
        }
    }
    display.into_boxed_str()
}

/// Stim's escape for a tag byte: `]` becomes `\C`, CR becomes `\r`, LF becomes `\n`, and `\`
/// becomes `\B`; every other byte is written unchanged.
///
/// This is the single owner of the tag escape table for circuit and DEM tags. Every escaped byte
/// is ASCII, so splitting text after one keeps each run on a character boundary.
const fn tag_escape(byte: u8) -> Option<&'static str> {
    match byte {
        b']' => Some("\\C"),
        b'\r' => Some("\\r"),
        b'\n' => Some("\\n"),
        b'\\' => Some("\\B"),
        _ => None,
    }
}

/// Writes tag text with Stim's tag escaping (see [`tag_escape`]), one push per unescaped run.
///
/// The byte-level twin is [`write_escaped_tag_bytes`].
pub(crate) fn write_escaped_tag_text(tag: &str, out: &mut String) {
    let escaped = |ch: char| u8::try_from(ch).is_ok_and(|byte| tag_escape(byte).is_some());
    for run in tag.split_inclusive(escaped) {
        let split = run
            .len()
            .checked_sub(1)
            .and_then(|last| run.split_at_checked(last))
            .and_then(|(unescaped, last)| {
                Some((unescaped, tag_escape(*last.as_bytes().first()?)?))
            });
        match split {
            Some((unescaped, escape)) => {
                out.push_str(unescaped);
                out.push_str(escape);
            }
            None => out.push_str(run),
        }
    }
}

/// Byte-level twin of [`write_escaped_tag_text`] for exact opaque-tag round trips, writing each
/// unescaped run with one `write_all`.
pub(crate) fn write_escaped_tag_bytes(tag: &[u8], out: &mut impl Write) -> io::Result<()> {
    for run in tag.split_inclusive(|byte| tag_escape(*byte).is_some()) {
        match run
            .split_last()
            .and_then(|(last, unescaped)| Some((unescaped, tag_escape(*last)?)))
        {
            Some((unescaped, escape)) => {
                out.write_all(unescaped)?;
                out.write_all(escape.as_bytes())?;
            }
            None => out.write_all(run)?,
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::{
        OpaqueTag, RecoveredTag, lossy_display, lossy_display_len, write_escaped_tag_bytes,
        write_escaped_tag_text,
    };

    /// Per-byte reference for Stim's tag escaping, kept independent of the run-based writers.
    fn reference_escape(tag: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        for &byte in tag {
            match byte {
                b']' => out.extend_from_slice(b"\\C"),
                b'\r' => out.extend_from_slice(b"\\r"),
                b'\n' => out.extend_from_slice(b"\\n"),
                b'\\' => out.extend_from_slice(b"\\B"),
                byte => out.push(byte),
            }
        }
        out
    }

    fn tag_bytes() -> impl Strategy<Value = Vec<u8>> {
        proptest::collection::vec(
            prop_oneof![
                any::<u8>(),
                Just(b']'),
                Just(b'\r'),
                Just(b'\n'),
                Just(b'\\'),
                Just(0xe2_u8),
                Just(0x82_u8),
                Just(0xac_u8),
            ],
            0..48,
        )
    }

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

        /// Run-based tag escaping writes exactly the per-byte reference, for bytes and for text.
        #[test]
        fn run_escaping_matches_the_per_byte_reference(bytes in tag_bytes()) {
            let expected = reference_escape(&bytes);
            let mut written = Vec::new();
            prop_assert!(write_escaped_tag_bytes(&bytes, &mut written).is_ok());
            prop_assert_eq!(&written, &expected);

            let text = String::from_utf8_lossy(&bytes);
            let mut escaped_text = String::new();
            write_escaped_tag_text(&text, &mut escaped_text);
            prop_assert_eq!(escaped_text.as_bytes(), &reference_escape(text.as_bytes())[..]);
        }

        /// One validation classifies recovered bytes, and the overhead it reports before the
        /// display exists is the display the opaque tag then builds.
        #[test]
        fn recovered_tags_charge_the_display_they_build(bytes in tag_bytes()) {
            let recovered = RecoveredTag::new(bytes.clone());
            match &recovered {
                RecoveredTag::Text(text) => prop_assert_eq!(text.as_bytes(), &bytes[..]),
                RecoveredTag::Opaque(opaque) => {
                    prop_assert!(std::str::from_utf8(&bytes).is_err());
                    prop_assert_eq!(opaque, &bytes);
                }
            }
            let charged = recovered.opaque_overhead_bytes();
            let built = match recovered {
                RecoveredTag::Text(_) => 0,
                RecoveredTag::Opaque(opaque) => {
                    let opaque = OpaqueTag::new(opaque);
                    prop_assert_eq!(opaque.display(), &*String::from_utf8_lossy(&bytes));
                    opaque.overhead_bytes()
                }
            };
            prop_assert_eq!(charged, built);
            if built != 0 {
                prop_assert!(built > size_of::<OpaqueTag>());
            }
        }
    }
}
