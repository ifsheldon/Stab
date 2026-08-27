use arrayvec::ArrayVec;
use stab_model::{Circuit, CircuitItem, Estimate, RepeatNestingLimit, ResourceEstimate};
use stab_records::{EncodedSizeEstimate, RecordFormat};

const INLINE_TRAVERSAL_FRAMES: usize = RepeatNestingLimit::HARD_MAX + 1;

/// Estimates cheaply knowable resource properties of a sampling request without compiling or
/// executing it.
///
/// Structural item and expanded-operation counts are exact when representable. Output bytes are
/// exact for fixed-width encodings and unknown for sparse encodings whose size depends on sampled
/// values. Execution work, scratch, and resident memory remain unknown until the plan/session
/// architecture can estimate them without guessing.
pub fn estimate_sampling_request(
    circuit: &Circuit,
    shots: usize,
    output_format: RecordFormat,
) -> ResourceEstimate {
    let (input_items, expanded_operations) = operation_counts(circuit);
    let output_bytes = circuit
        .count_measurements()
        .ok()
        .and_then(|measurements| usize::try_from(measurements).ok())
        .map_or(Estimate::Unknown, |measurements| {
            estimate_from_encoded(output_format.estimate_output_bytes(shots, measurements))
        });

    ResourceEstimate::builder()
        .input_items(input_items.map_or(Estimate::Unknown, Estimate::Exact))
        .expanded_operations(expanded_operations.map_or(Estimate::Unknown, Estimate::Exact))
        .folded_traversal(input_items.map_or(Estimate::Unknown, Estimate::Exact))
        .output_bytes(output_bytes)
        .build()
}

fn estimate_from_encoded<T>(estimate: EncodedSizeEstimate<T>) -> Estimate<T> {
    match estimate {
        EncodedSizeEstimate::Exact(value) => Estimate::Exact(value),
        EncodedSizeEstimate::Unknown => Estimate::Unknown,
    }
}

fn operation_counts(circuit: &Circuit) -> (Option<usize>, Option<usize>) {
    let mut structural_items = Some(0usize);
    let mut expanded_operations = Some(0usize);
    let mut stack = TraversalStack::new(circuit.items().iter(), 1);

    while let Some((item, multiplier)) = stack.next() {
        structural_items = structural_items.and_then(|count| count.checked_add(1));
        match item {
            CircuitItem::Instruction(_) => {
                expanded_operations =
                    expanded_operations.and_then(|count| count.checked_add(multiplier));
            }
            CircuitItem::RepeatBlock(repeat) => {
                let body_multiplier = usize::try_from(repeat.repeat_count().get())
                    .ok()
                    .and_then(|repeat_count| multiplier.checked_mul(repeat_count));
                match body_multiplier {
                    Some(body_multiplier) => {
                        stack.push(repeat.body().items().iter(), body_multiplier);
                    }
                    None => {
                        expanded_operations = None;
                        stack.push(repeat.body().items().iter(), 0);
                    }
                }
            }
        }
    }

    (structural_items, expanded_operations)
}

struct TraversalFrame<T> {
    iterator: T,
    multiplier: usize,
}

struct TraversalStack<T> {
    inline: ArrayVec<TraversalFrame<T>, INLINE_TRAVERSAL_FRAMES>,
    overflow: Vec<TraversalFrame<T>>,
}

impl<T> TraversalStack<T> {
    fn new(iterator: T, multiplier: usize) -> Self {
        let mut inline = ArrayVec::new();
        inline.push(TraversalFrame {
            iterator,
            multiplier,
        });
        Self {
            inline,
            overflow: Vec::new(),
        }
    }

    fn push(&mut self, iterator: T, multiplier: usize) {
        let frame = TraversalFrame {
            iterator,
            multiplier,
        };
        if self.overflow.is_empty() && self.inline.len() < self.inline.capacity() {
            self.inline.push(frame);
        } else {
            self.overflow.push(frame);
        }
    }

    fn last_mut(&mut self) -> Option<&mut TraversalFrame<T>> {
        self.overflow.last_mut().or_else(|| self.inline.last_mut())
    }

    fn pop(&mut self) {
        if self.overflow.pop().is_none() {
            self.inline.pop();
        }
    }
}

impl<'a> TraversalStack<std::slice::Iter<'a, CircuitItem>> {
    fn next(&mut self) -> Option<(&'a CircuitItem, usize)> {
        loop {
            let frame = self.last_mut()?;
            if let Some(item) = frame.iterator.next() {
                return Some((item, frame.multiplier));
            }
            self.pop();
        }
    }
}
