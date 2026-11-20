//! Walks shared by the circuit and DEM trees: clone, drop, equality, admitted bytes, and the
//! restore of tags recovered from non-UTF-8 source bytes.
//!
//! Both models are a list of items, each an instruction or a repeat block owning a nested body.
//! Every walk here keeps its own explicit stack, so a hostile nesting depth costs heap, never
//! call stack. Keeping one copy for both dialects is what stops their budget charges and tag
//! handling from drifting apart.

use std::slice;

use crate::model_bytes::{RecoveredTags, locate_recovered_tag};
use crate::model_tag::{RecoveredTag, TagSlot};
use crate::parse_limits::{ParseAdmission, model_declaration_admitted_bytes};
use crate::{ModelDialect, ModelError, ModelResult};

/// One borrowed item of a model tree.
pub(crate) enum Node<I, R> {
    Instruction(I),
    Repeat(R),
}

/// The shape a model's item list shares with the other dialect's.
pub(crate) trait ModelTree {
    /// The dialect whose declaration charges apply.
    const DIALECT: ModelDialect;
    /// Names the byte parser in internal errors.
    const BYTE_PARSER: &'static str;

    type Item;
    type Instruction: Clone;
    type Repeat;
    type Tag: TagSlot;

    fn node(item: &Self::Item) -> Node<&Self::Instruction, &Self::Repeat>;
    fn node_mut(item: &mut Self::Item) -> Node<&mut Self::Instruction, &mut Self::Repeat>;
    fn body(repeat: &Self::Repeat) -> &[Self::Item];
    fn body_mut(repeat: &mut Self::Repeat) -> &mut Vec<Self::Item>;
    fn tag(item: &Self::Item) -> &Self::Tag;
    fn tag_mut(item: &mut Self::Item) -> &mut Self::Tag;

    /// Target and argument counts of `instruction`, as its declaration charge counts them.
    fn instruction_counts(instruction: &Self::Instruction) -> (usize, usize);

    /// Compares two repeat blocks' counts and tags, leaving their bodies to the walk.
    fn repeat_headers_equal(left: &Self::Repeat, right: &Self::Repeat) -> bool;

    fn instruction_item(instruction: Self::Instruction) -> Self::Item;

    /// Builds a repeat item with `repeat`'s count and tag around the already cloned `body`.
    fn repeat_item_with_body(repeat: &Self::Repeat, body: Vec<Self::Item>) -> Self::Item;
}

/// Visits every item in source order, each repeat block before its body.
fn walk<T: ModelTree>(items: &[T::Item], mut visit: impl FnMut(&T::Item)) {
    let mut stack = vec![items.iter()];
    while let Some(items) = stack.last_mut() {
        let Some(item) = items.next() else {
            stack.pop();
            continue;
        };
        visit(item);
        if let Node::Repeat(repeat) = T::node(item) {
            stack.push(T::body(repeat).iter());
        }
    }
}

/// Conservative retained bytes of `items` under parse admission: every declaration's charge plus
/// what each opaque tag retains beyond it.
pub(crate) fn admitted_bytes<T: ModelTree>(items: &[T::Item]) -> u64 {
    let mut total = 0_u64;
    walk::<T>(items, |item| {
        let (targets, args) = match T::node(item) {
            Node::Instruction(instruction) => T::instruction_counts(instruction),
            Node::Repeat(_) => (0, 0),
        };
        let tag = T::tag(item);
        let declaration = model_declaration_admitted_bytes(
            T::DIALECT,
            targets,
            args,
            tag.bytes().map_or(0, <[u8]>::len),
        );
        total = total
            .saturating_add(declaration)
            .saturating_add(tag.opaque_overhead_bytes() as u64);
    });
    total
}

fn non_empty_tag_count<T: ModelTree>(items: &[T::Item]) -> usize {
    let mut count = 0_usize;
    walk::<T>(items, |item| {
        count += usize::from(T::tag(item).bytes().is_some())
    });
    count
}

/// Replaces the sanitized tags the parser produced with the exact recovered bytes, in source
/// order, charging what an opaque tag retains beyond its parsed text before it is built.
pub(crate) fn restore_byte_tags<T: ModelTree>(
    items: &mut [T::Item],
    tags: RecoveredTags,
    admission: &mut ParseAdmission,
    input: &[u8],
) -> ModelResult<()> {
    let expected = non_empty_tag_count::<T>(items);
    if expected != tags.len() {
        return Err(ModelError::invalid_domain_value(
            T::BYTE_PARSER,
            format!(
                "metadata scanner found {} tags but parser produced {expected}",
                tags.len()
            ),
        ));
    }
    let mut tags = tags.into_owned_tags().enumerate();
    let mut pending = vec![items.iter_mut()];
    while let Some(items) = pending.last_mut() {
        let Some(item) = items.next() else {
            pending.pop();
            continue;
        };
        let tag = T::tag_mut(item);
        if tag.bytes().is_some() {
            let Some((index, bytes)) = tags.next() else {
                return Err(ModelError::invalid_domain_value(
                    T::BYTE_PARSER,
                    "metadata scanner ran out of tags",
                ));
            };
            let recovered = RecoveredTag::new(bytes);
            admission.admit_opaque_tag_overhead(recovered.opaque_overhead_bytes(), || {
                locate_recovered_tag(input, index)
            })?;
            tag.set_recovered(recovered);
        }
        if let Node::Repeat(repeat) = T::node_mut(item) {
            pending.push(T::body_mut(repeat).iter_mut());
        }
    }
    Ok(())
}

/// Compares two trees item by item, with `instructions_equal` deciding instructions and
/// [`ModelTree::repeat_headers_equal`] deciding repeat blocks before their bodies.
pub(crate) fn trees_equal<T: ModelTree>(
    left: &[T::Item],
    right: &[T::Item],
    instructions_equal: impl Fn(&T::Instruction, &T::Instruction) -> bool,
) -> bool {
    let mut pending = vec![(left, right)];
    while let Some((left_items, right_items)) = pending.pop() {
        if left_items.len() != right_items.len() {
            return false;
        }
        for (left_item, right_item) in left_items.iter().zip(right_items) {
            match (T::node(left_item), T::node(right_item)) {
                (Node::Instruction(left), Node::Instruction(right))
                    if instructions_equal(left, right) => {}
                (Node::Repeat(left), Node::Repeat(right))
                    if T::repeat_headers_equal(left, right) =>
                {
                    pending.push((T::body(left), T::body(right)));
                }
                _ => return false,
            }
        }
    }
    true
}

struct CloneFrame<'a, T: ModelTree> {
    source: slice::Iter<'a, T::Item>,
    output: Vec<T::Item>,
}

impl<'a, T: ModelTree> CloneFrame<'a, T> {
    fn new(source: &'a [T::Item]) -> Self {
        Self {
            source: source.iter(),
            output: Vec::with_capacity(source.len()),
        }
    }
}

/// Deep-clones `items`, with each block's output sized exactly to its source.
pub(crate) fn clone_items<T: ModelTree>(items: &[T::Item]) -> Vec<T::Item> {
    let mut root = CloneFrame::<T>::new(items);
    // Each open repeat body, with the repeat block that owns it.
    let mut nested: Vec<(&T::Repeat, CloneFrame<'_, T>)> = Vec::new();
    loop {
        let frame = match nested.last_mut() {
            Some((_, frame)) => frame,
            None => &mut root,
        };
        if let Some(item) = frame.source.next() {
            match T::node(item) {
                Node::Instruction(instruction) => {
                    frame.output.push(T::instruction_item(instruction.clone()));
                }
                Node::Repeat(repeat) => nested.push((repeat, CloneFrame::new(T::body(repeat)))),
            }
            continue;
        }
        let Some((repeat, completed)) = nested.pop() else {
            return root.output;
        };
        let cloned = T::repeat_item_with_body(repeat, completed.output);
        match nested.last_mut() {
            Some((_, parent)) => parent.output.push(cloned),
            None => root.output.push(cloned),
        }
    }
}

const SHALLOW_DROP_DEPTH: usize = 32;

/// Drops `items` without recursing deeper than [`SHALLOW_DROP_DEPTH`] bodies.
pub(crate) fn drop_items<T: ModelTree>(items: &mut Vec<T::Item>) {
    drop_items_bounded::<T>(items, 0);
}

fn drop_items_bounded<T: ModelTree>(items: &mut Vec<T::Item>, depth: usize) {
    if items.is_empty() {
        return;
    }
    if depth == SHALLOW_DROP_DEPTH {
        drop_items_iterative::<T>(items);
        return;
    }
    // Native destruction is substantially cheaper for ordinary shallow trees. Empty each nested
    // body just before its item drops, in the same pass, and switch to the iterative path before
    // recursion can exhaust the stack.
    for mut item in items.drain(..) {
        if let Node::Repeat(repeat) = T::node_mut(&mut item) {
            drop_items_bounded::<T>(T::body_mut(repeat), depth + 1);
        }
    }
}

fn drop_items_iterative<T: ModelTree>(items: &mut Vec<T::Item>) {
    let mut pending = std::mem::take(items);
    while let Some(mut item) = pending.pop() {
        // Borrow the variant: a model's body cannot be moved out of a type implementing Drop.
        if let Node::Repeat(repeat) = T::node_mut(&mut item) {
            pending.append(T::body_mut(repeat));
        }
    }
}
