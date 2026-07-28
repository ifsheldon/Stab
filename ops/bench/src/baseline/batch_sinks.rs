use std::io::{self, Write};

use stab_core::{
    DemSampleBatchView, DemSampleSink, DetectionBatchView, DetectionSink, FormatError,
    PackedShotBatchView,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct OutputWitness {
    pub(super) bytes: usize,
    pub(super) digest: u64,
}

impl OutputWitness {
    pub(super) const fn new(bytes: usize, digest: u64) -> Self {
        Self { bytes, digest }
    }

    pub(super) fn from_bytes(bytes: &[u8]) -> Self {
        let mut digest = 0xcbf2_9ce4_8422_2325_u64;
        for byte in bytes {
            digest ^= u64::from(*byte);
            digest = digest.wrapping_mul(0x0000_0100_0000_01b3);
        }
        Self {
            bytes: bytes.len(),
            digest,
        }
    }
}

#[derive(Debug)]
pub(super) struct ByteDigestWriter {
    bytes: usize,
    digest: u64,
}

impl Default for ByteDigestWriter {
    fn default() -> Self {
        Self {
            bytes: 0,
            digest: 0xcbf2_9ce4_8422_2325,
        }
    }
}

impl ByteDigestWriter {
    pub(super) const fn witness(&self) -> OutputWitness {
        OutputWitness {
            bytes: self.bytes,
            digest: self.digest,
        }
    }
}

impl Write for ByteDigestWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.bytes = self
            .bytes
            .checked_add(buf.len())
            .ok_or_else(|| io::Error::other("benchmark output byte count overflowed"))?;
        for byte in buf {
            self.digest ^= u64::from(*byte);
            self.digest = self.digest.wrapping_mul(0x0000_0100_0000_01b3);
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Default)]
pub(super) struct DetectionDigestSink {
    digest: u64,
    shots: u64,
}

impl DetectionDigestSink {
    pub(super) fn reset(&mut self) {
        self.digest = 0;
        self.shots = 0;
    }

    pub(super) const fn witness(&self) -> (u64, u64) {
        (self.digest, self.shots)
    }

    fn observe(&mut self, batch: DetectionBatchView<'_>) -> Result<(), FormatError> {
        self.digest = mix(self.digest, batch.detector_width().get() as u64);
        self.digest = mix(self.digest, batch.observable_width().get() as u64);
        observe_records(batch.detectors(), &mut self.digest)?;
        observe_records(batch.observables(), &mut self.digest)?;
        self.shots = self
            .shots
            .checked_add(batch.shot_count() as u64)
            .ok_or_else(|| FormatError::invalid_data("benchmark shot count overflowed"))?;
        Ok(())
    }
}

impl DetectionSink for DetectionDigestSink {
    type Error = FormatError;

    fn write_batch(&mut self, batch: DetectionBatchView<'_>) -> Result<(), Self::Error> {
        self.observe(batch)
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[derive(Default)]
pub(super) struct DemDigestSink {
    detection: DetectionDigestSink,
    sampled_error_digest: u64,
}

impl DemDigestSink {
    pub(super) fn reset(&mut self) {
        self.detection.reset();
        self.sampled_error_digest = 0;
    }

    pub(super) const fn witness(&self) -> (u64, u64, u64) {
        let (detection_digest, shots) = self.detection.witness();
        (detection_digest, self.sampled_error_digest, shots)
    }
}

impl DemSampleSink for DemDigestSink {
    type Error = FormatError;

    fn write_batch(&mut self, batch: DemSampleBatchView<'_>) -> Result<(), Self::Error> {
        self.detection.observe(batch.detection())?;
        if let Some(errors) = batch.sampled_errors() {
            self.sampled_error_digest =
                mix(self.sampled_error_digest, errors.bits_per_shot() as u64);
            observe_records(errors, &mut self.sampled_error_digest)?;
        }
        Ok(())
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

fn observe_records(records: PackedShotBatchView<'_>, digest: &mut u64) -> Result<(), FormatError> {
    for shot_index in 0..records.shot_count() {
        let shot = records.shot(shot_index)?;
        *digest = mix(*digest, shot.len() as u64);
        for word in shot.words() {
            *digest = mix(*digest, *word);
        }
    }
    Ok(())
}

#[inline]
const fn mix(digest: u64, value: u64) -> u64 {
    digest.rotate_left(11) ^ value.wrapping_mul(0x9E37_79B1_85EB_CA87)
}
