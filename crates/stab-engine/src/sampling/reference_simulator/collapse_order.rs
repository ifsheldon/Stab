//! Collapse passes that follow Stim's order for fused X- and Y-basis instructions.

use stab_algebra::PauliBasis;

use super::{InverseTableauSimulator, SimResult, SimulatorWork, WorkModel, invariant, try_resize};
use crate::work_poll::WorkPoll;

impl InverseTableauSimulator {
    /// Collapses each distinct qubit of `targets` in `basis`, in ascending qubit order, failing
    /// on the first target that does not resolve.
    ///
    /// This is the order in which Stim's `collapse_x` and `collapse_y` collapse one
    /// instruction's targets before recording any of them.
    pub(in crate::sampling) fn collapse_ascending(
        &mut self,
        targets: impl Iterator<Item = SimResult<usize>>,
        basis: PauliBasis,
        poll: &mut WorkPoll<'_>,
        work: &mut SimulatorWork,
    ) -> SimResult<()> {
        let mut marks = std::mem::take(&mut self.order_marks);
        let result = self.collapse_marked(&mut marks, targets, basis, poll, work);
        self.order_marks = marks;
        result
    }

    fn collapse_marked(
        &mut self,
        marks: &mut Vec<u64>,
        targets: impl Iterator<Item = SimResult<usize>>,
        basis: PauliBasis,
        poll: &mut WorkPoll<'_>,
        work: &mut SimulatorWork,
    ) -> SimResult<()> {
        try_resize(marks, self.words, "reference collapse order marks")?;
        marks.fill(0);
        for qubit in targets {
            let qubit = qubit?;
            self.x_row_index(qubit)?;
            set_bit(marks, qubit)?;
            self.charge(WorkModel::DISPATCH);
        }
        work.charge(self, poll)?;
        for word_index in 0..marks.len() {
            let mut word = marks.get(word_index).copied().unwrap_or_default();
            while let Some(bit) = word.lowest_one() {
                word &= word - 1;
                let qubit = word_index * u64::BITS as usize + bit as usize;
                self.measure(qubit, basis, poll, work)?;
                work.charge(self, poll)?;
            }
        }
        Ok(())
    }

    /// Collapses the pair products `P_a P_b` of `pairs` in Stim's `MXX` and `MYY` order, failing
    /// on the first pair that does not resolve.
    ///
    /// Stim splits the pairs into maximal runs of disjoint pairs and collapses each run's
    /// pairs in ascending order of their first qubit.
    pub(in crate::sampling) fn collapse_pair_segments(
        &mut self,
        pairs: impl Iterator<Item = SimResult<(usize, usize)>>,
        basis: PauliBasis,
        poll: &mut WorkPoll<'_>,
        work: &mut SimulatorWork,
    ) -> SimResult<()> {
        let mut firsts = std::mem::take(&mut self.order_marks);
        let mut used = std::mem::take(&mut self.order_used);
        let mut partners = std::mem::take(&mut self.order_partners);
        let result = self.collapse_marked_pairs(
            (&mut firsts, &mut used, &mut partners),
            pairs,
            basis,
            poll,
            work,
        );
        self.order_marks = firsts;
        self.order_used = used;
        self.order_partners = partners;
        result
    }

    fn collapse_marked_pairs(
        &mut self,
        (firsts, used, partners): (&mut Vec<u64>, &mut Vec<u64>, &mut Vec<usize>),
        pairs: impl Iterator<Item = SimResult<(usize, usize)>>,
        basis: PauliBasis,
        poll: &mut WorkPoll<'_>,
        work: &mut SimulatorWork,
    ) -> SimResult<()> {
        try_resize(firsts, self.words, "reference pair collapse order marks")?;
        try_resize(used, self.words, "reference pair collapse segment marks")?;
        try_resize(
            partners,
            self.qubit_count,
            "reference pair collapse partners",
        )?;
        firsts.fill(0);
        used.fill(0);
        for pair in pairs {
            let (first, second) = pair?;
            self.x_row_index(first)?;
            self.x_row_index(second)?;
            if test_bit(used, first)? || test_bit(used, second)? {
                self.collapse_pair_segment(firsts, partners, basis, poll, work)?;
                used.fill(0);
            }
            set_bit(used, first)?;
            set_bit(used, second)?;
            set_bit(firsts, first)?;
            *partners
                .get_mut(first)
                .ok_or(invariant("pair collapse partner is outside the tableau"))? = second;
            self.charge(WorkModel::DISPATCH);
        }
        self.collapse_pair_segment(firsts, partners, basis, poll, work)
    }

    /// Collapses and clears the marked pairs of one segment in ascending first-qubit order.
    fn collapse_pair_segment(
        &mut self,
        firsts: &mut [u64],
        partners: &[usize],
        basis: PauliBasis,
        poll: &mut WorkPoll<'_>,
        work: &mut SimulatorWork,
    ) -> SimResult<()> {
        self.charge(self.model.row_copy());
        work.charge(self, poll)?;
        for (word_index, marked) in firsts.iter_mut().enumerate() {
            let mut word = std::mem::take(marked);
            while let Some(bit) = word.lowest_one() {
                word &= word - 1;
                let first = word_index * u64::BITS as usize + bit as usize;
                let second = partners
                    .get(first)
                    .copied()
                    .ok_or(invariant("pair collapse partner is outside the tableau"))?;
                // A pair on one qubit measures the identity, which collapses nothing.
                if first != second {
                    self.measure_product(&[(first, basis), (second, basis)], poll, work)?;
                }
                work.charge(self, poll)?;
            }
        }
        Ok(())
    }
}

fn set_bit(words: &mut [u64], bit: usize) -> SimResult<()> {
    let word = words
        .get_mut(bit / u64::BITS as usize)
        .ok_or(invariant("collapse order bit is outside its bitset"))?;
    *word |= 1 << (bit % u64::BITS as usize);
    Ok(())
}

fn test_bit(words: &[u64], bit: usize) -> SimResult<bool> {
    words
        .get(bit / u64::BITS as usize)
        .map(|word| word & (1 << (bit % u64::BITS as usize)) != 0)
        .ok_or(invariant("collapse order bit is outside its bitset"))
}
