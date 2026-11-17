//! Exact-size item storage of one parsed block.
//!
//! A parsed circuit retains exactly what admission charges: each item slot, each target, each
//! argument, and each tag. Two vectors still grow while a block is parsed, the block's item list
//! and the target list of an instruction that later lines fuse into, and both grow by doubling
//! so a long block or fusion run never walks a reallocation chain. Their spare capacity is
//! charged as transient parse storage before it is reserved and returned once the block (or the
//! fusion run) ends and the vector is shrunk to its length.

use crate::parse_limits::ParseAdmission;
use crate::{ByteSpan, ModelError, ModelResult, Target};

use super::super::{Circuit, CircuitInstruction, CircuitItem};

/// The smallest item capacity a block reserves.
const MIN_ITEM_CAPACITY: usize = 2;

#[derive(Default)]
pub(super) struct BlockItems {
    items: Vec<CircuitItem>,
    /// Transient bytes charged for spare item slots.
    spare_item_bytes: usize,
    /// Transient bytes charged for spare target slots of the last instruction.
    spare_target_bytes: usize,
}

impl BlockItems {
    /// Appends a repeat block or an instruction that does not fuse.
    pub(super) fn push(
        &mut self,
        item: CircuitItem,
        admission: &mut ParseAdmission,
        source_line: usize,
        span: ByteSpan,
    ) -> ModelResult<()> {
        self.close_fusion(admission);
        let slot = size_of::<CircuitItem>();
        if self.items.len() == self.items.capacity() {
            let additional = self.items.capacity().max(MIN_ITEM_CAPACITY);
            // The slot this item fills is retained and covered by its declaration's charge, so
            // only the remaining new slots are spare.
            let spare = (additional - 1)
                .checked_mul(slot)
                .ok_or_else(|| reserve_error("item", additional, "byte count overflow"))?;
            admission.admit_transient(spare, source_line, span)?;
            self.items
                .try_reserve_exact(additional)
                .map_err(|error| reserve_error("item", additional, error))?;
            self.spare_item_bytes += spare;
        } else {
            let used = slot.min(self.spare_item_bytes);
            admission.release_transient(used);
            self.spare_item_bytes -= used;
        }
        self.items.push(item);
        Ok(())
    }

    /// Appends an instruction, fusing it into the previous instruction when Stim formatting
    /// allows it, in which case the fused declaration's item slot, arguments, and tag are
    /// refunded because the previous instruction already holds them.
    pub(super) fn push_instruction(
        &mut self,
        instruction: CircuitInstruction,
        admission: &mut ParseAdmission,
        source_line: usize,
        span: ByteSpan,
    ) -> ModelResult<()> {
        let Some(CircuitItem::Instruction(previous)) = self.items.last_mut() else {
            return self.push(
                CircuitItem::Instruction(instruction),
                admission,
                source_line,
                span,
            );
        };
        if !previous.can_fuse(&instruction) {
            return self.push(
                CircuitItem::Instruction(instruction),
                admission,
                source_line,
                span,
            );
        }
        let additional = instruction.targets.len();
        let len = previous.targets.len();
        if previous.targets.capacity() - len < additional {
            let required = len
                .checked_add(additional)
                .ok_or_else(|| reserve_error("target", additional, "count overflow"))?;
            let capacity = required.max(previous.targets.capacity().saturating_mul(2));
            // Every target is already charged as retained, so only the spare slots of the new
            // heap storage are transient.
            let spare = (capacity - required)
                .checked_mul(size_of::<Target>())
                .ok_or_else(|| reserve_error("target", capacity, "byte count overflow"))?;
            if spare > self.spare_target_bytes {
                admission.admit_transient(spare - self.spare_target_bytes, source_line, span)?;
            } else {
                admission.release_transient(self.spare_target_bytes - spare);
            }
            self.spare_target_bytes = spare;
            previous
                .targets
                .try_reserve_exact(capacity - len)
                .map_err(|error| reserve_error("target", capacity - len, error))?;
        } else {
            let used = additional.saturating_mul(size_of::<Target>());
            let released = used.min(self.spare_target_bytes);
            admission.release_transient(released);
            self.spare_target_bytes -= released;
        }
        previous.targets.extend_from_slice(&instruction.targets);
        admission.refund_fused_declaration(
            instruction.args.len(),
            instruction.tag_bytes().map_or(0, <[u8]>::len),
        );
        Ok(())
    }

    /// Ends the block: shrinks its storage to the retained items and returns their transient
    /// spare capacity.
    pub(super) fn finish(mut self, admission: &mut ParseAdmission) -> Circuit {
        self.close_fusion(admission);
        self.items.shrink_to_fit();
        admission.release_transient(self.spare_item_bytes);
        Circuit { items: self.items }
    }

    /// Shrinks the last instruction's fused targets to their length.
    fn close_fusion(&mut self, admission: &mut ParseAdmission) {
        if self.spare_target_bytes == 0 {
            return;
        }
        if let Some(CircuitItem::Instruction(previous)) = self.items.last_mut() {
            previous.targets.shrink_to_fit();
        }
        admission.release_transient(self.spare_target_bytes);
        self.spare_target_bytes = 0;
    }
}

fn reserve_error(kind: &str, additional: usize, error: impl std::fmt::Display) -> ModelError {
    ModelError::invalid_domain_value(
        "circuit allocation",
        format!("unable to reserve {additional} {kind} slots: {error}"),
    )
}
