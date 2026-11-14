use std::assert_matches;
use std::cell::RefCell;
use std::io::{self, Write};
use std::path::Path;
use std::rc::Rc;

use stab_records::{DemSampleBatchView, DetectionBatchView, PackedShotBatch, RecordFormat};

use super::{DemSampleBatchEncoder, DetectionBatchEncoder, DetectionObservableOutputMode};
use crate::{
    CliError,
    io_plan::OutputFile,
    streaming::{FileOutputSink, OutputSink},
};

#[derive(Debug, Default)]
struct ProbeLog {
    bytes: Vec<u8>,
    writes: usize,
    flushes: usize,
}

/// A file destination that records its calls and can fail writes after a byte budget or at flush.
struct Probe {
    log: Rc<RefCell<ProbeLog>>,
    write_budget: usize,
    fail_flush: bool,
}

impl Write for Probe {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let mut log = self.log.borrow_mut();
        if log.bytes.len() + buffer.len() > self.write_budget {
            return Err(io::Error::other("injected file write failure"));
        }
        log.writes += 1;
        log.bytes.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.log.borrow_mut().flushes += 1;
        if self.fail_flush {
            return Err(io::Error::other("injected file flush failure"));
        }
        Ok(())
    }
}

fn probe_file(
    name: &str,
    write_budget: usize,
    fail_flush: bool,
) -> (FileOutputSink, Rc<RefCell<ProbeLog>>) {
    let log = Rc::new(RefCell::new(ProbeLog::default()));
    let probe = Probe {
        log: Rc::clone(&log),
        write_budget,
        fail_flush,
    };
    let file = OutputFile::probe(Path::new(name), Box::new(probe));
    (FileOutputSink::from_output(file), log)
}

fn detection_batch<'a>(
    detectors: &'a PackedShotBatch,
    observables: &'a PackedShotBatch,
) -> DetectionBatchView<'a> {
    DetectionBatchView::try_new(detectors.view(), observables.view()).expect("detection batch")
}

#[test]
fn prepended_dets_observables_keep_namespace_order_across_batches() {
    let mut encoder = DetectionBatchEncoder::try_new(
        2,
        1,
        DetectionObservableOutputMode::Prepend,
        RecordFormat::Dets,
        None,
    )
    .expect("DETS routing");
    let mut stdout = Vec::new();
    let mut primary = OutputSink::Stdout(&mut stdout);
    for (detectors, observable) in [(vec![true, false], true), (vec![false, true], false)] {
        let detectors = PackedShotBatch::from_records(&[detectors], 2).expect("detectors");
        let observables = PackedShotBatch::from_records(&[vec![observable]], 1).expect("obs");
        encoder
            .write_batch(
                detection_batch(&detectors, &observables),
                &mut primary,
                None,
            )
            .expect("batch");
    }
    encoder.finish(&mut primary, None).expect("finish");
    assert_eq!(stdout, b"shot L0 D0\nshot D1\n");
}

#[test]
fn sampled_errors_use_the_measurement_namespace_and_reject_partial_ptb64_groups() {
    let detectors = PackedShotBatch::from_records(&[vec![true]], 1).expect("detectors");
    let observables = PackedShotBatch::zeros(1, 0).expect("observables");
    let errors = PackedShotBatch::from_records(&[vec![true, false, true]], 3).expect("errors");
    let batch = DemSampleBatchView::try_new(
        detection_batch(&detectors, &observables),
        Some(errors.view().into()),
    )
    .expect("DEM batch");

    let mut encoder = DemSampleBatchEncoder::try_new(
        1,
        0,
        3,
        DetectionObservableOutputMode::DetectorsOnly,
        RecordFormat::Hits,
        None,
        Some(RecordFormat::Dets),
    )
    .expect("DEM routing");
    let mut stdout = Vec::new();
    let mut primary = OutputSink::Stdout(&mut stdout);
    let (mut error_file, log) = probe_file("errors.dets", usize::MAX, false);
    encoder
        .write_batch(batch, &mut primary, None, Some(&mut error_file))
        .expect("batch");
    encoder
        .finish(&mut primary, None, Some(&mut error_file))
        .expect("finish");
    assert_eq!(log.borrow().bytes, b"shot M0 M2\n");

    let mut encoder = DemSampleBatchEncoder::try_new(
        1,
        0,
        3,
        DetectionObservableOutputMode::DetectorsOnly,
        RecordFormat::B8,
        None,
        Some(RecordFormat::Ptb64),
    )
    .expect("PTB64 routing");
    let (mut error_file, _) = probe_file("errors.ptb64", usize::MAX, false);
    encoder
        .write_batch(batch, &mut primary, None, Some(&mut error_file))
        .expect("trailing record");
    let error = encoder
        .finish(&mut primary, None, Some(&mut error_file))
        .expect_err("partial PTB64 group");
    drop(primary);
    assert_eq!(stdout, b"0\n\x01");
    assert_matches!(error, CliError::IncompletePtb64OutputGroup { count: 1 });
}

#[test]
fn many_file_records_reach_the_file_in_buffered_chunks() {
    let shots = 64;
    let batches = 2_000;
    let detectors = PackedShotBatch::from_records(
        &(0..shots)
            .map(|shot| (0..40).map(|bit| (shot + bit) % 3 == 0).collect())
            .collect::<Vec<_>>(),
        40,
    )
    .expect("detectors");
    let observables = PackedShotBatch::zeros(shots, 2).expect("observables");
    for format in [RecordFormat::ZeroOne, RecordFormat::Hits, RecordFormat::B8] {
        let mut encoder = DetectionBatchEncoder::try_new(
            40,
            2,
            DetectionObservableOutputMode::DetectorsOnly,
            format,
            Some(RecordFormat::Dets),
        )
        .expect("routing");
        let (primary_file, primary_log) = probe_file("out", usize::MAX, false);
        let mut primary: OutputSink<'_, Vec<u8>> = OutputSink::File(primary_file);
        let (mut observable_file, observable_log) = probe_file("obs", usize::MAX, false);
        for _ in 0..batches {
            encoder
                .write_batch(
                    detection_batch(&detectors, &observables),
                    &mut primary,
                    Some(&mut observable_file),
                )
                .expect("batch");
        }
        encoder
            .finish(&mut primary, Some(&mut observable_file))
            .expect("finish");
        for log in [primary_log, observable_log] {
            let log = log.borrow();
            // A buffer spill can leave up to one batch's bytes for the next chunk, so each
            // write carries at least half a buffer except the last.
            let half_buffers = log.bytes.len() / (crate::io_plan::OUTPUT_BUFFER_BYTES / 2);
            assert!(
                log.writes <= half_buffers + 1,
                "{format:?}: {} writes for {} bytes of {} records",
                log.writes,
                log.bytes.len(),
                shots * batches
            );
            assert_eq!(log.flushes, 1);
        }
    }
}

#[test]
fn buffered_file_failures_surface_mid_stream_and_at_flush() {
    let detectors =
        PackedShotBatch::from_records(&vec![vec![true; 300]; 64], 300).expect("detectors");
    let observables = PackedShotBatch::zeros(64, 0).expect("observables");
    let new_encoder = || {
        DetectionBatchEncoder::try_new(
            300,
            0,
            DetectionObservableOutputMode::DetectorsOnly,
            RecordFormat::ZeroOne,
            None,
        )
        .expect("routing")
    };

    let mut encoder = new_encoder();
    let (file, _) = probe_file("mid.01", 100_000, false);
    let mut primary: OutputSink<'_, Vec<u8>> = OutputSink::File(file);
    let mut failure = None;
    for _ in 0..64 {
        if let Err(error) = encoder.write_batch(
            detection_batch(&detectors, &observables),
            &mut primary,
            None,
        ) {
            failure = Some(error);
            break;
        }
    }
    assert_matches!(
        failure,
        Some(CliError::WritePath { ref path, ref source })
            if path == Path::new("mid.01") && source.to_string() == "injected file write failure"
    );

    let mut encoder = new_encoder();
    let (file, log) = probe_file("flush.01", usize::MAX, true);
    let mut primary: OutputSink<'_, Vec<u8>> = OutputSink::File(file);
    encoder
        .write_batch(
            detection_batch(&detectors, &observables),
            &mut primary,
            None,
        )
        .expect("small batch stays buffered");
    assert_eq!(log.borrow().writes, 0);
    let error = encoder
        .finish(&mut primary, None)
        .expect_err("flush failure");
    assert_matches!(
        error,
        CliError::WritePath { ref path, ref source }
            if path == Path::new("flush.01") && source.to_string() == "injected file flush failure"
    );
    assert_eq!(log.borrow().bytes.len(), 64 * 301);
}
