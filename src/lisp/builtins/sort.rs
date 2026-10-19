//! GNU Emacs 31.1 src/sort.c's Timsort/powersort, derived from its
//! GPL-3.0-or-later implementation (Free Software Foundation, 2022–2026).
//! Comparison order is observable through Lisp callbacks and errors.

use crate::lisp::Interp;
use crate::lisp::error::Flow;
use crate::lisp::value::{Value, VecRef};

const GALLOP_WIN_MIN: usize = 7;

struct Comparator {
    function: Value,
    default: bool,
}

impl Comparator {
    fn less(&self, i: &mut Interp, a: Value, b: Value) -> Result<bool, Flow> {
        let result = if self.default {
            super::misc::f_value_lt(i, vec![a, b].into())
        } else {
            // resolve_fun captures the function object once, but GNU's
            // calln still performs normal arity and call-frame handling.
            i.apply(&self.function, vec![a, b])
        };
        result.map(|value| value.truthy())
    }
}

fn resolve_function(i: &mut Interp, function: &Value) -> Result<Value, Flow> {
    if !matches!(function, Value::Sym(_)) {
        return Ok(function.clone());
    }
    let resolved = super::data::f_indirect_function(i, vec![function.clone()].into())?;
    let autoload = matches!(&resolved, Value::Cons(cell)
        if matches!(&cell.borrow().car, Value::Sym(s) if i.symbol_name(*s) == "autoload"));
    Ok(if resolved.is_nil() || autoload {
        function.clone()
    } else {
        resolved
    })
}

#[derive(Clone)]
struct Item {
    key: Value,
    value: Value,
}

#[derive(Clone, Copy)]
struct Run {
    start: usize,
    len: usize,
    power: usize,
}

struct Sort {
    // Values stay in the actual vector. No RefCell borrow crosses a Lisp
    // call, so predicates can inspect or change an in-place vector.
    values: VecRef,
    keys: Option<Vec<Value>>,
    comparator: Comparator,
    pending: Vec<Run>,
    length: usize,
    min_gallop: usize,
    allocated: usize,
    cleanup_registered: bool,
}

impl Sort {
    fn key(&self, index: usize) -> Value {
        match &self.keys {
            Some(keys) => keys[index].clone(),
            None => self.values.borrow()[index].clone(),
        }
    }

    fn item(&self, index: usize) -> Item {
        Item {
            key: self.key(index),
            value: self.values.borrow()[index].clone(),
        }
    }

    fn set(&mut self, index: usize, item: Item) {
        if let Some(keys) = &mut self.keys {
            keys[index] = item.key;
        }
        self.values.borrow_mut()[index] = item.value;
    }

    fn copy(&mut self, source: usize, destination: usize, count: usize) {
        if destination < source {
            for offset in 0..count {
                self.set(destination + offset, self.item(source + offset));
            }
        } else {
            for offset in (0..count).rev() {
                self.set(destination + offset, self.item(source + offset));
            }
        }
    }

    fn from_temp(&mut self, source: &[Item], destination: usize) {
        for (offset, item) in source.iter().enumerate() {
            self.set(destination + offset, item.clone());
        }
    }

    fn reverse(&mut self, start: usize, count: usize) {
        self.values.borrow_mut()[start..start + count].reverse();
        if let Some(keys) = &mut self.keys {
            keys[start..start + count].reverse();
        }
    }

    fn less(&self, i: &mut Interp, a: usize, b: usize) -> Result<bool, Flow> {
        self.comparator.less(i, self.key(a), self.key(b))
    }

    fn count_run(&self, i: &mut Interp, start: usize) -> Result<(usize, bool), Flow> {
        if start + 1 == self.length {
            return Ok((1, false));
        }
        let descending = self.less(i, start + 1, start)?;
        let mut length = 2;
        while start + length < self.length {
            if self.less(i, start + length, start + length - 1)? != descending {
                break;
            }
            length += 1;
        }
        Ok((length, descending))
    }

    fn binary_sort(
        &mut self,
        i: &mut Interp,
        lo: usize,
        hi: usize,
        start: usize,
    ) -> Result<(), Flow> {
        for pivot_index in start.max(lo + 1)..hi {
            let pivot = self.item(pivot_index);
            let (mut left, mut right) = (lo, pivot_index);
            while left < right {
                let middle = left + (right - left) / 2;
                if self
                    .comparator
                    .less(i, pivot.key.clone(), self.key(middle))?
                {
                    right = middle;
                } else {
                    left = middle + 1;
                }
            }
            self.copy(left, left + 1, pivot_index - left);
            self.set(left, pivot);
        }
        Ok(())
    }

    fn need_memory(&mut self, length: usize) {
        if length > self.allocated {
            self.allocated = length;
            self.cleanup_registered = true;
        }
    }

    fn merge_lo(
        &mut self,
        i: &mut Interp,
        start: usize,
        mut na: usize,
        bstart: usize,
        mut nb: usize,
    ) -> Result<(), Flow> {
        self.need_memory(na);
        let temp: Vec<Item> = (start..start + na).map(|n| self.item(n)).collect();
        let (mut a, mut b, mut destination) = (0, bstart, start);
        // On an error GNU's registered unwind handler restores the
        // remaining temporary values at these current cursor positions.
        let result = (|| {
            self.set(destination, self.item(b));
            destination += 1;
            b += 1;
            nb -= 1;
            let mut min_gallop = self.min_gallop;
            'merge: while nb != 0 && na > 1 {
                let (mut acount, mut bcount) = (0, 0);
                loop {
                    if self.comparator.less(i, self.key(b), temp[a].key.clone())? {
                        self.set(destination, self.item(b));
                        destination += 1;
                        b += 1;
                        bcount += 1;
                        acount = 0;
                        nb -= 1;
                        if nb == 0 {
                            break 'merge;
                        }
                        if bcount >= min_gallop {
                            break;
                        }
                    } else {
                        self.set(destination, temp[a].clone());
                        destination += 1;
                        a += 1;
                        acount += 1;
                        bcount = 0;
                        na -= 1;
                        if na == 1 {
                            break 'merge;
                        }
                        if acount >= min_gallop {
                            break;
                        }
                    }
                }
                min_gallop += 1;
                loop {
                    min_gallop -= usize::from(min_gallop > 1);
                    self.min_gallop = min_gallop;
                    let k = gallop_right(
                        i,
                        &self.comparator,
                        self.key(b),
                        |n| temp[a + n].key.clone(),
                        na,
                        0,
                    )?;
                    acount = k;
                    self.from_temp(&temp[a..a + k], destination);
                    destination += k;
                    a += k;
                    na -= k;
                    if na <= 1 {
                        break 'merge;
                    }
                    self.set(destination, self.item(b));
                    destination += 1;
                    b += 1;
                    nb -= 1;
                    if nb == 0 {
                        break 'merge;
                    }
                    let k = gallop_left(
                        i,
                        &self.comparator,
                        temp[a].key.clone(),
                        |n| self.key(b + n),
                        nb,
                        0,
                    )?;
                    bcount = k;
                    self.copy(b, destination, k);
                    destination += k;
                    b += k;
                    nb -= k;
                    if nb == 0 {
                        break 'merge;
                    }
                    self.set(destination, temp[a].clone());
                    destination += 1;
                    a += 1;
                    na -= 1;
                    if na == 1 {
                        break 'merge;
                    }
                    if acount < GALLOP_WIN_MIN && bcount < GALLOP_WIN_MIN {
                        break;
                    }
                }
                min_gallop += 1;
                self.min_gallop = min_gallop;
            }
            Ok(())
        })();
        if result.is_ok() {
            if na == 1 && nb > 0 {
                self.copy(b, destination, nb);
                self.set(destination + nb, temp[a].clone());
            } else {
                self.from_temp(&temp[a..a + na], destination);
            }
        } else if self.cleanup_registered {
            self.from_temp(&temp[a..a + na], destination);
        }
        result
    }

    fn merge_hi(
        &mut self,
        i: &mut Interp,
        start: usize,
        mut na: usize,
        bstart: usize,
        mut nb: usize,
    ) -> Result<(), Flow> {
        self.need_memory(nb);
        let temp: Vec<Item> = (bstart..bstart + nb).map(|n| self.item(n)).collect();
        // Destination points one past the next slot, avoiding negative
        // cursors when a merge reaches the beginning of the vector.
        let mut destination = bstart + nb;
        let result = (|| {
            destination -= 1;
            self.set(destination, self.item(start + na - 1));
            na -= 1;
            let mut min_gallop = self.min_gallop;
            'merge: while na != 0 && nb > 1 {
                let (mut acount, mut bcount) = (0, 0);
                loop {
                    if self.comparator.less(
                        i,
                        temp[nb - 1].key.clone(),
                        self.key(start + na - 1),
                    )? {
                        destination -= 1;
                        self.set(destination, self.item(start + na - 1));
                        acount += 1;
                        bcount = 0;
                        na -= 1;
                        if na == 0 {
                            break 'merge;
                        }
                        if acount >= min_gallop {
                            break;
                        }
                    } else {
                        destination -= 1;
                        self.set(destination, temp[nb - 1].clone());
                        bcount += 1;
                        acount = 0;
                        nb -= 1;
                        if nb == 1 {
                            break 'merge;
                        }
                        if bcount >= min_gallop {
                            break;
                        }
                    }
                }
                min_gallop += 1;
                loop {
                    min_gallop -= usize::from(min_gallop > 1);
                    self.min_gallop = min_gallop;
                    let k = na
                        - gallop_right(
                            i,
                            &self.comparator,
                            temp[nb - 1].key.clone(),
                            |n| self.key(start + n),
                            na,
                            na - 1,
                        )?;
                    acount = k;
                    destination -= k;
                    na -= k;
                    self.copy(start + na, destination, k);
                    if na == 0 {
                        break 'merge;
                    }
                    destination -= 1;
                    self.set(destination, temp[nb - 1].clone());
                    nb -= 1;
                    if nb == 1 {
                        break 'merge;
                    }
                    let k = nb
                        - gallop_left(
                            i,
                            &self.comparator,
                            self.key(start + na - 1),
                            |n| temp[n].key.clone(),
                            nb,
                            nb - 1,
                        )?;
                    bcount = k;
                    destination -= k;
                    nb -= k;
                    self.from_temp(&temp[nb..nb + k], destination);
                    if nb <= 1 {
                        break 'merge;
                    }
                    destination -= 1;
                    self.set(destination, self.item(start + na - 1));
                    na -= 1;
                    if na == 0 {
                        break 'merge;
                    }
                    if acount < GALLOP_WIN_MIN && bcount < GALLOP_WIN_MIN {
                        break;
                    }
                }
                min_gallop += 1;
                self.min_gallop = min_gallop;
            }
            Ok(())
        })();
        if result.is_ok() && nb == 1 && na > 0 {
            self.copy(start, destination - na, na);
            self.set(destination - na - 1, temp[0].clone());
        } else if result.is_ok() || self.cleanup_registered {
            self.from_temp(&temp[..nb], destination - nb);
        }
        result
    }

    fn merge_at(&mut self, i: &mut Interp, index: usize) -> Result<(), Flow> {
        let a = self.pending[index];
        let b = self.pending[index + 1];
        self.pending[index].len = a.len + b.len;
        self.pending.remove(index + 1);
        let k = gallop_right(
            i,
            &self.comparator,
            self.key(b.start),
            |n| self.key(a.start + n),
            a.len,
            0,
        )?;
        let start = a.start + k;
        let na = a.len - k;
        if na == 0 {
            return Ok(());
        }
        let nb = gallop_left(
            i,
            &self.comparator,
            self.key(start + na - 1),
            |n| self.key(b.start + n),
            b.len,
            b.len - 1,
        )?;
        if nb == 0 {
            return Ok(());
        }
        if na <= nb {
            self.merge_lo(i, start, na, b.start, nb)
        } else {
            self.merge_hi(i, start, na, b.start, nb)
        }
    }

    fn found_run(&mut self, i: &mut Interp, length: usize) -> Result<(), Flow> {
        if let Some(last) = self.pending.last().copied() {
            let power = powerloop(last.start, last.len, length, self.length);
            while self.pending.len() > 1 && self.pending[self.pending.len() - 2].power > power {
                self.merge_at(i, self.pending.len() - 2)?;
            }
            self.pending.last_mut().unwrap().power = power;
        }
        Ok(())
    }

    fn run(&mut self, i: &mut Interp) -> Result<(), Flow> {
        let minrun = minrun(self.length);
        let mut start = 0;
        while start < self.length {
            let (mut length, descending) = self.count_run(i, start)?;
            if descending {
                self.reverse(start, length);
            }
            if length < minrun {
                let force = minrun.min(self.length - start);
                self.binary_sort(i, start, start + force, start + length)?;
                length = force;
            }
            self.found_run(i, length)?;
            self.pending.push(Run {
                start,
                len: length,
                power: 0,
            });
            start += length;
        }
        while self.pending.len() > 1 {
            let mut n = self.pending.len() - 2;
            if n > 0 && self.pending[n - 1].len < self.pending[n + 1].len {
                n -= 1;
            }
            self.merge_at(i, n)?;
        }
        Ok(())
    }
}

fn minrun(mut length: usize) -> usize {
    let mut remainder = 0;
    while length >= 64 {
        remainder |= length & 1;
        length >>= 1;
    }
    length + remainder
}

fn powerloop(start: usize, na: usize, nb: usize, length: usize) -> usize {
    let (mut a, mut b) = (2 * start + na, 2 * start + 2 * na + nb);
    let mut power = 0;
    loop {
        power += 1;
        if a >= length {
            a -= length;
            b -= length;
        } else if b >= length {
            return power;
        }
        a <<= 1;
        b <<= 1;
    }
}

fn gallop_left(
    i: &mut Interp,
    predicate: &Comparator,
    key: Value,
    at: impl Fn(usize) -> Value,
    length: usize,
    hint: usize,
) -> Result<usize, Flow> {
    let (mut last, mut offset) = (0, 1);
    let mut left;
    if predicate.less(i, at(hint), key.clone())? {
        let max = length - hint;
        while offset < max {
            if !predicate.less(i, at(hint + offset), key.clone())? {
                break;
            }
            last = offset;
            offset = offset * 2 + 1;
        }
        offset = offset.min(max) + hint;
        left = last + hint + 1;
    } else {
        let max = hint + 1;
        while offset < max {
            if predicate.less(i, at(hint - offset), key.clone())? {
                break;
            }
            last = offset;
            offset = offset * 2 + 1;
        }
        let lower = offset.min(max);
        offset = hint - last;
        left = hint + 1 - lower;
    }
    while left < offset {
        let middle = left + (offset - left) / 2;
        if predicate.less(i, at(middle), key.clone())? {
            left = middle + 1;
        } else {
            offset = middle;
        }
    }
    Ok(offset)
}

fn gallop_right(
    i: &mut Interp,
    predicate: &Comparator,
    key: Value,
    at: impl Fn(usize) -> Value,
    length: usize,
    hint: usize,
) -> Result<usize, Flow> {
    let (mut last, mut offset) = (0, 1);
    let mut left;
    if predicate.less(i, key.clone(), at(hint))? {
        let max = hint + 1;
        while offset < max {
            if !predicate.less(i, key.clone(), at(hint - offset))? {
                break;
            }
            last = offset;
            offset = offset * 2 + 1;
        }
        let lower = offset.min(max);
        offset = hint - last;
        left = hint + 1 - lower;
    } else {
        let max = length - hint;
        while offset < max {
            if predicate.less(i, key.clone(), at(hint + offset))? {
                break;
            }
            last = offset;
            offset = offset * 2 + 1;
        }
        offset = offset.min(max) + hint;
        left = last + hint + 1;
    }
    while left < offset {
        let middle = left + (offset - left) / 2;
        if predicate.less(i, key.clone(), at(middle))? {
            offset = middle;
        } else {
            left = middle + 1;
        }
    }
    Ok(offset)
}

pub(super) fn tim_sort(
    i: &mut Interp,
    values: VecRef,
    predicate: &Value,
    key_function: &Value,
    reverse: bool,
) -> Result<(), Flow> {
    let length = values.borrow().len();
    if length < 2 {
        return Ok(());
    }
    let default =
        predicate.is_nil() || matches!(predicate, Value::Sym(s) if i.symbol_name(*s) == "value<");
    let function = if default {
        Value::Nil
    } else {
        resolve_function(i, predicate)?
    };
    let keyed = key_function.truthy()
        && !matches!(key_function, Value::Sym(s) if i.symbol_name(*s) == "identity");
    if reverse {
        values.borrow_mut().reverse();
    }
    // Match resolve_fun's order: the key function is resolved after the
    // initial reversal, and then evaluated once per element, before any
    // comparisons. Snapshotting values here would hide callback mutations.
    let keys = if keyed {
        let function = resolve_function(i, key_function)?;
        let mut keys = Vec::with_capacity(length);
        for index in 0..length {
            let value = values.borrow()[index].clone();
            keys.push(i.apply(&function, vec![value])?);
        }
        Some(keys)
    } else {
        None
    };
    let mut sort = Sort {
        values,
        keys,
        comparator: Comparator { function, default },
        pending: Vec::new(),
        length,
        min_gallop: GALLOP_WIN_MIN,
        allocated: if keyed {
            length.div_ceil(2).min(128)
        } else {
            256
        },
        cleanup_registered: keyed && length >= 128,
    };
    sort.run(i)?;
    if reverse {
        sort.values.borrow_mut().reverse();
    }
    Ok(())
}
