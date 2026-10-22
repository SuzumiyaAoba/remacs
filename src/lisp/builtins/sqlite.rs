//! SQLite primitives (`sqlite-*') over rusqlite.
//!
//! GNU represents databases and result sets as `sqlite' pseudovectors;
//! we use Records so `type-of' reports `sqlite' for both:
//!   #s(sqlite db  ID)   open or closed database handle
//!   #s(sqlite set ID)   result set from (sqlite-select ... 'set)
//! Ids index per-thread registries, which also retain the original record
//! identity. Forged or copied records cannot impersonate a registered handle.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ffi::CStr;
use std::rc::{Rc, Weak};

use rusqlite::{Connection, OpenFlags, ffi};

use super::{S, arg, want_string};
use crate::lisp::error::Flow;
use crate::lisp::value::{Subr, Value, eight_bit_byte};
use crate::lisp::{EvalResult, Interp};

struct Set {
    object: Weak<RefCell<Vec<Value>>>,
    statement: Option<Statement>,
    /// Only stepping past the last row sets EOF; metadata access never steps.
    more: bool,
}

/// An owned SQLite statement whose connection outlives it, even if Lisp closes
/// the database handle. Eager and lazy operations share native preparation and
/// binding; each next performs exactly one sqlite3_step.
struct Statement {
    conn: Rc<Connection>,
    stmt: *mut ffi::sqlite3_stmt,
}

impl Drop for Statement {
    fn drop(&mut self) {
        // SAFETY: stmt is owned exclusively here; conn is still alive during
        // finalization. SQLite also permits finalizing a null statement.
        unsafe { ffi::sqlite3_finalize(self.stmt) };
    }
}

impl Statement {
    fn error(&self, code: i32, prepare: bool) -> SqlError {
        SqlError::new(&self.conn, code, prepare)
    }

    fn prepare(conn: Rc<Connection>, sql: &str, vals: &[SVal]) -> Result<Self, SqlError> {
        let len =
            i32::try_from(sql.len()).map_err(|_| SqlError::new(&conn, ffi::SQLITE_TOOBIG, true))?;
        let mut set = Self {
            conn,
            stmt: std::ptr::null_mut(),
        };
        // SAFETY: the connection and SQL bytes are live; len bounds SQLite's
        // read and stmt is a valid out pointer. Like rusqlite and GNU, accept
        // embedded NULs (SQLite stops there). This does not step the statement.
        let code = unsafe {
            ffi::sqlite3_prepare_v2(
                set.conn.handle(),
                sql.as_ptr().cast(),
                len,
                &mut set.stmt,
                std::ptr::null_mut(),
            )
        };
        if code != ffi::SQLITE_OK {
            return Err(set.error(code, true));
        }
        // Bind in order, including excess values: SQLite supplies the native
        // RANGE message, or its unchanged connection message for a null stmt.
        // Omitted slots already have SQLite's NULL default.
        for (k, value) in vals.iter().enumerate() {
            let index = (k + 1) as i32;
            // SAFETY: SQLite validates the parameter index. SQLITE_TRANSIENT copies
            // strings/blobs before returning, so Lisp mutations cannot alter
            // the saved parameters and no Rust slice outlives this call.
            let code = unsafe {
                match value {
                    SVal::N => ffi::sqlite3_bind_null(set.stmt, index),
                    SVal::I(n) => ffi::sqlite3_bind_int64(set.stmt, index, *n),
                    SVal::F(f) => ffi::sqlite3_bind_double(set.stmt, index, *f),
                    SVal::T(bytes) => ffi::sqlite3_bind_text64(
                        set.stmt,
                        index,
                        bytes.as_ptr().cast(),
                        bytes.len() as u64,
                        ffi::SQLITE_TRANSIENT(),
                        ffi::SQLITE_UTF8 as u8,
                    ),
                    SVal::B(bytes) => ffi::sqlite3_bind_blob64(
                        set.stmt,
                        index,
                        bytes.as_ptr().cast(),
                        bytes.len() as u64,
                        ffi::SQLITE_TRANSIENT(),
                    ),
                }
            };
            if code != ffi::SQLITE_OK {
                return Err(set.error(code, false));
            }
        }
        Ok(set)
    }

    fn columns(&self) -> Result<Vec<String>, SqlError> {
        // SAFETY: stmt remains prepared until Drop. Column names are copied
        // before stepping/repreparing can invalidate SQLite's string pointers.
        unsafe {
            (0..ffi::sqlite3_column_count(self.stmt))
                .map(|k| {
                    let name = ffi::sqlite3_column_name(self.stmt, k);
                    if name.is_null() {
                        return Err(self.error(ffi::SQLITE_NOMEM, false));
                    }
                    Ok(CStr::from_ptr(name).to_string_lossy().into_owned())
                })
                .collect()
        }
    }

    fn next(&mut self) -> Result<Option<Vec<SVal>>, SqlError> {
        // SAFETY: stmt and its connection are live and used on this thread
        // only. Copy every column before a subsequent step can invalidate it.
        unsafe {
            match ffi::sqlite3_step(self.stmt) {
                ffi::SQLITE_DONE => Ok(None),
                ffi::SQLITE_ROW => {
                    let mut row = Vec::new();
                    for k in 0..ffi::sqlite3_column_count(self.stmt) {
                        row.push(match ffi::sqlite3_column_type(self.stmt, k) {
                            ffi::SQLITE_INTEGER => SVal::I(ffi::sqlite3_column_int64(self.stmt, k)),
                            ffi::SQLITE_FLOAT => SVal::F(ffi::sqlite3_column_double(self.stmt, k)),
                            kind @ (ffi::SQLITE_TEXT | ffi::SQLITE_BLOB) => {
                                let ptr = if kind == ffi::SQLITE_TEXT {
                                    ffi::sqlite3_column_text(self.stmt, k)
                                } else {
                                    ffi::sqlite3_column_blob(self.stmt, k).cast()
                                };
                                let len = ffi::sqlite3_column_bytes(self.stmt, k) as usize;
                                if ptr.is_null() && (kind == ffi::SQLITE_TEXT || len != 0) {
                                    return Err(self.error(ffi::SQLITE_NOMEM, false));
                                }
                                // Empty BLOBs can have a null pointer, which
                                // Rust slices prohibit even for zero length.
                                let bytes = if len == 0 {
                                    Vec::new()
                                } else {
                                    std::slice::from_raw_parts(ptr, len).to_vec()
                                };
                                if kind == ffi::SQLITE_TEXT {
                                    SVal::T(bytes)
                                } else {
                                    SVal::B(bytes)
                                }
                            }
                            _ => SVal::N,
                        });
                    }
                    Ok(Some(row))
                }
                code => Err(self.error(code, false)),
            }
        }
    }
}

#[derive(Clone)]
enum SVal {
    I(i64),
    F(f64),
    T(Vec<u8>),
    B(Vec<u8>),
    N,
}

struct DbEntry {
    object: Weak<RefCell<Vec<Value>>>,
    conn: Option<Rc<Connection>>,
}

thread_local! {
    static DBS: RefCell<HashMap<u64, DbEntry>> = RefCell::new(HashMap::new());
    static SETS: RefCell<HashMap<u64, Set>> = RefCell::new(HashMap::new());
    static NEXT_ID: Cell<u64> = const { Cell::new(1) };
    static MEM_N: Cell<u64> = const { Cell::new(0) };
}

fn next_id() -> u64 {
    NEXT_ID.with(|n| {
        let id = n.get();
        n.set(id + 1);
        id
    })
}

/// Snapshot SQLite's error before finalizing a statement or reusing its DB.
/// Only prepare failures carry (ERRSTR ERRMSG CODE EXTCODE); bind and step
/// failures carry a plain message. Never expose rusqlite enum discriminants.
struct SqlError {
    code: i32,
    extended_code: i32,
    message: String,
    prepare: bool,
}

impl SqlError {
    fn new(conn: &Connection, code: i32, prepare: bool) -> Self {
        // SAFETY: conn owns a live database. Copy SQLite's message before
        // another operation can change it. rusqlite enables extended return
        // codes, whereas GNU exposes the primary prepare result separately.
        unsafe {
            Self {
                code: code & 0xff,
                extended_code: ffi::sqlite3_extended_errcode(conn.handle()),
                message: CStr::from_ptr(ffi::sqlite3_errmsg(conn.handle()))
                    .to_string_lossy()
                    .into_owned(),
                prepare,
            }
        }
    }
}

fn sqlite_err(i: &mut Interp, data: crate::lisp::Argv) -> Flow {
    let se = i.intern("sqlite-error");
    i.signal_data(se, data.as_slice().to_vec())
}

fn sql_err(i: &mut Interp, e: SqlError, execute: bool) -> Flow {
    let data = if e.prepare {
        // SAFETY: sqlite3_errstr returns a static string for every code.
        let name = unsafe { CStr::from_ptr(ffi::sqlite3_errstr(e.code)) };
        Value::list(vec![
            Value::string(name.to_string_lossy().into_owned()),
            Value::string(e.message),
            Value::Int(e.code as i128),
            Value::Int(e.extended_code as i128),
        ])
    } else {
        Value::string(e.message)
    };
    // GNU uses the locked subtype only in sqlite-execute. Select preparation
    // and sqlite-next always signal sqlite-error, even for BUSY/LOCKED.
    if execute && matches!(e.code, ffi::SQLITE_LOCKED | ffi::SQLITE_BUSY) {
        let le = i.intern("sqlite-locked-error");
        i.signal_data(le, vec![data])
    } else {
        sqlite_err(i, vec![data].into())
    }
}

/// An authenticated object and its native lifecycle, independent of other
/// arguments. Closed objects retain their SQLite type, just as GNU does.
enum Handle {
    Db { id: u64, open: bool },
    Set { id: u64, open: bool },
}

/// Authenticate both the record layout and its allocation identity. The ID
/// alone is public Lisp data, not proof that sqlite-open/select created it.
/// A weak reference avoids keeping the Lisp object alive and prevents a later
/// allocation at the same address from being mistaken for the original.
fn handle(v: &Value, i: &mut Interp) -> Option<Handle> {
    let Value::Record(object) = v else {
        return None;
    };
    let (tag, kind, id) = match object.borrow().as_slice() {
        [Value::Sym(tag), Value::Sym(kind), Value::Int(id)] => (*tag, *kind, *id),
        _ => return None,
    };
    if tag != i.intern("sqlite") {
        return None;
    }
    let id = u64::try_from(id).ok()?;
    let object = Rc::downgrade(object);
    match i.symbol_name(kind).as_str() {
        "db" => DBS.with(|dbs| {
            dbs.borrow().get(&id).and_then(|entry| {
                entry.object.ptr_eq(&object).then_some(Handle::Db {
                    id,
                    open: entry.conn.is_some(),
                })
            })
        }),
        "set" => SETS.with(|sets| {
            sets.borrow().get(&id).and_then(|entry| {
                entry.object.ptr_eq(&object).then_some(Handle::Set {
                    id,
                    open: entry.statement.is_some(),
                })
            })
        }),
        _ => None,
    }
}

/// GNU checks identity, kind, and liveness before SQL or other arguments.
fn want_db_id(i: &mut Interp, v: &Value) -> Result<u64, Flow> {
    match handle(v, i) {
        Some(Handle::Db { id, open: true }) => Ok(id),
        Some(Handle::Db { open: false, .. }) => {
            Err(sqlite_err(i, vec![Value::string("Database closed")].into()))
        }
        Some(Handle::Set { .. }) => Err(sqlite_err(
            i,
            vec![Value::string("Invalid database object")].into(),
        )),
        None => Err(i.wrong_type_mut("sqlitep", v)),
    }
}

/// Clone a live connection without keeping the registry borrowed.
fn connection(i: &mut Interp, id: u64) -> Result<Rc<Connection>, Flow> {
    DBS.with(|d| d.borrow().get(&id).and_then(|e| e.conn.clone()))
        .ok_or_else(|| sqlite_err(i, vec![Value::string("Database closed")].into()))
}

/// t for genuine SQLite objects, including closed databases and sets.
fn is_sqlite(i: &mut Interp, v: &Value) -> bool {
    handle(v, i).is_some()
}

/// Registered live set → mutable access via closure.
fn with_set<R>(
    i: &mut Interp,
    v: &Value,
    f: impl FnOnce(&mut Set) -> Result<R, Flow>,
) -> Result<R, Flow> {
    match handle(v, i) {
        Some(Handle::Set { id, open: true }) => SETS.with(|sets| {
            // No Lisp callback can change the registry between validation and
            // this borrow; the closure only accesses native statement state.
            f(sets.borrow_mut().get_mut(&id).unwrap())
        }),
        Some(Handle::Set { open: false, .. }) => Err(sqlite_err(
            i,
            vec![Value::string("Statement closed")].into(),
        )),
        Some(Handle::Db { .. }) => Err(sqlite_err(
            i,
            vec![Value::string("Invalid set object")].into(),
        )),
        None => Err(i.wrong_type_mut("sqlitep", v)),
    }
}

fn make_db_record(i: &mut Interp, id: u64) -> Rc<RefCell<Vec<Value>>> {
    crate::lisp::Value::vector_ref(vec![
        Value::Sym(i.intern("sqlite")),
        Value::Sym(i.intern("db")),
        Value::Int(id as i128),
    ])
}

fn make_set_record(i: &mut Interp, id: u64) -> Rc<RefCell<Vec<Value>>> {
    crate::lisp::Value::vector_ref(vec![
        Value::Sym(i.intern("sqlite")),
        Value::Sym(i.intern("set")),
        Value::Int(id as i128),
    ])
}

/// Lisp VALUE → sqlite bind value.
fn bind_val(i: &mut Interp, v: &Value) -> Result<SVal, Flow> {
    use SVal as R;
    Ok(match v {
        Value::Nil => R::N,
        Value::Int(_) | Value::BigInt(_) => R::I(
            v.int()
                .and_then(|n| i64::try_from(n).ok())
                .ok_or_else(|| sqlite_err(i, vec![Value::string("integer out of range")].into()))?,
        ),
        Value::Float(f) => R::F(**f),
        Value::Str(s) => R::T(if i.is_unibyte_str(s) {
            s.borrow()
                .chars()
                .map(|c| eight_bit_byte(c).unwrap_or(c as u8))
                .collect()
        } else {
            s.borrow().as_bytes().to_vec()
        }),
        // GNU binds `t' as 1.
        Value::Sym(s) if *s == crate::lisp::obarray::sym::T => R::I(1),
        _ => return Err(i.wrong_type_mut("stringp", v)),
    })
}

fn bind_args(i: &mut Interp, v: &Value) -> Result<Vec<SVal>, Flow> {
    let items = match v {
        Value::Nil => return Ok(vec![]),
        Value::Vec(vec) => vec.borrow().clone(),
        _ => match v.list_to_vec() {
            Ok(items) => items,
            Err(_) => return Err(i.wrong_type_mut("listp", v)),
        },
    };
    items.iter().map(|x| bind_val(i, x)).collect()
}

fn sval_to_value(i: &mut Interp, s: &SVal) -> Value {
    match s {
        SVal::I(n) => Value::integer(*n as i128),
        SVal::F(f) => Value::float(*f),
        SVal::T(t) => Value::string(String::from_utf8_lossy(t).into_owned()),
        // BLOBs are byte sequences, even when they happen to be valid UTF-8.
        // Use the interpreter's unibyte representation without decoding them.
        SVal::B(b) => {
            let value = Value::string(b.iter().map(|&byte| byte as char).collect::<String>());
            if let Value::Str(s) = &value {
                i.mark_unibyte(s);
            }
            value
        }
        SVal::N => Value::Nil,
    }
}

fn f_sqlite_open(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let file = arg(&a, 0);
    let readonly = arg(&a, 1).truthy();
    let disable_uri = arg(&a, 2).truthy();
    let mut flags = OpenFlags::SQLITE_OPEN_NO_MUTEX;
    if !disable_uri {
        flags |= OpenFlags::SQLITE_OPEN_URI;
    }
    if readonly {
        flags |= OpenFlags::SQLITE_OPEN_READ_ONLY;
    } else {
        flags |= OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE;
    }
    let conn = match &file {
        Value::Nil => Connection::open_in_memory_with_flags(flags),
        Value::Str(_) => {
            // GNU expands every non-nil filename before opening SQLite.
            let expanded = crate::editor::f_expand_file_name(i, vec![file.clone()].into())?;
            let name = want_string(i, &expanded)?;
            Connection::open_with_flags(&name, flags)
        }
        _ => return Err(i.wrong_type_mut("stringp", &file)),
    };
    match conn {
        Ok(c) => {
            let id = next_id();
            let object = make_db_record(i, id);
            DBS.with(|d| {
                d.borrow_mut().insert(
                    id,
                    DbEntry {
                        object: Rc::downgrade(&object),
                        conn: Some(Rc::new(c)),
                    },
                )
            });
            Ok(Value::Record(object))
        }
        // GNU returns nil when the database can't be opened.
        Err(_) => Ok(Value::Nil),
    }
}

fn f_sqlite_close(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match handle(&a[0], i) {
        // GNU's idempotent-close shortcut also accepts a finalized set.
        Some(Handle::Db { open: false, .. } | Handle::Set { open: false, .. }) => {}
        Some(Handle::Db { id, open: true }) => DBS.with(|dbs| {
            dbs.borrow_mut().get_mut(&id).unwrap().conn = None;
        }),
        Some(Handle::Set { open: true, .. }) => {
            return Err(sqlite_err(
                i,
                vec![Value::string("Invalid database object")].into(),
            ));
        }
        None => return Err(i.wrong_type_mut("sqlitep", &a[0])),
    }
    Ok(Value::Sym(1))
}

fn f_sqlite_execute(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let id = want_db_id(i, &a[0])?;
    let sql = want_string(i, &a[1])?;
    let vals = bind_args(i, &arg(&a, 2))?;
    let conn = connection(i, id)?;
    let mut stmt =
        Statement::prepare(conn.clone(), &sql, &vals).map_err(|e| sql_err(i, e, true))?;
    // GNU signals a failed first step, but once rows have started it returns
    // the rows seen before DONE or an error, just like sqlite-select.
    let first = stmt.next().map_err(|e| sql_err(i, e, true))?;
    let (cols_rows, n) = if let Some(first) = first {
        let mut rows = vec![first];
        while let Ok(Some(row)) = stmt.next() {
            rows.push(row);
        }
        (Some(rows), 0)
    } else {
        (None, conn.changes())
    };
    match cols_rows {
        Some(rows) => Ok(Value::list(
            rows.iter()
                .map(|r| Value::list(r.iter().map(|value| sval_to_value(i, value)).collect()))
                .collect(),
        )),
        None => Ok(Value::Int(n as i128)),
    }
}

fn f_sqlite_execute_batch(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let id = want_db_id(i, &a[0])?;
    let sql = want_string(i, &a[1])?;
    // GNU wraps sqlite3_exec: SQL failure returns nil, no signal.
    exec_quiet(i, id, &sql)
}

/// sqlite3_exec-style helper: t on success, nil on SQL failure.
fn exec_quiet(i: &mut Interp, id: u64, sql: &str) -> EvalResult {
    DBS.with(|d| {
        let mut m = d.borrow_mut();
        match m.get_mut(&id).and_then(|e| e.conn.as_mut()) {
            Some(c) => Ok(Value::from_bool(c.execute_batch(sql).is_ok())),
            None => Err(sqlite_err(i, vec![Value::string("Database closed")].into())),
        }
    })
}

fn f_sqlite_select(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let id = want_db_id(i, &a[0])?;
    let sql = want_string(i, &a[1])?;
    let vals = bind_args(i, &arg(&a, 2))?;
    let rtype = arg(&a, 3);
    let is_set = matches!(&rtype, Value::Sym(s) if i.symbol_name(*s) == "set");
    let is_full = matches!(&rtype, Value::Sym(s) if i.symbol_name(*s) == "full");
    let conn = connection(i, id)?;
    let mut statement = Statement::prepare(conn, &sql, &vals).map_err(|e| sql_err(i, e, false))?;
    if is_set {
        let sid = next_id();
        let object = make_set_record(i, sid);
        SETS.with(|s| {
            s.borrow_mut().insert(
                sid,
                Set {
                    object: Rc::downgrade(&object),
                    statement: Some(statement),
                    more: true,
                },
            )
        });
        return Ok(Value::Record(object));
    }
    // GNU's eager select stops at any non-row result, including a step
    // failure, and returns the rows collected so far. Prepare/bind still fail.
    let mut rows = Vec::new();
    while let Ok(Some(row)) = statement.next() {
        rows.push(row);
    }
    let cols = statement.columns().map_err(|e| sql_err(i, e, false))?;
    let mut out: Vec<Value> = Vec::new();
    if is_full {
        out.push(Value::list(
            cols.iter().map(|c| Value::string(c.clone())).collect(),
        ));
    }
    for r in &rows {
        out.push(Value::list(
            r.iter().map(|value| sval_to_value(i, value)).collect(),
        ));
    }
    Ok(Value::list(out))
}

/// GNU's transaction helpers return nil (not sqlite-error) when SQLite
/// reports failure — e.g. commit/rollback with no active transaction.
fn f_sqlite_execute1(i: &mut Interp, a: crate::lisp::Argv, sql: &str) -> EvalResult {
    let id = want_db_id(i, &a[0])?;
    exec_quiet(i, id, sql)
}

fn f_sqlite_transaction(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    f_sqlite_execute1(i, a, "BEGIN TRANSACTION")
}

fn f_sqlite_commit(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    f_sqlite_execute1(i, a, "COMMIT")
}

fn f_sqlite_rollback(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    f_sqlite_execute1(i, a, "ROLLBACK")
}

fn f_sqlite_next(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let row = with_set(i, &a[0], |s| {
        if !s.more {
            return Ok(Ok(None));
        }
        let row = s.statement.as_mut().unwrap().next();
        if matches!(row, Ok(None)) {
            s.more = false;
        }
        Ok(row)
    })?
    // GNU reports step failures here, not while constructing the set. It
    // leaves EOF false so a later sqlite-next can retry the statement.
    .map_err(|e| sql_err(i, e, false))?;
    Ok(match row {
        Some(row) => Value::list(row.iter().map(|value| sval_to_value(i, value)).collect()),
        None => Value::Nil,
    })
}

fn f_sqlite_more_p(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    with_set(i, &a[0], |s| Ok(Value::from_bool(s.more)))
}

fn f_sqlite_columns(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let columns = with_set(i, &a[0], |s| Ok(s.statement.as_ref().unwrap().columns()))?
        .map_err(|e| sql_err(i, e, false))?;
    Ok(Value::list(
        columns.into_iter().map(Value::string).collect(),
    ))
}

fn f_sqlite_finalize(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    with_set(i, &a[0], |s| {
        s.statement = None;
        Ok(Value::Sym(1))
    })
}

fn f_sqlite_pragma(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let id = want_db_id(i, &a[0])?;
    let p = want_string(i, &a[1])?;
    exec_quiet(i, id, &format!("PRAGMA {p}"))
}

fn f_sqlite_load_extension(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let id = want_db_id(i, &a[0])?;
    let module = want_string(i, &a[1])?;
    // GNU's allowlist is hardcoded in sqlite.c: the file's basename
    // (sans any "libsqlite3_mod_" prefix) must be one of these names
    // plus a .so/.dylib/.dll extension.
    const ALLOWLIST: &[&str] = &[
        "base64",
        "cksumvfs",
        "compress",
        "csv",
        "csvtable",
        "fts3",
        "icu",
        "pcre",
        "percentile",
        "regexp",
        "rot13",
        "rtree",
        "sha1",
        "uuid",
        "vec0",
        "vector0",
        "vfslog",
        "vss0",
        "zipfile",
    ];
    let base = module.rsplit('/').next().unwrap_or(&module);
    let base = base.strip_prefix("libsqlite3_mod_").unwrap_or(base);
    let allowed = ALLOWLIST.iter().any(|name| {
        base.strip_prefix(name).is_some_and(|ext| {
            matches!(ext.to_ascii_lowercase().as_str(), ".so" | ".dylib" | ".dll")
        })
    });
    if !allowed {
        return Err(sqlite_err(
            i,
            vec![Value::string("Module name not on allowlist")].into(),
        ));
    }
    let conn = connection(i, id)?;
    // Like execute-batch, extension load failure is a quiet nil in GNU.
    let loaded = unsafe {
        if conn.load_extension_enable().is_err() {
            false
        } else {
            let result = conn.load_extension(&module, None::<&str>);
            let _ = conn.load_extension_disable();
            result.is_ok()
        }
    };
    Ok(Value::from_bool(loaded))
}

fn f_sqlite_version(i: &mut Interp, _a: crate::lisp::Argv) -> EvalResult {
    let _ = i;
    Ok(Value::string(rusqlite::version()))
}

pub(crate) fn f_sqlitep(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    Ok(Value::from_bool(is_sqlite(i, &a[0])))
}

/// Called once from `builtins::install': give `sqlite-error' a real
/// error-conditions property so `condition-case ... (error ...)' catches it.
pub(crate) fn install(i: &mut Interp) {
    let ec = i.intern("error-conditions");
    let em = i.intern("error-message");
    let se = i.intern("sqlite-error");
    let conds = Value::list(vec![Value::Sym(se), Value::Sym(i.intern("error"))]);
    i.put_prop(se, ec, conds);
    i.put_prop(se, em, Value::string("SQLite error"));
    // `sqlite-locked-error' is a subcase of `sqlite-error' in GNU.
    let le = i.intern("sqlite-locked-error");
    let err = i.intern("error");
    i.put_prop(
        le,
        ec,
        Value::list(vec![Value::Sym(le), Value::Sym(se), Value::Sym(err)]),
    );
    i.put_prop(le, em, Value::string("Database locked"));
}

pub(crate) static SUBRS: &[Subr] = &[
    S!(
        "sqlite-open",
        0,
        3,
        f_sqlite_open,
        "Open FILE as an sqlite database.\nIf FILE is nil or omitted, an in-memory database is opened.\nIf READONLY is non-nil, open read-only; URIs are recognized\nunless DISABLE-URI is non-nil."
    ),
    S!(
        "sqlite-close",
        1,
        1,
        f_sqlite_close,
        "Close the sqlite database DB."
    ),
    S!(
        "sqlite-execute",
        2,
        3,
        f_sqlite_execute,
        "Execute a non-select SQL statement.\nVALUES is a list or vector bound to `?' parameters.\nValue is the number of affected rows."
    ),
    S!(
        "sqlite-execute-batch",
        2,
        2,
        f_sqlite_execute_batch,
        "Execute multiple SQL STATEMENTS in DB."
    ),
    S!(
        "sqlite-select",
        2,
        4,
        f_sqlite_select,
        "Select data from DB matching QUERY.\nVALUES is a list or vector bound to `?' parameters.\nRETURN-TYPE nil: list of rows; `full': column names then rows;\n`set': a set object for `sqlite-next' etc."
    ),
    S!(
        "sqlite-transaction",
        1,
        1,
        f_sqlite_transaction,
        "Start a transaction in DB."
    ),
    S!(
        "sqlite-commit",
        1,
        1,
        f_sqlite_commit,
        "Commit a transaction in DB."
    ),
    S!(
        "sqlite-rollback",
        1,
        1,
        f_sqlite_rollback,
        "Roll back a transaction in DB."
    ),
    S!(
        "sqlite-next",
        1,
        1,
        f_sqlite_next,
        "Return the next result set from SET.\nnil when the statement has finished."
    ),
    S!(
        "sqlite-more-p",
        1,
        1,
        f_sqlite_more_p,
        "t if there are further results in SET."
    ),
    S!(
        "sqlite-columns",
        1,
        1,
        f_sqlite_columns,
        "Return the column names of SET."
    ),
    S!(
        "sqlite-finalize",
        1,
        1,
        f_sqlite_finalize,
        "Mark SET finished; frees its resources."
    ),
    S!(
        "sqlite-pragma",
        2,
        2,
        f_sqlite_pragma,
        "Execute PRAGMA in DB."
    ),
    S!(
        "sqlite-load-extension",
        2,
        2,
        f_sqlite_load_extension,
        "Load SQlite MODULE into DB.\nOnly modules on `sqlite-allowed-modules' can be loaded."
    ),
    S!(
        "sqlite-version",
        0,
        0,
        f_sqlite_version,
        "Return the version string of the SQLite library."
    ),
];
