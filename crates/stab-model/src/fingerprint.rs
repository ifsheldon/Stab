use arrayvec::ArrayVec;
use sha2::{Digest as _, Sha256};

use crate::{
    Circuit, CircuitItem, DemInstructionKind, DemItem, DemTarget, DetectorErrorModel, ModelDialect,
    Pauli, RepeatNestingLimit, Target,
};

const MODEL_FINGERPRINT_DOMAIN: &[u8] = b"stab:model-fingerprint\0";
const INLINE_TRAVERSAL_FRAMES: usize = RepeatNestingLimit::HARD_MAX + 1;

/// Versioned SHA-256 identity of a circuit or detector error model.
///
/// Schema one starts with the fixed `stab:model-fingerprint\0` domain, a big-endian `u16` schema,
/// and a one-byte dialect discriminator. It then streams a structural encoding that length-frames
/// sequences and UTF-8 strings with big-endian `u128` values, discriminates every item and target
/// variant, and encodes integers in fixed-width big-endian form. Floating-point arguments use
/// their exact `f64` bits, except that signed zero is normalized because `-0.0` and `0.0` have the
/// same model semantics.
///
/// This identity is independent of textual printer precision and does not allocate storage
/// proportional to model volume. Its complete byte contract and frozen vectors are in the
/// [schema-one architecture document].
///
/// It is not a compiled-plan identity.
///
/// [schema-one architecture document]: https://github.com/ifsheldon/Stab/blob/main/docs/architecture/model-fingerprint-schema-v1.md
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ModelFingerprint {
    schema_version: u16,
    dialect: ModelDialect,
    digest: [u8; 32],
}

impl ModelFingerprint {
    pub const SCHEMA_VERSION: u16 = 1;
    pub const ALGORITHM: &'static str = "sha256";

    pub(crate) fn for_circuit(circuit: &Circuit) -> Self {
        let mut encoder = StructuralEncoder::new(ModelDialect::StimCircuit);
        encoder.circuit(circuit);
        encoder.finish()
    }

    pub(crate) fn for_dem(model: &DetectorErrorModel) -> Self {
        let mut encoder = StructuralEncoder::new(ModelDialect::DetectorErrorModel);
        encoder.dem(model);
        encoder.finish()
    }

    pub const fn schema_version(self) -> u16 {
        self.schema_version
    }

    pub const fn dialect(self) -> ModelDialect {
        self.dialect
    }

    pub const fn digest(self) -> [u8; 32] {
        self.digest
    }

    pub fn digest_hex(self) -> String {
        hex::encode(self.digest)
    }
}

struct StructuralEncoder {
    dialect: ModelDialect,
    hasher: Sha256,
}

impl StructuralEncoder {
    fn new(dialect: ModelDialect) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(MODEL_FINGERPRINT_DOMAIN);
        hasher.update(ModelFingerprint::SCHEMA_VERSION.to_be_bytes());
        hasher.update([crate::advanced::model_dialect_fingerprint_discriminator(
            dialect,
        )]);
        Self { dialect, hasher }
    }

    fn finish(self) -> ModelFingerprint {
        ModelFingerprint {
            schema_version: ModelFingerprint::SCHEMA_VERSION,
            dialect: self.dialect,
            digest: self.hasher.finalize().into(),
        }
    }

    fn circuit(&mut self, circuit: &Circuit) {
        self.len(circuit.items().len());
        let mut stack = TraversalStack::new(circuit.items().iter());
        while let Some(item) = stack.next() {
            match item {
                CircuitItem::Instruction(instruction) => {
                    self.byte(1);
                    self.text(instruction.gate().canonical_name());
                    let args = instruction.args();
                    let targets = instruction.targets();
                    if args.is_empty() {
                        let mut counts = [0_u8; 32];
                        let (_, tail) = counts.split_at_mut(16);
                        let target_len = u128::try_from(targets.len()).unwrap_or(u128::MAX);
                        tail.copy_from_slice(&target_len.to_be_bytes());
                        self.hasher.update(counts);
                    } else {
                        self.floats(args);
                        self.len(targets.len());
                    }
                    for target in targets {
                        self.circuit_target(target);
                    }
                    self.optional_bytes(instruction.tag_bytes());
                }
                CircuitItem::RepeatBlock(repeat) => {
                    self.byte(2);
                    self.u64(repeat.repeat_count().get());
                    self.optional_bytes(repeat.tag_bytes());
                    self.len(repeat.body().items().len());
                    stack.push(repeat.body().items().iter());
                }
            }
        }
    }

    #[inline(always)]
    fn circuit_target(&mut self, target: &Target) {
        match target {
            Target::Qubit { id, inverted } => {
                let [b0, b1, b2, b3] = id.get().to_be_bytes();
                self.hasher.update([1, b0, b1, b2, b3, u8::from(*inverted)]);
            }
            Target::MeasurementRecord { offset } => {
                let [b0, b1, b2, b3] = offset.get().to_be_bytes();
                self.hasher
                    .update([2, b0, b1, b2, b3, u8::from(offset.is_negative_zero())]);
            }
            Target::SweepBit { id } => {
                let [b0, b1, b2, b3] = id.to_be_bytes();
                self.hasher.update([3, b0, b1, b2, b3]);
            }
            Target::Pauli {
                pauli,
                id,
                inverted,
            } => {
                let disc = match pauli {
                    Pauli::X => 1,
                    Pauli::Y => 2,
                    Pauli::Z => 3,
                };
                let [b0, b1, b2, b3] = id.get().to_be_bytes();
                self.hasher
                    .update([4, disc, b0, b1, b2, b3, u8::from(*inverted)]);
            }
            Target::Combiner => self.byte(5),
        }
    }

    fn dem(&mut self, model: &DetectorErrorModel) {
        self.len(model.items().len());
        let mut stack = TraversalStack::new(model.items().iter());
        while let Some(item) = stack.next() {
            match item {
                DemItem::Instruction(instruction) => {
                    let kind = match instruction.kind() {
                        DemInstructionKind::Error => 1,
                        DemInstructionKind::Detector => 2,
                        DemInstructionKind::LogicalObservable => 3,
                        DemInstructionKind::ShiftDetectors => 4,
                    };
                    self.hasher.update([1, kind]);
                    self.floats(instruction.args());
                    self.len(instruction.targets().len());
                    for target in instruction.targets() {
                        self.dem_target(target);
                    }
                    self.optional_bytes(instruction.tag_bytes());
                }
                DemItem::RepeatBlock(repeat) => {
                    self.byte(2);
                    self.u64(repeat.repeat_count().get());
                    self.optional_bytes(repeat.tag_bytes());
                    self.len(repeat.body().items().len());
                    stack.push(repeat.body().items().iter());
                }
            }
        }
    }

    #[inline(always)]
    fn dem_target(&mut self, target: &DemTarget) {
        match target {
            DemTarget::RelativeDetector(id) => {
                let [b0, b1, b2, b3, b4, b5, b6, b7] = id.get().to_be_bytes();
                self.hasher.update([1, b0, b1, b2, b3, b4, b5, b6, b7]);
            }
            DemTarget::LogicalObservable(id) => {
                let [b0, b1, b2, b3, b4, b5, b6, b7] = id.get().to_be_bytes();
                self.hasher.update([2, b0, b1, b2, b3, b4, b5, b6, b7]);
            }
            DemTarget::Separator => self.byte(3),
            DemTarget::Numeric(value) => {
                let [b0, b1, b2, b3, b4, b5, b6, b7] = value.to_be_bytes();
                self.hasher.update([4, b0, b1, b2, b3, b4, b5, b6, b7]);
            }
        }
    }

    #[inline]
    fn optional_bytes(&mut self, bytes: Option<&[u8]>) {
        match bytes {
            Some(bytes) => {
                self.byte(1);
                self.len(bytes.len());
                self.hasher.update(bytes);
            }
            None => self.byte(0),
        }
    }

    #[inline(always)]
    fn text(&mut self, text: &str) {
        self.len(text.len());
        self.hasher.update(text.as_bytes());
    }

    #[inline]
    fn floats(&mut self, values: &[f64]) {
        self.len(values.len());
        for value in values {
            let bits = if *value == 0.0 { 0 } else { value.to_bits() };
            self.u64(bits);
        }
    }

    #[inline(always)]
    fn len(&mut self, value: usize) {
        let wide = u128::try_from(value).unwrap_or(u128::MAX);
        self.hasher.update(wide.to_be_bytes());
    }

    #[inline(always)]
    fn byte(&mut self, value: u8) {
        self.hasher.update([value]);
    }

    #[inline(always)]
    fn u64(&mut self, value: u64) {
        self.hasher.update(value.to_be_bytes());
    }
}

struct TraversalStack<T> {
    inline: ArrayVec<T, INLINE_TRAVERSAL_FRAMES>,
    overflow: Vec<T>,
}

impl<T> TraversalStack<T>
where
    T: Iterator,
{
    fn new(root: T) -> Self {
        let mut stack = Self {
            inline: ArrayVec::new(),
            overflow: Vec::new(),
        };
        stack.push(root);
        stack
    }

    fn push(&mut self, frame: T) {
        let frame = if self.overflow.is_empty() {
            match self.inline.try_push(frame) {
                Ok(()) => return,
                Err(error) => error.element(),
            }
        } else {
            frame
        };
        self.overflow.push(frame);
    }

    fn next(&mut self) -> Option<T::Item> {
        loop {
            let item = if let Some(frame) = self.overflow.last_mut() {
                frame.next()
            } else {
                self.inline.last_mut()?.next()
            };
            if item.is_some() {
                return item;
            }
            if self.overflow.is_empty() {
                self.inline.pop();
            } else {
                self.overflow.pop();
            }
        }
    }
}
