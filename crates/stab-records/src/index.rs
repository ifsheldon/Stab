//! Typed positions inside a record batch, so a shot index and a bit index cannot be swapped.

/// The position of one shot (one record) in a batch.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ShotIndex(pub usize);

/// The position of one bit inside each record of a batch.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BitIndex(pub usize);
