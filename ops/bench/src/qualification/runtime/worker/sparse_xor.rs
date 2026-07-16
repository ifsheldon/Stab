use std::hint::black_box;
use std::sync::atomic::{Ordering, compiler_fence};

use stab_core::SparseXorVec;

use super::{WorkerError, byte_digest, byte_digest_words};

pub(super) const SPARSE_ROW_BASE_WORK_ITEMS: u64 = 1_997;
pub(super) const SPARSE_ROW_MAX_WORK_ITEMS: u64 = SPARSE_ROW_BASE_WORK_ITEMS * 4_096;
pub(super) const SPARSE_ITEM_BASE_WORK_ITEMS: u64 = 7;
pub(super) const SPARSE_ITEM_MAX_WORK_ITEMS: u64 = SPARSE_ITEM_BASE_WORK_ITEMS * 4_096;

const SPARSE_ROW_COUNT: u64 = 1_000;
const SPARSE_ROW_MARKER: u64 = 1;
const SPARSE_ITEM_MARKER: u64 = 2;
const SPARSE_ITEM_SEQUENCE: [u32; 7] = [2, 5, 9, 5, 3, 6, 10];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SparseXorKind {
    Row,
    Item,
}

impl SparseXorKind {
    const fn workload(self) -> &'static str {
        match self {
            Self::Row => "sparse-xor-row",
            Self::Item => "sparse-xor-item",
        }
    }

    const fn base_work_items(self) -> u64 {
        match self {
            Self::Row => SPARSE_ROW_BASE_WORK_ITEMS,
            Self::Item => SPARSE_ITEM_BASE_WORK_ITEMS,
        }
    }

    const fn max_work_items(self) -> u64 {
        match self {
            Self::Row => SPARSE_ROW_MAX_WORK_ITEMS,
            Self::Item => SPARSE_ITEM_MAX_WORK_ITEMS,
        }
    }

    const fn marker(self) -> u64 {
        match self {
            Self::Row => SPARSE_ROW_MARKER,
            Self::Item => SPARSE_ITEM_MARKER,
        }
    }
}

pub(super) struct SparseXorFixture {
    state: SparseXorState,
    sweeps: u64,
    pub(super) input_bytes: u64,
    pub(super) input_digest: [u64; 4],
    kind: SparseXorKind,
}

enum SparseXorState {
    Row(Vec<SparseXorVec>),
    Item(SparseXorVec),
}

impl SparseXorFixture {
    pub(super) fn prepare(kind: SparseXorKind, work_items: u64) -> Result<Self, WorkerError> {
        let sweeps = complete_sweeps(kind, work_items)?;
        let mut state = match kind {
            SparseXorKind::Row => SparseXorState::Row(sparse_row_table()?),
            SparseXorKind::Item => SparseXorState::Item(SparseXorVec::new()),
        };
        let canonical_input = canonical_workload_input(&state)?;
        let canonical_initial_state = canonical_state_output(&state)?;
        let input_bytes =
            u64::try_from(canonical_input.len()).map_err(|_| WorkerError::InputSizeRange)?;
        let input_digest = byte_digest(&canonical_input);

        execute_sweeps(&mut state, 2);
        if canonical_state_output(&state)? != canonical_initial_state {
            return Err(WorkerError::SparseXorPrimingState(kind.workload()));
        }

        Ok(Self {
            state,
            sweeps,
            input_bytes,
            input_digest,
            kind,
        })
    }

    pub(super) fn execute(&mut self, iterations: u64) {
        for _ in 0..iterations {
            for _ in 0..self.sweeps {
                compiler_fence(Ordering::SeqCst);
                execute_sweep(black_box(&mut self.state));
            }
        }
    }

    pub(super) fn output_digest(
        &self,
        iterations: u64,
        work_items: u64,
    ) -> Result<[u64; 4], WorkerError> {
        let final_state = canonical_state_output(&self.state)?;
        let final_state_digest = byte_digest(&final_state);
        Ok(byte_digest_words(&[
            iterations,
            work_items,
            self.kind.marker(),
            self.kind.base_work_items(),
            self.input_digest[0],
            self.input_digest[1],
            self.input_digest[2],
            self.input_digest[3],
            final_state_digest[0],
            final_state_digest[1],
            final_state_digest[2],
            final_state_digest[3],
        ]))
    }

    #[cfg(test)]
    pub(super) const fn sweeps(&self) -> u64 {
        self.sweeps
    }

    #[cfg(test)]
    pub(super) fn row_state(&self) -> Option<&[SparseXorVec]> {
        match &self.state {
            SparseXorState::Row(table) => Some(table),
            SparseXorState::Item(_) => None,
        }
    }

    #[cfg(test)]
    pub(super) fn item_state(&self) -> Option<&SparseXorVec> {
        match &self.state {
            SparseXorState::Row(_) => None,
            SparseXorState::Item(buffer) => Some(buffer),
        }
    }
}

fn complete_sweeps(kind: SparseXorKind, work_items: u64) -> Result<u64, WorkerError> {
    let maximum = kind.max_work_items();
    if work_items > maximum {
        return Err(WorkerError::SparseXorWorkLimit {
            workload: kind.workload(),
            actual: work_items,
            maximum,
        });
    }
    let base = kind.base_work_items();
    if work_items < base || !work_items.is_multiple_of(base) {
        return Err(WorkerError::SparseXorWorkShape {
            workload: kind.workload(),
            actual: work_items,
            base,
        });
    }
    Ok(work_items / base)
}

fn sparse_row_table() -> Result<Vec<SparseXorVec>, WorkerError> {
    let row_count = usize::try_from(SPARSE_ROW_COUNT)
        .map_err(|_| WorkerError::SparseXorFixtureRange(SPARSE_ROW_COUNT))?;
    let mut table = Vec::new();
    table
        .try_reserve_exact(row_count)
        .map_err(WorkerError::SparseXorFixtureAllocation)?;
    for row in 0..SPARSE_ROW_COUNT {
        let row = u32::try_from(row).map_err(|_| WorkerError::SparseXorFixtureRange(row))?;
        let mut sparse = SparseXorVec::new();
        for item in [row, row + 1, row + 4, row + 8, row + 15] {
            sparse.xor_item(item);
        }
        table.push(sparse);
    }
    Ok(table)
}

fn execute_sweeps(state: &mut SparseXorState, sweeps: u64) {
    for _ in 0..sweeps {
        execute_sweep(state);
    }
}

fn execute_sweep(state: &mut SparseXorState) {
    match state {
        SparseXorState::Row(table) => {
            sparse_row_sweep(table);
        }
        SparseXorState::Item(buffer) => {
            sparse_item_sweep(buffer);
        }
    }
}

fn sparse_row_sweep(table: &mut [SparseXorVec]) {
    for row in 1..table.len() {
        let (prefix, suffix) = table.split_at_mut(row);
        if let (Some(target), Some(source)) = (prefix.last_mut(), suffix.first()) {
            target.xor_assign(source);
        }
    }
    for row in (2..table.len()).rev() {
        let (prefix, suffix) = table.split_at_mut(row);
        if let (Some(target), Some(source)) = (prefix.last_mut(), suffix.first()) {
            target.xor_assign(source);
        }
    }
}

fn sparse_item_sweep(buffer: &mut SparseXorVec) {
    for item in SPARSE_ITEM_SEQUENCE {
        buffer.xor_item(item);
    }
}

fn canonical_workload_input(state: &SparseXorState) -> Result<Vec<u8>, WorkerError> {
    match state {
        SparseXorState::Row(table) => canonical_table(table),
        SparseXorState::Item(_) => canonical_items(&SPARSE_ITEM_SEQUENCE),
    }
}

fn canonical_state_output(state: &SparseXorState) -> Result<Vec<u8>, WorkerError> {
    match state {
        SparseXorState::Row(table) => canonical_table(table),
        SparseXorState::Item(buffer) => canonical_items(buffer.items()),
    }
}

fn canonical_table(table: &[SparseXorVec]) -> Result<Vec<u8>, WorkerError> {
    let item_count = table.iter().try_fold(0_usize, |count, row| {
        count
            .checked_add(row.items().len())
            .ok_or(WorkerError::SparseXorEncodingOverflow)
    })?;
    let capacity = table
        .len()
        .checked_mul(u64::BITS as usize / 8)
        .and_then(|row_lengths| row_lengths.checked_add(u64::BITS as usize / 8))
        .and_then(|header_bytes| {
            item_count
                .checked_mul(u32::BITS as usize / 8)
                .and_then(|item_bytes| header_bytes.checked_add(item_bytes))
        })
        .ok_or(WorkerError::SparseXorEncodingOverflow)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(capacity)
        .map_err(WorkerError::SparseXorEncodingAllocation)?;
    append_len(&mut output, table.len())?;
    for row in table {
        append_len(&mut output, row.items().len())?;
        for item in row.items() {
            output.extend_from_slice(&item.to_le_bytes());
        }
    }
    Ok(output)
}

fn canonical_items(items: &[u32]) -> Result<Vec<u8>, WorkerError> {
    let capacity = items
        .len()
        .checked_mul(u32::BITS as usize / 8)
        .and_then(|item_bytes| item_bytes.checked_add(u64::BITS as usize / 8))
        .ok_or(WorkerError::SparseXorEncodingOverflow)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(capacity)
        .map_err(WorkerError::SparseXorEncodingAllocation)?;
    append_len(&mut output, items.len())?;
    for item in items {
        output.extend_from_slice(&item.to_le_bytes());
    }
    Ok(output)
}

fn append_len(output: &mut Vec<u8>, len: usize) -> Result<(), WorkerError> {
    output.extend_from_slice(
        &u64::try_from(len)
            .map_err(|_| WorkerError::SparseXorEncodingOverflow)?
            .to_le_bytes(),
    );
    Ok(())
}
