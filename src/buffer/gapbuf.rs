//! Gap buffer: Emacs's classic text storage. Contiguous text with a
//! movable gap so insert/delete near point is O(gap move + insert size).

use std::cell::{RefCell, RefMut};
use std::rc::{Rc, Weak};

#[derive(Clone, Copy)]
pub enum TextChange<'a> {
    Insert { at: usize, text: &'a str },
    Delete { start: usize, end: usize },
    Character { at: usize, character: char },
    Reset { text: &'a str },
}

pub type EditObserver = Rc<RefCell<dyn for<'a> FnMut(TextChange<'a>)>>;
type WeakObserver = Weak<RefCell<dyn for<'a> FnMut(TextChange<'a>)>>;

/// Weak registration: text storage never owns a parser or its callbacks.
pub struct EditSubscription {
    source: Weak<RefCell<GapBuffer>>,
    observers: Weak<RefCell<Vec<WeakObserver>>>,
    observer: WeakObserver,
}

impl EditSubscription {
    pub fn matches(&self, text: &SharedText) -> bool {
        self.source.ptr_eq(&Rc::downgrade(&text.inner))
    }
}

impl Drop for EditSubscription {
    fn drop(&mut self) {
        if let Some(observers) = self.observers.upgrade() {
            observers
                .borrow_mut()
                .retain(|observer| !observer.ptr_eq(&self.observer));
        }
    }
}

/// A buffer of `char`s with a gap at the editing position.
#[derive(Clone)]
pub struct GapBuffer {
    buf: Vec<char>,
    gap_start: usize,
    gap_end: usize, // exclusive
    /// Live text's Emacs byte count. The vector length budgets byte
    /// capacity even though individual slots store decoded characters.
    text_bytes: usize,
    multibyte: bool,
    /// Contiguous read projection for a gap in the middle of the text.
    flat: Option<Vec<char>>,
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
// gap. Storage is char-indexed; allocation headroom is budgeted in bytes.
const GAP_INIT: usize = 20;

pub(super) fn storage_bytes(c: char, multibyte: bool) -> usize {
    if !multibyte {
        return 1;
    }
    match crate::lisp::value::lisp_char_code(c) {
        0..=0x7f => 1,
        0x80..=0x7ff | 0x3fff80..=0x3fffff => 2,
        0x800..=0xffff => 3,
        0x10000..=0x1fffff => 4,
        _ => 5,
    }
}

impl GapBuffer {
    fn from_dump(text: &str, gap: (usize, usize, u64, bool)) -> Result<Self, String> {
        let chars: Vec<_> = text.chars().collect();
        if gap.0 > chars.len() {
            return Err("Invalid dumped gap position".into());
        }
        let total = chars
            .len()
            .checked_add(gap.1)
            .ok_or("Dumped gap size overflow")?;
        let mut buf = Vec::new();
        buf.try_reserve_exact(total)
            .map_err(|_| "Cannot allocate dumped buffer gap")?;
        buf.extend_from_slice(&chars[..gap.0]);
        buf.resize(gap.0 + gap.1, '\0');
        buf.extend_from_slice(&chars[gap.0..]);
        let text_bytes = text.chars().map(|c| storage_bytes(c, gap.3)).sum();
        if text_bytes > total {
            return Err("Invalid dumped byte capacity".into());
        }
        Ok(Self {
            buf,
            gap_start: gap.0,
            gap_end: gap.0 + gap.1,
            text_bytes,
            multibyte: gap.3,
            flat: None,
            mod_count: gap.2,
            lines: RefCell::new(None),
        })
    }
    pub fn new() -> Self {
        GapBuffer {
            buf: vec![0 as char; GAP_INIT],
            gap_start: 0,
            gap_end: GAP_INIT,
            text_bytes: 0,
            multibyte: true,
            flat: None,
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
        self.buf.len() - self.text_bytes
    }

    fn gap_slots(&self) -> usize {
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
    }

    /// GNU make_gap/make_gap_larger: add the missing bytes (at least
    /// 1/64 of existing characters), then 2000 bytes of spare headroom.
    fn ensure_gap(&mut self, need: usize) {
        if self.gap_size() >= need {
            return;
        }
        const GAP_BYTES_DFL: usize = 2000;
        let grow = (need - self.gap_size()).max(self.len() / 64) + GAP_BYTES_DFL;
        let old_len = self.buf.len();
        self.buf.resize(old_len + grow, 0 as char);
        // Move post-gap text to the new end.
        self.buf
            .copy_within(self.gap_end..old_len, self.gap_end + grow);
        self.gap_end += grow;
    }

    /// The logical text as one contiguous slice without moving the gap.
    /// Used by scan/regexp paths that need random `&[char]` access
    /// without allocating a `Vec<char>` copy per call.
    pub fn as_slice(&mut self) -> &[char] {
        self.as_slice_mut()
    }

    /// Mutable variant used to project a `RefCell<GapBuffer>` borrow
    /// into `&mut [char]` (`SharedText::as_slice`).
    fn as_slice_mut(&mut self) -> &mut [char] {
        if self.gap_end == self.buf.len() {
            return &mut self.buf[..self.gap_start];
        }
        if self.gap_start == 0 {
            return &mut self.buf[self.gap_end..];
        }
        if self.flat.is_none() {
            let mut flat = Vec::with_capacity(self.len());
            flat.extend_from_slice(&self.buf[..self.gap_start]);
            flat.extend_from_slice(&self.buf[self.gap_end..]);
            self.flat = Some(flat);
        }
        self.flat.as_mut().unwrap().as_mut_slice()
    }

    /// Insert `text` at logical position `pos`.
    pub fn insert(&mut self, pos: usize, text: &str) {
        if text.is_empty() {
            return;
        }
        self.mod_count = self.mod_count.wrapping_add(1);
        self.flat = None;
        self.move_gap(pos);
        let bytes = text.chars().map(|c| storage_bytes(c, self.multibyte)).sum();
        self.ensure_gap(bytes);
        for c in text.chars() {
            self.buf[self.gap_start] = c;
            self.gap_start += 1;
        }
        self.text_bytes += bytes;
    }

    /// Delete `[start, end)`.
    pub fn delete(&mut self, start: usize, end: usize) {
        let end = end.min(self.len());
        if start >= end {
            return;
        }
        self.mod_count = self.mod_count.wrapping_add(1);
        self.flat = None;
        let bytes: usize = (start..end)
            .map(|k| storage_bytes(self.char_at(k), self.multibyte))
            .sum();
        self.move_gap(start);
        self.gap_end = (self.gap_end + (end - start)).min(self.buf.len());
        self.text_bytes -= bytes;
    }

    /// Overwrite the char at logical index `i` in place. Unlike
    /// delete+insert this does not move the gap, so markers and
    /// point are undisturbed (GNU's `subst-char-in-region` semantics).
    pub fn set_char_at(&mut self, i: usize, c: char) {
        debug_assert!(i < self.len());
        self.mod_count = self.mod_count.wrapping_add(1);
        self.flat = None;
        let before = storage_bytes(self.char_at(i), self.multibyte);
        let after = storage_bytes(c, self.multibyte);
        if after > before {
            self.ensure_gap(after - before);
        }
        if i < self.gap_start {
            self.buf[i] = c;
        } else {
            self.buf[i + (self.gap_end - self.gap_start)] = c;
        }
        self.text_bytes = self.text_bytes - before + after;
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
        self.delete(0, self.len());
        self.insert(0, s);
    }

    pub fn set_multibyte(&mut self, multibyte: bool) {
        if self.multibyte == multibyte {
            return;
        }
        let bytes = (0..self.len())
            .map(|k| storage_bytes(self.char_at(k), multibyte))
            .sum();
        if bytes > self.text_bytes {
            self.ensure_gap(bytes - self.text_bytes);
        }
        self.text_bytes = bytes;
        self.multibyte = multibyte;
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
    edit_observers: Rc<RefCell<Vec<WeakObserver>>>,
    /// Buffers viewing this text (self-registered at Buffer::new /
    /// make-indirect-buffer). GNU keeps the marker chain + BEGV/ZV
    /// inside the shared `buffer_text'; viewers let edits seen through
    /// one buffer re-bound the rest.
    viewers: Rc<RefCell<Vec<std::rc::Weak<RefCell<crate::buffer::Buffer>>>>>,
    /// GNU `b->text->intervals': text properties belong to the shared
    /// text, not to the viewing buffer.
    pub props: Rc<RefCell<crate::buffer::textprops::TextProperties>>,
    /// GNU `b->text->modiff'.
    pub mod_tick: Rc<std::cell::Cell<u64>>,
    /// GNU `b->text->chars_modiff'.
    pub chars_mod_tick: Rc<std::cell::Cell<u64>>,
    /// GNU `b->text->save_modiff': the modified flag is shared —
    /// `set-buffer-modified-p' on any view affects them all.
    pub save_tick: Rc<std::cell::Cell<u64>>,
    /// GNU `buffer_intervals (buf) != NULL': once any text property
    /// call has run the interval tree persists even if every property
    /// is later removed.  `verify_interval_modification' consults it.
    pub had_intervals: Rc<std::cell::Cell<bool>>,
}

impl SharedText {
    pub(crate) fn dump_identity(&self) -> usize {
        Rc::as_ptr(&self.inner) as usize
    }

    pub(crate) fn dump_gap(&self) -> (usize, usize, u64, bool) {
        let inner = self.inner.borrow();
        (
            inner.gap_start,
            inner.gap_slots(),
            inner.mod_count,
            inner.multibyte,
        )
    }

    pub(crate) fn from_dump(
        text: &str,
        intervals: Vec<(usize, usize, Vec<crate::lisp::Value>)>,
        ticks: [u64; 3],
        had_intervals: bool,
        gap: (usize, usize, u64, bool),
    ) -> Result<Self, String> {
        let shared = Self::new();
        *shared.inner.borrow_mut() = GapBuffer::from_dump(text, gap)?;
        *shared.props.borrow_mut() =
            crate::buffer::textprops::TextProperties::from_dump(text.chars().count(), intervals)?;
        shared.mod_tick.set(ticks[0]);
        shared.chars_mod_tick.set(ticks[1]);
        shared.save_tick.set(ticks[2]);
        shared.had_intervals.set(had_intervals);
        Ok(shared)
    }

    pub fn new() -> Self {
        SharedText {
            inner: Rc::new(RefCell::new(GapBuffer::new())),
            edit_observers: Rc::new(RefCell::new(Vec::new())),
            viewers: Rc::new(RefCell::new(Vec::new())),
            props: Rc::new(RefCell::new(crate::buffer::textprops::TextProperties::new())),
            // Creation counts as the first modification (Emacs's fresh
            // buffers report buffer-modified-tick = 1).
            mod_tick: Rc::new(std::cell::Cell::new(1)),
            chars_mod_tick: Rc::new(std::cell::Cell::new(1)),
            save_tick: Rc::new(std::cell::Cell::new(1)),
            had_intervals: Rc::new(std::cell::Cell::new(false)),
        }
    }

    /// An independent copy of the text (used by `clone-buffer', whose
    /// copy does not share edits with the original).
    pub fn deep_clone(&self) -> Self {
        SharedText {
            inner: Rc::new(RefCell::new(self.inner.borrow().clone())),
            edit_observers: Rc::new(RefCell::new(Vec::new())),
            viewers: Rc::new(RefCell::new(Vec::new())),
            props: Rc::new(RefCell::new(self.props.borrow().clone())),
            mod_tick: Rc::new(std::cell::Cell::new(self.mod_tick.get())),
            chars_mod_tick: Rc::new(std::cell::Cell::new(self.chars_mod_tick.get())),
            save_tick: Rc::new(std::cell::Cell::new(self.save_tick.get())),
            had_intervals: Rc::new(std::cell::Cell::new(self.had_intervals.get())),
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

    pub fn observe_edits(&self, observer: &EditObserver) -> EditSubscription {
        let observer = Rc::downgrade(observer);
        self.edit_observers.borrow_mut().push(observer.clone());
        EditSubscription {
            source: Rc::downgrade(&self.inner),
            observers: Rc::downgrade(&self.edit_observers),
            observer,
        }
    }

    fn notify_edit(&self, change: TextChange<'_>) {
        let mut observers = self.edit_observers.borrow_mut();
        observers.retain(|observer| observer.strong_count() != 0);
        let callbacks: Vec<_> = observers.iter().filter_map(Weak::upgrade).collect();
        drop(observers);
        // The text/property borrows have ended. These are low-level Rust
        // callbacks, not Lisp modification hooks, so inhibited hooks do
        // not hide edits from incremental parsers.
        for callback in callbacks {
            callback.borrow_mut()(change);
        }
    }

    /// Drop `b` from the viewer list (`buffer-swap-text' migrates
    /// registrations along with the text storage).
    pub fn remove_viewer(&self, b: &Rc<RefCell<crate::buffer::Buffer>>) {
        self.viewers
            .borrow_mut()
            .retain(|w| w.upgrade().is_some_and(|u| !Rc::ptr_eq(&u, b)));
    }

    /// Mutating handle to the property records — marks
    /// `had_intervals' so `verify_interval_modification' keeps its
    /// GNU `buffer_intervals != NULL' shape once any write happened.
    pub fn props_write(&self) -> std::cell::RefMut<'_, crate::buffer::textprops::TextProperties> {
        self.had_intervals.set(true);
        self.props.borrow_mut()
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

    /// Unused encoded-byte headroom (GNU GAP_SIZE), consumed by insertion
    /// and increased by deletion or allocation. Char slots are separate.
    pub fn gap_size(&self) -> usize {
        self.inner.borrow().gap_size()
    }

    pub fn set_multibyte(&self, multibyte: bool) {
        self.inner.borrow_mut().set_multibyte(multibyte);
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
        self.props.borrow_mut().insert(pos, n);
    }

    fn adjust_props_delete(&self, start: usize, end: usize) {
        self.props.borrow_mut().delete(start, end);
    }

    /// Insert `text` at logical position `pos`.
    pub fn insert(&self, pos: usize, text: &str) {
        let pos = pos.min(self.len());
        let n = text.chars().count();
        self.inner.borrow_mut().insert(pos, text);
        // Props live in the shared text (GNU buffer_text intervals):
        // adjust once here so every viewer sees the same shift.
        self.adjust_props_insert(pos, n);
        if n != 0 {
            self.notify_edit(TextChange::Insert { at: pos, text });
        }
    }

    /// Delete `[start, end)`.
    pub fn delete(&self, start: usize, end: usize) {
        let end = end.min(self.len());
        self.adjust_props_delete(start, end);
        self.inner.borrow_mut().delete(start, end);
        if start < end {
            self.notify_edit(TextChange::Delete { start, end });
        }
    }

    /// Overwrite the char at logical index `i` in place.
    pub fn set_char_at(&self, i: usize, c: char) {
        self.inner.borrow_mut().set_char_at(i, c);
        self.notify_edit(TextChange::Character {
            at: i,
            character: c,
        });
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
        self.inner.borrow_mut().set_text(s);
        let mut props = crate::buffer::textprops::TextProperties::new();
        props.insert(0, s.chars().count());
        *self.props.borrow_mut() = props;
        self.notify_edit(TextChange::Reset { text: s });
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

#[cfg(test)]
mod tests {
    #[test]
    fn edit_observers_follow_shared_storage_without_owning_it() {
        let text = super::SharedText::new();
        let snapshots = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let output = snapshots.clone();
        let viewed = text.clone();
        let observer: super::EditObserver = std::rc::Rc::new(std::cell::RefCell::new(
            move |_change: super::TextChange<'_>| output.borrow_mut().push(viewed.text()),
        ));
        let subscription = text.observe_edits(&observer);
        assert!(subscription.matches(&text));
        text.insert(0, "ab");
        text.clone().set_char_at(1, 'x');
        text.delete(0, 1);
        text.set_text("[]");
        assert_eq!(&*snapshots.borrow(), &["ab", "ax", "x", "[]"]);
        let independent = text.deep_clone();
        assert!(!subscription.matches(&independent));
        independent.insert(0, "copy");
        assert_eq!(snapshots.borrow().len(), 4);
        drop(subscription);
        text.insert(0, "later");
        assert_eq!(snapshots.borrow().len(), 4);
    }

    #[test]
    fn abandoned_edit_observers_are_not_called() {
        let text = super::SharedText::new();
        let observer: super::EditObserver =
            std::rc::Rc::new(std::cell::RefCell::new(|_change: super::TextChange<'_>| {
                panic!("dropped observer called")
            }));
        let subscription = text.observe_edits(&observer);
        drop(observer);
        text.insert(0, "safe");
        drop(subscription);
        assert_eq!(text.text(), "safe");
    }

    use super::*;

    #[test]
    fn byte_budget_tracks_multibyte_insert_delete_and_snapshot() {
        let mut gap = GapBuffer::new();
        gap.insert(0, "日本語");
        assert_eq!((gap.len(), gap.gap_position(), gap.gap_size()), (3, 3, 11));
        gap.insert(1, "é");
        assert_eq!(gap.text(), "日é本語");
        assert_eq!(gap.gap_size(), 9);
        gap.delete(0, 2);
        assert_eq!(gap.text(), "本語");
        assert_eq!(gap.gap_size(), 14);
        let restored = GapBuffer::from_dump(
            &gap.text(),
            (gap.gap_start, gap.gap_slots(), gap.mod_count, gap.multibyte),
        )
        .unwrap();
        assert_eq!(
            (
                restored.text(),
                restored.gap_position(),
                restored.gap_size()
            ),
            ("本語".into(), 0, 14)
        );
    }

    #[test]
    fn read_projection_preserves_gap_and_invalidates_after_mutation() {
        let mut gap = GapBuffer::new();
        gap.insert(0, "abcdef");
        gap.insert(2, "X");
        assert_eq!(gap.as_slice().iter().collect::<String>(), "abXcdef");
        assert_eq!(gap.gap_position(), 3);
        gap.set_char_at(0, 'Z');
        assert_eq!(gap.as_slice().iter().collect::<String>(), "ZbXcdef");
        assert_eq!(gap.gap_position(), 3);
        gap.delete(1, 4);
        assert_eq!(gap.as_slice().iter().collect::<String>(), "Zdef");
        assert_eq!(gap.gap_position(), 1);
        let before = (gap.gap_position(), gap.gap_size());
        gap.insert(4, "");
        assert_eq!((gap.gap_position(), gap.gap_size()), before);
    }

    #[test]
    fn replacing_text_reuses_capacity_and_gap_growth_scales_with_existing_text() {
        let mut gap = GapBuffer::new();
        gap.insert(0, &"a".repeat(100_000));
        assert_eq!(gap.gap_size(), 2000);
        for _ in 0..2000 {
            gap.insert(gap.len(), "b");
        }
        assert_eq!(gap.gap_size(), 0);
        gap.insert(gap.len(), "c");
        assert_eq!(gap.gap_size(), 3592);
        gap.set_text("x");
        assert_eq!(gap.text(), "x");
        assert_eq!(gap.gap_size(), 105592);
    }
}
