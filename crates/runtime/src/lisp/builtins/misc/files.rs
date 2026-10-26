//! File predicates, metadata updates, temporary names and symlink creation.

use super::time::lisp_time_to_ns;
use crate::lisp::builtins::{arg, want_string};
use crate::lisp::{Interp, error::EvalResult, obarray::sym, value::Value};

// ---------- files ----------

pub(super) fn f_make_temp_file_internal(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
    // GNU Fmake_temp_file_internal: PREFIX is used VERBATIM — never
    // expanded against temporary-file-directory — so a relative prefix
    // resolves in the working directory (make-temp-file wraps it to
    // expand first).  DIR-FLAG selects create-file (nil), name-only
    // verification (0) or mkdir (other non-nil).  "X"*6 is filled from
    // the base-62 alphabet; files get mode 0600, dirs 0700.
    let prefix = want_string(i, &args[0])?;
    let dir_flag = arg(&args, 1);
    let suffix = match arg(&args, 2) {
        Value::Str(s) => s.borrow().clone(),
        v => return Err(i.wrong_type_mut("stringp", &v)),
    };
    let text = arg(&args, 3);
    enum Kind {
        File,
        NoCreate,
        Dir,
    }
    let kind = if dir_flag.is_nil() {
        Kind::File
    } else if matches!(dir_flag, Value::Int(0)) {
        Kind::NoCreate
    } else {
        Kind::Dir
    };
    let kind_message = match kind {
        Kind::File => "Creating file with prefix",
        Kind::Dir => "Creating directory with prefix",
        Kind::NoCreate => "Creating file name with prefix",
    };
    let mut counter = 0u64;
    for _ in 0..62 * 62 * 62 {
        counter = counter.wrapping_add(1);
        let cand = format!("{}{}{}", prefix, tempname_suffix(counter), suffix);
        let res: Result<Option<std::fs::File>, std::io::Error> = match kind {
            Kind::Dir => std::fs::DirBuilder::new()
                .mode(0o700)
                .create(&cand)
                .map(|_| None),
            Kind::NoCreate => match std::fs::symlink_metadata(&cand) {
                Ok(_) => Err(std::io::Error::from_raw_os_error(libc::EEXIST)),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(e) => Err(e),
            },
            Kind::File => std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&cand)
                .map(Some),
        };
        match res {
            Ok(mut f) => {
                if let (Kind::File, Some(fh)) = (&kind, &mut f) {
                    if matches!(&text, Value::Str(string) if !string.borrow().is_empty()) {
                        use std::io::Write;
                        let bytes = crate::editor::temp_file_text_bytes(i, &cand, &text)?;
                        if let Err(e) = fh.write_all(&bytes) {
                            let _ = std::fs::remove_file(&cand);
                            return Err(crate::editor::report_file_errno(
                                i,
                                "Write error",
                                vec![Value::string(cand)],
                                &e,
                            ));
                        }
                    }
                }
                return Ok(Value::string(cand));
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => {
                // GNU report_file_error: the *unexpanded* PREFIX is the
                // reported name, errno picks the symbol.
                return Err(crate::editor::report_file_errno(
                    i,
                    kind_message,
                    vec![args[0].clone()],
                    &e,
                ));
            }
        }
    }
    Err(crate::editor::report_file_errno(
        i,
        kind_message,
        vec![args[0].clone()],
        &std::io::Error::from_raw_os_error(libc::EEXIST),
    ))
}

/// gnulib `gen_tempname' fills "X"*6 from this base-62 alphabet.
fn tempname_suffix(seed: u64) -> String {
    const LETTERS: &[u8; 62] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64)
        .unwrap_or(0)
        ^ ((std::process::id() as u64) << 16)
        ^ seed.wrapping_mul(0x9E3779B97F4A7C15);
    let mut v = n ^ n.rotate_left(32);
    let mut out = String::with_capacity(6);
    for _ in 0..6 {
        out.push(LETTERS[(v % 62) as usize] as char);
        v /= 62;
    }
    out
}

pub(super) fn f_make_symbolic_link(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU Fmake_symbolic_link: TARGET stays verbatim (only an integer
    // OK-IF-ALREADY-EXISTS expands a leading `~'), while LINKNAME is
    // expand_cp_target'd — expanded, and a trailing-slash LINKNAME means
    // "TARGET's basename under that directory".
    let target = match &args[0] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    let mut target = target;
    let ok_if_exists = args.get(2).cloned().unwrap_or(Value::Nil);
    if matches!(ok_if_exists, Value::Int(_)) && target.starts_with('~') {
        target = crate::editor::expand_file_name_str(i, &target);
    }
    let name_raw = match &args[1] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    let name = if name_raw.ends_with('/') {
        // expand_cp_target: LINKNAME a directory name → LINKNAME/basename(TARGET).
        let base = target
            .rsplit('/')
            .find(|s| !s.is_empty())
            .unwrap_or(&target);
        {
            let mut joined = name_raw.clone();
            joined.push_str(base);
            crate::editor::expand_file_name_str(i, &joined)
        }
    } else {
        crate::editor::expand_file_name_str(i, &name_raw)
    };
    let native_target = crate::editor::native_path(i, &target)?;
    let native_name = crate::editor::native_path(i, &name)?;
    match std::os::unix::fs::symlink(&native_target, &native_name) {
        Ok(()) => Ok(Value::Nil),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            match &ok_if_exists {
                // GNU barf_or_query_if_file_exists: nil →
                // file-already-exists; an integer would ask (batch → EOF).
                Value::Nil => {
                    let s = i.q("file-already-exists");
                    Err(i.signal_data(
                        s,
                        vec![Value::string("File already exists"), Value::string(name)],
                    ))
                }
                Value::Int(_) => {
                    let s = i.q("end-of-file");
                    Err(i.signal_data(s, vec![Value::string("Error reading from stdin")]))
                }
                // Other non-nil: unlink and retry, like GNU.
                _ => {
                    let _ = std::fs::remove_file(&native_name);
                    match std::os::unix::fs::symlink(&native_target, &native_name) {
                        Ok(()) => Ok(Value::Nil),
                        Err(e2) => Err(crate::editor::report_file_errno(
                            i,
                            "Making symbolic link",
                            vec![Value::string(target), Value::string(name)],
                            &e2,
                        )),
                    }
                }
            }
        }
        // GNU report_file_error: ("Making symbolic link" strerror TARGET
        // LINKNAME) — errno picks file-missing/permission-denied/file-error.
        Err(e) => Err(crate::editor::report_file_errno(
            i,
            "Making symbolic link",
            vec![Value::string(target), Value::string(name)],
            &e,
        )),
    }
}

pub(super) fn f_directory_name_p(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let s = want_string(i, &args[0])?;
    Ok(Value::from_bool(s.ends_with('/')))
}

pub(crate) fn f_file_name_case_insensitive_p(
    i: &mut Interp,
    args: crate::lisp::Argv,
) -> EvalResult {
    let path = want_string(i, &args[0])?;
    let operation = Value::Sym(i.q("file-name-case-insensitive-p"));
    if let Some(handler) = crate::buffer::extra::find_file_name_handler(i, &path, operation.clone())
    {
        return i.apply(&handler, vec![operation, args[0].clone()]);
    }
    // GNU fileio.c `check_case_insensitive': query the FILESYSTEM of
    // the parent directory (expand-file-name + directory-name), so a
    // nonexistent file under an existing dir still answers t on APFS.
    let expanded = crate::editor::expand_file_name_str(i, &path);
    let dir = {
        let t = expanded.trim_end_matches('/');
        match t.rfind('/') {
            Some(0) => crate::lisp::text::EmacsText::from("/"),
            Some(k) => t[..k].to_string(),
            None => crate::lisp::text::EmacsText::from("."),
        }
    };
    #[cfg(unix)]
    {
        use std::ffi::CString;
        let mut probe = crate::editor::native_path(i, &dir)?;
        loop {
            if let Ok(c) = CString::new(probe.to_string_lossy().as_bytes()) {
                // _PC_CASE_SENSITIVE (11): 0 → case-insensitive FS → t;
                // 1 → sensitive → nil; -1/EINVAL → unknown → nil.
                let r = unsafe { libc::pathconf(c.as_ptr(), 11) };
                if r == 0 {
                    return Ok(Value::t());
                }
                if r == 1 {
                    return Ok(Value::Nil);
                }
            }
            // GNU walks up until pathconf can query an existing ancestor.
            let Some(parent) = probe.parent() else {
                break;
            };
            if parent == probe {
                break;
            }
            probe = parent.to_path_buf();
        }
    }
    // Fallback (or non-unix): probe by statting a case-flipped name.
    let flipped: crate::lisp::text::EmacsText = path
        .chars()
        .map(|c| {
            if c.is_lowercase() {
                c.to_ascii_uppercase()
            } else {
                c.to_ascii_lowercase()
            }
        })
        .collect();
    if flipped == path {
        // A name without case-changing letters cannot establish that the
        // filesystem is case-insensitive; unknown filesystems default nil.
        return Ok(Value::Nil);
    }
    Ok(Value::from_bool(
        crate::editor::native_path(i, &flipped)?.exists()
            && crate::editor::native_path(i, &path)?.exists(),
    ))
}

pub(super) fn f_file_attributes_lessp(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU CHECK_LIST on both attribute lists.
    for v in args.iter().take(2) {
        match v {
            Value::Nil | Value::Cons(_) => {}
            other => return Err(i.wrong_type_mut("listp", other)),
        }
    }
    // Compare mtime (index 5 in the attributes list) — simplified:
    // compare nth 5 element if present, else nil.
    let get_mtime = |v: &Value| -> Option<i128> {
        let items = v.list_to_vec().ok()?;
        items.get(5).and_then(|x| x.int())
    };
    match (get_mtime(&args[0]), get_mtime(&args[1])) {
        (Some(a), Some(b)) => Ok(Value::from_bool(a < b)),
        _ => Ok(Value::Nil),
    }
}

pub(super) fn f_set_file_times(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    // GNU Fset_file_times order: symlink_nofollow_flag (FLAG), then
    // lisp_time_argument (TIMESTAMP), then expand FILENAME — so a bogus
    // TIMESTAMP signals "Invalid time specification" before FILENAME is
    // checked.
    // GNU symlink_nofollow_flag treats every non-nil value as nofollow.
    let nofollow = args.get(2).is_some_and(Value::truthy);
    let time = args.get(1).cloned().unwrap_or(Value::Nil);
    let ns = lisp_time_to_ns(i, &time)?;
    let name = {
        let f = want_string(i, &args[0])?;
        crate::editor::expand_file_name_str(i, &f)
    };
    if let Some((handler, op)) = crate::editor::file_handler(i, "set-file-times", &[&name]) {
        return i.apply(
            &handler,
            vec![
                op,
                crate::editor::file_name_argument(&args[0], &name),
                time,
                args.get(2).cloned().unwrap_or(Value::Nil),
            ],
        );
    }
    let secs = ns.div_euclid(1_000_000_000) as i64;
    let nsec = ns.rem_euclid(1_000_000_000) as i64;
    #[cfg(unix)]
    {
        let c = std::ffi::CString::new(name.to_external_bytes())
            .map_err(|_| i.signal_data(sym::FILE_ERROR, vec![Value::string("bad filename")]))?;
        let ts = libc::timespec {
            tv_sec: secs as libc::time_t,
            tv_nsec: nsec as _,
        };
        let times = [ts, ts];
        let flag = if nofollow {
            libc::AT_SYMLINK_NOFOLLOW
        } else {
            0
        };
        let r = unsafe { libc::utimensat(libc::AT_FDCWD, c.as_ptr(), times.as_ptr(), flag) };
        if r != 0 {
            // GNU report_file_error: ("Setting file times" strerror ABSNAME),
            // errno choosing file-missing/permission-denied/file-error.
            let e = std::io::Error::last_os_error();
            return Err(crate::editor::report_file_errno(
                i,
                "Setting file times",
                vec![Value::string(name)],
                &e,
            ));
        }
    }
    Ok(Value::t())
}

pub(super) fn f_get_file_buffer(i: &mut Interp, args: crate::lisp::Argv) -> EvalResult {
    let name = want_string(i, &args[0])?;
    let predicate = args.get(1).cloned();
    // GNU files.el `find-buffer-visiting' (our find-buffer-visiting
    // shares this entry point): get-file-buffer first, then the
    // truename lookup — a buffer is eligible only when PREDICATE (if
    // any) passes on it.
    let canonical = crate::editor::expand_file_name_str(i, &name);
    let mut check = |i: &mut Interp, id: usize| -> EvalResult {
        match &predicate {
            None | Some(Value::Nil) => Ok(i.buffer_value(id).unwrap_or(Value::Nil)),
            Some(p) => {
                let bv = i.buffer_value(id).unwrap_or(Value::Nil);
                match i.apply(p, vec![bv]) {
                    Err(fl) => Err(fl),
                    Ok(v) if v.truthy() => Ok(i.buffer_value(id).unwrap_or(Value::Nil)),
                    _ => Ok(Value::Nil),
                }
            }
        }
    };
    for id in i.buffers.list() {
        if let Some(b) = i.buffers.get(id) {
            let matches = {
                let bb = b.borrow();
                bb.file_name.as_deref() == Some(canonical.as_str())
            };
            if matches {
                let r = check(i, id)?;
                if r.truthy() {
                    return Ok(r);
                }
            }
        }
    }
    if predicate.is_none() {
        return Ok(Value::Nil);
    }
    // (get-truename-buffer truename): buffer-file-truename equal.
    let truename = crate::buffer::file_truename(&canonical);
    let bft = i.q_soft("buffer-file-truename").unwrap_or(u32::MAX);
    for id in i.buffers.list() {
        if let Some(b) = i.buffers.get(id) {
            let matches = {
                let bb = b.borrow();
                match bb.locals.get(&bft) {
                    Some(Value::Str(t)) => *t.borrow() == truename,
                    _ => false,
                }
            };
            if matches {
                let r = check(i, id)?;
                if r.truthy() {
                    return Ok(r);
                }
            }
        }
    }
    Ok(Value::Nil)
}

/// `set-file-acl` — best-effort like GNU: validate strings, no ACL
/// support → nil.
pub(super) fn f_set_file_acl(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = want_string(i, &a[0])?;
    let _ = want_string(i, &a[1])?;
    Ok(Value::Nil)
}

pub(super) fn f_move_file_to_trash(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    // GNU nsfns.m Fsystem_move_file_to_trash: a non-symlink directory
    // switches the operation to delete-directory with the directory-file
    // name, then FILENAME is expand-file-name'd *fully qualified* before
    // the trash attempt; failure signals report_file_error
    // "Removing old name" with the errno-picked symbol.
    let raw = match &a[0] {
        Value::Str(s) => s.borrow().clone(),
        other => return Err(i.wrong_type_mut("stringp", other)),
    };
    let mut name = raw.clone();
    let expanded = crate::editor::expand_file_name_str(i, &name);
    let native_expanded = crate::editor::native_path(i, &expanded)?;
    let meta = std::fs::metadata(&native_expanded).ok();
    let is_link = std::fs::symlink_metadata(&native_expanded)
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false);
    if meta.as_ref().map(|m| m.is_dir()).unwrap_or(false) && !is_link {
        // Qdelete_directory path: directory_file_name strips the slash.
        while name.len() > 1 && name.ends_with('/') {
            name.pop();
        }
    }
    let path = crate::editor::expand_file_name_str(i, &name);
    let native_src = crate::editor::native_path(i, &path)?;
    let src = native_src.as_path();
    // GNU's NS trash call fails on a missing path with ENOENT →
    // (file-missing "Removing old name" strerror ABSNAME).
    if let Err(e) = std::fs::symlink_metadata(src) {
        return Err(crate::editor::report_file_errno(
            i,
            "Removing old name",
            vec![Value::string(path)],
            &e,
        ));
    }
    // Moving $HOME or / into Trash via rename would succeed where GNU's
    // NS trash refuses with EPERM — and would be destructive.  Refuse the
    // same way: file-error "Removing old name" EPERM ABSNAME.
    let home = std::env::var("HOME").unwrap_or_else(|_| String::new());
    if path == "/" || (!home.is_empty() && path == home) {
        return Err(crate::editor::report_file_errno(
            i,
            "Removing old name",
            vec![Value::string(path)],
            &std::io::Error::from_raw_os_error(libc::EPERM),
        ));
    }
    // freedesktop trash: ~/.local/share/Trash/{files,info}
    let trash = std::path::Path::new(&home).join(".local/share/Trash");
    let files = trash.join("files");
    let info = trash.join("info");
    let _ = std::fs::create_dir_all(&files);
    let _ = std::fs::create_dir_all(&info);
    let name = src
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("unnamed");
    let mut dest = files.join(name);
    let mut n = 1;
    while dest.exists() {
        dest = files.join(format!("{}.{}", name, n));
        n += 1;
    }
    if let Err(e) = std::fs::rename(src, &dest) {
        return Err(crate::editor::report_file_errno(
            i,
            "Removing old name",
            vec![Value::string(path.clone())],
            &e,
        ));
    }
    if let Some(stem) = dest.file_name().and_then(|s| s.to_str()) {
        let _ = std::fs::write(
            info.join(format!("{}.trashinfo", stem)),
            format!("[Trash Info]\nPath={}\nDeletionDate=0\n", path),
        );
    }
    Ok(Value::Nil)
}

/// `file-selinux-context` — no SELinux: GNU shape (nil nil nil nil).
pub(super) fn f_file_selinux_context(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    want_string(i, &a[0])?;
    Ok(Value::list(vec![
        Value::Nil,
        Value::Nil,
        Value::Nil,
        Value::Nil,
    ]))
}

/// GNU `set-file-selinux-context': CHECK_STRING on FILENAME (a bare
/// `stringp' — GNU's Fexpand_file_name path isn't reached on this
/// no-libselinux build, so "~" isn't expanded and CONTEXT is never
/// type-checked), then nil.
pub(super) fn f_set_file_selinux_context(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let _ = want_string(i, &a[0])?;
    Ok(Value::Nil)
}
