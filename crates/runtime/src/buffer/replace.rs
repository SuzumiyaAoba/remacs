//! GNU editfns.c replace-region-contents and gnulib's bidirectional
//! Myers comparison (diffseq.h). Equal characters retain their old objects
//! and properties; only the marked edit runs are replaced, backwards.
//
// Adapted from GNU editfns.c and gnulib diffseq.h; Rust port for Remacs.
// Copyright (C) 1988-1989, 1992-1995, 2001-2004, 2006-2026
// Free Software Foundation, Inc.
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
// General Public License for more details.
// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

use super::extra::region_text;
use super::primitives::{chg_replace_contents, cur, f_buffer_substring};
use crate::lisp::Interp;
use crate::lisp::builtins::data::functionp_val;
use crate::lisp::builtins::evalfn::{
    mod_hooks_inhibited, signal_after_change, signal_before_change,
};
use crate::lisp::builtins::{arg, want_int};
use crate::lisp::error::EvalResult;
use crate::lisp::value::Value;
use num_traits::ToPrimitive;
use std::time::Instant;

pub(super) fn replace_region_contents(i: &mut Interp, args: &[Value]) -> EvalResult {
    let (start, end, _) = region_text(i, args)?;
    let target = cur(i);
    let mut source = args[2].clone();
    if functionp_val(i, &source) {
        let excursion = i.save_excursion_state(false);
        let restriction = i.save_restriction_state();
        {
            let mut buffer = target.borrow_mut();
            buffer.begv = start;
            buffer.zv = end;
            let point = buffer.point();
            buffer.set_point(point);
        }
        let result = i.apply(&source, vec![]);
        i.restore_restriction_state(restriction);
        i.restore_excursion_state(excursion);
        source = result?;
    }
    let source_buffer = match &source {
        Value::Str(_) => None,
        Value::Buffer(buffer) => Some((buffer.clone(), None)),
        Value::Vec(vector) => {
            let vector = vector.borrow();
            if vector.len() < 3 {
                let signal = i.q("args-out-of-range");
                return Err(i.signal_data(signal, vec![source.clone(), Value::Int(2)]));
            }
            match &vector[0] {
                Value::Buffer(buffer) => Some((buffer.clone(), Some(vector[1..3].to_vec()))),
                value => return Err(i.wrong_type_mut("bufferp", value)),
            }
        }
        _ => {
            let predicate = Value::list(
                ["or", "string", "buffer", "vector"]
                    .map(|s| Value::Sym(i.intern(s)))
                    .to_vec(),
            );
            let signal = i.q("wrong-type-argument");
            return Err(i.signal_data(signal, vec![predicate, source]));
        }
    };
    let same_buffer = source_buffer
        .as_ref()
        .is_some_and(|(buffer, _)| std::rc::Rc::ptr_eq(buffer, &target));
    if let Some((buffer, bounds)) = source_buffer {
        if !buffer.borrow().live {
            return Err(i.error("Selecting deleted buffer"));
        }
        let current = i.current_buffer;
        i.set_current_buffer(buffer.borrow().id);
        let result = (|| {
            let (start, end, _) = if let Some(bounds) = bounds {
                region_text(i, &bounds)?
            } else {
                let buffer = buffer.borrow();
                (buffer.begv, buffer.zv, crate::lisp::text::EmacsText::default())
            };
            f_buffer_substring(
                i,
                vec![Value::Int(start as i128 + 1), Value::Int(end as i128 + 1)].into(),
            )
        })();
        i.set_current_buffer(current);
        source = result?;
    }
    let Value::Str(source) = source else {
        unreachable!()
    };
    let replacement = source.borrow().clone();
    let multibyte = i.q("enable-multibyte-characters");
    let unibyte = target
        .borrow()
        .locals
        .get(&multibyte)
        .is_some_and(|v| !v.truthy());
    // GNU compares unibyte bytes as eight-bit characters, rather than
    // confusing byte 0xff with the multibyte character U+00FF.
    let as_multibyte = |c: crate::lisp::text::EmacsChar| {
        if (0x80..=0xff).contains(&(c.code())) {
            crate::lisp::value::lisp_char(0x3fff00 + c.code()).unwrap()
        } else {
            c
        }
    };
    let new: Vec<_> = replacement
        .chars()
        .map(|c| {
            if i.is_unibyte_str(&source) {
                as_multibyte(c)
            } else {
                c
            }
        })
        .collect();
    let old: Vec<_> = target
        .borrow()
        .text
        .substring(start, end)
        .chars()
        .map(|c| if unibyte { as_multibyte(c) } else { c })
        .collect();
    let properties = i.str_props(&source).to_vec();
    let inherit = arg(args, 5).truthy();
    if old.is_empty() && new.is_empty() {
        return Ok(Value::t());
    }
    let seconds = arg(args, 3);
    let costs = arg(args, 4);
    if old.is_empty()
        || new.is_empty()
        || matches!(seconds, Value::Int(0))
        || matches!(costs, Value::Int(0))
    {
        chg_replace_contents(i, start, end, &replacement, &properties, inherit)?;
        return Ok(Value::t());
    }
    if same_buffer {
        return Err(i.error("Cannot replace a buffer with itself"));
    }
    let max_cost = if costs.is_nil() {
        1_000_000
    } else if let Value::BigInt(number) = &costs {
        number
            .to_isize()
            .unwrap_or_else(|| {
                if number.sign() == num_bigint::Sign::Minus {
                    0
                } else {
                    isize::MAX
                }
            })
            .max(0)
    } else {
        want_int(i, &costs)?.clamp(0, isize::MAX as i128) as isize
    };
    let deadline = if seconds.is_nil() {
        None
    } else {
        Some((
            Instant::now(),
            crate::lisp::builtins::misc::lisp_time_to_ns(i, &seconds)?,
        ))
    };
    let mut diff = Diff::new(&old, &new, max_cost, deadline);
    if !diff.compare() {
        chg_replace_contents(i, start, end, &replacement, &properties, inherit)?;
        return Ok(Value::Nil);
    }
    target.borrow_mut().undo_boundary();
    let hooks = !mod_hooks_inhibited(i);
    let depth = i.specbind_depth();
    if hooks {
        super::primitives::check_writable(i)?;
        super::primitives::barf_if_file_locked(i)?;
        super::primitives::text_ro_range(i, start, end)?;
        signal_before_change(i, start + 1, end + 1)?;
        let inhibit = i.q("inhibit-modification-hooks");
        i.specbind(inhibit, Value::t())?;
    }
    let result = (|| {
        let (mut x, mut y) = (old.len(), new.len());
        loop {
            if (x > 0 && diff.deleted[x - 1]) || (y > 0 && diff.inserted[y - 1]) {
                let (xe, ye) = (x, y);
                while x > 0 && diff.deleted[x - 1] {
                    x -= 1;
                }
                while y > 0 && diff.inserted[y - 1] {
                    y -= 1;
                }
                let text: crate::lisp::text::EmacsText = new[y..ye].iter().collect();
                let intervals: Vec<_> = properties
                    .iter()
                    .filter_map(|(a, b, plist)| {
                        let (a, b) = ((*a).max(y), (*b).min(ye));
                        (a < b).then(|| (a - y, b - y, plist.clone()))
                    })
                    .collect();
                chg_replace_contents(i, start + x, start + xe, &text, &intervals, inherit)?;
            }
            if x == 0 && y == 0 {
                break;
            }
            x = x.saturating_sub(1);
            y = y.saturating_sub(1);
        }
        Ok(Value::t())
    })();
    i.unbind_to(depth)?;
    result?;
    if hooks {
        signal_after_change(i, start + 1, start + new.len() + 1, old.len() as i128)?;
    }
    Ok(Value::t())
}

struct Diff<'a> {
    old: &'a [crate::lisp::text::EmacsChar],
    new: &'a [crate::lisp::text::EmacsChar],
    deleted: Vec<bool>,
    inserted: Vec<bool>,
    forward: Vec<isize>,
    backward: Vec<isize>,
    offset: isize,
    max_cost: isize,
    deadline: Option<(Instant, i128)>,
    comparisons: u16,
}

impl<'a> Diff<'a> {
    fn new(
        old: &'a [crate::lisp::text::EmacsChar],
        new: &'a [crate::lisp::text::EmacsChar],
        max_cost: isize,
        deadline: Option<(Instant, i128)>,
    ) -> Self {
        let size = old.len() + new.len() + 3;
        Self {
            old,
            new,
            deleted: vec![false; old.len()],
            inserted: vec![false; new.len()],
            forward: vec![0; size],
            backward: vec![0; size],
            offset: new.len() as isize + 1,
            max_cost,
            deadline,
            comparisons: 0,
        }
    }

    fn expired(&self) -> bool {
        self.deadline
            .is_some_and(|(start, ns)| start.elapsed().as_nanos() as i128 > ns)
    }

    fn equal(&mut self, x: isize, y: isize) -> Result<bool, ()> {
        self.comparisons = self.comparisons.wrapping_add(1);
        if self.comparisons == 0 && self.expired() {
            return Err(());
        }
        Ok(self.old[x as usize] == self.new[y as usize])
    }

    fn compare(&mut self) -> bool {
        // An explicit stack also bounds Rust stack use for adversarial input.
        let mut pending = vec![(
            0,
            self.old.len() as isize,
            0,
            self.new.len() as isize,
            false,
        )];
        while let Some((mut x, mut xe, mut y, mut ye, minimal)) = pending.pop() {
            let result = (|| {
                while x < xe && y < ye && self.equal(x, y)? {
                    x += 1;
                    y += 1;
                }
                while x < xe && y < ye && self.equal(xe - 1, ye - 1)? {
                    xe -= 1;
                    ye -= 1;
                }
                if x == xe {
                    for p in y..ye {
                        self.inserted[p as usize] = true;
                        if self.expired() {
                            return Err(());
                        }
                    }
                } else if y == ye {
                    for p in x..xe {
                        self.deleted[p as usize] = true;
                        if self.expired() {
                            return Err(());
                        }
                    }
                } else {
                    let (xm, ym, low, high) = self.partition(x, xe, y, ye, minimal)?;
                    pending.push((xm, xe, ym, ye, high));
                    pending.push((x, xm, y, ym, low));
                }
                Ok(())
            })();
            if result.is_err() {
                return false;
            }
        }
        true
    }

    fn partition(
        &mut self,
        x: isize,
        xe: isize,
        y: isize,
        ye: isize,
        minimal: bool,
    ) -> Result<(isize, isize, bool, bool), ()> {
        let (dmin, dmax, fmid, bmid) = (x - ye, xe - y, x - y, xe - ye);
        let (mut fmin, mut fmax, mut bmin, mut bmax) = (fmid, fmid, bmid, bmid);
        let offset = self.offset;
        let index = |d: isize| (d + offset) as usize;
        let odd = (fmid - bmid) & 1 != 0;
        self.forward[index(fmid)] = x;
        self.backward[index(bmid)] = xe;
        for cost in 1.. {
            let mut big_snake = false;
            if fmin > dmin {
                fmin -= 1;
                self.forward[index(fmin - 1)] = -1;
            } else {
                fmin += 1;
            }
            if fmax < dmax {
                fmax += 1;
                self.forward[index(fmax + 1)] = -1;
            } else {
                fmax -= 1;
            }
            let mut d = fmax;
            while d >= fmin {
                let (lo, hi) = (self.forward[index(d - 1)], self.forward[index(d + 1)]);
                let initial = if lo < hi { hi } else { lo + 1 };
                let (mut a, mut b) = (initial, initial - d);
                while a < xe && b < ye && self.equal(a, b)? {
                    a += 1;
                    b += 1;
                }
                big_snake |= a - initial > 20;
                self.forward[index(d)] = a;
                if odd && bmin <= d && d <= bmax && self.backward[index(d)] <= a {
                    return Ok((a, b, true, true));
                }
                d -= 2;
            }
            if bmin > dmin {
                bmin -= 1;
                self.backward[index(bmin - 1)] = isize::MAX;
            } else {
                bmin += 1;
            }
            if bmax < dmax {
                bmax += 1;
                self.backward[index(bmax + 1)] = isize::MAX;
            } else {
                bmax -= 1;
            }
            let mut d = bmax;
            while d >= bmin {
                let (lo, hi) = (self.backward[index(d - 1)], self.backward[index(d + 1)]);
                let initial = if lo < hi { lo } else { hi - 1 };
                let (mut a, mut b) = (initial, initial - d);
                while x < a && y < b && self.equal(a - 1, b - 1)? {
                    a -= 1;
                    b -= 1;
                }
                big_snake |= initial - a > 20;
                self.backward[index(d)] = a;
                if !odd && fmin <= d && d <= fmax && a <= self.forward[index(d)] {
                    return Ok((a, b, true, true));
                }
                d -= 2;
            }
            if minimal {
                continue;
            }
            if cost > 200 && big_snake {
                for forward in [true, false] {
                    let mut best = 0;
                    let mut midpoint = (0, 0);
                    let (mut d, limit, mid) = if forward {
                        (fmax, fmin, fmid)
                    } else {
                        (bmax, bmin, bmid)
                    };
                    while d >= limit {
                        let a = if forward {
                            self.forward[index(d)]
                        } else {
                            self.backward[index(d)]
                        };
                        let b = a - d;
                        let dd = d - mid;
                        let progress = if forward {
                            (a - x) * 2 - dd
                        } else {
                            (xe - a) * 2 + dd
                        };
                        let inside = if forward {
                            x + 20 <= a && a < xe && y + 20 <= b && b < ye
                        } else {
                            x < a && a <= xe - 20 && y < b && b <= ye - 20
                        };
                        if progress > best && progress > 12 * (cost + dd.abs()) && inside {
                            let mut snake = true;
                            for k in 0..20 {
                                let shift = if forward { -1 - k } else { k };
                                if !self.equal(a + shift, b + shift)? {
                                    snake = false;
                                    break;
                                }
                            }
                            if snake {
                                best = progress;
                                midpoint = (a, b);
                            }
                        }
                        d -= 2;
                    }
                    if best > 0 {
                        return Ok((midpoint.0, midpoint.1, forward, !forward));
                    }
                }
            }
            if cost >= self.max_cost {
                let (mut fsum, mut fx, mut bsum, mut bx) = (-1, 0, isize::MAX, 0);
                let mut d = fmax;
                while d >= fmin {
                    let mut a = self.forward[index(d)].min(xe);
                    let mut b = a - d;
                    if b > ye {
                        a = ye + d;
                        b = ye;
                    }
                    if fsum < a + b {
                        fsum = a + b;
                        fx = a;
                    }
                    d -= 2;
                }
                let mut d = bmax;
                while d >= bmin {
                    let mut a = self.backward[index(d)].max(x);
                    let mut b = a - d;
                    if b < y {
                        a = y + d;
                        b = y;
                    }
                    if a + b < bsum {
                        bsum = a + b;
                        bx = a;
                    }
                    d -= 2;
                }
                return if (xe + ye) - bsum < fsum - (x + y) {
                    Ok((fx, fsum - fx, true, false))
                } else {
                    Ok((bx, bsum - bx, false, true))
                };
            }
        }
        unreachable!()
    }
}

#[cfg(test)]
mod wide_diff_tests {
    use super::*;
    use crate::lisp::text::EmacsChar;

    #[test]
    fn distinct_private_use_and_extended_codes_are_not_equal() {
        let old: Vec<_> = [0xf0080, 0xd800, 0x1a0000, 65].into_iter()
            .map(|code| EmacsChar::new(code).unwrap()).collect();
        let new: Vec<_> = [0x3fff80, 0xd800, 0x1a0000, 66].into_iter()
            .map(|code| EmacsChar::new(code).unwrap()).collect();
        let mut diff = Diff::new(&old, &new, 100, None);
        assert!(diff.compare());
        assert_eq!(diff.deleted, vec![true, false, false, true]);
        assert_eq!(diff.inserted, vec![true, false, false, true]);
    }
}
