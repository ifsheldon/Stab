//! Compact deterministic reference samples owned by the execution engine.

use stab_model::{Circuit, RepeatNestingLimit};
use thiserror::Error;

use crate::{SamplingCompileError, SamplingCompiler, SamplingExecutionError};

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ReferenceSampleTreeError {
    #[error(transparent)]
    SamplingCompile(#[from] SamplingCompileError),

    #[error(transparent)]
    SamplingExecution(#[from] SamplingExecutionError),
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ReferenceSampleTree {
    prefix_bits: Vec<bool>,
    suffix_children: Vec<ReferenceSampleTree>,
    repetitions: u64,
    body_size: usize,
    logical_size: usize,
    nesting_depth: usize,
}

impl ReferenceSampleTree {
    /// Maximum child nesting accepted by [`Self::try_new`].
    pub const MAX_NESTING: usize = RepeatNestingLimit::HARD_MAX;

    /// Constructs a tree after admitting its nesting and exact logical size.
    ///
    /// Child trees must already have passed the same admission. Validation checks nesting before
    /// size arithmetic so callers get the first structural rejection deterministically.
    pub fn try_new(
        prefix_bits: Vec<bool>,
        suffix_children: Vec<Self>,
        repetitions: u64,
    ) -> Result<Self, ReferenceSampleTreeError> {
        let nesting_depth = suffix_children
            .iter()
            .map(|child| child.nesting_depth)
            .max()
            .map_or(Ok(0), |depth| {
                depth.checked_add(1).ok_or_else(|| {
                    tree_storage_error("reference sample tree nesting depth overflowed")
                })
            })?;
        if nesting_depth > Self::MAX_NESTING {
            return Err(tree_storage_error(format!(
                "reference sample tree nesting depth {nesting_depth} exceeds limit {}",
                Self::MAX_NESTING
            )));
        }

        let body_size = suffix_children.iter().enumerate().try_fold(
            prefix_bits.len(),
            |size, (child_index, child)| {
                size.checked_add(child.logical_size).ok_or_else(|| {
                    tree_storage_error(format!(
                        "reference sample tree body size overflowed while adding child {child_index}"
                    ))
                })
            },
        )?;
        let logical_size = (body_size as u128)
            .checked_mul(u128::from(repetitions))
            .and_then(|size| usize::try_from(size).ok())
            .ok_or_else(|| {
                tree_storage_error(format!(
                    "reference sample tree logical size overflowed for body size {body_size} and {repetitions} repetitions"
                ))
            })?;

        Ok(Self {
            prefix_bits,
            suffix_children,
            repetitions,
            body_size,
            logical_size,
            nesting_depth,
        })
    }

    pub fn from_circuit_reference_sample(
        circuit: &Circuit,
    ) -> Result<Self, ReferenceSampleTreeError> {
        let sampler = SamplingCompiler::new().compile(circuit)?;
        let mut sweep_record = try_bool_buffer(
            sampler.sweep_bit_count(),
            "reference sample tree sweep record",
        )?;
        sweep_record.resize(sampler.sweep_bit_count(), false);
        let mut prefix_bits = try_bool_buffer(
            sampler.measurement_width().get(),
            "reference sample tree prefix",
        )?;
        sampler.reference_measurement_record_with_sweep_into(&sweep_record, &mut prefix_bits)?;
        Self::try_new(prefix_bits, Vec::new(), 1)
    }

    pub fn prefix_bits(&self) -> &[bool] {
        &self.prefix_bits
    }

    pub fn suffix_children(&self) -> &[Self] {
        &self.suffix_children
    }

    pub const fn repetitions(&self) -> u64 {
        self.repetitions
    }

    pub const fn nesting_depth(&self) -> usize {
        self.nesting_depth
    }

    pub const fn size(&self) -> usize {
        self.logical_size
    }

    pub fn get(&self, index: usize) -> Option<bool> {
        if self.body_size == 0 || index >= self.logical_size {
            return None;
        }

        let mut node = self;
        let mut remaining = index;
        loop {
            if node.body_size == 0 {
                return None;
            }
            remaining %= node.body_size;
            if remaining < node.prefix_bits.len() {
                return node.prefix_bits.get(remaining).copied();
            }
            remaining -= node.prefix_bits.len();

            let mut next = None;
            for child in &node.suffix_children {
                if remaining < child.logical_size {
                    next = Some(child);
                    break;
                }
                remaining -= child.logical_size;
            }
            node = next?;
        }
    }

    /// Materializes all represented bits.
    ///
    /// # Errors
    ///
    /// Returns a typed execution-storage error when the destination or bounded traversal stack
    /// cannot be allocated.
    pub fn decompress(&self) -> Result<Vec<bool>, ReferenceSampleTreeError> {
        let mut out = Vec::new();
        self.decompress_into(&mut out)?;
        Ok(out)
    }

    /// Appends all represented bits after reserving every required destination bit.
    ///
    /// The destination contents are unchanged when admission or allocation fails.
    ///
    /// # Errors
    ///
    /// Returns a typed execution-storage error when the final length overflows or when either
    /// destination or traversal storage cannot be allocated.
    pub fn decompress_into(&self, out: &mut Vec<bool>) -> Result<(), ReferenceSampleTreeError> {
        if self.logical_size == 0 {
            return Ok(());
        }

        let final_len = out.len().checked_add(self.logical_size).ok_or_else(|| {
            tree_storage_error(format!(
                "reference sample tree destination length overflowed for {} existing and {} appended bits",
                out.len(),
                self.logical_size
            ))
        })?;
        out.try_reserve_exact(self.logical_size).map_err(|error| {
            tree_storage_error(format!(
                "reference sample tree destination capacity {}: {error}",
                self.logical_size
            ))
        })?;

        let frame_capacity = self.nesting_depth.checked_add(1).ok_or_else(|| {
            tree_storage_error("reference sample tree traversal frame count overflowed")
        })?;
        let mut stack = Vec::new();
        stack.try_reserve_exact(frame_capacity).map_err(|error| {
            tree_storage_error(format!(
                "reference sample tree traversal capacity {frame_capacity}: {error}"
            ))
        })?;
        stack.push(DecompressionFrame::new(self));

        while let Some(frame) = stack.last_mut() {
            if frame.completed_repetitions == frame.tree.repetitions {
                stack.pop();
                continue;
            }
            if !frame.prefix_written {
                out.extend_from_slice(&frame.tree.prefix_bits);
                frame.prefix_written = true;
                continue;
            }
            if let Some(child) = frame.tree.suffix_children.get(frame.next_child) {
                frame.next_child += 1;
                stack.push(DecompressionFrame::new(child));
                continue;
            }
            frame.completed_repetitions += 1;
            frame.next_child = 0;
            frame.prefix_written = false;
        }

        if out.len() != final_len {
            return Err(ReferenceSampleTreeError::SamplingExecution(
                SamplingExecutionError::InternalInvariant {
                    message: format!(
                        "reference sample tree materialized {} bits instead of admitted length {final_len}",
                        out.len()
                    ),
                },
            ));
        }
        Ok(())
    }

    /// Returns an equivalent tree with empty children removed and unary repetitions folded.
    ///
    /// # Errors
    ///
    /// Returns a typed execution-storage error when simplified storage cannot be allocated.
    pub fn simplified(&self) -> Result<Self, ReferenceSampleTreeError> {
        if self.logical_size == 0 {
            return Ok(Self::default());
        }

        let mut children = Vec::new();
        children
            .try_reserve_exact(self.suffix_children.len())
            .map_err(|error| {
                tree_storage_error(format!(
                    "reference sample tree simplified child capacity {}: {error}",
                    self.suffix_children.len()
                ))
            })?;
        for child in &self.suffix_children {
            let child = child.simplified()?;
            if child.logical_size != 0 {
                children.push(child);
            }
        }

        if self.prefix_bits.is_empty() && children.len() == 1 {
            let child = children.pop().ok_or_else(|| {
                ReferenceSampleTreeError::SamplingExecution(
                    SamplingExecutionError::InternalInvariant {
                        message: "reference sample tree lost its sole simplified child".to_owned(),
                    },
                )
            })?;
            let repetitions = child
                .repetitions
                .checked_mul(self.repetitions)
                .ok_or_else(|| {
                    tree_storage_error(format!(
                        "reference sample tree simplified repetition count overflowed for {} and {}",
                        child.repetitions, self.repetitions
                    ))
                })?;
            return Self::try_new(child.prefix_bits, child.suffix_children, repetitions);
        }
        if self.prefix_bits.is_empty() && children.is_empty() {
            return Ok(Self::default());
        }

        Self::try_new(
            try_clone_bits(&self.prefix_bits, "reference sample tree simplified prefix")?,
            children,
            self.repetitions,
        )
    }

    pub fn stim_string(&self) -> String {
        let mut out = format!("{}*('", self.repetitions);
        for bit in &self.prefix_bits {
            out.push(if *bit { '1' } else { '0' });
        }
        out.push('\'');
        for child in &self.suffix_children {
            out.push('+');
            out.push_str(&child.stim_string());
        }
        out.push(')');
        out
    }
}

struct DecompressionFrame<'a> {
    tree: &'a ReferenceSampleTree,
    completed_repetitions: u64,
    next_child: usize,
    prefix_written: bool,
}

impl<'a> DecompressionFrame<'a> {
    const fn new(tree: &'a ReferenceSampleTree) -> Self {
        Self {
            tree,
            completed_repetitions: 0,
            next_child: 0,
            prefix_written: false,
        }
    }
}

fn try_bool_buffer(
    capacity: usize,
    label: &'static str,
) -> Result<Vec<bool>, ReferenceSampleTreeError> {
    let mut buffer = Vec::new();
    buffer
        .try_reserve_exact(capacity)
        .map_err(|error| tree_storage_error(format!("{label} capacity {capacity}: {error}")))?;
    Ok(buffer)
}

fn try_clone_bits(
    bits: &[bool],
    label: &'static str,
) -> Result<Vec<bool>, ReferenceSampleTreeError> {
    let mut cloned = try_bool_buffer(bits.len(), label)?;
    cloned.extend_from_slice(bits);
    Ok(cloned)
}

fn tree_storage_error(message: impl Into<String>) -> ReferenceSampleTreeError {
    ReferenceSampleTreeError::SamplingExecution(SamplingExecutionError::SessionStorageAllocation {
        message: message.into(),
    })
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        clippy::indexing_slicing,
        clippy::panic,
        reason = "reference-sample-tree tests use compact upstream-style assertions"
    )]

    use stab_model::Circuit;

    use super::ReferenceSampleTree;
    use crate::{ReferenceSampleTreeError, SamplingCompiler, SamplingExecutionError};

    fn tree(
        prefix_bits: Vec<bool>,
        suffix_children: Vec<ReferenceSampleTree>,
        repetitions: u64,
    ) -> ReferenceSampleTree {
        ReferenceSampleTree::try_new(prefix_bits, suffix_children, repetitions)
            .expect("construct admitted reference sample tree")
    }

    #[test]
    fn reference_sample_tree_matches_upstream_structure_and_string_subset() {
        let empty = tree(Vec::new(), Vec::new(), 0);
        assert!(empty.prefix_bits().is_empty());
        assert!(empty.suffix_children().is_empty());
        assert_eq!(empty.repetitions(), 0);
        assert_eq!(empty.nesting_depth(), 0);
        assert_eq!(empty.stim_string(), "0*('')");
        assert_eq!(
            tree(vec![true, true, false, true], Vec::new(), 2).stim_string(),
            "2*('1101')"
        );
        assert_eq!(
            tree(
                vec![true, true, false, true],
                vec![tree(vec![true], Vec::new(), 5)],
                2,
            )
            .stim_string(),
            "2*('1101'+5*('1'))"
        );
    }

    #[test]
    fn reference_sample_tree_simplifies_empty_and_zero_repetition_children() {
        let raw = tree(
            Vec::new(),
            vec![
                tree(Vec::new(), Vec::new(), 1),
                tree(
                    vec![true, false, true],
                    vec![ReferenceSampleTree::default()],
                    0,
                ),
                tree(vec![true, true, true], Vec::new(), 2),
            ],
            3,
        );
        assert_eq!(
            raw.simplified()
                .expect("simplify admitted tree")
                .stim_string(),
            "6*('111')"
        );
    }

    #[test]
    fn reference_sample_tree_decompresses_and_supports_random_access() {
        let tree = tree(
            vec![true, true, false, true],
            vec![tree(vec![true], Vec::new(), 5)],
            2,
        );
        let expected = vec![
            true, true, false, true, true, true, true, true, true, true, true, false, true, true,
            true, true, true, true,
        ];
        assert_eq!(tree.decompress().expect("decompress tree"), expected);
        for (index, bit) in expected.iter().copied().enumerate() {
            assert_eq!(tree.get(index), Some(bit), "index {index}");
        }
        assert_eq!(tree.get(expected.len()), None);

        let large = ReferenceSampleTree::try_new(
            tree.prefix_bits().to_vec(),
            tree.suffix_children().to_vec(),
            1_000_000,
        )
        .expect("construct large admitted tree");
        assert_eq!(large.size(), 9_000_000);
        assert_eq!(large.get(8_999_999), Some(true));
        assert_eq!(large.get(9_000_000), None);
    }

    #[test]
    fn reference_sample_tree_from_circuit_matches_sampling_plan_reference() {
        let circuit = Circuit::from_stim_str(
            "
            M 0
            X 0
            M 0
            ",
        )
        .expect("parse circuit");
        let tree =
            ReferenceSampleTree::from_circuit_reference_sample(&circuit).expect("reference tree");
        let plan = SamplingCompiler::new()
            .compile(&circuit)
            .expect("compile sampling plan");
        assert_eq!(
            tree.decompress().expect("decompress reference tree"),
            plan.try_reference_sample().expect("reference sample")
        );
        assert_eq!(tree.size(), 2);
        assert_eq!(tree.stim_string(), "1*('01')");
    }

    #[test]
    fn reference_sample_tree_construction_preserves_storage_admission() {
        let circuit = Circuit::from_stim_str(
            "
            REPEAT 300000000 {
                M 0
            }
            ",
        )
        .expect("parse compact circuit");
        assert!(matches!(
            ReferenceSampleTree::from_circuit_reference_sample(&circuit),
            Err(ReferenceSampleTreeError::SamplingExecution(
                SamplingExecutionError::SessionStorageLimit { .. }
            ))
        ));
    }

    #[test]
    fn checked_construction_rejects_size_overflow_without_saturation() {
        let maximal_repetitions = u64::try_from(usize::MAX).unwrap_or(u64::MAX);
        let maximal = tree(vec![true], Vec::new(), maximal_repetitions);
        assert_eq!(
            maximal.size(),
            usize::try_from(maximal_repetitions).expect("test target fits repetitions")
        );

        let error = ReferenceSampleTree::try_new(Vec::new(), vec![maximal], 2)
            .expect_err("logical size multiplication must reject overflow");
        match error {
            ReferenceSampleTreeError::SamplingExecution(
                SamplingExecutionError::SessionStorageAllocation { message },
            ) => assert!(
                message.contains("logical size overflowed"),
                "unexpected overflow diagnostic: {message}"
            ),
            other => panic!("unexpected logical-size rejection: {other:?}"),
        }
    }

    #[test]
    fn checked_construction_admits_maximum_depth_then_rejects_nesting_first() {
        let mut deep = tree(vec![true], Vec::new(), 1);
        for _ in 0..ReferenceSampleTree::MAX_NESTING {
            deep = tree(Vec::new(), vec![deep], 1);
        }
        assert_eq!(deep.nesting_depth(), ReferenceSampleTree::MAX_NESTING);
        assert_eq!(deep.get(0), Some(true));
        assert_eq!(
            deep.decompress().expect("decompress maximum-depth tree"),
            vec![true]
        );
        assert_eq!(
            deep.simplified()
                .expect("simplify maximum-depth tree")
                .decompress()
                .expect("decompress simplified tree"),
            vec![true]
        );

        let error = ReferenceSampleTree::try_new(vec![true], vec![deep], u64::MAX)
            .expect_err("first child beyond maximum depth must be rejected");
        match error {
            ReferenceSampleTreeError::SamplingExecution(
                SamplingExecutionError::SessionStorageAllocation { message },
            ) => {
                assert!(
                    message.contains("nesting depth"),
                    "nesting must reject before logical-size overflow: {message}"
                );
                assert!(
                    !message.contains("logical size"),
                    "later size rejection escaped first-rejection ordering: {message}"
                );
            }
            other => panic!("unexpected nesting rejection: {other:?}"),
        }
    }

    #[test]
    fn fallible_decompression_rejects_before_mutating_destination() {
        let maximal_repetitions = u64::try_from(usize::MAX).unwrap_or(u64::MAX);
        let maximal = tree(vec![true], Vec::new(), maximal_repetitions);
        let mut output = vec![false, true, false];
        let original = output.clone();

        let error = maximal
            .decompress_into(&mut output)
            .expect_err("destination length must reject before traversal");
        match error {
            ReferenceSampleTreeError::SamplingExecution(
                SamplingExecutionError::SessionStorageAllocation { message },
            ) => assert!(
                message.contains("destination length overflowed"),
                "unexpected destination rejection: {message}"
            ),
            other => panic!("unexpected decompression rejection: {other:?}"),
        }
        assert_eq!(output, original);
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn fallible_decompression_reports_capacity_failure() {
        let maximal = tree(vec![true], Vec::new(), u64::MAX);
        let error = maximal
            .decompress()
            .expect_err("address-space-sized materialization must fail allocation");
        match error {
            ReferenceSampleTreeError::SamplingExecution(
                SamplingExecutionError::SessionStorageAllocation { message },
            ) => assert!(
                message.contains("destination capacity"),
                "unexpected allocation diagnostic: {message}"
            ),
            other => panic!("unexpected allocation rejection: {other:?}"),
        }
    }
}
