use arrayvec::ArrayString;

const INLINE_DEM_TAG_BYTES: usize = 16;

#[derive(Clone, Debug, PartialEq)]
pub(super) enum DemTag {
    Inline(ArrayString<INLINE_DEM_TAG_BYTES>),
    Heap(Box<str>),
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

    pub(super) fn as_str(&self) -> &str {
        match self {
            Self::Inline(tag) => tag.as_str(),
            Self::Heap(tag) => tag,
        }
    }
}
