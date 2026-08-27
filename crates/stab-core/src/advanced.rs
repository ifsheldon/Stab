//! Supported lower-level APIs for explicit control over storage, layouts, backends, and traversal.
//!
//! These APIs are part of Stab's supported pre-1.0 surface, but they expose more implementation
//! detail than the facade root. Prefer the root API unless a caller needs one of these controls.

/// Packed storage values and kernels.
pub mod storage {
    pub use stab_bits::*;
}

/// Lower-level algebra traversal and admitted constructors.
pub mod algebra {
    pub use stab_algebra::advanced::{
        pauli_from_bases_unchecked, pauli_identity_unchecked,
        tableau_from_output_columns_unchecked, tableau_identity_unchecked,
    };
    pub use stab_algebra::{CommutingPauliStringIterator, PauliStringIterator, TableauIterator};
}

/// Result layouts, concrete codecs, and bounded record visitors.
pub mod records {
    pub use crate::result_formats::{
        DemSampleCodecSink, DemSampleEncodedRecords, DetectionCodecSink, DetsLayout,
        DetsResultType, DetsToken, MeasureRecord, MeasureRecordBatch, MeasureRecordBatchWriter,
        MeasureRecordWriter, MeasurementCodecSink, SparseShot, ptb64_record_count,
        read_dets_records, read_measurement_records, read_ptb64_records, read_ptb64_records_all,
        read_records, validate_ptb64_shot_count, write_bit_plane_64_batch,
        write_ptb64_records_checked, write_records,
    };
    pub use crate::result_streaming::{
        for_each_dets_packed_record, for_each_dets_record, for_each_dets_sparse_shot,
        for_each_dets_token_record, for_each_packed_record, for_each_ptb64_record,
        for_each_ptb64_record_all, for_each_record, for_each_sparse_record,
    };
    /// Component-tier record error identity, re-exported so facade consumers can name the typed
    /// sink and stream-reader failure types (for example `MeasurementCodecSink::Error`) without
    /// depending on `stab-records` directly.
    pub use stab_records::{FormatError, FormatErrorCode, RecordResult};
    /// Per-record streaming decode over byte transports, shared by every CLI record reader.
    pub use stab_records::{RecordStreamReadError, RecordStreamReader};
}

/// Bounded and folded model traversal primitives.
pub mod traversal {
    pub use stab_model::advanced::{
        DemBlockSummary, DemRepeatSelection, DemTraversalState, FoldedDemBlock, FoldedDemItem,
        FoldedDemTraversal, FoldedDemVisitor, MAX_DEM_REPEAT_NESTING, shifted_coordinates,
        shifted_detector, shifted_targets,
    };
    pub use stab_model::{
        CircuitFlattenedInstructionIter, CircuitFlattenedInstructionRevIter,
        DemFlattenedInstructionIter,
    };
}
