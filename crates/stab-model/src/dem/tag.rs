use std::io::{self, Write};

use arrayvec::ArrayString;

use crate::model_tag::{
    RecoveredTag, lossy_display, write_escaped_tag_bytes, write_escaped_tag_text,
};

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
        Self::from_recovered(RecoveredTag::new(tag))
    }

    /// Builds the tag from bytes validated once by [`RecoveredTag::new`].
    pub(super) fn from_recovered(tag: RecoveredTag) -> Option<Self> {
        match tag {
            RecoveredTag::Text(tag) => Self::from_string(tag),
            RecoveredTag::Opaque(bytes) => {
                let bytes = bytes.into_boxed_slice();
                let display = lossy_display(&bytes);
                Some(Self::Opaque(Box::new(OpaqueDemTag { bytes, display })))
            }
        }
    }

    /// Bytes `tag` will retain as a DEM tag beyond the tag charge every tag pays; see
    /// [`RecoveredTag::opaque_overhead_bytes`].
    pub(super) fn opaque_overhead_bytes_of(tag: &RecoveredTag) -> usize {
        tag.opaque_overhead_bytes(size_of::<OpaqueDemTag>())
    }

    /// [`Self::opaque_overhead_bytes_of`] for this tag.
    pub(super) fn opaque_overhead_bytes(&self) -> usize {
        match self {
            Self::Inline(_) | Self::Heap(_) => 0,
            Self::Opaque(tag) => tag.display.len().saturating_add(size_of::<OpaqueDemTag>()),
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
        write_escaped_tag_text(self.as_str(), out);
    }

    pub(super) fn write_escaped_bytes(&self, out: &mut impl Write) -> io::Result<()> {
        write_escaped_tag_bytes(self.as_bytes(), out)
    }
}
