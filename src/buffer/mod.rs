//! Buffer object model: text, point, mark, narrowing, buffer-local
//! variables, markers, undo records — plus the registry (`BufferSet`)
//! and all buffer primitives installed into the interpreter.

pub mod extra;
mod gapbuf;
pub mod primitives;

pub use primitives::install_primitives;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use crate::lisp::value::{BufferRef, Marker, SymId, Value};

pub use gapbuf::GapBuffer;

/// One editor buffer.
pub struct Buffer {
    /// Unique id in the BufferSet.
    pub id: usize,
    pub name: String,
    pub text: GapBuffer,
    /// Point (0-based char offset). Emacs `point` = `self.point + 1`.
    pub point: usize,
    /// The mark (0-based), if set.
    pub mark: Option<usize>,
    pub mark_active: bool,
    /// Narrowing bounds (0-based, inclusive-exclusive).
    pub begv: usize,
    pub zv: usize,
    /// Buffer-local variable bindings.
    pub locals: HashMap<SymId, Value>,
    /// Markers pointing into this buffer.
    pub markers: Vec<Weak<RefCell<Marker>>>,
    /// Visited file name (or nil).
    pub file_name: Option<String>,
    /// Modified since last save.
    pub modified: bool,
    /// Undo records.
    pub undo: Vec<UndoEntry>,
    /// Inhibit undo recording.
    pub undo_enabled: bool,
    /// Time of last modification (for tick tracking).
    pub mod_tick: u64,
    /// Text property intervals (start, end, property, value).
    /// Simplified model: a linear list, last write wins.
    pub text_props: Vec<TextProp>,
    /// Read-only regions aren't modeled; `read-only` text prop is checked
    /// at edit time in primitives.
    pub overlays: Vec<Overlay>,
    /// False once the buffer has been killed (the object may still be
    /// referenced by variables, markers, or window configurations).
    pub live: bool,
}

/// One text-property interval.
#[derive(Clone)]
pub struct TextProp {
    pub start: usize,
    pub end: usize,
    pub prop: SymId,
    pub value: Value,
}

/// A simplified overlay (start, end, plist).
#[derive(Clone)]
pub struct Overlay {
    pub start: usize,
    pub end: usize,
    pub plist: Value,
}

/// Undo record kinds (mirroring buffer-undo-list semantics loosely).
#[derive(Clone)]
pub enum UndoEntry {
    /// Text inserted at `start..end` — undo = delete.
    Insertion { start: usize, end: usize },
    /// Text deleted at `pos` — undo = reinsert.
    Deletion { pos: usize, text: String },
    /// Point was at `pos` before the command.
    Point(usize),
    /// Boundary marker (nil in the undo list).
    Boundary,
}

impl Buffer {
    pub fn new(id: usize, name: String) -> Buffer {
        let undo_enabled = !name.starts_with(' ');
        Buffer {
            id,
            name,
            text: GapBuffer::new(),
            point: 0,
            mark: None,
            mark_active: false,
            begv: 0,
            zv: 0,
            locals: HashMap::new(),
            markers: Vec::new(),
            file_name: None,
            modified: false,
            undo: Vec::new(),
            // Buffers with space-prefixed (internal) names start with
            // undo disabled, like GNU get-buffer-create.
            undo_enabled,
            // Creation counts as the first modification (Emacs's
            // fresh buffers report buffer-modified-tick = 1).
            mod_tick: 1,
            text_props: Vec::new(),
            overlays: Vec::new(),
            live: true,
        }
    }

    /// Total characters (ignores narrowing).
    pub fn size(&self) -> usize {
        self.text.len()
    }

    /// Effective text end honoring narrowing.
    pub fn text_len(&self) -> usize {
        self.zv.max(self.begv).min(self.size())
    }

    /// Clamp a 0-based position into `[begv, zv]`.
    pub fn clip(&self, pos: usize) -> usize {
        pos.max(self.begv).min(self.text_len())
    }

    /// `point` is stored 0-based.
    pub fn point(&self) -> usize {
        self.point.max(self.begv).min(self.text_len())
    }

    pub fn set_point(&mut self, pos: usize) {
        self.point = pos.max(self.begv).min(self.text_len());
    }

    /// Insert text at `pos`, adjusting point/mark/markers.
    /// `before_markers`: text goes before markers at pos (insert-before-markers).
    pub fn insert_at(&mut self, pos: usize, s: &str) {
        let pos = pos.min(self.text.len());
        let n = s.chars().count();
        if n == 0 {
            return;
        }
        if self.undo_enabled {
            self.undo.push(UndoEntry::Insertion {
                start: pos,
                end: pos + n,
            });
        }
        self.text.insert(pos, s);
        self.adjust_insert(pos, n, before_markers_flag(pos, self.point));
        self.modified = true;
        self.mod_tick += 1;
    }

    /// Like Emacs's `insert`: inserted text goes *before* point when
    /// inserting at point — i.e. point ends up after the new text.
    /// (Emacs `insert` inserts before point, advancing it.)
    pub fn insert(&mut self, s: &str) {
        let p = self.point();
        let n = s.chars().count();
        if self.undo_enabled {
            self.undo.push(UndoEntry::Insertion {
                start: p,
                end: p + n,
            });
        }
        self.text.insert(p, s);
        self.point = p + n;
        self.adjust_markers_insert(p, n, false);
        self.modified = true;
        self.mod_tick += 1;
    }

    /// `insert-before-markers`.
    pub fn insert_before_markers(&mut self, s: &str) {
        let p = self.point();
        let n = s.chars().count();
        if self.undo_enabled {
            self.undo.push(UndoEntry::Insertion {
                start: p,
                end: p + n,
            });
        }
        self.text.insert(p, s);
        self.point = p + n;
        self.adjust_markers_insert(p, n, true);
        self.modified = true;
        self.mod_tick += 1;
    }

    /// Point adjustment for `insert_at` (non-point-aware variant).
    fn adjust_insert(&mut self, pos: usize, n: usize, before: bool) {
        let _ = before;
        if self.point >= pos {
            self.point += n;
        }
        self.adjust_markers_insert(pos, n, false);
    }

    /// Move markers after inserting `n` chars at `pos`.
    /// `before_markers` moves markers at exactly `pos` too.
    pub fn adjust_markers_insert(&mut self, pos: usize, n: usize, before_markers: bool) {
        for w in &self.markers {
            if let Some(m) = w.upgrade() {
                let mut mm = m.borrow_mut();
                if mm.buffer == Some(self.id) {
                    if mm.position > pos
                        || (mm.position == pos && (before_markers || mm.insertion_type))
                    {
                        mm.position += n;
                    }
                }
            }
        }
        // The mark behaves like a marker with insertion_type = false.
        if let Some(m) = self.mark {
            if m > pos {
                self.mark = Some(m + n);
            }
        }
        // Widen narrowing bounds if needed.
        if self.zv >= pos {
            self.zv += n;
        }
    }

    /// Delete `[start, end)`, adjusting everything.
    pub fn delete_region(&mut self, start: usize, end: usize) -> String {
        let start = start.min(self.text.len());
        let end = end.min(self.text.len());
        if start >= end {
            return String::new();
        }
        let removed = self.text.substring(start, end);
        if self.undo_enabled {
            self.undo.push(UndoEntry::Deletion {
                pos: start,
                text: removed.clone(),
            });
        }
        self.text.delete(start, end);
        let n = end - start;
        // Point: Emacs clamps it into [start] if inside, shifts if after.
        if self.point >= end {
            self.point -= n;
        } else if self.point > start {
            self.point = start;
        }
        if let Some(m) = self.mark {
            if m >= end {
                self.mark = Some(m - n);
            } else if m > start {
                self.mark = Some(start);
            }
        }
        for w in &self.markers {
            if let Some(m) = w.upgrade() {
                let mut mm = m.borrow_mut();
                if mm.buffer == Some(self.id) {
                    if mm.position >= end {
                        mm.position -= n;
                    } else if mm.position > start {
                        mm.position = start;
                    }
                }
            }
        }
        if self.zv >= end {
            self.zv -= n;
        } else if self.zv > start {
            self.zv = start;
        }
        if self.begv >= end {
            self.begv -= n;
        } else if self.begv > start {
            self.begv = start;
        }
        self.modified = true;
        self.mod_tick += 1;
        removed
    }

    pub fn register_marker(&mut self, m: &Rc<RefCell<Marker>>) {
        self.markers.push(Rc::downgrade(m));
    }

    /// Drop dead marker entries occasionally.
    pub fn sweep_markers(&mut self) {
        self.markers.retain(|w| {
            w.upgrade()
                .map(|m| m.borrow().buffer == Some(self.id))
                .unwrap_or(false)
        });
    }
}

fn before_markers_flag(_pos: usize, _point: usize) -> bool {
    false
}

/// Registry of live buffers: id -> BufferRef, plus name lookup and
/// buffer-list ordering (most recently used first).
pub struct BufferSet {
    bufs: Vec<Option<BufferRef>>,
    /// Buffer ids in LRU order (index 0 = most recent).
    order: Vec<usize>,
    name_map: HashMap<String, usize>,
    counter: u64,
}

impl BufferSet {
    pub fn new() -> Self {
        BufferSet {
            bufs: Vec::new(),
            order: Vec::new(),
            name_map: HashMap::new(),
            counter: 0,
        }
    }

    /// Create a buffer; if the name exists, uniquify with `<N>`.
    pub fn create(&mut self, name: &str) -> usize {
        let mut final_name = name.to_string();
        let mut n = 2;
        while self.name_map.contains_key(&final_name) {
            final_name = format!("{}<{}>", name, n);
            n += 1;
        }
        let id = self.bufs.len();
        let buf = Buffer::new(id, final_name.clone());
        self.bufs.push(Some(Rc::new(RefCell::new(buf))));
        self.name_map.insert(final_name, id);
        self.order.push(id);
        self.counter += 1;
        id
    }

    /// Create a buffer with exactly this name (kills none; used at init).
    pub fn create_exact(&mut self, name: &str) -> usize {
        let id = self.bufs.len();
        let buf = Buffer::new(id, name.to_string());
        self.bufs.push(Some(Rc::new(RefCell::new(buf))));
        self.name_map.insert(name.to_string(), id);
        self.order.push(id);
        id
    }

    pub fn get(&self, id: usize) -> Option<BufferRef> {
        self.bufs.get(id).and_then(|o| o.as_ref()).cloned()
    }

    pub fn by_name(&self, name: &str) -> Option<usize> {
        self.name_map.get(name).copied()
    }

    /// Kill buffer `id`.
    pub fn kill(&mut self, id: usize) -> bool {
        if id >= self.bufs.len() || self.bufs[id].is_none() {
            return false;
        }
        {
            let mut bb = self.bufs[id].as_ref().unwrap().borrow_mut();
            bb.live = false;
            // Emacs unchains markers on kill: they point nowhere.
            for w in &bb.markers {
                if let Some(m) = w.upgrade() {
                    m.borrow_mut().buffer = None;
                }
            }
            bb.markers.clear();
        }
        let name = self.bufs[id].as_ref().unwrap().borrow().name.clone();
        self.name_map.remove(&name);
        self.bufs[id] = None;
        self.order.retain(|&x| x != id);
        true
    }

    /// Rename buffer `id`; returns the actual (possibly uniquified) name.
    pub fn rename(&mut self, id: usize, new_name: &str) -> String {
        let old_name = match self.get(id) {
            Some(b) => b.borrow().name.clone(),
            None => return String::new(),
        };
        self.name_map.remove(&old_name);
        let mut final_name = new_name.to_string();
        let mut n = 2;
        while self.name_map.contains_key(&final_name) {
            final_name = format!("{}<{}>", new_name, n);
            n += 1;
        }
        if let Some(b) = self.get(id) {
            b.borrow_mut().name = final_name.clone();
        }
        self.name_map.insert(final_name.clone(), id);
        final_name
    }

    /// Move `id` to the front of the LRU order.
    pub fn touch(&mut self, id: usize) {
        self.order.retain(|&x| x != id);
        self.order.insert(0, id);
    }

    /// `bury-buffer`: move `id` to the end of the order.
    pub fn bury(&mut self, id: usize) {
        self.order.retain(|&x| x != id);
        self.order.push(id);
    }

    /// All live buffers in buffer-list order.
    pub fn list(&self) -> Vec<usize> {
        self.order
            .iter()
            .copied()
            .filter(|&id| self.get(id).is_some())
            .collect()
    }

    /// The next non-internal buffer in the order (for
    /// `other-buffer`; Emacs skips buffers with space-prefixed
    /// names).
    pub fn other(&self, exclude: usize) -> Option<usize> {
        self.order.iter().copied().find(|&id| {
            id != exclude
                && self
                    .get(id)
                    .map(|b| !b.borrow().name.starts_with(' '))
                    .unwrap_or(false)
        })
    }

    /// Generate a unique buffer name based on `base`.
    pub fn unique_name(&self, base: &str) -> String {
        if !self.name_map.contains_key(base) {
            return base.to_string();
        }
        for n in 2.. {
            let cand = format!("{}<{}>", base, n);
            if !self.name_map.contains_key(&cand) {
                return cand;
            }
        }
        unreachable!()
    }

    pub fn len(&self) -> usize {
        self.order.len()
    }
}

impl Default for BufferSet {
    fn default() -> Self {
        Self::new()
    }
}
