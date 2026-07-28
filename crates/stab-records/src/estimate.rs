/// Encoded byte-size estimate for a fixed record request.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum EncodedSizeEstimate<T> {
    Exact(T),
    #[default]
    Unknown,
}

impl<T> EncodedSizeEstimate<T> {
    pub const fn value(&self) -> Option<&T> {
        match self {
            Self::Exact(value) => Some(value),
            Self::Unknown => None,
        }
    }
}
