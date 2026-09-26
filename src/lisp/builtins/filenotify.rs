//! File-system notification primitives, a port of GNU `src/kqueue.c`
//! (plus `insert-special-event` delivery and the `special-event-map'
//! dispatch GNU's keyboard loop performs).
//!
//! Events from the kernel kqueue fd are collected on a reader thread
//! into `PENDING'; the Lisp thread drains them in the
//! `sleep-firing-timers' wait loop (the remacs equivalent of GNU's
//! `kbd_buffer_store_event' + command-loop dispatch).

use std::collections::HashMap;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Mutex, OnceLock};

use super::{want_list, want_string, S};
use crate::lisp::error::{EvalResult, Flow};
use crate::lisp::obarray::sym;
use crate::lisp::value::{SymId, Value};

// kqueue vnode note flags (FreeBSD/macOS <sys/event.h>).
const NOTE_DELETE: u32 = 0x0001;
const NOTE_WRITE: u32 = 0x0002;
const NOTE_EXTEND: u32 = 0x0004;
const NOTE_ATTRIB: u32 = 0x0008;
const NOTE_LINK: u32 = 0x0010;
const NOTE_RENAME: u32 = 0x0020;
const NOTE_REVOKE: u32 = 0x0040;

// kqueue() itself is only real on BSD-ish targets; elsewhere the
// primitives report GNU's "File watching is not available" error.
#[cfg(any(
    target_os = "macos",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
const HAVE_KQUEUE: bool = true;
#[cfg(not(any(
    target_os = "macos",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
)))]
const HAVE_KQUEUE: bool = false;

// ---------------------------------------------------------------------------
// Global kernel-side state
// ---------------------------------------------------------------------------

/// The shared kqueue fd (GNU's `kqueuefd').
static KQ_FD: AtomicI32 = AtomicI32::new(-1);
/// Raw `(fd, fflags)' events read off the kqueue fd by the reader thread.
static PENDING: Mutex<Vec<(i32, u32)>> = Mutex::new(Vec::new());
static KQ_THREAD_ONCE: OnceLock<()> = OnceLock::new();

#[derive(Clone)]
pub(crate) struct DirEnt {
    ino: u64,
    name: String,
    /// mtime in nanoseconds since epoch.
    mtime_ns: i64,
    /// ctime (status-change) in nanoseconds since epoch.
    ctime_ns: i64,
    size: u64,
}

/// A `kqueue' watch (GNU's watch_object: (descriptor file flags
/// callback [dir-list])).
pub(crate) struct KqWatch {
    /// Descriptor returned to Lisp — the vnode fd, like GNU.
    desc: i64,
    file: String,
    callback: Value,
    /// `Some' for directory watches: last directory listing.
    dir_list: Option<Vec<DirEnt>>,
}

/// Interp-side watch table (GNU's `watch_list').
pub struct FnState {
    watches: Vec<KqWatch>,
}

impl FnState {
    pub fn new() -> Self {
        FnState {
            watches: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// kqueue plumbing
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
mod sys {
    // O_EVTONLY on macOS; elsewhere falls back to O_RDONLY.
    pub const O_EVTONLY: i32 = 0x8000;
    pub const O_SYMLINK: i32 = 0x200000;
}
#[cfg(not(target_os = "macos"))]
mod sys {
    pub const O_EVTONLY: i32 = 0;
    pub const O_SYMLINK: i32 = 0;
}

fn ensure_kqueue() -> i32 {
    let cur = KQ_FD.load(Ordering::SeqCst);
    if cur >= 0 {
        return cur;
    }
    if !HAVE_KQUEUE {
        return -1;
    }
    let fd = unsafe { libc::kqueue() };
    if fd < 0 {
        return -1;
    }
    if KQ_FD
        .compare_exchange(-1, fd, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        unsafe { libc::close(fd) };
        return KQ_FD.load(Ordering::SeqCst);
    }
    KQ_THREAD_ONCE.get_or_init(|| {
        std::thread::Builder::new()
            .name("filenotify-kqueue".into())
            .spawn(kq_reader_loop)
            .ok();
    });
    fd
}

fn kq_reader_loop() {
    loop {
        let kq = KQ_FD.load(Ordering::SeqCst);
        if kq < 0 {
            return;
        }
        let mut evs: [libc::kevent; 16] = unsafe { std::mem::zeroed() };
        let n = unsafe {
            libc::kevent(
                kq,
                std::ptr::null(),
                0,
                evs.as_mut_ptr(),
                evs.len() as i32,
                std::ptr::null(),
            )
        };
        if n <= 0 {
            if n < 0 && KQ_FD.load(Ordering::SeqCst) != kq {
                continue; // fd was recycled
            }
            if n < 0 {
                // kqueue fd closed — retry after a tick; if it stays
                // closed the watches are gone and we simply sleep.
                std::thread::sleep(std::time::Duration::from_millis(50));
                continue;
            }
            continue;
        }
        let mut pend = PENDING.lock().unwrap();
        for ev in &evs[..n as usize] {
            if ev.filter == libc::EVFILT_VNODE as i16 {
                pend.push((ev.ident as i32, ev.fflags));
            }
        }
    }
}

fn register_watch_fd(kq: i32, fd: i32, fflags: u32) -> i32 {
    let kev = libc::kevent {
        ident: fd as usize,
        filter: libc::EVFILT_VNODE as i16,
        flags: (libc::EV_ADD | libc::EV_ENABLE | libc::EV_CLEAR) as u16,
        fflags,
        data: 0,
        udata: std::ptr::null_mut(),
    };
    unsafe { libc::kevent(kq, &kev, 1, std::ptr::null_mut(), 0, std::ptr::null()) }
}

fn directory_listing(dir: &str) -> Vec<DirEnt> {
    use std::os::unix::fs::MetadataExt;
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name == "." || name == ".." {
                continue;
            }
            if let Ok(md) = std::fs::symlink_metadata(e.path()) {
                out.push(DirEnt {
                    ino: md.ino(),
                    name,
                    mtime_ns: md.mtime() * 1_000_000_000 + md.mtime_nsec(),
                    ctime_ns: md.ctime() * 1_000_000_000 + md.ctime_nsec(),
                    size: md.len(),
                });
            }
        }
    }
    out
}

fn is_dir(p: &str) -> bool {
    std::fs::metadata(p).map(|m| m.is_dir()).unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Lisp helpers
// ---------------------------------------------------------------------------

fn intern(i: &mut crate::lisp::eval::Interp, s: &str) -> SymId {
    i.intern(s)
}

fn file_notify_error(i: &mut crate::lisp::eval::Interp, data: Vec<Value>) -> Flow {
    let s = intern(i, "file-notify-error");
    i.signal_data(s, data)
}

fn call_lisp(
    i: &mut crate::lisp::eval::Interp,
    f: &str,
    args: Vec<Value>,
) -> Result<Value, Flow> {
    let fun = Value::Sym(i.intern(f));
    i.apply(&fun, args)
}

fn car_safe(v: &Value) -> Value {
    match v {
        Value::Cons(c) => c.borrow().car.clone(),
        _ => Value::Nil,
    }
}

/// Dispatch a lispy special event the way GNU's command loop does:
/// look up `(car EVENT)' in `special-event-map' and call the binding
/// with EVENT.  Used by `insert-special-event' and the filenotify
/// event pump (GNU dispatches on the next event read; a batch
/// interpreter has no read loop, so we dispatch synchronously).
pub(crate) fn dispatch_special_event(
    i: &mut crate::lisp::eval::Interp,
    event: &Value,
) -> EvalResult {
    let key = car_safe(event);
    if key.is_nil() {
        return Ok(Value::Nil);
    }
    let sem = intern(i, "special-event-map");
    let map = i.symbol_value(sem);
    if map.is_nil() {
        return Ok(Value::Nil);
    }
    let lookup = Value::Sym(intern(i, "lookup-key"));
    let vec = match &key {
        Value::Sym(s) => Value::Vec(std::rc::Rc::new(std::cell::RefCell::new(vec![
            Value::Sym(*s),
        ]))),
        _ => return Ok(Value::Nil),
    };
    let binding = i.apply(&lookup, vec![map, vec, Value::t()])?;
    if binding.is_nil() || matches!(binding, Value::Sym(s) if s == sym::UNBOUND) {
        return Ok(Value::Nil);
    }
    i.apply(&binding, vec![event.clone()])
}

fn f_insert_special_event(i: &mut crate::lisp::eval::Interp, a: Vec<Value>) -> EvalResult {
    if !matches!(&a[0], Value::Cons(_)) {
        return Err(i.wrong_type_mut("consp", &a[0]));
    }
    // GNU: check that the event kind has a binding in special-event-map.
    let key = car_safe(&a[0]);
    let sem = intern(i, "special-event-map");
    let map = i.symbol_value(sem);
    let lookup = Value::Sym(intern(i, "lookup-key"));
    let vec = match &key {
        Value::Sym(s) => Value::Vec(std::rc::Rc::new(std::cell::RefCell::new(vec![
            Value::Sym(*s),
        ]))),
        _ => return Ok(Value::Nil),
    };
    let binding = i.apply(&lookup, vec![map, vec, Value::t()])?;
    if binding.is_nil() || matches!(binding, Value::Sym(s) if s == sym::UNBOUND) {
        return Err(i.error_obj("Invalid special event kind", &key));
    }
    // GNU queues the event for the next command-loop iteration; batch
    // remacs dispatches synchronously.
    i.apply(&binding, vec![a[0].clone()])
}

// ---------------------------------------------------------------------------
// kqueue DEFUNs
// ---------------------------------------------------------------------------

fn flag_sym(i: &mut crate::lisp::eval::Interp, name: &str) -> Value {
    Value::Sym(intern(i, name))
}

/// `directory-file-name' equivalent (trim trailing /, keep all-/').
fn directory_file_name(s: &str) -> String {
    if s.chars().all(|c| c == '/') {
        return s.to_string();
    }
    s.trim_end_matches('/').to_string()
}

fn f_kqueue_add_watch(i: &mut crate::lisp::eval::Interp, a: Vec<Value>) -> EvalResult {
    let file = want_string(i, &a[0])?;
    let flags = want_list(i, &a[1])?;
    let callback = a[2].clone();
    let fp = call_lisp(i, "functionp", vec![callback.clone()])?;
    if fp.is_nil() {
        return Err(i.wrong_type_mut("invalid-function", &callback));
    }

    // file = Fdirectory_file_name (Fexpand_file_name (file, Qnil))
    let file = crate::editor::expand_file_name_str(i, &file);
    let file = directory_file_name(&file);

    if !std::path::Path::new(&file).exists() {
        return Err(i.error_obj("File does not exist", &Value::string(file)));
    }

    if !HAVE_KQUEUE {
        return Err(file_notify_error(
            i,
            vec![
                Value::string("File watching is not available"),
                Value::Nil,
            ],
        ));
    }
    let kq = ensure_kqueue();
    if kq < 0 {
        return Err(file_notify_error(
            i,
            vec![
                Value::string("File watching is not available"),
                Value::Nil,
            ],
        ));
    }

    // Assemble note flags from the flags list.
    let mut has = |name: &str| flags.iter().any(|f| i.sym_id(f) == Some(intern(i, name)));
    let mut fflags = 0u32;
    if has("delete") {
        fflags |= NOTE_DELETE;
    }
    if has("write") {
        fflags |= NOTE_WRITE;
    }
    if has("extend") {
        fflags |= NOTE_EXTEND;
    }
    if has("attrib") {
        fflags |= NOTE_ATTRIB;
    }
    if has("link") {
        fflags |= NOTE_LINK;
    }
    if has("rename") {
        fflags |= NOTE_RENAME;
    }
    if has("revoke") {
        fflags |= NOTE_REVOKE;
    }

    // Open the file the way GNU does (O_EVTONLY|O_SYMLINK preferred).
    let cpath = std::ffi::CString::new(file.clone())
        .map_err(|_| i.error("File name contains NUL"))?;
    let mut oflags = libc::O_NONBLOCK | sys::O_EVTONLY;
    if sys::O_SYMLINK != 0 {
        oflags |= sys::O_SYMLINK;
    } else {
        oflags |= libc::O_NOFOLLOW;
    }
    if sys::O_EVTONLY == 0 {
        oflags |= libc::O_RDONLY;
    }
    let fd = unsafe { libc::open(cpath.as_ptr(), oflags) };
    if fd < 0 {
        return Err(i.error_obj("File cannot be opened", &Value::string(file)));
    }

    if register_watch_fd(kq, fd, fflags) < 0 {
        unsafe { libc::close(fd) };
        return Err(i.error_obj("Cannot watch file", &Value::string(file)));
    }

    let dir_watch = is_dir(&file);
    let watch = KqWatch {
        desc: fd as i64,
        file: file.clone(),
        callback: callback.clone(),
        dir_list: if dir_watch {
            Some(directory_listing(&file))
        } else {
            None
        },
    };
    i.filenotify.watches.push(watch);
    Ok(Value::Int(fd as i64 as i128))
}

fn find_watch_pos(state: &FnState, desc: i64) -> Option<usize> {
    state.watches.iter().position(|w| w.desc == desc)
}

fn f_kqueue_rm_watch(i: &mut crate::lisp::eval::Interp, a: Vec<Value>) -> EvalResult {
    let desc = match &a[0] {
        Value::Int(n) => *n as i64,
        _ => {
            return Err(file_notify_error(
                i,
                vec![Value::string("Not a watch descriptor"), a[0].clone()],
            ))
        }
    };
    let Some(pos) = find_watch_pos(&i.filenotify, desc) else {
        return Err(file_notify_error(
            i,
            vec![Value::string("Not a watch descriptor"), a[0].clone()],
        ));
    };
    i.filenotify.watches.remove(pos);
    unsafe { libc::close(desc as i32) };

    // GNU closes the kqueue fd when the watch list is empty.
    if i.filenotify.watches.is_empty() {
        let kq = KQ_FD.swap(-1, Ordering::SeqCst);
        if kq >= 0 {
            unsafe { libc::close(kq) };
        }
    }
    Ok(Value::t())
}

fn f_kqueue_valid_p(i: &mut crate::lisp::eval::Interp, a: Vec<Value>) -> EvalResult {
    let ok = matches!(&a[0], Value::Int(n) if find_watch_pos(&i.filenotify, *n as i64).is_some());
    if ok {
        Ok(Value::t())
    } else {
        Ok(Value::Nil)
    }
}

// ---------------------------------------------------------------------------
// Event translation + delivery
// ---------------------------------------------------------------------------

/// Build `(file-notify (DESC ACTIONS FILE [FILE1]) CALLBACK)' — GNU's
/// `(Qfile_notify . arg)' event, i.e. the `file-notify' cl-defstruct.
fn make_event(
    i: &mut crate::lisp::eval::Interp,
    desc: i64,
    actions: Vec<Value>,
    file: &str,
    file1: Option<&str>,
    callback: &Value,
) -> Value {
    let mut ev = vec![
        Value::Int(desc as i128),
        Value::list(actions),
        Value::string(file),
    ];
    if let Some(f1) = file1 {
        ev.push(Value::string(f1));
    }
    Value::list(vec![
        flag_sym(i, "file-notify"),
        Value::list(ev),
        callback.clone(),
    ])
}

/// Port of GNU `kqueue_generate_event' + queueing.
fn emit(
    i: &mut crate::lisp::eval::Interp,
    w: &KqWatch,
    actions: &[&str],
    file: &str,
    file1: Option<&str>,
) -> EvalResult {
    if actions.is_empty() {
        return Ok(Value::Nil);
    }
    let acts: Vec<Value> = actions.iter().map(|n| flag_sym(i, n)).collect();
    let ev = make_event(i, w.desc, acts, file, file1, &w.callback);
    dispatch_special_event(i, &ev)
}

/// Port of GNU `kqueue_compare_dir_list'.
fn compare_dir_list(
    i: &mut crate::lisp::eval::Interp,
    idx: usize,
) -> EvalResult {
    let (dir, old) = {
        let w = &i.filenotify.watches[idx];
        (w.file.clone(), w.dir_list.clone().unwrap_or_default())
    };

    if !is_dir(&dir) {
        let w = i.filenotify.watches[idx].clone_watch();
        return emit(i, &w, &["delete"], &dir, None);
    }
    let mut new_dl = directory_listing(&dir);
    let mut pending: Vec<DirEnt> = Vec::new();
    let mut deleted: Vec<DirEnt> = Vec::new();

    for old_e in &old {
        // Same inode?
        if let Some(np) = new_dl.iter().position(|n| n.ino == old_e.ino) {
            let ne = new_dl[np].clone();
            if ne.name == old_e.name
                && ne.mtime_ns == old_e.mtime_ns
                && ne.ctime_ns == old_e.ctime_ns
                && ne.size == old_e.size
            {
                new_dl.remove(np);
                continue;
            }
            if ne.name == old_e.name {
                if ne.mtime_ns != old_e.mtime_ns {
                    let w = i.filenotify.watches[idx].clone_watch();
                    emit(i, &w, &["write"], &old_e.name, None)?;
                }
                if ne.ctime_ns != old_e.ctime_ns {
                    let w = i.filenotify.watches[idx].clone_watch();
                    emit(i, &w, &["attrib"], &old_e.name, None)?;
                }
            } else {
                let w = i.filenotify.watches[idx].clone_watch();
                emit(
                    i,
                    &w,
                    &["rename"],
                    &old_e.name,
                    Some(&ne.name),
                )?;
                deleted.push(ne.clone());
            }
            new_dl.remove(np);
            continue;
        }

        // Same name, different inode → park into pending.
        if let Some(np) = new_dl.iter().position(|n| n.name == old_e.name) {
            pending.push(new_dl.remove(np));
            continue;
        }

        // Pending rename target (same inode).
        if let Some(pp) = pending.iter().position(|n| n.ino == old_e.ino) {
            let ne = pending.remove(pp);
            let w = i.filenotify.watches[idx].clone_watch();
            emit(
                i,
                &w,
                &["rename"],
                &old_e.name,
                Some(&ne.name),
            )?;
            continue;
        }

        // Renamed-away (name claimed by an earlier rename target).
        if let Some(dp) = deleted.iter().position(|n| n.name == old_e.name) {
            deleted.remove(dp);
            continue;
        }

        // Deleted.
        let w = i.filenotify.watches[idx].clone_watch();
        emit(i, &w, &["delete"], &old_e.name, None)?;
    }

    // New files.
    for ne in &new_dl {
        let w = i.filenotify.watches[idx].clone_watch();
        emit(i, &w, &["create"], &ne.name, None)?;
        if ne.size > 0 {
            let w = i.filenotify.watches[idx].clone_watch();
            emit(i, &w, &["write"], &ne.name, None)?;
        }
    }
    // Still-pending files: assume write.
    for pe in &pending {
        let w = i.filenotify.watches[idx].clone_watch();
        emit(i, &w, &["write"], &pe.name, None)?;
    }

    i.filenotify.watches[idx].dir_list = Some(directory_listing(&dir));
    Ok(Value::Nil)
}

/// Port of GNU `kqueue_callback' for one kernel event.
fn dispatch_kevent(i: &mut crate::lisp::eval::Interp, fd: i32, fflags: u32) -> EvalResult {
    let Some(idx) = find_watch_pos(&i.filenotify, fd as i64) else {
        return Ok(Value::Nil);
    };

    let mut actions: Vec<&'static str> = Vec::new();
    if fflags & NOTE_DELETE != 0 {
        actions.push("delete");
    }
    if fflags & NOTE_WRITE != 0 {
        if i.filenotify.watches[idx].dir_list.is_none() {
            actions.push("write");
        } else {
            compare_dir_list(i, idx)?;
        }
    }
    if fflags & NOTE_EXTEND != 0 {
        actions.push("extend");
    }
    if fflags & NOTE_ATTRIB != 0 {
        actions.push("attrib");
    }
    if fflags & NOTE_LINK != 0 {
        actions.push("link");
    }
    if fflags & NOTE_RENAME != 0 {
        actions.push("rename");
    }
    if fflags & NOTE_REVOKE != 0 {
        actions.push("revoke");
    }

    if !actions.is_empty() {
        // GNU builds the action list with Fcons (prepend) over the
        // same flag-scan order — net order is the reverse of ours.
        actions.reverse();
        let file = i.filenotify.watches[idx].file.clone();
        let w = i.filenotify.watches[idx].clone_watch();
        emit(i, &w, &actions, &file, None)?;
    }

    // GNU removes the watch on delete/rename/revoke.
    if fflags & (NOTE_DELETE | NOTE_RENAME | NOTE_REVOKE) != 0 {
        if let Some(pos) = find_watch_pos(&i.filenotify, fd as i64) {
            i.filenotify.watches.remove(pos);
        }
        unsafe { libc::close(fd) };
        if i.filenotify.watches.is_empty() {
            let kq = KQ_FD.swap(-1, Ordering::SeqCst);
            if kq >= 0 {
                unsafe { libc::close(kq) };
            }
        }
    }
    Ok(Value::Nil)
}

/// Drain kernel events into Lisp callbacks — called from the
/// `sleep_firing_timers' wait pump (and `accept-process-output').
pub fn drain(i: &mut crate::lisp::eval::Interp) -> EvalResult {
    let raw: Vec<(i32, u32)> = {
        let mut p = PENDING.lock().unwrap();
        std::mem::take(&mut *p)
    };
    if std::env::var_os("FN_DEBUG").is_some() && !raw.is_empty() {
        eprintln!("[fndbg] raw events: {:?}", raw);
    }
    for (fd, fflags) in raw {
        dispatch_kevent(i, fd, fflags)?;
    }
    Ok(Value::Nil)
}

// ---------------------------------------------------------------------------

impl KqWatch {
    /// Cheap clone of the pieces `emit' needs — keeps `self.callback'
    /// alive by clone, like GNU's reference into watch_object.
    fn clone_watch(&self) -> KqWatch {
        KqWatch {
            desc: self.desc,
            file: self.file.clone(),
            callback: self.callback.clone(),
            dir_list: None,
        }
    }
}

pub(crate) static SUBRS: &[crate::lisp::value::Subr] = &[
    S!("kqueue-add-watch", 3, 3, f_kqueue_add_watch, "Add a watch for filesystem events pertaining to FILE."),
    S!("kqueue-rm-watch", 1, 1, f_kqueue_rm_watch, "Remove an existing WATCH-DESCRIPTOR."),
    S!("kqueue-valid-p", 1, 1, f_kqueue_valid_p, "Check a watch specified by its WATCH-DESCRIPTOR."),
    S!("insert-special-event", 1, 1, f_insert_special_event, "Insert the special EVENT into the input event queue."),
];

/// `(provide 'kqueue)' — GNU registers it in `syms_of_kqueue' on
/// platforms where kqueue exists.
pub(crate) fn install(i: &mut crate::lisp::eval::Interp) {
    if HAVE_KQUEUE {
        let f = i.intern("kqueue");
        if !i.features.contains(&f) {
            i.features.push(f);
        }
    }
}

// silence unused warnings for helpers used by later modules
#[allow(dead_code)]
fn _unused(_: HashMap<i32, i32>, _: Mutex<()>) {}
