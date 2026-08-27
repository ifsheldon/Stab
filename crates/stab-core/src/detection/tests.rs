#![allow(
    clippy::unwrap_used,
    reason = "compatibility output tests use exact fixture assertions"
)]

use super::*;
use crate::RecordFormat;

#[test]
fn facade_detection_writers_preserve_text_and_packed_formats() {
    let output = DetectionConversionOutput {
        detector_count: 2,
        observable_count: 2,
        records: vec![
            DetectionEventRecord {
                detectors: vec![true, false],
                observables: vec![false, true],
            },
            DetectionEventRecord {
                detectors: vec![false, true],
                observables: vec![true, false],
            },
        ],
    };

    assert_eq!(
        write_detection_records(
            &output,
            DetectionObservableOutputMode::Append,
            RecordFormat::ZeroOne,
        )
        .unwrap(),
        b"1001\n0110\n"
    );
    assert_eq!(
        write_detection_records(
            &output,
            DetectionObservableOutputMode::Append,
            RecordFormat::Dets,
        )
        .unwrap(),
        b"shot D0 L1\nshot D1 L0\n"
    );
    assert_eq!(
        write_detection_records(
            &output,
            DetectionObservableOutputMode::Prepend,
            RecordFormat::Dets,
        )
        .unwrap(),
        b"shot L1 D0\nshot L0 D1\n"
    );
    assert_eq!(
        write_detection_records(
            &output,
            DetectionObservableOutputMode::Append,
            RecordFormat::Hits,
        )
        .unwrap(),
        b"0,3\n1,2\n"
    );
    assert_eq!(
        write_detection_records(
            &output,
            DetectionObservableOutputMode::Append,
            RecordFormat::B8,
        )
        .unwrap(),
        [0b0000_1001, 0b0000_0110]
    );
    assert_eq!(
        write_observable_records(&output, RecordFormat::B8).unwrap(),
        [0b0000_0010, 0b0000_0001]
    );
}

#[test]
fn facade_detection_writers_preserve_ptb64_routing() {
    let output = DetectionConversionOutput {
        detector_count: 2,
        observable_count: 1,
        records: vec![
            DetectionEventRecord {
                detectors: vec![true, false],
                observables: vec![true],
            };
            64
        ],
    };

    assert_eq!(
        write_ptb64_detection_records(&output, DetectionObservableOutputMode::Append).unwrap(),
        [
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        ]
    );
    assert_eq!(
        write_ptb64_observable_records(&output).unwrap(),
        [0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]
    );
}
