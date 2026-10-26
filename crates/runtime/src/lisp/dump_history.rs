//! Definition provenance for the GNU dump represented by the bootstrap.
//! Paths are resolved to readable ported sources, including copied binaries.
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::OnceLock;

use serde::Deserialize;

use super::{EvalResult, Interp, Value};

#[derive(Deserialize)]
struct Manifest {
    format: u32,
    reference_version: String,
    known_symbols: HashSet<String>,
    libraries: Vec<Library>,
}

#[derive(Deserialize)]
struct Library {
    key: String,
    entries: Vec<String>,
}

fn manifest() -> &'static Manifest {
    static MANIFEST: OnceLock<Manifest> = OnceLock::new();
    MANIFEST.get_or_init(|| {
        let manifest: Manifest = serde_json::from_str(include_str!("dump_history.json"))
            .expect("valid generated dump history");
        assert_eq!(manifest.format, 1);
        assert_eq!(manifest.reference_version, "31.1");
        manifest
    })
}

fn private_definition(i: &Interp, entry: &Value, known: &HashSet<String>) -> bool {
    let (kind, name) = match entry {
        Value::Sym(name) => (None, *name),
        Value::Cons(cell) => {
            let cell = cell.borrow();
            let (Value::Sym(kind), Value::Sym(name)) = (&cell.car, &cell.cdr) else {
                return false;
            };
            (Some(*kind), *name)
        }
        _ => return false,
    };
    if known.contains(&i.symbol_name(name)) {
        return false;
    }
    match kind.map(|kind| i.symbol_name(kind)).as_deref() {
        None => i.bound_p(name),
        Some("defun") => i.fbound_p(name),
        Some("defface") => true,
        _ => false,
    }
}

fn prelude_source(i: &mut Interp, file: &str) -> Result<String, super::Flow> {
    let development =
        std::env::current_exe().is_ok_and(|exe| exe.starts_with(env!("REMACS_SOURCE_ROOT")));
    if development && Path::new(file).is_file() {
        return Ok(file.to_string());
    }
    super::embedded_cache::materialize("remacs-prelude", super::prelude::PRELUDE)
        .map(|path| path.to_string_lossy().into_owned())
        .map_err(|error| i.error(format!("Publishing bootstrap source: {error}")))
}

/// Replace implementation-only bootstrap records with the original module
/// ownership. Keep external loads and live Remacs-only definitions intact.
/// No definitions are evaluated and no feature cells are changed here.
pub(crate) fn restore(i: &mut Interp) -> EvalResult {
    let manifest = manifest();
    let history = i.q("load-history");
    let old = i.symbol_value(history).list_to_vec().unwrap_or_default();
    let prelude = concat!(env!("REMACS_SOURCE_ROOT"), "/crates/runtime/src/lisp/prelude.el");
    let mut external = Vec::new();
    let mut private = Vec::new();
    for entry in old {
        let Value::Cons(cell) = &entry else { continue };
        let (file, definitions) = {
            let cell = cell.borrow();
            (cell.car.clone(), cell.cdr.clone())
        };
        let Value::Str(file) = file else { continue };
        let file = file.borrow().to_string();
        if file != prelude && !super::load::bundled_history_path(&file) {
            external.push(entry);
            continue;
        }
        let definitions: Vec<_> = definitions
            .list_to_vec()
            .unwrap_or_default()
            .into_iter()
            .filter(|definition| private_definition(i, definition, &manifest.known_symbols))
            .collect();
        if definitions.is_empty() {
            continue;
        }
        let file = if file == prelude {
            crate::lisp::text::EmacsText::from(prelude_source(i, &file)?)
        } else if let Some(key) = file.strip_prefix("builtin:") {
            super::load::dump_source_file(i, key)?
        } else {
            file.into()
        };
        private.push((file, definitions));
    }

    let mut canonical = Vec::new();
    let mut indices = HashMap::new();
    for library in &manifest.libraries {
        let file = super::load::dump_source_file(i, &library.key)?;
        let source = format!("({})", library.entries.join(" "));
        let definitions = super::reader::Reader::new(i, &source)
            .read()?
            .ok_or_else(|| i.error("Missing dumped history definitions"))?;
        indices.insert(file.clone(), canonical.len());
        canonical.push((file, definitions.list_to_vec().unwrap_or_default()));
    }
    for (file, definitions) in private {
        if let Some(index) = indices.get(&file) {
            canonical[*index].1.extend(definitions);
        } else {
            indices.insert(file.clone(), canonical.len());
            canonical.push((file, definitions));
        }
    }
    external.extend(
        canonical
            .into_iter()
            .map(|(file, definitions)| Value::cons(Value::string(file), Value::list(definitions))),
    );
    i.obarray.symbol_mut(history).value = Value::list(external);
    Ok(Value::Nil)
}
