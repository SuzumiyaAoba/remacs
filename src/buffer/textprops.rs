//! Canonical buffer text-property intervals and their legacy projection.
//!
//! A missing interval tree is different from a tree whose plists are all
//! empty. Once created, the tree covers the complete text and retains
//! explicit setter boundaries, including boundaries between equal plists.

use std::ops::Deref;

use crate::buffer::TextProp;
use crate::lisp::builtins::eq_values;
use crate::lisp::Value;

#[derive(Clone)]
pub struct TextProperties {
    flat: Vec<TextProp>,
    intervals: Vec<(usize, usize, Vec<Value>)>,
    len: usize,
}

impl Deref for TextProperties {
    type Target = [TextProp];

    fn deref(&self) -> &Self::Target {
        &self.flat
    }
}

impl TextProperties {
    pub(crate) fn new() -> Self {
        Self {
            flat: Vec::new(),
            intervals: Vec::new(),
            len: 0,
        }
    }

    pub(crate) fn intervals(&self) -> &[(usize, usize, Vec<Value>)] {
        &self.intervals
    }

    /// Property-change readers ordinarily skip boundaries with equivalent
    /// plists. GNU's LIMIT=t variant instead exposes every interval cut.
    pub(crate) fn change_points(&self, raw_intervals: bool) -> impl Iterator<Item = usize> + '_ {
        self.intervals
            .windows(2)
            .filter(move |ivs| raw_intervals || !Self::plists_equal(&ivs[0].2, &ivs[1].2))
            .map(|ivs| ivs[1].0)
    }

    /// Copy the interval structure, including empty plists, rebased to zero.
    pub(crate) fn slice(&self, start: usize, end: usize) -> Vec<(usize, usize, Vec<Value>)> {
        let end = end.min(self.len);
        if start >= end {
            return Vec::new();
        }
        self.intervals
            .iter()
            .filter_map(|(a, b, plist)| {
                let a = (*a).max(start);
                let b = (*b).min(end);
                (a < b).then(|| (a - start, b - start, plist.clone()))
            })
            .collect()
    }

    /// Set a single key without changing the cuts of unaffected intervals.
    /// An explicitly nil value is a present property, not its removal.
    pub(crate) fn put(&mut self, prop: TextProp) {
        let end = prop.end.min(self.len);
        if prop.start >= end {
            return;
        }
        self.ensure_tree();
        let key = Value::Sym(prop.prop);
        self.edit(prop.start, end, |plist| {
            let slot = plist
                .chunks_exact(2)
                .position(|pair| eq_values(&pair[0], &key));
            if let Some(slot) = slot {
                if eq_values(&plist[slot * 2 + 1], &prop.value) {
                    return None;
                }
                let mut updated = plist.to_vec();
                updated[slot * 2 + 1] = prop.value.clone();
                Some(updated)
            } else {
                let mut updated = Vec::with_capacity(plist.len() + 2);
                updated.push(key.clone());
                updated.push(prop.value.clone());
                updated.extend_from_slice(plist);
                Some(updated)
            }
        });
        self.rebuild_flat();
    }

    pub(crate) fn push(&mut self, prop: TextProp) {
        self.put(prop);
    }

    pub(crate) fn extend(&mut self, props: impl IntoIterator<Item = TextProp>) {
        for prop in props {
            self.put(prop);
        }
    }

    /// Remove matching keys, splitting only intervals that contain one.
    pub(crate) fn remove(&mut self, start: usize, end: usize, pred: impl Fn(u32) -> bool) {
        let end = end.min(self.len);
        if start >= end || self.intervals.is_empty() {
            return;
        }
        self.edit(start, end, |plist| {
            let mut updated = Vec::with_capacity(plist.len());
            let mut changed = false;
            for pair in plist.chunks_exact(2) {
                if Self::key_id(&pair[0]).is_some_and(&pred) {
                    changed = true;
                } else {
                    updated.extend_from_slice(pair);
                }
            }
            changed.then_some(updated)
        });
        self.rebuild_flat();
    }

    /// Replace a range by exactly one interval, even if its plist is equal
    /// to its neighbors. A nil setter on text without a tree creates none.
    pub(crate) fn set(&mut self, start: usize, end: usize, plist: Vec<Value>) {
        let end = end.min(self.len);
        if start >= end || (self.intervals.is_empty() && plist.is_empty()) {
            return;
        }
        self.ensure_tree();
        let mut out = Vec::with_capacity(self.intervals.len() + 2);
        let mut replacement = Some(plist);
        for (a, b, old) in std::mem::take(&mut self.intervals) {
            if b <= start || a >= end {
                out.push((a, b, old));
                continue;
            }
            if a < start {
                out.push((a, start, old.clone()));
            }
            if let Some(plist) = replacement.take() {
                out.push((start, end, plist));
            }
            if b > end {
                out.push((end, b, old));
            }
        }
        self.intervals = out;
        self.rebuild_flat();
    }

    /// Graft a rebased source tree onto text already inserted into the
    /// buffer. Preserve source and target cuts, even for empty plists.
    /// With inheritance, existing keys win and absent source keys are
    /// prepended in source order, as in GNU's merge_properties.
    pub(crate) fn graft(
        &mut self,
        start: usize,
        source: &[(usize, usize, Vec<Value>)],
        inherit: bool,
    ) {
        if source.is_empty() || start >= self.len {
            return;
        }
        // GNU reproduces the source tree directly when the inserted text
        // is the entire buffer. Even inherited insertion preserves the
        // source's plist order in this case.
        if start == 0 && source.last().is_some_and(|iv| iv.1 == self.len) {
            self.intervals = source.to_vec();
            self.rebuild_flat();
            return;
        }
        self.ensure_tree();
        for (a, b, plist) in source {
            let a = start.saturating_add(*a).min(self.len);
            let b = start.saturating_add(*b).min(self.len);
            if a >= b {
                continue;
            }
            self.edit(a, b, |old| {
                if !inherit {
                    return Some(plist.clone());
                }
                let mut merged = old.to_vec();
                for pair in plist.chunks_exact(2) {
                    if !merged.chunks_exact(2).any(|p| eq_values(&p[0], &pair[0])) {
                        merged.splice(0..0, pair.iter().cloned());
                    }
                }
                Some(merged)
            });
        }
        self.rebuild_flat();
    }

    /// Insert unpropertized text. Inheritance and source grafting are
    /// separate operations performed by the buffer insertion primitives.
    pub(crate) fn insert(&mut self, pos: usize, n: usize) {
        if n == 0 {
            return;
        }
        let pos = pos.min(self.len);
        self.len += n;
        if self.intervals.is_empty() {
            return;
        }
        let mut out = Vec::with_capacity(self.intervals.len() + 2);
        let mut inserted = false;
        for (a, b, plist) in std::mem::take(&mut self.intervals) {
            if b <= pos {
                out.push((a, b, plist));
                continue;
            }
            if !inserted {
                if a < pos {
                    out.push((a, pos, plist.clone()));
                }
                out.push((pos, pos + n, Vec::new()));
                inserted = true;
            }
            out.push((a.max(pos) + n, b + n, plist));
        }
        if !inserted {
            out.push((pos, pos + n, Vec::new()));
        }
        self.intervals = out;
        self.rebuild_flat();
    }

    /// Finish an inherited insertion whose source had no interval tree.
    /// SPLIT_ORIGINAL records whether BASE lay strictly inside an interval
    /// before insertion. Only the new boundaries may be removed: at an
    /// existing cut, GNU prefers extending the left interval and retains
    /// the original cut even when the interval on the right is equal.
    pub(crate) fn finish_inherited_insert(&mut self, base: usize, n: usize, split_original: bool) {
        if n == 0 {
            return;
        }
        let end = base.saturating_add(n);
        let Some(index) = self
            .intervals
            .iter()
            .position(|(a, b, _)| *a == base && *b == end)
        else {
            // A missing tree needs no adjustment. If another operation
            // split the inserted span, do not erase its explicit cuts.
            return;
        };
        let join_left =
            index > 0 && Self::plists_equal(&self.intervals[index - 1].2, &self.intervals[index].2);
        let join_right = index + 1 < self.intervals.len()
            && Self::plists_equal(&self.intervals[index].2, &self.intervals[index + 1].2);
        if join_left {
            self.intervals[index - 1].1 = end;
            self.intervals.remove(index);
            if split_original && join_right {
                self.intervals[index - 1].1 = self.intervals[index].1;
                self.intervals.remove(index);
            }
        } else if join_right {
            self.intervals[index + 1].0 = base;
            self.intervals.remove(index);
        } else {
            return;
        }
        self.rebuild_flat();
    }

    /// GNU's deletion adjustment shrinks or drops intervals without
    /// coalescing surviving neighbors, even when their plists are equal.
    pub(crate) fn delete(&mut self, start: usize, end: usize) {
        let end = end.min(self.len);
        if start >= end {
            return;
        }
        let n = end - start;
        self.len -= n;
        let adjusted = |p: usize| {
            if p <= start {
                p
            } else if p < end {
                start
            } else {
                p - n
            }
        };
        self.intervals = std::mem::take(&mut self.intervals)
            .into_iter()
            .filter_map(|(a, b, plist)| {
                let a = adjusted(a);
                let b = adjusted(b);
                (a < b).then_some((a, b, plist))
            })
            .collect();
        self.rebuild_flat();
    }

    fn ensure_tree(&mut self) {
        if self.intervals.is_empty() && self.len != 0 {
            self.intervals.push((0, self.len, Vec::new()));
        }
    }

    /// Apply an optional change independently to each intersecting
    /// interval. None means preserve that interval and all its boundaries.
    fn edit(&mut self, start: usize, end: usize, change: impl Fn(&[Value]) -> Option<Vec<Value>>) {
        let mut out = Vec::with_capacity(self.intervals.len() + 2);
        for (a, b, plist) in std::mem::take(&mut self.intervals) {
            if b <= start || a >= end {
                out.push((a, b, plist));
                continue;
            }
            let Some(updated) = change(&plist) else {
                out.push((a, b, plist));
                continue;
            };
            if a < start {
                out.push((a, start, plist.clone()));
            }
            out.push((a.max(start), b.min(end), updated));
            if b > end {
                out.push((end, b, plist));
            }
        }
        self.intervals = out;
    }

    fn key_id(key: &Value) -> Option<u32> {
        match key {
            Value::Nil => Some(0),
            Value::Sym(id) => Some(*id),
            _ => None,
        }
    }

    /// GNU intervals_equal compares pair counts and values by identity,
    /// looking up the first matching key without regard to plist order.
    /// A present nil property is therefore distinct from an absent key.
    fn plists_equal(left: &[Value], right: &[Value]) -> bool {
        left.len() == right.len()
            && left.chunks_exact(2).all(|pair| {
                right
                    .chunks_exact(2)
                    .find(|other| eq_values(&pair[0], &other[0]))
                    .is_some_and(|other| eq_values(&pair[1], &other[1]))
            })
    }

    fn rebuild_flat(&mut self) {
        self.flat.clear();
        for (start, end, plist) in &self.intervals {
            // Legacy readers scan records backwards. Reverse pairs here
            // so those readers see the canonical plist's original order.
            for pair in plist.chunks_exact(2).rev() {
                if let Some(prop) = Self::key_id(&pair[0]) {
                    self.flat.push(TextProp {
                        start: *start,
                        end: *end,
                        prop,
                        value: pair[1].clone(),
                    });
                }
            }
        }
        debug_assert!(self.intervals.is_empty() || self.intervals[0].0 == 0);
        debug_assert!(self.intervals.last().is_none_or(|iv| iv.1 == self.len));
        debug_assert!(self
            .intervals
            .iter()
            .all(|(a, b, pl)| a < b && pl.len() % 2 == 0));
        debug_assert!(self.intervals.windows(2).all(|ivs| ivs[0].1 == ivs[1].0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plist(key: u32, value: i128) -> Vec<Value> {
        vec![Value::Sym(key), Value::Int(value)]
    }

    fn spans(props: &TextProperties) -> Vec<(usize, usize)> {
        props.intervals().iter().map(|(a, b, _)| (*a, *b)).collect()
    }

    fn text(len: usize) -> TextProperties {
        let mut props = TextProperties::new();
        props.insert(0, len);
        props
    }

    #[test]
    fn nil_set_preserves_absent_tree_but_not_existing_tree() {
        let mut props = text(6);
        props.set(1, 4, Vec::new());
        assert!(props.intervals().is_empty());
        props.set(0, 6, plist(2, 1));
        props.set(0, 6, Vec::new());
        assert_eq!(spans(&props), [(0, 6)]);
        assert!(props.is_empty());
        props.set(2, 4, Vec::new());
        assert_eq!(spans(&props), [(0, 2), (2, 4), (4, 6)]);
    }

    #[test]
    fn put_only_splits_intervals_with_a_changed_key() {
        let mut props = text(6);
        props.set(0, 3, plist(2, 1));
        props.set(3, 6, plist(2, 2));
        props.put(TextProp {
            start: 1,
            end: 5,
            prop: 2,
            value: Value::Int(1),
        });
        assert_eq!(spans(&props), [(0, 3), (3, 5), (5, 6)]);
        props.put(TextProp {
            start: 1,
            end: 2,
            prop: 3,
            value: Value::Nil,
        });
        let middle = &props.intervals()[1].2;
        assert!(matches!(
            middle.as_slice(),
            [Value::Sym(3), Value::Nil, Value::Sym(2), Value::Int(1)]
        ));
    }

    #[test]
    fn remove_absent_key_does_not_split_and_removal_keeps_empty_intervals() {
        let mut props = text(6);
        props.set(0, 6, plist(2, 1));
        props.remove(1, 5, |key| key == 3);
        assert_eq!(spans(&props), [(0, 6)]);
        props.remove(1, 5, |key| key == 2);
        assert_eq!(spans(&props), [(0, 1), (1, 5), (5, 6)]);
        assert!(props.intervals()[1].2.is_empty());
    }

    #[test]
    fn setter_replaces_whole_interior_and_keeps_equal_outside_cuts() {
        let mut props = text(8);
        props.set(0, 8, plist(2, 1));
        props.set(2, 4, plist(2, 1));
        props.set(4, 6, plist(2, 1));
        props.set(1, 7, plist(2, 1));
        assert_eq!(spans(&props), [(0, 1), (1, 7), (7, 8)]);
    }

    #[test]
    fn projection_preserves_plist_order_for_reverse_readers() {
        let mut props = text(4);
        props.set(
            0,
            4,
            vec![Value::Sym(2), Value::Int(1), Value::Sym(3), Value::Nil],
        );
        assert_eq!(
            props.iter().rev().map(|p| p.prop).collect::<Vec<_>>(),
            [2, 3]
        );
        props.put(TextProp {
            start: 0,
            end: 4,
            prop: 2,
            value: Value::Int(2),
        });
        assert_eq!(
            props.iter().rev().map(|p| p.prop).collect::<Vec<_>>(),
            [2, 3]
        );
    }

    #[test]
    fn insertion_and_deletion_keep_dead_interval_boundaries() {
        let mut props = text(6);
        props.set(0, 6, plist(2, 1));
        props.set(2, 4, plist(2, 1));
        props.insert(3, 2);
        assert_eq!(spans(&props), [(0, 2), (2, 3), (3, 5), (5, 6), (6, 8)]);
        assert!(props.intervals()[2].2.is_empty());
        props.delete(2, 6);
        assert_eq!(spans(&props), [(0, 2), (2, 4)]);
        props.delete(0, usize::MAX);
        assert_eq!(props.len, 0);
        assert!(props.intervals().is_empty());
    }

    #[test]
    fn slice_rebases_empty_and_equal_intervals() {
        let mut props = text(6);
        props.set(2, 4, plist(2, 1));
        let slice = props.slice(1, 5);
        assert_eq!(
            slice.iter().map(|(a, b, _)| (*a, *b)).collect::<Vec<_>>(),
            [(0, 1), (1, 3), (3, 4)]
        );
        assert!(slice[0].2.is_empty());
        assert!(slice[2].2.is_empty());
    }

    #[test]
    fn graft_preserves_source_cuts_and_inherited_values() {
        let mut props = text(8);
        props.set(0, 8, plist(2, 1));
        props.graft(
            2,
            &[
                (
                    0,
                    2,
                    vec![Value::Sym(2), Value::Int(9), Value::Sym(3), Value::Int(3)],
                ),
                (2, 4, Vec::new()),
            ],
            true,
        );
        assert_eq!(spans(&props), [(0, 2), (2, 4), (4, 6), (6, 8)]);
        assert!(matches!(
            props.intervals()[1].2.as_slice(),
            [Value::Sym(3), Value::Int(3), Value::Sym(2), Value::Int(1)]
        ));
        props.graft(
            2,
            &[(
                0,
                2,
                vec![Value::Sym(2), Value::Int(9), Value::Sym(3), Value::Int(3)],
            )],
            false,
        );
        assert!(matches!(
            props.intervals()[1].2.as_slice(),
            [Value::Sym(2), Value::Int(9), Value::Sym(3), Value::Int(3)]
        ));
    }

    #[test]
    fn whole_buffer_graft_copies_source_order_and_nil_cuts_for_both_modes() {
        for inherit in [false, true] {
            let mut props = text(6);
            props.set(0, 6, plist(4, 9));
            props.set(2, 3, plist(4, 9));
            props.graft(
                0,
                &[
                    (0, 1, Vec::new()),
                    (
                        1,
                        5,
                        vec![Value::Sym(2), Value::Int(1), Value::Sym(3), Value::Int(2)],
                    ),
                    (5, 6, Vec::new()),
                ],
                inherit,
            );
            assert_eq!(spans(&props), [(0, 1), (1, 5), (5, 6)]);
            assert!(props.intervals()[0].2.is_empty());
            assert!(matches!(
                props.intervals()[1].2.as_slice(),
                [Value::Sym(2), Value::Int(1), Value::Sym(3), Value::Int(2)]
            ));
        }
    }

    #[test]
    fn inherited_interior_removes_only_new_cuts_and_keeps_original_order() {
        let original = vec![Value::Sym(2), Value::Int(1), Value::Sym(3), Value::Int(2)];
        let reversed = vec![Value::Sym(3), Value::Int(2), Value::Sym(2), Value::Int(1)];
        let mut props = text(6);
        props.set(0, 6, original.clone());
        props.set(2, 4, reversed.clone());
        props.insert(3, 2);
        props.set(3, 5, original);
        props.finish_inherited_insert(3, 2, true);
        assert_eq!(spans(&props), [(0, 2), (2, 6), (6, 8)]);
        assert!(matches!(
            props.intervals()[1].2.as_slice(),
            [Value::Sym(3), Value::Int(2), Value::Sym(2), Value::Int(1)]
        ));
    }

    #[test]
    fn inherited_boundary_prefers_left_without_merging_existing_equal_cut() {
        let mut props = text(6);
        props.set(
            0,
            6,
            vec![Value::Sym(2), Value::Int(1), Value::Sym(3), Value::Int(2)],
        );
        props.set(
            2,
            4,
            vec![Value::Sym(3), Value::Int(2), Value::Sym(2), Value::Int(1)],
        );
        props.insert(2, 2);
        props.set(
            2,
            4,
            vec![Value::Sym(3), Value::Int(2), Value::Sym(2), Value::Int(1)],
        );
        props.finish_inherited_insert(2, 2, false);
        assert_eq!(spans(&props), [(0, 4), (4, 6), (6, 8)]);
        assert!(matches!(
            props.intervals()[0].2.as_slice(),
            [Value::Sym(2), Value::Int(1), Value::Sym(3), Value::Int(2)]
        ));
    }

    #[test]
    fn inherited_boundary_joins_only_a_matching_neighbor() {
        let mut props = text(6);
        props.set(0, 2, plist(2, 1));
        props.insert(2, 2);
        props.finish_inherited_insert(2, 2, false);
        assert_eq!(spans(&props), [(0, 2), (2, 8)]);
        let mut props = text(6);
        props.set(0, 6, plist(2, 1));
        props.insert(3, 2);
        props.finish_inherited_insert(3, 2, true);
        assert_eq!(spans(&props), [(0, 3), (3, 5), (5, 8)]);
    }

    #[test]
    fn inherited_plain_edges_merge_nil_tree_but_do_not_create_one() {
        for base in [0, 3, 6] {
            let mut props = text(6);
            props.set(0, 6, plist(2, 1));
            props.set(0, 6, Vec::new());
            props.insert(base, 2);
            props.finish_inherited_insert(base, 2, base == 3);
            assert_eq!(spans(&props), [(0, 8)]);
            let mut props = text(6);
            props.insert(base, 2);
            props.finish_inherited_insert(base, 2, false);
            assert!(props.intervals().is_empty());
        }
    }

    #[test]
    fn change_points_distinguish_raw_cuts_from_eq_plist_changes() {
        let mut props = text(8);
        props.set(
            0,
            4,
            vec![Value::Sym(2), Value::Int(1), Value::Sym(3), Value::Int(2)],
        );
        props.set(
            2,
            4,
            vec![Value::Sym(3), Value::Int(2), Value::Sym(2), Value::Int(1)],
        );
        props.set(4, 6, vec![Value::Sym(2), Value::Nil]);
        assert_eq!(props.change_points(true).collect::<Vec<_>>(), [2, 4, 6]);
        assert_eq!(props.change_points(false).collect::<Vec<_>>(), [4, 6]);
        let same = Value::string("same");
        props.set(0, 2, vec![Value::Sym(2), same.clone()]);
        props.set(2, 4, vec![Value::Sym(2), same]);
        props.set(4, 6, vec![Value::Sym(2), Value::string("same")]);
        assert_eq!(props.change_points(false).collect::<Vec<_>>(), [4, 6]);
    }

    #[test]
    fn plist_equality_retains_duplicate_key_counts_and_first_value_rule() {
        let mut props = text(6);
        props.set(0, 2, plist(2, 1));
        props.set(
            2,
            4,
            vec![Value::Sym(2), Value::Int(1), Value::Sym(2), Value::Int(1)],
        );
        props.set(
            4,
            6,
            vec![Value::Sym(2), Value::Int(1), Value::Sym(2), Value::Int(2)],
        );
        assert_eq!(props.change_points(false).collect::<Vec<_>>(), [2]);
    }
}
