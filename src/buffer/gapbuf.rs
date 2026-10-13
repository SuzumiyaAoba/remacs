//! Gap buffer: Emacs's classic text storage. Contiguous text with a
//! movable gap so insert/delete near point is O(gap move + insert size).

use std::cell::{RefCell, RefMut};
use std::rc::Rc;

/// A buffer of `char`s with a gap at the editing position.
#[derive(Clone)]
pub struct GapBuffer {
    buf: Vec<char>,
    gap_start: usize,
    gap_end: usize, // exclusive
    /// Bumped by every content mutation; the newline cache below is
    /// valid only while it matches.  (`as_slice_mut' callers must not
    /// write — they don't today; mutating there would bypass this.)
    mod_count: u64,
    /// Lazily-built sorted newline positions with the mod_count they
    /// were computed for.  Line ops used to scan the whole text every
    /// call, which made any per-line Lisp loop quadratic (Dired's
    /// insert-set-properties took minutes on large listings).
    lines: RefCell<Option<(u64, Rc<Vec<usize>>)>>,
}

// GNU `GAP_INIT' (buffer.c): a fresh buffer starts with a 20-byte
// gap — here 20 chars (our gap is char-indexed).
const GAP_INIT: usize = 20;

impl GapBuffer {
    pub fn new() -> Self {
        GapBuffer {
            buf: vec![0 as char; GAP_INIT],
            gap_start: 0,
            gap_end: GAP_INIT,
            mod_count: 0,
            lines: RefCell::new(None),
        }
    }

    /// Total character count (text length).
    pub fn len(&self) -> usize {
        self.buf.len() - (self.gap_end - self.gap_start)
    }

    /// Char at logical index `i` (0-based).
    pub fn char_at(&self, i: usize) -> char {
        debug_assert!(i < self.len());
        if i < self.gap_start {
            self.buf[i]
        } else {
            self.buf[i + (self.gap_end - self.gap_start)]
        }
    }

    pub fn gap_size(&self) -> usize {
        self.gap_end - self.gap_start
    }

    /// The gap's leading edge in logical chars (GNU `GPT').
    pub fn gap_position(&self) -> usize {
        self.gap_start
    }

    /// Move the gap so it starts at logical position `pos`.
    fn move_gap(&mut self, pos: usize) {
        let pos = pos.min(self.len());
        if pos == self.gap_start {
            return;
        }
        let gsize = self.gap_size();
        if pos < self.gap_start {
            // Shift text right into the gap.
            let n = self.gap_start - pos;
            self.buf.copy_within(pos..self.gap_start, self.gap_end - n);
            self.gap_start = pos;
            self.gap_end -= n;
        } else {
            // Shift text left into the gap.
            let n = pos - self.gap_start;
            self.buf
                .copy_within(self.gap_end..self.gap_end + n, self.gap_start);
            self.gap_start = pos;
            self.gap_end += n;
        }
        let _ = gsize;
    }

    /// Grow the gap so at least `need` chars fit, GNU make_gap_larger
    /// style: grow by `GAP_BYTES_DFL' (2000) minimum so a fresh
    /// 20-char gap reports (gap-size) => 2000 after any insert.
    fn ensure_gap(&mut self, need: usize) {
        if self.gap_size() >= need {
            return;
        }
        const GAP_BYTES_DFL: usize = 2000;
        let grow = need.max(GAP_BYTES_DFL);
        let old_len = self.buf.len();
        self.buf.resize(old_len + grow, 0 as char);
        // Move post-gap text to the new end.
        self.buf
            .copy_within(self.gap_end..old_len, self.gap_end + grow);
        self.gap_end += grow;
    }

    /// The logical text as one contiguous slice: moves the gap to the
    /// end first (O(tail) once; free on repeat calls while unedited).
    /// Used by scan/regexp paths that need random `&[char]` access
    /// without allocating a `Vec<char>` copy per call.
    pub fn as_slice(&mut self) -> &[char] {
        self.as_slice_mut()
    }

    /// Mutable variant used to project a `RefCell<GapBuffer>` borrow
    /// into `&mut [char]` (`SharedText::as_slice`).
    fn as_slice_mut(&mut self) -> &mut [char] {
        self.move_gap(self.len());
        &mut self.buf[..self.gap_start]
    }

    /// Insert `text` at logical position `pos`.
    pub fn insert(&mut self, pos: usize, text: &str) {
        self.mod_count = self.mod_count.wrapping_add(1);
        self.move_gap(pos);
        let n = text.chars().count();
        self.ensure_gap(n);
        for c in text.chars() {
            self.buf[self.gap_start] = c;
            self.gap_start += 1;
        }
    }

    /// Delete `[start, end)`.
    pub fn delete(&mut self, start: usize, end: usize) {
        let end = end.min(self.len());
        if start >= end {
            return;
        }
        self.mod_count = self.mod_count.wrapping_add(1);
        self.move_gap(start);
        self.gap_end = (self.gap_end + (end - start)).min(self.buf.len());
    }

    /// Overwrite the char at logical index `i` in place. Unlike
    /// delete+insert this does not move the gap, so markers and
    /// point are undisturbed (GNU's `subst-char-in-region` semantics).
    pub fn set_char_at(&mut self, i: usize, c: char) {
        debug_assert!(i < self.len());
        self.mod_count = self.mod_count.wrapping_add(1);
        if i < self.gap_start {
            self.buf[i] = c;
        } else {
            self.buf[i + (self.gap_end - self.gap_start)] = c;
        }
    }

    /// Extract `[start, end)` as a String.
    pub fn substring(&self, start: usize, end: usize) -> String {
        let end = end.min(self.len());
        let start = start.min(end);
        let mut s = String::with_capacity(end - start);
        for i in start..end {
            s.push(self.char_at(i));
        }
        s
    }

    /// Whole contents.
    pub fn text(&self) -> String {
        self.substring(0, self.len())
    }

    /// Replace whole contents.
    pub fn set_text(&mut self, s: &str) {
        self.buf.clear();
        self.buf
            .resize(GAP_INIT.max(s.chars().count() * 2), 0 as char);
        self.gap_start = 0;
        self.gap_end = self.buf.len();
        self.insert(0, s);
    }

    /// Sorted char positions of every `\n', built once per mod_count
    /// and cached — callers hold `&self`, so the list lives behind a
    /// RefCell and is returned as an `Rc` snapshot.
    fn newline_index(&self) -> Rc<Vec<usize>> {
        if let Some((mc, v)) = &*self.lines.borrow() {
            if *mc == self.mod_count {
                return v.clone();
            }
        }
        let mut v = Vec::new();
        for i in 0..self.len() {
            if self.char_at(i) == '\n' {
                v.push(i);
            }
        }
        let v = Rc::new(v);
        *self.lines.borrow_mut() = Some((self.mod_count, v.clone()));
        v
    }

    /// Which line `pos` is on (0-based): the count of newlines
    /// strictly before `pos`.
    pub fn line_of_pos(&self, pos: usize) -> usize {
        self.newline_index()
            .partition_point(|&n| n < pos.min(self.len()))
    }

    /// Start position of line `line` (0-based). Past-the-end returns len.
    pub fn line_start(&self, line: usize) -> usize {
        if line == 0 {
            return 0;
        }
        let idx = self.newline_index();
        match idx.get(line - 1) {
            Some(&n) => n + 1,
            None => self.len(),
        }
    }

    /// Char position of end of the line containing `pos` (index of the
    /// newline, or len).
    pub fn line_end(&self, pos: usize) -> usize {
        let idx = self.newline_index();
        let k = idx.partition_point(|&n| n < pos.min(self.len()));
        idx.get(k).copied().unwrap_or_else(|| self.len())
    }
}

impl Default for GapBuffer {
    fn default() -> Self {
        Self::new()
    }
}

/// Buffer text shared between a base buffer and its indirect buffers.
/// `clone()` shares the same `GapBuffer`; `deep_clone()` copies it.
/// Methods forward through the inner `RefCell`, so callers see the same
/// API surface as `GapBuffer` — mutations become visible to every
/// buffer holding a clone.
///
/// GNU `struct buffer_text' also owns the interval tree and the MODIFF
/// counters, so those live here too: `props' is the shared
/// text-property list, `mod_tick'/`chars_mod_tick' are the shared
/// modification ticks (edits through any view bump all views' ticks).
#[derive(Clone)]
pub struct SharedText {
    inner: Rc<RefCell<GapBuffer>>,
    /// Buffers viewing this text (self-registered at Buffer::new /
    /// make-indirect-buffer). GNU keeps the marker chain + BEGV/ZV
    /// inside the shared `buffer_text'; viewers let edits seen through
    /// one buffer re-bound the rest.
    viewers: Rc<RefCell<Vec<std::rc::Weak<RefCell<crate::buffer::Buffer>>>>>,
    /// GNU `b->text->intervals': text properties belong to the shared
    /// text, not to the viewing buffer.
    pub props: Rc<RefCell<Vec<crate::buffer::TextProp>>>,
    /// GNU `b->text->modiff'.
    pub mod_tick: Rc<std::cell::Cell<u64>>,
    /// GNU `b->text->chars_modiff'.
    pub chars_mod_tick: Rc<std::cell::Cell<u64>>,
    /// GNU `b->text->save_modiff': the modified flag is shared —
    /// `set-buffer-modified-p' on any view affects them all.
    pub save_tick: Rc<std::cell::Cell<u64>>,
}

impl SharedText {
    pub fn new() -> Self {
        SharedText {
            inner: Rc::new(RefCell::new(GapBuffer::new())),
            viewers: Rc::new(RefCell::new(Vec::new())),
            props: Rc::new(RefCell::new(Vec::new())),
            // Creation counts as the first modification (Emacs's fresh
            // buffers report buffer-modified-tick = 1).
            mod_tick: Rc::new(std::cell::Cell::new(1)),
            chars_mod_tick: Rc::new(std::cell::Cell::new(1)),
            save_tick: Rc::new(std::cell::Cell::new(1)),
        }
    }

    /// An independent copy of the text (used by `clone-buffer', whose
    /// copy does not share edits with the original).
    pub fn deep_clone(&self) -> Self {
        SharedText {
            inner: Rc::new(RefCell::new(self.inner.borrow().clone())),
            viewers: Rc::new(RefCell::new(Vec::new())),
            props: Rc::new(RefCell::new(self.props.borrow().clone())),
            mod_tick: Rc::new(std::cell::Cell::new(self.mod_tick.get())),
            chars_mod_tick: Rc::new(std::cell::Cell::new(self.chars_mod_tick.get())),
            save_tick: Rc::new(std::cell::Cell::new(self.save_tick.get())),
        }
    }

    /// Register `b` as a viewer of this text (called when a buffer
    /// adopts it — make-indirect-buffer, clone-indirect-buffer,
    /// buffer-swap-text).  No-op if `b` is already a viewer.
    pub fn add_viewer(&self, b: &Rc<RefCell<crate::buffer::Buffer>>) {
        let mut viewers = self.viewers.borrow_mut();
        viewers.retain(|w| w.upgrade().is_some());
        if viewers
            .iter()
            .any(|w| w.upgrade().is_some_and(|u| Rc::ptr_eq(&u, b)))
        {
            return;
        }
        viewers.push(Rc::downgrade(b));
    }

    /// Drop `b` from the viewer list (`buffer-swap-text' migrates
    /// registrations along with the text storage).
    pub fn remove_viewer(&self, b: &Rc<RefCell<crate::buffer::Buffer>>) {
        self.viewers
            .borrow_mut()
            .retain(|w| w.upgrade().is_some_and(|u| !Rc::ptr_eq(&u, b)));
    }

    /// `f' receives each OTHER live buffer sharing this text.  `self_id'
    /// is the editing buffer's id; buffers that can't be mutably
    /// borrowed right now (the editor holds its own `borrow_mut' across
    /// the text mutation) are skipped rather than panic.
    pub fn each_sibling(&self, self_id: usize, mut f: impl FnMut(&mut crate::buffer::Buffer)) {
        let mut viewers = self.viewers.borrow_mut();
        viewers.retain(|w| w.upgrade().is_some());
        let ups: Vec<Rc<RefCell<crate::buffer::Buffer>>> =
            viewers.iter().filter_map(|w| w.upgrade()).collect();
        drop(viewers);
        for b in ups {
            let Ok(mut sib) = b.try_borrow_mut() else {
                continue;
            };
            if sib.id == self_id {
                continue;
            }
            f(&mut sib);
        }
    }

    /// Total character count (text length).
    pub fn len(&self) -> usize {
        self.inner.borrow().len()
    }

    /// The unused gap between `gap_start' and `gap_end' (GNU
    /// `gap-size'; grows only — deletions don't shrink it).
    pub fn gap_size(&self) -> usize {
        self.inner.borrow().gap_size()
    }

    /// The gap's leading edge in logical chars (GNU `GPT').
    pub fn gap_position(&self) -> usize {
        self.inner.borrow().gap_position()
    }
    /// Char at logical index `i` (0-based).
    pub fn char_at(&self, i: usize) -> char {
        self.inner.borrow().char_at(i)
    }

    /// The logical text as one contiguous slice (see `GapBuffer::as_slice`).
    /// Returns a mapped mutable borrow; do not touch this `SharedText`
    /// again while the result is alive.
    pub fn as_slice(&self) -> RefMut<'_, [char]> {
        RefMut::map(self.inner.borrow_mut(), |g| g.as_slice_mut())
    }

    /// GNU `adjust_intervals_for_insert': an insert at POS splits a
    /// covering interval (the new text is unpropertized); intervals
    /// starting at POS shift right wholesale.
    fn adjust_props_insert(&self, pos: usize, n: usize) {
        let mut props = self.props.borrow_mut();
        if props.is_empty() {
            return;
        }
        let mut extra: Vec<crate::buffer::TextProp> = Vec::new();
        for tp in props.iter_mut() {
            if tp.end <= pos {
                continue;
            }
            if tp.start >= pos {
                tp.start += n;
                tp.end += n;
            } else {
                // pos strictly inside [start, end): split.
                extra.push(crate::buffer::TextProp {
                    start: pos + n,
                    end: tp.end + n,
                    prop: tp.prop,
                    value: tp.value.clone(),
                });
                tp.end = pos;
            }
        }
        props.extend(extra);
    }

    /// GNU `adjust_intervals_for_delete': boundaries inside the deleted
    /// span clamp to START, those past END shift down; an interval left
    /// empty is dropped.
    fn adjust_props_delete(&self, start: usize, end: usize) {
        let mut props = self.props.borrow_mut();
        if props.is_empty() {
            return;
        }
        let n = end - start;
        for tp in props.iter_mut() {
            if tp.start >= end {
                tp.start -= n;
            } else if tp.start > start {
                tp.start = start;
            }
            if tp.end >= end {
                tp.end -= n;
            } else if tp.end > start {
                tp.end = start;
            }
        }
        props.retain(|tp| tp.start < tp.end);
    }

    /// Insert `text` at logical position `pos`.
    pub fn insert(&self, pos: usize, text: &str) {
        let n = text.chars().count();
        self.inner.borrow_mut().insert(pos, text);
        // Props live in the shared text (GNU buffer_text intervals):
        // adjust once here so every viewer sees the same shift.
        self.adjust_props_insert(pos, n);
    }

    /// Delete `[start, end)`.
    pub fn delete(&self, start: usize, end: usize) {
        self.adjust_props_delete(start, end);
        self.inner.borrow_mut().delete(start, end)
    }

    /// Overwrite the char at logical index `i` in place.
    pub fn set_char_at(&self, i: usize, c: char) {
        self.inner.borrow_mut().set_char_at(i, c)
    }

    /// Extract `[start, end)` as a String.
    pub fn substring(&self, start: usize, end: usize) -> String {
        self.inner.borrow().substring(start, end)
    }

    /// Whole contents.
    pub fn text(&self) -> String {
        self.inner.borrow().text()
    }

    /// Replace whole contents.
    pub fn set_text(&self, s: &str) {
        self.inner.borrow_mut().set_text(s)
    }

    /// Which line `pos` is on (0-based).
    pub fn line_of_pos(&self, pos: usize) -> usize {
        self.inner.borrow().line_of_pos(pos)
    }

    /// Start position of line `line` (0-based). Past-the-end returns len.
    pub fn line_start(&self, line: usize) -> usize {
        self.inner.borrow().line_start(line)
    }

    /// Char position of end of the line containing `pos`.
    pub fn line_end(&self, pos: usize) -> usize {
        self.inner.borrow().line_end(pos)
    }
}

impl Default for SharedText {
    fn default() -> Self {
        Self::new()
    }
}
