//! GNU DOC-file indexing and built-in documentation lookup.

use crate::lisp::{Interp, error::EvalResult, value::Value};

// ---------- GNU DOC file ----------

/// Lazily-parsed index of the GNU `etc/DOC' file named by
/// `doc-directory': "Fname"/"Vname" -> byte range of the doc text.
/// Shared across interpreters via thread-local cache keyed by path.
fn doc_index(
    i: &mut Interp,
) -> Option<std::rc::Rc<(Vec<u8>, std::collections::HashMap<String, usize>)>> {
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::rc::Rc;
    thread_local! {
        static CACHE: RefCell<HashMap<String, Rc<(Vec<u8>, HashMap<String, usize>)>>> =
            RefCell::new(HashMap::new());
    }
    let dir_sym = i.q("doc-directory");
    let dir = match i.symbol_value(dir_sym) {
        Value::Str(s) => s.borrow().clone(),
        _ => return None,
    };
    let path = format!("{dir}DOC");
    CACHE.with(|c| {
        if let Some(hit) = c.borrow().get(&path) {
            return Some(hit.clone());
        }
        let bytes = std::fs::read(&path).ok()?;
        // Entries: 0x1F KIND NAME '\n' TEXT ... until next 0x1F.
        let mut map = HashMap::new();
        let mut p = 0;
        while p < bytes.len() {
            if bytes[p] != 0x1f || p + 2 >= bytes.len() {
                p += 1;
                continue;
            }
            let kind = bytes[p + 1];
            if !matches!(kind, b'F' | b'V' | b'S') {
                p += 1;
                continue;
            }
            let name_start = p + 1;
            let nl = match bytes[name_start..].iter().position(|&b| b == b'\n') {
                Some(o) => name_start + o,
                None => break,
            };
            let key = String::from_utf8_lossy(&bytes[name_start..nl]).into_owned();
            // GNU Snarf-documentation ignores placeholder entries, including
            // later duplicates, rather than replacing a real doc with SKIP.
            if !bytes[nl + 1..].starts_with(b"SKIP") {
                map.insert(key, nl + 1);
            }
            p = nl + 1;
        }
        let rc = Rc::new((bytes, map));
        c.borrow_mut().insert(path, rc.clone());
        Some(rc)
    })
}

/// Install GNU's variable documentation offsets after the startup libraries
/// have supplied their definitions, like `Snarf-documentation' in doc.c.
/// Do not intern absent platform-specific variables or document void ones.
pub(crate) fn install_variable_docs(i: &mut Interp) {
    let Some(index) = doc_index(i) else {
        return;
    };
    let prop = i.q("variable-documentation");
    for (key, &offset) in &index.1 {
        let Some(name) = key.strip_prefix('V') else {
            continue;
        };
        let Some(id) = i.obarray.intern_soft(name) else {
            continue;
        };
        if i.bound_p(id) {
            i.put_prop(id, prop, Value::Int(offset as i128));
        }
    }
}

/// DOC-entry text for KEY ("Fcar"): bytes from the recorded offset to
/// the next 0x1F separator.
pub(crate) fn doc_text(i: &mut Interp, key: &str) -> Option<String> {
    let rc = doc_index(i)?;
    let (bytes, map) = &*rc;
    let start = *map.get(key)?;
    let end = bytes[start..]
        .iter()
        .position(|&b| b == 0x1f)
        .map(|o| start + o)
        .unwrap_or(bytes.len());
    Some(String::from_utf8_lossy(&bytes[start..end]).into_owned())
}

/// Byte offset of KEY's doc text (GNU's `function-documentation'
/// returns this integer for DOC-backed builtins).
pub(crate) fn doc_offset(i: &mut Interp, key: &str) -> Option<usize> {
    let rc = doc_index(i)?;
    rc.1.get(key).copied()
}

/// DOC-entry text starting at byte offset OFF (GNU `get_doc_string'
/// for `variable-documentation'/function cells that store raw
/// positions).
pub(crate) fn doc_text_at(i: &mut Interp, off: usize) -> Option<String> {
    let rc = doc_index(i)?;
    let (bytes, _map) = &*rc;
    if off < 2 || off >= bytes.len() || bytes[off - 1] != b'\n' {
        return None;
    }
    // GNU rejects stale DOC references rather than returning a suffix
    // from arbitrary file contents: a header is US + NAME + newline.
    let mut header = off - 2;
    while header > 0 && bytes[header] > b' ' {
        header -= 1;
    }
    if bytes[header] != 31 {
        return None;
    }
    let end = bytes[off..]
        .iter()
        .position(|&b| b == 0x1f)
        .map(|o| off + o)
        .unwrap_or(bytes.len());
    Some(String::from_utf8_lossy(&bytes[off..end]).into_owned())
}

pub(super) fn f_function_documentation(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    match i.indirect_function_value(&a[0]) {
        Value::Subr(s) => {
            if let Some(documentation) = crate::lisp::builtins::comp::native_documentation(s) {
                return Ok(documentation);
            }
            // GNU's doc slot for a dumped builtin is the DOC offset.
            if let Some(off) = doc_offset(i, &format!("F{}", s.name)) {
                return Ok(Value::Int(off as i128));
            }
            if !s.doc.is_empty() {
                return Ok(Value::string(s.doc));
            }
            Ok(Value::Nil)
        }
        _ => Ok(Value::Nil),
    }
}
