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
    /// SymId of `buffer-undo-list'.  The undo list itself is the
    /// buffer-local variable binding (`locals[undo_sym]'), a Lisp list
    /// in GNU's format: (BEG . END) insertions, (TEXT . POS) deletions,
    /// integer point entries, (t . MODTIME) first-change records,
    /// (MARKER . ADJ) marker adjustments, and nil boundaries.
    pub undo_sym: SymId,
    /// Shared flag mirroring the global `undo-inhibit-record-point'
    /// variable's current dynamic value; the interpreter keeps it in
    /// sync so `record_point' can consult it without the evaluator.
    pub undo_inhibit: Rc<std::cell::Cell<bool>>,
    /// GNU `point_before_last_command_or_undo': point (1-based) at the
    /// last undo boundary/command, or None when this buffer was not
    /// current at that time (GNU's `buffer_before_last_command_or_undo').
    pub undo_pt_before: Option<usize>,
    /// Time of last modification (for tick tracking).
    pub mod_tick: u64,
    /// GNU CHARS_MODIFF: bumped only when the buffer *text* changes
    /// (`buffer-chars-modified-tick'); text-property changes bump
    /// `mod_tick' alone.
    pub chars_mod_tick: u64,
    /// GNU SAVE_MODIFF: `mod_tick' at the last save or
    /// `set-buffer-modified-p' nil.  When `mod_tick <= save_tick' the
    /// next recorded change pushes a `(t . MODTIME)' first-change entry.
    pub save_tick: u64,
    /// Text property intervals (start, end, property, value).
    /// Simplified model: a linear list, last write wins.
    pub text_props: Vec<TextProp>,
    /// Read-only regions aren't modeled; `read-only` text prop is checked
    /// at edit time in primitives.
    pub overlays: Vec<Overlay>,
    /// False once the buffer has been killed (the object may still be
    /// referenced by variables, markers, or window configurations).
    pub live: bool,
    /// For indirect buffers, the id of the base buffer whose text is
    /// shared. (Text itself is currently copied at creation rather than
    /// aliased — edit propagation is not yet modeled.)
    pub base_buffer: Option<usize>,
    /// Buffer-local syntax table, installed by `set-syntax-table'
    /// (GNU: a C-level buffer field, NOT a Lisp variable — plain
    /// `setq' on `syntax-table' does not affect the scanner).
    /// None means the standard syntax table.
    pub syntax_table: Option<crate::lisp::value::Value>,
    /// Buffer-local case table (`current-case-table'), or None for the
    /// standard table.
    pub case_table: Option<crate::lisp::value::Value>,
    /// Buffer-local category table (`category-table'), or None for the
    /// standard table.
    pub category_table: Option<crate::lisp::value::Value>,
    /// Stack of (previous begv, previous zv, label) pushed by
    /// `internal--labeled-narrow-to-region' so `internal--labeled-widen'
    /// can restore the bounds it replaced.
    pub narrow_labels: Vec<(usize, usize, crate::lisp::value::Value)>,
    /// Recorded visited-file modtime, in nanoseconds since the epoch.
    /// GNU encodes flags in the timespec's tv_nsec: -2 means "modtime
    /// unknown" (`visited-file-modtime' returns 0) and -1 means "the
    /// visited file did not exist" (returns -1).
    pub file_modtime_ns: i128,
    /// Recorded visited-file size, or -1 when unknown (GNU
    /// modtime_size); verified by `verify-visited-file-modtime'.
    pub file_modtime_size: i128,
    /// Name of the lock file (`.#FILE') created by `lock-buffer'.
    pub file_lock_name: Option<String>,
    /// Cached snapshot of the `create-lockfiles' Lisp variable, taken
    /// when the file is visited (Buffer methods cannot see Lisp state).
    pub create_lockfiles: bool,
    /// Mode remembered by `major-mode-suspend' (GNU records the local
    /// `major-mode' so `major-mode-restore' can re-run it after a
    /// suspend/undump).
    pub suspended_mode: Option<crate::lisp::value::Value>,
}

/// Resolve symlinks like GNU's `file-truename'.  When FILE doesn't
/// exist, canonicalize resolves nothing — resolve the longest
/// existing ancestor and reattach the missing tail instead.
pub(crate) fn file_truename(path: &str) -> String {
    let mut missing: Vec<&str> = Vec::new();
    let mut rest = path;
    loop {
        if let Ok(real) = std::fs::canonicalize(rest) {
            let mut out = real.to_string_lossy().into_owned();
            for c in missing.iter().rev() {
                if !out.ends_with('/') {
                    out.push('/');
                }
                out.push_str(c);
            }
            return out;
        }
        match rest.rfind('/') {
            Some(0) => {
                // Only "/" remains; it always canonicalizes, so the
                // loop above must have returned — unreachable.
                return path.to_string();
            }
            Some(pos) => {
                missing.push(&rest[pos + 1..]);
                rest = &rest[..pos];
            }
            None => return path.to_string(),
        }
    }
}

/// GNU file-lock path: `.#NAME' in FILE's directory.
pub(crate) fn lock_file_name(file: &str) -> String {
    match file.rfind('/') {
        Some(pos) => format!("{}/.#{}", &file[..pos], &file[pos + 1..]),
        None => format!(".#{}", file),
    }
}

/// Our hostname as GNU records it in lock files (`system-name').
pub(crate) fn our_host_name() -> String {
    let mut host = std::env::var("HOSTNAME").unwrap_or_default();
    if host.is_empty() {
        #[cfg(unix)]
        unsafe {
            let mut buf = [0i8; 256];
            if libc::gethostname(buf.as_mut_ptr(), buf.len()) == 0 {
                host = std::ffi::CStr::from_ptr(buf.as_ptr())
                    .to_string_lossy()
                    .into_owned();
            }
        }
    }
    if host.is_empty() {
        host = "localhost".into();
    }
    host
}

/// Seconds since the epoch of the last system boot, or 0 when it
/// cannot be determined (GNU filelock.c `get_boot_time').
fn boot_time() -> i64 {
    #[cfg(target_os = "macos")]
    unsafe {
        let name = std::ffi::CString::new("kern.boottime").unwrap();
        let mut tv: libc::timeval = std::mem::zeroed();
        let mut len = std::mem::size_of::<libc::timeval>();
        if libc::sysctlbyname(
            name.as_ptr(),
            &mut tv as *mut _ as *mut _,
            &mut len,
            std::ptr::null_mut(),
            0,
        ) == 0
        {
            return tv.tv_sec as i64;
        }
        0
    }
    #[cfg(target_os = "linux")]
    {
        if let Ok(stat) = std::fs::read_to_string("/proc/stat") {
            for line in stat.lines() {
                if let Some(n) = line.strip_prefix("btime ") {
                    return n.trim().parse().unwrap_or(0);
                }
            }
        }
        0
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        0
    }
}

/// GNU lock contents: `USER@HOST.PID:BOOT' (the `:BOOT' suffix is
/// appended when the boot time is known, e.g. on macOS).
pub(crate) fn lock_owner_string() -> String {
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .unwrap_or_else(|_| "unknown".into());
    let base = format!("{}@{}.{}", user, our_host_name(), std::process::id());
    match boot_time() {
        0 => base,
        bt => format!("{}:{}", base, bt),
    }
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
///
/// Entries are never removed from `Buffer::overlays' (handles hold
/// `(buffer-id, index)` pairs); `buffer' becomes `None' when the
/// overlay is deleted or moved to another buffer.
#[derive(Clone)]
pub struct Overlay {
    pub start: usize,
    pub end: usize,
    /// Buffer the overlay is attached to; `None' after `delete-overlay'
    /// or `kill-buffer' (GNU: `overlay-buffer' returns nil).
    pub buffer: Option<usize>,
    pub plist: Value,
    /// `front-advance': insertions exactly at `start' push it forward.
    pub front_advance: bool,
    /// `rear-advance': insertions exactly at `end' push it forward.
    pub rear_advance: bool,
    /// The canonical `[overlay BID IDX]' Lisp handle — reused by
    /// `overlays-at' & friends so `eq' holds for the same overlay.
    pub handle: Value,
}

impl Buffer {
    pub fn new(
        id: usize,
        name: String,
        undo_sym: SymId,
        undo_inhibit: Rc<std::cell::Cell<bool>>,
    ) -> Buffer {
        let mut locals = HashMap::new();
        // Buffers with space-prefixed (internal) names start with undo
        // disabled, like GNU get-buffer-create (undo_list = Qt).
        if name.starts_with(' ') {
            locals.insert(undo_sym, Value::t());
        }
        Buffer {
            id,
            name,
            text: GapBuffer::new(),
            point: 0,
            mark: None,
            begv: 0,
            zv: 0,
            locals,
            markers: Vec::new(),
            file_name: None,
            modified: false,
            undo_sym,
            undo_inhibit,
            undo_pt_before: None,
            // Creation counts as the first modification (Emacs's
            // fresh buffers report buffer-modified-tick = 1).
            mod_tick: 1,
            chars_mod_tick: 1,
            save_tick: 1,
            text_props: Vec::new(),
            overlays: Vec::new(),
            live: true,
            base_buffer: None,
            syntax_table: None,
            suspended_mode: None,
            case_table: None,
            category_table: None,
            narrow_labels: Vec::new(),
            file_modtime_ns: -2,
            file_modtime_size: -1,
            file_lock_name: None,
            create_lockfiles: true,
        }
    }

    /// Set the modified flag, running the GNU lock/unlock side
    /// effects: becoming modified locks the visited file
    /// (filelock.c `lock_file' via `prepare_to_modify_buffer'), and
    /// becoming unmodified releases our lock (`unlock_file').
    pub fn note_modified(&mut self, flag: bool) {
        if !flag {
            // GNU Fset_buffer_modified_p: SAVE_MODIFF = MODIFF, so the
            // next recorded change pushes a `(t . MODTIME)' entry.
            self.save_tick = self.mod_tick;
        }
        if flag == self.modified {
            return;
        }
        self.modified = flag;
        if flag {
            self.maybe_lock_file();
        } else {
            self.release_lock_file();
        }
    }

    /// Create `.#FILE' for the visited file unless already locked by us.
    /// A lock owned by another process is left in place;
    /// `file-locked-p' still reports its owner.
    fn maybe_lock_file(&mut self) {
        if !self.create_lockfiles || self.file_lock_name.is_some() {
            return;
        }
        let file = match &self.file_name {
            Some(f) => f.clone(),
            None => return,
        };
        #[cfg(unix)]
        {
            // GNU locks the visited file's truename.
            let lname = lock_file_name(&file_truename(&file));
            if std::os::unix::fs::symlink(&lock_owner_string(), &lname).is_ok() {
                self.file_lock_name = Some(lname);
            }
        }
    }

    /// Remove the lock file this buffer created, if any.
    pub fn release_lock_file(&mut self) {
        if let Some(lname) = self.file_lock_name.take() {
            let _ = std::fs::remove_file(&lname);
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
        let n = s.chars().count();
        if n == 0 {
            return;
        }
        self.raw_insert_at(pos, s);
        self.note_text_change(n);
    }

    /// `insert_at' without the MODIFF bump — for ops that report a
    /// composite change (`replace_range', casify) with a single tick.
    pub fn raw_insert_at(&mut self, pos: usize, s: &str) {
        let pos = pos.min(self.text.len());
        let n = s.chars().count();
        if n == 0 {
            return;
        }
        self.record_insert(pos, n);
        self.text.insert(pos, s);
        self.adjust_insert(pos, n, before_markers_flag(pos, self.point));
    }

    /// Like Emacs's `insert`: inserted text goes *before* point when
    /// inserting at point — i.e. point ends up after the new text.
    /// (Emacs `insert` inserts before point, advancing it.)
    pub fn insert(&mut self, s: &str) {
        let n = s.chars().count();
        if n == 0 {
            return;
        }
        self.raw_insert(s);
        self.note_text_change(n);
    }

    /// `insert' without the MODIFF bump.
    pub fn raw_insert(&mut self, s: &str) {
        let p = self.point();
        let n = s.chars().count();
        if n == 0 {
            return;
        }
        self.record_insert(p, n);
        self.text.insert(p, s);
        self.point = p + n;
        self.adjust_markers_insert(p, n, false);
    }

    /// `insert-before-markers`.
    pub fn insert_before_markers(&mut self, s: &str) {
        let n = s.chars().count();
        if n == 0 {
            return;
        }
        self.raw_insert_before_markers(s);
        self.note_text_change(n);
    }

    /// `insert_before_markers' without the MODIFF bump.
    pub fn raw_insert_before_markers(&mut self, s: &str) {
        let p = self.point();
        let n = s.chars().count();
        if n == 0 {
            return;
        }
        self.record_insert(p, n);
        self.text.insert(p, s);
        self.point = p + n;
        self.adjust_markers_insert(p, n, true);
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
        self.adjust_overlays_insert(pos, n, before_markers);
        self.adjust_text_props_insert(pos, n);
    }

    /// GNU `insertion' of N chars at POS splits a covering text-property
    /// interval — the new text is unpropertized (property inheritance
    /// is a separate `insert-and-inherit' concern).  Intervals starting
    /// at POS shift right wholesale.
    fn adjust_text_props_insert(&mut self, pos: usize, n: usize) {
        if self.text_props.is_empty() {
            return;
        }
        let mut extra: Vec<TextProp> = Vec::new();
        for tp in &mut self.text_props {
            if tp.end <= pos {
                continue;
            }
            if tp.start >= pos {
                tp.start += n;
                tp.end += n;
            } else {
                // pos strictly inside [start, end): split.
                extra.push(TextProp {
                    start: pos + n,
                    end: tp.end + n,
                    prop: tp.prop,
                    value: tp.value.clone(),
                });
                tp.end = pos;
            }
        }
        self.text_props.extend(extra);
    }

    /// GNU `adjust_markers_for_insert' applied to overlays: boundaries
    /// strictly past POS shift; a boundary at POS follows its own
    /// advance flag (`front-advance' for start, `rear-advance' for
    /// end), or always shifts for `insert-before-markers'.
    fn adjust_overlays_insert(&mut self, pos: usize, n: usize, before: bool) {
        for ov in &mut self.overlays {
            if ov.buffer != Some(self.id) {
                continue;
            }
            if ov.start > pos || (ov.start == pos && (before || ov.front_advance)) {
                ov.start += n;
            }
            if ov.end > pos || (ov.end == pos && (before || ov.rear_advance)) {
                ov.end += n;
            }
        }
    }

    /// GNU `adjust_markers_for_delete': positions inside [START, END)
    /// clamp to START, positions at or past END shift down by the
    /// removed length.  Overlays follow the same rule.
    pub fn adjust_markers_delete(&mut self, start: usize, end: usize) {
        let n = end - start;
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
        self.adjust_overlays_delete(start, end);
        self.adjust_text_props_delete(start, end);
    }

    /// GNU `adjust_intervals_for_delete': interval boundaries inside the
    /// deleted span clamp to START, those past END shift down; an
    /// interval left empty is dropped.
    fn adjust_text_props_delete(&mut self, start: usize, end: usize) {
        if self.text_props.is_empty() {
            return;
        }
        let n = end - start;
        for tp in &mut self.text_props {
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
        self.text_props.retain(|tp| tp.start < tp.end);
    }

    /// GNU `adjust_markers_for_delete' applied to overlays: boundaries
    /// inside the deleted span clamp to START, those past END shift.
    fn adjust_overlays_delete(&mut self, start: usize, end: usize) {
        let n = end - start;
        for ov in &mut self.overlays {
            if ov.buffer != Some(self.id) {
                continue;
            }
            for p in [&mut ov.start, &mut ov.end] {
                if *p >= end {
                    *p -= n;
                } else if *p > start {
                    *p = start;
                }
            }
        }
    }

    /// Delete `[start, end)`, adjusting everything.
    pub fn delete_region(&mut self, start: usize, end: usize) -> String {
        let n = end.saturating_sub(start);
        let removed = self.raw_delete_region(start, end);
        if n > 0 {
            self.note_text_change(n);
        }
        removed
    }

    /// `delete_region' without the MODIFF bump.
    pub fn raw_delete_region(&mut self, start: usize, end: usize) -> String {
        let start = start.min(self.text.len());
        let end = end.min(self.text.len());
        if start >= end {
            return String::new();
        }
        let removed = self.text.substring(start, end);
        self.record_delete(start, removed.clone());
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
        self.adjust_markers_delete(start, end);
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
        removed
    }

    pub fn register_marker(&mut self, m: &Rc<RefCell<Marker>>) {
        self.markers.push(Rc::downgrade(m));
    }

    // ---------- undo recording (GNU undo.c) ----------

    /// Current `buffer-undo-list' value (nil when unbound).
    pub fn undo_list(&self) -> Value {
        match self.locals.get(&self.undo_sym) {
            Some(Value::Sym(s)) if *s == crate::lisp::sym::UNBOUND => Value::Nil,
            Some(v) => v.clone(),
            None => Value::Nil,
        }
    }

    /// Set `buffer-undo-list' buffer-locally (what `setq' sees).
    pub fn set_undo_list(&mut self, v: Value) {
        self.locals.insert(self.undo_sym, v);
    }

    /// Undo recording is disabled iff `buffer-undo-list' is `t'.
    pub fn undo_disabled(&self) -> bool {
        matches!(self.undo_list(), Value::Sym(s) if s == crate::lisp::sym::T)
    }

    /// Cons an entry onto `buffer-undo-list'.
    pub fn push_undo(&mut self, entry: Value) {
        let list = self.undo_list();
        self.set_undo_list(Value::cons(entry, list));
    }

    /// GNU `record_first_change': push `(t . MODTIME)' where MODTIME is
    /// `buffer_visited_file_modtime' (0 for non-file buffers).
    fn record_first_change(&mut self) {
        if self.undo_disabled() {
            return;
        }
        let mt = if self.file_name.is_none() {
            Value::Int(0)
        } else if self.file_modtime_ns < 0 {
            Value::Int(-2 - self.file_modtime_ns)
        } else {
            crate::lisp::builtins::misc::ns_to_lisp_time(self.file_modtime_ns)
        };
        self.push_undo(Value::cons(Value::t(), mt));
    }

    /// GNU `record_point': BEG is the 1-based position that the undo
    /// record about to be pushed will restore point to.
    fn record_point(&mut self, beg: usize) {
        // `undo_inhibit_record_point' suppresses the point record and
        // the first-change timestamp it would otherwise write.
        if self.undo_inhibit.get() {
            return;
        }
        let at_boundary = match &self.undo_list() {
            Value::Cons(c) => matches!(c.borrow().car, Value::Nil),
            _ => true,
        };
        // First change since save gets a timestamp record.
        if self.mod_tick <= self.save_tick {
            self.record_first_change();
        }
        // Right after a boundary, record where point was before the
        // command started so undo can restore it.
        if at_boundary
            && self.undo_pt_before.map_or(false, |p| p != beg)
        {
            let p = self.undo_pt_before.unwrap();
            self.push_undo(Value::Int(p as i128));
        }
    }

    /// GNU `record_insert': LENGTH chars were inserted at BEG (0-based).
    pub fn record_insert(&mut self, beg: usize, length: usize) {
        if self.undo_disabled() {
            return;
        }
        self.record_point(beg + 1);
        // Amalgamate with a preceding consecutive insertion record.
        if let Value::Cons(top) = self.undo_list() {
            let car = top.borrow().car.clone();
            if let Value::Cons(elt) = &car {
                let (ebeg, eend) = {
                    let e = elt.borrow();
                    (e.car.clone(), e.cdr.clone())
                };
                if let (Value::Int(_), Value::Int(end)) = (&ebeg, &eend) {
                    if *end == beg as i128 + 1 {
                        elt.borrow_mut().cdr =
                            Value::Int(beg as i128 + 1 + length as i128);
                        return;
                    }
                }
            }
        }
        self.push_undo(Value::cons(
            Value::Int(beg as i128 + 1),
            Value::Int(beg as i128 + 1 + length as i128),
        ));
    }

    /// GNU `record_delete': TEXT is about to be deleted at BEG (0-based).
    /// The position is recorded negative when point is right after the
    /// deleted text (so undo leaves point before the reinserted text).
    pub fn record_delete(&mut self, beg: usize, text: String) {
        if self.undo_disabled() {
            return;
        }
        self.record_point(beg + 1);
        let n = text.chars().count();
        let sbeg = if self.point + 1 == beg + 1 + n {
            -(beg as i128 + 1)
        } else {
            beg as i128 + 1
        };
        self.record_marker_adjustments(beg, beg + n);
        self.push_undo(Value::cons(Value::string(text), Value::Int(sbeg)));
    }

    /// The effective value of PROP (last write wins) at 0-based POS,
    /// or nil when no covering entry exists.
    pub fn prop_value_at(&self, pos: usize, prop: u32) -> Value {
        for tp in self.text_props.iter().rev() {
            if tp.prop == prop && tp.start <= pos && pos < tp.end {
                return tp.value.clone();
            }
        }
        Value::Nil
    }

    /// The effective plist (prop -> value, last write wins) at POS.
    pub fn plist_at(&self, pos: usize) -> std::collections::BTreeMap<u32, Value> {
        let mut m = std::collections::BTreeMap::new();
        for tp in self.text_props.iter().rev() {
            if tp.start <= pos && pos < tp.end {
                m.entry(tp.prop).or_insert_with(|| tp.value.clone());
            }
        }
        m
    }

    /// GNU `record_property_change': push `(nil PROP OLD BEG . END)'.
    /// BEG/END are 1-based Lisp positions.  Property changes bump
    /// MODIFF like text changes, so the first record after a save
    /// also writes the `(t . MODTIME)' entry.
    pub fn record_prop_change(&mut self, prop: Value, old: Value, beg: usize, end: usize) {
        if self.undo_disabled() {
            return;
        }
        if self.mod_tick <= self.save_tick {
            self.record_first_change();
        }
        let entry = Value::cons(
            Value::Nil,
            Value::cons(
                prop,
                Value::cons(
                    old,
                    Value::cons(Value::Int(beg as i128), Value::Int(end as i128)),
                ),
            ),
        );
        self.push_undo(entry);
    }

    /// GNU `modiff_incr': MODIFF grows logarithmically with the number
    /// of changed characters — floor(log2(len)) + 1 for len > 0, else 1.
    fn modiff_incr(len: usize) -> u64 {
        if len == 0 {
            1
        } else {
            (usize::BITS - len.leading_zeros()) as u64
        }
    }

    /// GNU insdel tick: `MODIFF += elogb(nchars) + 1' then
    /// `CHARS_MODIFF = MODIFF' (the chars counter mirrors MODIFF's
    /// absolute value, catching up any property-only bumps).
    pub fn note_text_change(&mut self, nchars: usize) {
        self.note_modified(true);
        self.mod_tick += Self::modiff_incr(nchars);
        self.chars_mod_tick = self.mod_tick;
    }

    /// GNU `modify_text_properties': first-change record (when this is
    /// the first change since save) + `MODIFF += 1'.  CHARS_MODIFF is
    /// not touched.
    pub fn note_prop_modified(&mut self) {
        if self.mod_tick <= self.save_tick {
            self.record_first_change();
        }
        self.mod_tick += 1;
        self.note_modified(true);
    }

    /// GNU `record_marker_adjustments': markers inside [FROM, TO] get
    /// (MARKER . ADJUSTMENT) entries pushed before the deletion record.
    fn record_marker_adjustments(&mut self, from: usize, to: usize) {
        let mut adjs = Vec::new();
        for w in &self.markers {
            if let Some(m) = w.upgrade() {
                let mm = m.borrow();
                if mm.buffer == Some(self.id)
                    && from <= mm.position
                    && mm.position <= to
                {
                    let base = if mm.insertion_type { to } else { from };
                    let adj = base as i128 - mm.position as i128;
                    if adj != 0 {
                        adjs.push((m.clone(), adj));
                    }
                }
            }
        }
        for (m, adj) in adjs {
            self.push_undo(Value::cons(Value::Marker(m), Value::Int(adj)));
        }
    }

    /// GNU `undo-boundary': push nil unless the list already starts
    /// with a boundary; always records the pre-command point.
    pub fn undo_boundary(&mut self) {
        if self.undo_disabled() {
            return;
        }
        let at_boundary = match &self.undo_list() {
            Value::Cons(c) => matches!(c.borrow().car, Value::Nil),
            _ => false,
        };
        if !at_boundary {
            self.push_undo(Value::Nil);
        }
        self.undo_pt_before = Some(self.point() + 1);
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
    /// SymId of `buffer-undo-list', stamped onto new buffers (set by
    /// the interpreter once the obarray exists).
    undo_sym: SymId,
    /// Shared `undo-inhibit-record-point' flag for all buffers.
    undo_inhibit: Rc<std::cell::Cell<bool>>,
}

impl BufferSet {
    pub fn new() -> Self {
        BufferSet {
            bufs: Vec::new(),
            order: Vec::new(),
            name_map: HashMap::new(),
            counter: 0,
            undo_sym: 0,
            undo_inhibit: Rc::new(std::cell::Cell::new(false)),
        }
    }

    /// The shared `undo-inhibit-record-point' flag cell.
    pub fn undo_inhibit_cell(&self) -> Rc<std::cell::Cell<bool>> {
        self.undo_inhibit.clone()
    }

    /// Set the `buffer-undo-list' SymId for all buffers (existing and
    /// future).  Called by `Interp::new' after the obarray is seeded.
    pub fn set_undo_sym(&mut self, sym: SymId) {
        self.undo_sym = sym;
        for b in self.bufs.iter().flatten() {
            let mut bb = b.borrow_mut();
            if bb.undo_sym != sym {
                // Re-key the space-name `t' seed planted with sym 0.
                let old = bb.undo_sym;
                if let Some(v) = bb.locals.remove(&old) {
                    bb.locals.insert(sym, v);
                }
                bb.undo_sym = sym;
            }
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
        let buf = Buffer::new(
            id,
            final_name.clone(),
            self.undo_sym,
            self.undo_inhibit.clone(),
        );
        self.bufs.push(Some(Rc::new(RefCell::new(buf))));
        self.name_map.insert(final_name, id);
        self.order.push(id);
        self.counter += 1;
        id
    }

    /// Create a buffer with exactly this name (kills none; used at init).
    pub fn create_exact(&mut self, name: &str) -> usize {
        let id = self.bufs.len();
        let buf = Buffer::new(
            id,
            name.to_string(),
            self.undo_sym,
            self.undo_inhibit.clone(),
        );
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
            // Overlays detach too (`overlay-buffer' → nil).
            for ov in &mut bb.overlays {
                ov.buffer = None;
            }
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
}

impl Default for BufferSet {
    fn default() -> Self {
        Self::new()
    }
}
