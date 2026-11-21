/// A resource quantity together with the strength of the estimate.
///
/// Producers that can only compute a quantity exactly or not at all, such as encoded record
/// sizes, never produce [`Estimate::UpperBound`].
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum Estimate<T> {
    Exact(T),
    UpperBound(T),
    #[default]
    Unknown,
}

impl<T> Estimate<T> {
    /// The exact value or the upper bound, or `None` when the quantity is unknown.
    pub const fn value(&self) -> Option<&T> {
        match self {
            Self::Exact(value) | Self::UpperBound(value) => Some(value),
            Self::Unknown => None,
        }
    }
}
