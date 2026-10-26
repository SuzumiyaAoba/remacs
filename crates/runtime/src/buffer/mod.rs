//! Buffer object model: text, point, mark, narrowing, buffer-local
//! variables, markers, undo records — plus the registry (`BufferSet`)
//! and all buffer primitives installed into the interpreter.

pub mod extra;
mod base64_data;
mod gapbuf;
pub mod primitives;
mod replace;
pub(crate) mod textprops;

pub use primitives::install_primitives;

use rustc_hash::FxHashMap as HashMap;
use std::cell::RefCell;
use std::cmp::Ordering;
use std::rc::{Rc, Weak};

use crate::lisp::value::{BufferRef, Marker, MarkerRef, SymId, Value};
use crate::lisp::text::{EmacsText, TextSource};

pub use gapbuf::{EditObserver, EditSubscription, GapBuffer, SharedText, TextChange};

/// Ordinary locals retain their creation order, as GNU's local-var alist does.
pub type BufferLocals =
    indexmap::IndexMap<SymId, Value, std::hash::BuildHasherDefault<rustc_hash::FxHasher>>;

/// One editor buffer.
pub struct Buffer {
    /// Unique id in the BufferSet.
    pub id: usize,
    pub name: EmacsText,
    /// Buffer text. Indirect buffers share the base buffer's storage
    /// via `SharedText` (`Rc<RefCell<GapBuffer>>`), so edits made in
    /// any buffer of the chain are visible in all of them.
    pub text: SharedText,
    /// GNU BUF_AUTOSAVE_MODIFF: per-buffer, unlike the shared text's
    /// modification/save ticks. Indirect views keep their own value.
    pub auto_save_tick: u64,
    /// Point (0-based char offset). Emacs `point` = `self.point + 1`.
    pub point: usize,
    /// Start remembered when this buffer was last removed from a window.
    pub last_window_start: usize,
    /// Last selected window still displaying this buffer (GNU point owner).
    pub last_selected_window: Option<Weak<RefCell<crate::editor::Window>>>,
    /// The mark marker (GNU `buffer->mark'), if the mark was ever
    /// touched.  `mark-marker' hands this object out, so `set-marker'
    /// on it moves the mark.  A Some value with `buffer = None' is a
    /// detached mark: the mark reads as unset but identity persists.
    pub mark: Option<MarkerRef>,
    /// Narrowing bounds (0-based, inclusive-exclusive).
    pub begv: usize,
    pub zv: usize,
    /// Buffer-local variable bindings.
    pub locals: BufferLocals,
    /// Markers pointing into this buffer.
    pub markers: Vec<Weak<RefCell<Marker>>>,
    /// Visited file name (or nil).
    pub file_name: Option<EmacsText>,
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
    /// GNU SAVE_MODIFF lives in `text.save_tick' (shared): the modified
    /// flag `mod_tick > save_tick' is the same from every indirect view.
    /// Text-property intervals live in `text.props', shared with
    /// indirect buffers like GNU's `b->text->intervals' — last write
    /// wins within the shared list.
    /// Read-only regions aren't modeled; `read-only` text prop is checked
    /// at edit time in primitives.
    pub overlays: Vec<Overlay>,
    /// False once the buffer has been killed (the object may still be
    /// referenced by variables, markers, or window configurations).
    pub live: bool,
    /// For indirect buffers, the base buffer whose text is shared
    /// (GNU `b->base_buffer'; always the ultimate base — GNU forbids
    /// double indirection).  Holding the Rc keeps the base object
    /// alive after `kill-buffer' so `buffer-base-buffer' still returns
    /// the killed buffer object. (Text-props, markers, and undo stay
    /// per-buffer; only the `text' storage is aliased.)
    pub base_buffer: Option<BufferRef>,
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
    pub file_lock_name: Option<EmacsText>,
    /// Cached snapshot of the `create-lockfiles' Lisp variable, taken
    /// when the file is visited (Buffer methods cannot see Lisp state).
    pub create_lockfiles: bool,
    /// Mode remembered by `major-mode-suspend' (GNU records the local
    /// `major-mode' so `major-mode-restore' can re-run it after a
    /// suspend/undump).
    pub suspended_mode: Option<crate::lisp::value::Value>,
    /// Name before the last rename or kill (GNU `b->last_name'),
    /// reported by `buffer-last-name'.
    pub last_name: Option<EmacsText>,
    /// GNU `b->inhibit_buffer_hooks': set by make-indirect-buffer's
    /// INHIBIT-BUFFER-HOOKS arg; such buffers skip `kill-buffer-hook'
    /// and `kill-buffer-query-functions' when killed.
    pub inhibit_buffer_hooks: bool,
}

/// Resolve path components in GNU file-truename order. Missing components
/// remain usable names; a later `..' can cancel one and expose a real link.
/// Relative links preserve the root, while absolute links replace it.
pub fn file_truename<T: TextSource + ?Sized>(path: &T) -> EmacsText {
    path.with_text(|path| {
        use std::collections::VecDeque;
        let absolute = if path.starts_with('/') { path.to_owned() } else {
            let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/"));
            let mut absolute = EmacsText::from_os_str(cwd.as_os_str());
            if !absolute.ends_with('/') { absolute.push('/'); }
            absolute.push_str(path);
            absolute
        };
        let root = |name: &crate::lisp::text::EmacsStr| {
            if name.starts_with("//") && !name.starts_with("///") { "//" } else { "/" }
        };
        let mut out = EmacsText::from(root(&absolute));
        let mut root_len = out.len();
        let mut pending: VecDeque<EmacsText> = absolute.split('/').map(ToOwned::to_owned).collect();
        let mut dirlike = path.ends_with('/');
        let mut links = 0;
        while let Some(component) = pending.pop_front() {
            if component.is_empty() || component == "." { continue; }
            if component == ".." {
                if out.len() > root_len { out.truncate(out.rfind('/').unwrap().max(root_len)); }
                continue;
            }
            let mut candidate = out.clone();
            if !candidate.ends_with('/') { candidate.push('/'); }
            candidate.append(&component);
            if links < 100 {
                let target = candidate.to_os_string().ok().and_then(|path| std::fs::read_link(path).ok());
                if let Some(target) = target {
                    links += 1;
                    let target = EmacsText::from_os_str(target.as_os_str());
                    dirlike |= pending.iter().all(EmacsText::is_empty) && target.ends_with('/');
                    if target.starts_with('/') { out = EmacsText::from(root(&target)); root_len = out.len(); }
                    for part in target.split('/').rev() { pending.push_front(part.to_owned()); }
                    continue;
                }
            }
            out = candidate;
        }
        if dirlike && !out.ends_with('/') { out.push('/'); }
        out
    })
}

#[cfg(test)]
mod wide_truename_tests {
    use super::*;
    #[test]
    fn root_and_lock_names_preserve_extended_characters() {
        assert_eq!(file_truename("//.."), "//");
        assert_eq!(file_truename("///.."), "/");
        let name = EmacsText::from_codes([47, 0xf0080, 47, 0x3fff80, 0xd800, 0x110000]).unwrap();
        assert_eq!(lock_file_name(&name).codes().collect::<Vec<_>>(), [47, 0xf0080, 47, 46, 35, 0x3fff80, 0xd800, 0x110000]);
    }
    #[cfg(unix)]
    #[test]
    fn physical_symlinks_and_missing_components_retain_character_identity() {
        let unique = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let directory = std::env::temp_dir().join(format!("remacs-wide-truename-{}-{unique}", std::process::id()));
        std::fs::create_dir(&directory).unwrap();
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); } }
        let _cleanup = Cleanup(directory.clone());
        let child = EmacsText::from_codes([0xf0080, 0xf0100]).unwrap();
        std::fs::create_dir(directory.join(child.to_os_string().unwrap())).unwrap();
        std::os::unix::fs::symlink(child.to_os_string().unwrap(), directory.join("alias")).unwrap();
        let mut input = EmacsText::from_os_str(directory.as_os_str());
        input.push_str("/alias/"); input.push(crate::lisp::text::EmacsChar::new(0x110000).unwrap());
        input.push_str("/../"); input.push(crate::lisp::text::EmacsChar::new(0xd800).unwrap());
        let mut expected = EmacsText::from_os_str(std::fs::canonicalize(&directory).unwrap().as_os_str());
        expected.push('/'); expected.append(&child); expected.push('/');
        expected.push(crate::lisp::text::EmacsChar::new(0xd800).unwrap());
        assert_eq!(file_truename(&input), expected);
    }
}

/// GNU file-lock path: `.#NAME' in FILE's directory.
pub(crate) fn lock_file_name<T: TextSource + ?Sized>(file: &T) -> EmacsText {
    file.with_text(|file| {
        let result = match file.rfind('/') {
            Some(position) => { let mut directory = file[..=position].to_owned(); directory.push_str(".#"); directory.push_str(&file[position + 1..]); directory }
            None => { let mut name = EmacsText::from(".#"); name.push_str(file); name }
        };
        result
    })
}

pub(crate) fn our_host_name() -> String {
    let mut host = String::new();
    #[cfg(unix)]
    unsafe {
        let mut buf = [0i8; 256];
        if libc::gethostname(buf.as_mut_ptr(), buf.len()) == 0 {
            host = std::ffi::CStr::from_ptr(buf.as_ptr())
                .to_string_lossy()
                .into_owned();
        }
    }
    if host.is_empty() {
        host = "localhost".into();
    }
    host
}

/// Seconds since the epoch of the last system boot, or 0 when it
/// cannot be determined (GNU filelock.c `get_boot_time').
pub(crate) fn boot_time() -> i64 {
    #[cfg(target_os = "macos")]
    unsafe {
        // gnulib get_boot_time consults utmpx first. This remains readable
        // in environments that deny kern.boottime through sysctl.
        let recorded_boot = {
            static UTMP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
            let _guard = UTMP_LOCK.lock().unwrap_or_else(|error| error.into_inner());
            let mut recorded_boot = 0;
            libc::setutxent();
            loop {
                let entry = libc::getutxent();
                if entry.is_null() {
                    break;
                }
                if (*entry).ut_type == libc::BOOT_TIME {
                    recorded_boot = (*entry).ut_tv.tv_sec as i64;
                }
            }
            libc::endutxent();
            recorded_boot
        };
        if recorded_boot != 0 {
            return recorded_boot;
        }
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
    let base = format!(
        "{}@{}.{}",
        user,
        our_host_name().replace('@', "-"),
        std::process::id()
    );
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
/// Entries keep stable indices while their owner buffer lives. Before the
/// buffer is destroyed, canonical handles retain their detached Lisp state.
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
    /// Effective `evaporate', refreshed after before-change hooks by Lisp
    /// deletion paths, including category defaults and shared-text views.
    pub evaporate: bool,
    /// The canonical `[overlay BID IDX]' Lisp handle — reused by
    /// `overlays-at' & friends so `eq' holds for the same overlay.
    pub handle: Value,
}

impl Overlay {
    pub(crate) fn matches_window(&self, i: &crate::lisp::Interp, window: &Value) -> bool {
        let Some(property) = i.q_soft("window") else {
            return true;
        };
        let restriction = self.property(i, &Value::Sym(property));
        !matches!(restriction, Value::Window(_))
            || crate::lisp::builtins::eq_values(&restriction, window)
    }

    pub(crate) fn property(&self, i: &crate::lisp::Interp, property: &Value) -> Value {
        if let Some(value) = crate::lisp::eval::plist_lookup_v(&self.plist, property) {
            return value;
        }
        let category =
            i.q_soft("category").and_then(|key| {
                match crate::lisp::eval::plist_get(&self.plist, key) {
                    Value::Sym(symbol) => Some(symbol),
                    _ => None,
                }
            });
        match (category, i.sym_id(property)) {
            (Some(category), Some(property)) => i.get_prop(category, property),
            _ => Value::Nil,
        }
    }

    fn priority(&self, i: &crate::lisp::Interp) -> (i128, i128) {
        fn part(value: &Value) -> i128 {
            if value.fixnump() {
                value.int().unwrap_or(0)
            } else {
                0
            }
        }
        let Some(priority) = i.q_soft("priority") else {
            return (0, 0);
        };
        let value = self.property(i, &Value::Sym(priority));
        if let Value::Cons(cell) = &value {
            let cell = cell.borrow();
            (part(&cell.car), part(&cell.cdr))
        } else {
            (part(&value), 0)
        }
    }

    /// GNU compare_overlays, low precedence first. Containment wins over
    /// secondary priority; exact remaining ties are resolved by the caller.
    fn compare_priority(&self, other: &Self, i: &crate::lisp::Interp) -> Ordering {
        let (primary, secondary) = self.priority(i);
        let (other_primary, other_secondary) = other.priority(i);
        if primary != other_primary {
            primary.cmp(&other_primary)
        } else if self.start < other.start {
            if self.end < other.end && secondary > other_secondary {
                Ordering::Greater
            } else {
                Ordering::Less
            }
        } else if self.start > other.start {
            if self.end > other.end && secondary < other_secondary {
                Ordering::Less
            } else {
                Ordering::Greater
            }
        } else if self.end != other.end {
            other.end.cmp(&self.end)
        } else {
            secondary.cmp(&other_secondary)
        }
    }

    /// Detached state belongs to the handle, without retaining the dead
    /// buffer or adding strong roots to the interpreter's weak registry.
    pub(crate) fn detached_state(handle: &Value) -> Option<Self> {
        let Value::Vec(values) = handle else {
            return None;
        };
        let values = values.borrow();
        if values.len() != 7 {
            return None;
        }
        Some(Self {
            start: 0,
            end: 0,
            buffer: None,
            plist: values[3].clone(),
            front_advance: values[4].truthy(),
            rear_advance: values[5].truthy(),
            evaporate: values[6].truthy(),
            handle: handle.clone(),
        })
    }

    pub(crate) fn save_detached_state(&self) {
        let Value::Vec(handle) = &self.handle else {
            return;
        };
        let mut values = handle.borrow_mut();
        if values.len() < 3 {
            return;
        }
        values.truncate(3);
        values.extend([
            self.plist.clone(),
            Value::from_bool(self.front_advance),
            Value::from_bool(self.rear_advance),
            Value::from_bool(self.evaporate),
        ]);
    }

    fn owns_handle_at(&self, buffer: usize, slot: usize) -> bool {
        let Value::Vec(handle) = &self.handle else {
            return false;
        };
        matches!(handle.borrow().as_slice(),
            [_, Value::Int(owner), Value::Int(index)]
                if *owner == buffer as i128 && *index == slot as i128)
    }
}

/// Order low to high, preserving the existing slot order for exact ties.
/// GNU's containment/secondary comparison can be non-transitive for crossing
/// intervals, so use bounded sorting without std's total-order requirement.
pub(crate) fn sort_overlays_by_priority(
    i: &crate::lisp::Interp,
    overlays: &mut [(&Overlay, usize)],
) {
    let compare = |left: &(&Overlay, usize), right: &(&Overlay, usize)| {
        left.0
            .compare_priority(right.0, i)
            .then_with(|| left.1.cmp(&right.1))
    };
    let count = overlays.len();
    if count <= 16 {
        for index in 1..count {
            let mut position = index;
            while position > 0 && compare(&overlays[position - 1], &overlays[position]).is_gt() {
                overlays.swap(position - 1, position);
                position -= 1;
            }
        }
        return;
    }
    let mut source = overlays.to_vec();
    let mut width = 1;
    while width < count {
        for begin in (0..count).step_by(width * 2) {
            let middle = (begin + width).min(count);
            let end = (middle + width).min(count);
            let (mut left, mut right) = (begin, middle);
            for destination in &mut overlays[begin..end] {
                if right == end
                    || (left < middle && !compare(&source[left], &source[right]).is_gt())
                {
                    *destination = source[left];
                    left += 1;
                } else {
                    *destination = source[right];
                    right += 1;
                }
            }
        }
        source.copy_from_slice(overlays);
        width *= 2;
    }
}

impl Buffer {
    pub fn new(
        id: usize,
        name: impl Into<EmacsText>,
        undo_sym: SymId,
        undo_inhibit: Rc<std::cell::Cell<bool>>,
    ) -> Buffer {
        let name: EmacsText = name.into();
        let mut locals = indexmap::IndexMap::default();
        // Buffers with space-prefixed (internal) names start with undo
        // disabled, like GNU get-buffer-create (undo_list = Qt).
        if name.starts_with(' ') {
            locals.insert(undo_sym, Value::t());
        }
        Buffer {
            id,
            name: name.clone(),
            text: SharedText::new(),
            auto_save_tick: 0,
            point: 0,
            last_window_start: 0,
            last_selected_window: None,
            mark: None,
            begv: 0,
            zv: 0,
            locals,
            markers: Vec::new(),
            file_name: None,
            undo_sym,
            undo_inhibit,
            undo_pt_before: None,
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
            // GNU get-buffer-create sets last_name to the creation
            // name (renames/kill update it later); only `*scratch*',
            // born via init_buffer, keeps nil — see
            // `create_exact_nolast'.
            last_name: Some(name),
            inhibit_buffer_hooks: false,
        }
    }

    /// GNU `BUFFER_MODIFIED_P': the shared MODIFF vs our SAVE_MODIFF —
    /// edits through any buffer sharing this text count.
    pub fn is_modified(&self) -> bool {
        // `internal--set-buffer-modified-tick' stores raw fixnums
        // (negatives included), so compare as signed.
        self.text.mod_tick.get() as i64 > self.text.save_tick.get() as i64
    }

    pub fn is_auto_saved(&self) -> bool {
        self.is_modified() && self.auto_save_tick == self.text.mod_tick.get()
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

    /// Set the modified flag, running the GNU lock/unlock side
    /// effects: becoming modified locks the visited file
    /// (filelock.c `lock_file' via `prepare_to_modify_buffer'), and
    /// becoming unmodified releases our lock (`unlock_file').
    pub fn note_modified(&mut self, flag: bool) {
        // Real edits advance MODIFF themselves; mark the text dirty
        // before that change without adding another tick here.
        let already = self.is_modified();
        let mod_tick = self.text.mod_tick.get();
        if flag {
            if !already {
                self.text.save_tick.set(mod_tick.wrapping_sub(1));
            }
        } else {
            self.text.save_tick.set(mod_tick);
        }
        self.sync_modified_lock(flag, already);
    }

    /// GNU Frestore_buffer_modified_p: making clean text modified
    /// advances MODIFF and retains its old value in SAVE_MODIFF.
    /// CHARS_MODIFF is unchanged, and an old autosave is not revived.
    pub fn restore_modified(&mut self, flag: bool) {
        let already = self.is_modified();
        let tick = self.text.mod_tick.get();
        if !flag {
            self.text.save_tick.set(tick);
        } else if !already {
            self.text.save_tick.set(tick);
            self.text.mod_tick.set(tick.wrapping_add(1));
        }
        self.sync_modified_lock(flag, already);
    }

    fn sync_modified_lock(&mut self, flag: bool, already: bool) {
        // GNU locks/unlocks the file of the BASE buffer (an indirect
        // buffer never visits a file itself), driven by the flag
        // transition on that base.
        let target = self.base_buffer.clone();
        match (flag, already, target) {
            (true, false, Some(b)) => b.borrow_mut().maybe_lock_file(),
            (true, false, None) => self.maybe_lock_file(),
            (false, true, Some(b)) => b.borrow_mut().release_lock_file(),
            (false, true, None) => self.release_lock_file(),
            _ => {}
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
            let Ok(native) = lname.to_os_string() else { return; };
            if std::os::unix::fs::symlink(&lock_owner_string(), native).is_ok() {
                self.file_lock_name = Some(lname);
            }
        }
    }

    /// Remove the lock file this buffer created, if any.
    pub fn release_lock_file(&mut self) {
        if let Some(lname) = self.file_lock_name.take() {
            if let Ok(native) = lname.to_os_string() { let _ = std::fs::remove_file(native); }
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
    pub fn insert_at<T: TextSource + ?Sized>(&mut self, pos: usize, s: &T) {
        s.with_text(|s| {
        let n = s.chars().count();
        if n == 0 {
            return;
        }
        self.raw_insert_at(pos, s);
        self.note_text_change(n);

        })
    }

    /// `insert_at' without the MODIFF bump — for ops that report a
    /// composite change (`replace_range', casify) with a single tick.
    pub fn raw_insert_at<T: TextSource + ?Sized>(&mut self, pos: usize, s: &T) {
        s.with_text(|s| {
        let pos = pos.min(self.text.len());
        let n = s.chars().count();
        if n == 0 {
            return;
        }
        self.record_insert(pos, n);
        self.text.insert(pos, s);
        self.adjust_insert(pos, n, before_markers_flag(pos, self.point));
        let id = self.id;
        let text = self.text.clone();
        text.each_sibling(id, move |sib| {
            // GNU PT is an insertion_type=false marker: insert at the
            // exact position does not advance it.
            if sib.point > pos {
                sib.point += n;
            }
            sib.adjust_markers_insert(pos, n, false)
        });

        })
    }

    /// Like Emacs's `insert`: inserted text goes *before* point when
    /// inserting at point — i.e. point ends up after the new text.
    /// (Emacs `insert` inserts before point, advancing it.)
    pub fn insert<T: TextSource + ?Sized>(&mut self, s: &T) {
        s.with_text(|s| {
        let n = s.chars().count();
        if n == 0 {
            return;
        }
        self.raw_insert(s);
        self.note_text_change(n);

        })
    }

    /// `insert' without the MODIFF bump.
    pub fn raw_insert<T: TextSource + ?Sized>(&mut self, s: &T) {
        s.with_text(|s| {
        let p = self.point();
        let n = s.chars().count();
        if n == 0 {
            return;
        }
        self.record_insert(p, n);
        self.text.insert(p, s);
        self.point = p + n;
        self.adjust_markers_insert(p, n, false);
        let id = self.id;
        let text = self.text.clone();
        text.each_sibling(id, move |sib| {
            if sib.point > p {
                sib.point += n;
            }
            sib.adjust_markers_insert(p, n, false)
        });

        })
    }

    /// `insert-before-markers`.
    pub fn insert_before_markers<T: TextSource + ?Sized>(&mut self, s: &T) {
        s.with_text(|s| {
        let n = s.chars().count();
        if n == 0 {
            return;
        }
        self.raw_insert_before_markers(s);
        self.note_text_change(n);

        })
    }

    /// `insert_before_markers' without the MODIFF bump.
    pub fn raw_insert_before_markers<T: TextSource + ?Sized>(&mut self, s: &T) {
        s.with_text(|s| {
        let p = self.point();
        let n = s.chars().count();
        if n == 0 {
            return;
        }
        self.record_insert(p, n);
        self.text.insert(p, s);
        self.point = p + n;
        // Active BEGV is a buffer field, not an insertion marker.  Text
        // inserted at point-min remains accessible even before markers.
        let begv = self.begv;
        self.adjust_markers_insert(p, n, true);
        self.begv = begv;
        let id = self.id;
        let text = self.text.clone();
        text.each_sibling(id, move |sib| {
            if sib.point >= p {
                sib.point += n;
            }
            sib.adjust_markers_insert(p, n, true)
        });

        })
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
    /// Also applied to sibling buffers sharing this text (their
    /// markers/zv/begv live in the same shared chain in GNU).
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
        // Inactive sibling restrictions are stored in markers: ZV advances
        // at insertion, while BEGV advances at equality only before markers.
        // Active before-markers insertion restores its field-valued BEGV.
        if self.zv >= pos {
            self.zv += n;
        }
        if self.begv > pos || (self.begv == pos && before_markers) {
            self.begv += n;
        }
        self.adjust_overlays_insert(pos, n, before_markers);
        // Text props ride along inside SharedText::insert.
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
            // GNU itree_insert_gap treats an empty front-advancing,
            // non-rear-advancing overlay as non-front-advancing, so
            // insertion cannot move its beginning beyond its end.
            let front_advance = ov.front_advance && (ov.start != ov.end || ov.rear_advance);
            if ov.start > pos || (ov.start == pos && (before || front_advance)) {
                ov.start += n;
            }
            if ov.end > pos || (ov.end == pos && (before || ov.rear_advance)) {
                ov.end += n;
            }
        }
    }

    /// GNU `adjust_markers_for_delete': positions inside [START, END)
    /// clamp to START, positions at or past END shift down by the
    /// removed length.  Overlays follow the same rule.  BEGV/ZV are
    /// markers in the shared text and follow it too — this also runs
    /// on sibling buffers when an indirect buffer edits shared text.
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
        self.adjust_overlays_delete(start, end);
        // Text props ride along inside SharedText::delete.
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
            // GNU `evaporate-overlays': an overlay left empty by the
            // deletion detaches when its `evaporate' property is set.
            if ov.evaporate && ov.start == start && ov.end == start {
                ov.buffer = None;
            }
        }
    }

    /// Delete `[start, end)`, adjusting everything.
    pub fn delete_region(&mut self, start: usize, end: usize) -> EmacsText {
        let n = end.saturating_sub(start);
        let removed = self.raw_delete_region(start, end);
        if n > 0 {
            self.note_text_change(n);
        }
        removed
    }

    /// `delete_region' without the MODIFF bump.
    pub fn raw_delete_region(&mut self, start: usize, end: usize) -> EmacsText {
        self.raw_delete_region_with_undo(start, end).0
    }

    /// Return the newly recorded string so the interpreter can attach its
    /// property intervals and byte kind before exposing the undo entry.
    pub(crate) fn raw_delete_region_with_undo(
        &mut self,
        start: usize,
        end: usize,
    ) -> (EmacsText, Option<crate::lisp::value::StrRef>) {
        let start = start.min(self.text.len());
        let end = end.min(self.text.len());
        if start >= end {
            return (EmacsText::new(), None);
        }
        let removed = self.text.substring(start, end);
        let undo = self.record_delete(start, removed.clone());
        self.text.delete(start, end);
        let n = end - start;
        // Point: Emacs clamps it into [start] if inside, shifts if after.
        if self.point >= end {
            self.point -= n;
        } else if self.point > start {
            self.point = start;
        }
        self.adjust_markers_delete(start, end);
        // Every buffer sharing this text re-bounds — GNU's BEGV/ZV/
        // PT/markers live in the shared chain.
        let id = self.id;
        let text = self.text.clone();
        text.each_sibling(id, |sib| {
            if sib.point >= end {
                sib.point -= n;
            } else if sib.point > start {
                sib.point = start;
            }
            sib.adjust_markers_delete(start, end)
        });
        (removed, undo)
    }

    /// GNU replace_range: record insertion before deletion, and keep the
    /// two endpoints distinct even when markers have opposite insertion types.
    pub(crate) fn raw_replace_region_with_undo<T: TextSource + ?Sized>(
        &mut self,
        start: usize,
        end: usize,
        replacement: &T,
    ) -> Option<crate::lisp::value::StrRef> {
        replacement.with_text(|replacement| {
        let inserted = replacement.chars().count();
        let deleted = end - start;
        if inserted == 0 && deleted == 0 {
            return None;
        }
        // Undo inserts the old text before removing the new text.
        self.record_insert(end, inserted);
        let undo = if deleted > 0 {
            self.record_delete_with_markers(start, self.text.substring(start, end), false)
        } else {
            None
        };
        self.text.delete(start, end);
        self.text.insert(start, replacement);
        let adjust = |buffer: &mut Buffer, active: bool| {
            let relocate = |position: usize| {
                if position >= end {
                    position - deleted + inserted
                } else if position > start {
                    start
                } else {
                    position
                }
            };
            if active && (buffer.point > start || buffer.point == end) {
                buffer.point = start + inserted + buffer.point.saturating_sub(end);
            } else if !active {
                buffer.point = if deleted == 0 && buffer.point == start {
                    start
                } else {
                    relocate(buffer.point)
                };
            }
            if deleted == 0 {
                buffer.adjust_markers_insert(start, inserted, false);
                return;
            }
            for marker in buffer.markers.iter().filter_map(Weak::upgrade) {
                let mut marker = marker.borrow_mut();
                if marker.buffer == Some(buffer.id) {
                    marker.position = relocate(marker.position);
                }
            }
            buffer.begv = if buffer.begv == start {
                start
            } else {
                relocate(buffer.begv)
            };
            buffer.zv = relocate(buffer.zv);
            // GNU moves overlays by inserting at the OLD end first; doing
            // deletion first would collapse or evaporate live overlays.
            buffer.adjust_overlays_insert(end, inserted, true);
            buffer.adjust_overlays_delete(start, end);
        };
        adjust(self, true);
        self.text
            .clone()
            .each_sibling(self.id, |sibling| adjust(sibling, false));
        self.note_text_change(deleted + inserted);
        undo

        })
    }

    /// GNU editfns.c transpose_markers. Overlays keep their coordinates,
    /// and marker insertion types do not participate in this permutation.
    pub(crate) fn transpose_markers(&mut self, regions: [usize; 4]) {
        let [start1, end1, start2, end2] = regions;
        let map = |position: usize| {
            if position < start1 || position >= end2 {
                position
            } else if position < end1 {
                position + end2 - end1
            } else if position < start2 {
                position + (end2 - start2) - (end1 - start1)
            } else {
                position - (start2 - start1)
            }
        };
        let adjust = |buffer: &mut Buffer, inactive: bool| {
            buffer.point = map(buffer.point);
            for marker in buffer.markers.iter().filter_map(Weak::upgrade) {
                let mut marker = marker.borrow_mut();
                if marker.buffer == Some(buffer.id) {
                    marker.position = map(marker.position);
                }
            }
            if inactive {
                let (begv, zv) = (map(buffer.begv), map(buffer.zv));
                // GNU can reverse inactive restriction markers when both
                // boundaries cross regions. Keep our buffer interval valid.
                buffer.begv = begv.min(zv);
                buffer.zv = begv.max(zv);
            }
        };
        adjust(self, false);
        self.text
            .clone()
            .each_sibling(self.id, |buffer| adjust(buffer, true));
    }

    pub fn register_marker(&mut self, m: &Rc<RefCell<Marker>>) {
        // A marker must appear once per buffer: repeated `set-marker'
        // calls on the same marker must not multiply-count its
        // position adjustments on edits.
        if self
            .markers
            .iter()
            .any(|w| w.upgrade().is_some_and(|x| Rc::ptr_eq(&x, m)))
        {
            return;
        }
        self.markers.push(Rc::downgrade(m));
    }

    /// The mark as a plain position (0-based char index), or None when
    /// the mark is unset — i.e. the mark marker is absent or detached
    /// (GNU `marker_position (BVAR (b, mark))' → nil when buffer = 0).
    pub fn mark_position(&self) -> Option<usize> {
        self.mark
            .as_ref()
            .and_then(|m| m.borrow().buffer.is_some().then(|| m.borrow().position))
    }

    /// The live mark marker object (GNU `mark-marker'): hands out the
    /// same Rc every time, so `set-marker' on it moves the mark.
    /// Lazily allocates a detached marker (buffer = None → the mark
    /// reads as unset) when none exists yet.
    pub fn mark_marker(&mut self) -> MarkerRef {
        if self.mark.is_none() {
            self.mark = Some(crate::lisp::memory::allocate(
                Marker {
                    buffer: None,
                    position: 0,
                    insertion_type: false,
                },
                crate::lisp::Value::Marker,
            ));
        }
        self.mark.as_ref().unwrap().clone()
    }

    /// GNU `set-mark' core: point the mark marker at `pos' (0-based).
    /// An existing marker (even detached) is re-attached in place so
    /// handles obtained via `mark-marker' stay live; otherwise a fresh
    /// marker is allocated and registered for text adjustment.
    pub fn set_mark(&mut self, pos: usize) {
        let m = self.mark_marker();
        {
            let mut mm = m.borrow_mut();
            mm.buffer = Some(self.id);
            mm.position = pos;
        }
        self.register_marker(&m);
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
        if self.text.mod_tick.get() as i64 <= self.text.save_tick.get() as i64 {
            self.record_first_change();
        }
        // Right after a boundary, record where point was before the
        // command started so undo can restore it.
        if at_boundary && self.undo_pt_before.map_or(false, |p| p != beg) {
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
                        elt.borrow_mut().cdr = Value::Int(beg as i128 + 1 + length as i128);
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
    /// deleted text, using GNU's deletion-position convention.
    pub fn record_delete(
        &mut self,
        beg: usize,
        text: EmacsText,
    ) -> Option<crate::lisp::value::StrRef> {
        self.record_delete_with_markers(beg, text, true)
    }

    fn record_delete_with_markers(
        &mut self,
        beg: usize,
        text: EmacsText,
        record_markers: bool,
    ) -> Option<crate::lisp::value::StrRef> {
        if self.undo_disabled() {
            return None;
        }
        self.record_point(beg + 1);
        let n = text.chars().count();
        let sbeg = if self.point + 1 == beg + 1 + n {
            -(beg as i128 + 1)
        } else {
            beg as i128 + 1
        };
        if record_markers {
            self.record_marker_adjustments(beg, beg + n);
        }
        let text = Value::string_ref(text);
        self.push_undo(Value::cons(Value::Str(text.clone()), Value::Int(sbeg)));
        Some(text)
    }

    /// Record an in-place replacement without relocating markers or
    /// adding deletion's marker-adjustment records.
    pub(crate) fn record_change<T: TextSource + ?Sized>(
        &mut self,
        start: usize,
        old: &T,
        new_len: usize,
    ) -> Option<crate::lisp::value::StrRef> {
        old.with_text(|old| {
        if self.undo_disabled() {
            return None;
        }
        self.record_point(start + 1);
        let position = if self.point == start + old.chars().count() {
            -(start as i128 + 1)
        } else {
            start as i128 + 1
        };
        let text = Value::string_ref(old);
        self.push_undo(Value::cons(Value::Str(text.clone()), Value::Int(position)));
        self.record_insert(start, new_len);
        Some(text)

        })
    }

    /// The effective value of PROP (last write wins) at 0-based POS,
    /// or nil when no covering entry exists.
    pub fn prop_value_at(&self, pos: usize, prop: u32) -> Value {
        for tp in self.text.props.borrow().iter().rev() {
            if tp.prop == prop && tp.start <= pos && pos < tp.end {
                return tp.value.clone();
            }
        }
        Value::Nil
    }

    /// The effective plist (prop -> value, last write wins) at POS.
    pub fn plist_at(&self, pos: usize) -> std::collections::BTreeMap<u32, Value> {
        let mut m = std::collections::BTreeMap::new();
        for tp in self.text.props.borrow().iter().rev() {
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
        if self.text.mod_tick.get() as i64 <= self.text.save_tick.get() as i64 {
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

    /// GNU insdel tick: `MODIFF += elogb(nchars) + 1' then
    /// `CHARS_MODIFF = MODIFF' (the chars counter mirrors MODIFF's
    /// absolute value, catching up any property-only bumps).
    pub fn note_text_change(&mut self, nchars: usize) {
        self.note_modified(true);
        let t = self.text.mod_tick.get() + Self::modiff_incr(nchars);
        self.text.mod_tick.set(t);
        self.text.chars_mod_tick.set(t);
    }

    /// GNU modify_text for an in-place edit: one first-change record,
    /// one composite tick. NOUNDO suppresses undo/file locking and follows
    /// subst-char-in-region's SAVE_MODIFF adjustment.
    pub(crate) fn prepare_inplace_change(&mut self, nchars: usize, no_undo: bool) {
        let saved = self.text.save_tick.get();
        if !no_undo {
            if self.text.mod_tick.get() as i64 <= saved as i64 {
                self.record_first_change();
            }
            self.note_modified(true);
        }
        let tick = self.text.mod_tick.get() + Self::modiff_incr(nchars);
        self.text.mod_tick.set(tick);
        self.text.chars_mod_tick.set(tick);
        self.text.save_tick.set(if no_undo && saved == tick - 1 {
            tick
        } else {
            saved
        });
    }

    /// GNU `modify_text_properties': first-change record (when this is
    /// the first change since save) + `MODIFF += 1'.  CHARS_MODIFF is
    /// not touched.
    pub fn note_prop_modified(&mut self) {
        if self.text.mod_tick.get() as i64 <= self.text.save_tick.get() as i64 {
            self.record_first_change();
        }
        self.text.mod_tick.set(self.text.mod_tick.get() + 1);
        self.note_modified(true);
    }

    /// GNU `record_marker_adjustments': markers inside [FROM, TO] get
    /// (MARKER . ADJUSTMENT) entries pushed before the deletion record.
    fn record_marker_adjustments(&mut self, from: usize, to: usize) {
        let mut adjs = Vec::new();
        for w in &self.markers {
            if let Some(m) = w.upgrade() {
                let mm = m.borrow();
                if mm.buffer == Some(self.id) && from <= mm.position && mm.position <= to {
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

    /// Add a boundary only to nonempty history without one. Remember
    /// the pre-command point even when recording history is empty.
    pub fn undo_boundary(&mut self) {
        if self.undo_disabled() {
            return;
        }
        let at_boundary = match &self.undo_list() {
            Value::Nil => true,
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
    name_map: HashMap<EmacsText, usize>,
    counter: u64,
    /// SymId of `buffer-undo-list', stamped onto new buffers (set by
    /// the interpreter once the obarray exists).
    undo_sym: SymId,
    /// Shared `undo-inhibit-record-point' flag for all buffers.
    undo_inhibit: Rc<std::cell::Cell<bool>>,
}

impl BufferSet {
    pub(crate) fn dump_registry(&self) -> (Vec<Option<BufferRef>>, Vec<usize>, u64, SymId, bool) {
        (
            self.bufs.clone(),
            self.order.clone(),
            self.counter,
            self.undo_sym,
            self.undo_inhibit.get(),
        )
    }

    pub(crate) fn prepare_dump_slots(names: &[Option<EmacsText>]) -> Self {
        let mut set = Self::new();
        for (id, name) in names.iter().enumerate() {
            let buffer = name.as_ref().map(|name| {
                let buffer = crate::lisp::memory::allocate(
                    Buffer::new(id, name.clone(), 0, set.undo_inhibit.clone()),
                    Value::Buffer,
                );
                buffer.borrow().text.add_viewer(&buffer);
                buffer
            });
            set.bufs.push(buffer);
        }
        set
    }

    pub(crate) fn finish_dump_registry(
        &mut self,
        order: Vec<usize>,
        counter: u64,
        undo_sym: SymId,
        undo_inhibit: bool,
    ) -> Result<(), String> {
        let mut names = HashMap::default();
        for (id, buffer) in self.bufs.iter().enumerate() {
            if let Some(buffer) = buffer {
                let mut buffer = buffer.borrow_mut();
                if buffer.id != id {
                    return Err("Invalid dumped buffer id".into());
                }
                buffer.undo_sym = undo_sym;
                buffer.undo_inhibit = self.undo_inhibit.clone();
                if buffer.live && names.insert(buffer.name.clone(), id).is_some() {
                    return Err("Duplicate live buffer names in dump".into());
                }
            }
        }
        if order
            .iter()
            .any(|id| self.get(*id).is_none_or(|b| !b.borrow().live))
        {
            return Err("Invalid dumped buffer order".into());
        }
        self.name_map = names;
        self.order = order;
        self.counter = counter;
        self.undo_sym = undo_sym;
        self.undo_inhibit.set(undo_inhibit);
        Ok(())
    }

    pub fn new() -> Self {
        BufferSet {
            bufs: Vec::new(),
            order: Vec::new(),
            name_map: HashMap::default(),
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
                if let Some(v) = bb.locals.shift_remove(&old) {
                    bb.locals.insert(sym, v);
                }
                bb.undo_sym = sym;
            }
        }
    }

    /// Create a buffer; if the name exists, uniquify with `<N>`.
    pub fn create<T: TextSource + ?Sized>(&mut self, name: &T) -> usize {
        name.with_text(|name| {
        let mut final_name = name.to_string();
        let mut n = 2;
        while self.name_map.contains_key(&final_name) {
            final_name = name.to_string();
            final_name.push_str(format!("<{n}>"));
            n += 1;
        }
        let id = self.bufs.len();
        let buf = Buffer::new(
            id,
            final_name.clone(),
            self.undo_sym,
            self.undo_inhibit.clone(),
        );
        let rc = crate::lisp::memory::allocate(buf, crate::lisp::Value::Buffer);
        rc.borrow().text.add_viewer(&rc);
        self.bufs.push(Some(rc));
        self.name_map.insert(final_name, id);
        self.order.push(id);
        self.counter += 1;
        id

        })
    }

    /// Create a buffer with exactly this name (kills none; used at init).
    /// `last_name' is the creation name (GNU get-buffer-create).
    pub fn create_exact<T: TextSource + ?Sized>(&mut self, name: &T) -> usize {
        name.with_text(|name| {
        self.create_exact_named(name, true)

        })
    }

    /// Boot-time `*scratch*'-only variant: GNU `init_buffer' leaves
    /// `b->last_name' nil, so `(buffer-last-name " *scratch*")' is nil.
    pub fn create_exact_nolast<T: TextSource + ?Sized>(&mut self, name: &T) -> usize {
        name.with_text(|name| {
        self.create_exact_named(name, false)

        })
    }

    fn create_exact_named<T: TextSource + ?Sized>(&mut self, name: &T, with_last: bool) -> usize {
        name.with_text(|name| {
        let id = self.bufs.len();
        let mut buf = Buffer::new(
            id,
            name.to_string(),
            self.undo_sym,
            self.undo_inhibit.clone(),
        );
        if !with_last {
            buf.last_name = None;
        }
        let rc = crate::lisp::memory::allocate(buf, crate::lisp::Value::Buffer);
        rc.borrow().text.add_viewer(&rc);
        self.bufs.push(Some(rc));
        self.name_map.insert(name.to_string(), id);
        self.order.push(id);
        id

        })
    }

    pub fn get(&self, id: usize) -> Option<BufferRef> {
        self.bufs.get(id).and_then(|o| o.as_ref()).cloned()
    }

    pub fn by_name<T: TextSource + ?Sized>(&self, name: &T) -> Option<usize> {
        name.with_text(|name| {
        self.name_map.get(name).copied()

        })
    }

    /// Kill buffer `id`.  `always_local` identifies DEFVAR_PER_BUFFER
    /// variables: GNU's kill_buffer clears the buffer-local alist but
    /// keeps the per-buffer slots' values (so `default-directory' etc.
    /// still read on the killed object) while `buffer-undo-list' is
    /// explicitly reset to nil.
    pub fn kill(&mut self, id: usize, always_local: impl Fn(SymId) -> bool) -> bool {
        if id >= self.bufs.len() || self.bufs[id].is_none() {
            return false;
        }
        {
            let mut bb = self.bufs[id].as_ref().unwrap().borrow_mut();
            bb.live = false;
            // GNU: last_name = name before unchaining from the alist.
            bb.last_name = Some(bb.name.clone());
            // GNU: `bset_undo_list (b, Qnil)' happens before teardown.
            let undo = bb.undo_sym;
            bb.locals.insert(undo, Value::Nil);
            // GNU reset_buffer_local_variables: drop ordinary locals,
            // keep the per-buffer slot bindings (always_local).
            bb.locals.retain(|s, _| always_local(*s));
            // Emacs unchains markers on kill: they point nowhere.
            for w in &bb.markers {
                if let Some(m) = w.upgrade() {
                    // Registration chains can retain a weak entry after a
                    // marker has moved to another buffer.
                    let mut marker = m.borrow_mut();
                    if marker.buffer == Some(id) {
                        marker.buffer = None;
                    }
                }
            }
            bb.markers.clear();
            // The mark marker detaches too.
            if let Some(m) = &bb.mark {
                m.borrow_mut().buffer = None;
            }
            // Overlays detach too (`overlay-buffer' → nil).
            for (slot, ov) in bb.overlays.iter_mut().enumerate() {
                ov.buffer = None;
                // Moved overlays leave stubs behind. Only the current owner
                // may snapshot a handle, or an old stub could overwrite it.
                if ov.owns_handle_at(id, slot) {
                    ov.save_detached_state();
                }
            }
        }
        let name = self.bufs[id].as_ref().unwrap().borrow().name.clone();
        self.name_map.remove(&name);
        self.bufs[id] = None;
        self.order.retain(|&x| x != id);
        true
    }

    /// Rename buffer `id`; returns the actual (possibly uniquified) name.
    pub fn rename<T: TextSource + ?Sized>(&mut self, id: usize, new_name: &T) -> EmacsText {
        new_name.with_text(|new_name| {
        let old_name = match self.get(id) {
            Some(b) => b.borrow().name.clone(),
            None => return EmacsText::new(),
        };
        self.name_map.remove(&old_name);
        let mut final_name = new_name.to_string();
        let mut n = 2;
        while self.name_map.contains_key(&final_name) {
            final_name = new_name.to_string();
            final_name.push_str(format!("<{n}>"));
            n += 1;
        }
        if let Some(b) = self.get(id) {
            let mut bb = b.borrow_mut();
            // GNU Frename_buffer: last_name records the previous name.
            bb.last_name = Some(old_name.clone());
            bb.name = final_name.clone();
        }
        self.name_map.insert(final_name.clone(), id);
        final_name

        })
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
    /// GNU scans the global list from its head, skipping BUFFER by identity.
    fn other_scan(&self, exclude: usize, pred: &dyn Fn(usize) -> bool) -> Option<usize> {
        self.order.iter().copied().find(|&id| id != exclude && pred(id))
    }
    /// First live non-space-named buffer in selection order.
    pub fn other(&self, exclude: usize) -> Option<usize> {
        self.other_scan(exclude, &|id| self.live_user_buffer(id))
    }
    /// Live non-space buffer not in `shown', in selection order.
    pub fn other_unshown(
        &self,
        exclude: usize,
        shown: &std::collections::HashSet<usize>,
    ) -> Option<usize> {
        self.other_scan(exclude, &|id| {
            self.live_user_buffer(id) && !shown.contains(&id)
        })
    }

    fn live_user_buffer(&self, id: usize) -> bool {
        self.get(id)
            .map(|b| {
                let bb = b.borrow();
                bb.live && !bb.name.starts_with(' ')
            })
            .unwrap_or(false)
    }

    /// GNU `other_buffer' (buffer.c): prefer a buffer not shown in any
    /// window of the frame; fall back to the first candidate when all
    /// are displayed.  `visible_ok' is Fother_buffer's VISIBLE-OK arg.
    pub fn other_visible_pref(
        &self,
        exclude: usize,
        shown: &std::collections::HashSet<usize>,
        visible_ok: bool,
    ) -> Option<usize> {
        if !visible_ok {
            if let Some(id) = self.other_unshown(exclude, shown) {
                return Some(id);
            }
        }
        self.other(exclude)
    }
    pub fn unique_name<T: TextSource + ?Sized>(&self, base: &T) -> EmacsText {
        base.with_text(|base| {
        if !self.name_map.contains_key(base) {
            return base.to_string();
        }
        for n in 2.. {
            let mut cand = base.to_string();
            cand.push_str(format!("<{n}>"));
            if !self.name_map.contains_key(&cand) {
                return cand;
            }
        }
        unreachable!()

        })
    }
}

impl Default for BufferSet {
    fn default() -> Self {
        Self::new()
    }
}
