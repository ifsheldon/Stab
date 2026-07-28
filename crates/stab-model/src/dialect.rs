/// Canonical syntax dialect for a Stim model.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ModelDialect {
    StimCircuit,
    DetectorErrorModel,
}

impl ModelDialect {
    pub const ALL: [Self; 2] = [Self::StimCircuit, Self::DetectorErrorModel];

    pub fn all() -> impl ExactSizeIterator<Item = Self> {
        Self::ALL.into_iter()
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StimCircuit => "stim-circuit",
            Self::DetectorErrorModel => "detector-error-model",
        }
    }

    pub(crate) const fn fingerprint_discriminator(self) -> u8 {
        match self {
            Self::StimCircuit => 1,
            Self::DetectorErrorModel => 2,
        }
    }
}
