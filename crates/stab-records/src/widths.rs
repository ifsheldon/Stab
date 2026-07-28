macro_rules! semantic_width {
    ($name:ident, $description:literal) => {
        #[doc = $description]
        #[repr(transparent)]
        #[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(usize);

        impl $name {
            pub const fn new(bits: usize) -> Self {
                Self(bits)
            }

            pub const fn get(self) -> usize {
                self.0
            }

            pub const fn is_zero(self) -> bool {
                self.0 == 0
            }
        }

        impl From<$name> for usize {
            fn from(value: $name) -> Self {
                value.get()
            }
        }
    };
}

semantic_width!(
    MeasurementWidth,
    "The number of measurement bits in each shot."
);
semantic_width!(
    DetectorWidth,
    "The number of detector bits in each detection record."
);
semantic_width!(
    ObservableWidth,
    "The number of logical-observable bits in each detection record."
);
semantic_width!(
    SampledErrorWidth,
    "The number of sampled-error mechanism bits in each DEM sample."
);
semantic_width!(
    CorrectionWidth,
    "The number of correction bits in each decoder prediction."
);
