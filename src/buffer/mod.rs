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
        self.note_modified(true);
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
        self.note_modified(true);
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
        self.note_modified(true);
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
        self.note_modified(true);
        self.mod_tick += 1;
        removed
    }

    pub fn register_marker(&mut self, m: &Rc<RefCell<Marker>>) {
        self.markers.push(Rc::downgrade(m));
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
}

impl Default for BufferSet {
    fn default() -> Self {
        Self::new()
    }
}
