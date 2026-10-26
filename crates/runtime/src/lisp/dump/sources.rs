//! Relocate source metadata before decoding the object graph, so shared
//! filenames and hash keys keep their identity. Equal user strings are data.
use std::path::Path;

use rustc_hash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};

use super::graph::{Encoder, Node, Ref, Result, decode_text, encode_text};
use crate::lisp::{Interp, Value};

#[derive(Default, Serialize, Deserialize)]
pub(super) struct Sources {
    files: Vec<Source>,
    directories: Vec<Directory>,
}

#[derive(Serialize, Deserialize)]
struct Source {
    key: String,
    old: Vec<u32>,
    objects: Vec<u32>,
}

#[derive(Serialize, Deserialize)]
struct Directory {
    old: Vec<u32>,
    objects: Vec<u32>,
    trailing_slash: bool,
}

impl Sources {
    pub fn capture(i: &Interp, encoder: &mut Encoder<'_>) -> Result<Self> {
        let mut sources = Self::default();
        let mut indices = FxHashMap::default();
        let mut roots = FxHashSet::default();
        let mut file = |value: &Value, encoder: &mut Encoder<'_>| -> Result<()> {
            let Value::Str(text) = value else {
                return Ok(());
            };
            let text = text.borrow().clone();
            let Some(key) = crate::lisp::load::dump_source_identity(&text) else {
                return Ok(());
            };
            let native = text.to_os_string().map_err(|_| "Cannot encode image source path")?;
            if let Some(parent) = Path::new(&native).parent() {
                if let Ok(parent) = std::fs::canonicalize(parent) {
                    roots.insert(parent);
                }
            }
            let index = *indices.entry(text.clone()).or_insert_with(|| {
                let index = sources.files.len();
                sources.files.push(Source {
                    key: key.into(),
                    old: encode_text(&text),
                    objects: Vec::new(),
                });
                index
            });
            if let Ref::Object(id) = encoder.value(value)? {
                if !sources.files[index].objects.contains(&id) {
                    sources.files[index].objects.push(id);
                }
            }
            Ok(())
        };
        if let Some(history) = i.q_soft("load-history") {
            for entry in i.symbol_value(history).list_to_vec().unwrap_or_default() {
                if let Value::Cons(cell) = entry {
                    file(&cell.borrow().car, encoder)?;
                }
            }
        }
        if let Some(property) = i.q_soft("function-history") {
            for id in 0..i.obarray.len() as crate::lisp::value::SymId {
                let history = i.get_prop(id, property).list_to_vec().unwrap_or_default();
                for filename in history.iter().step_by(2) {
                    file(filename, encoder)?;
                }
            }
        }
        let (buffers, ..) = i.buffers.dump_registry();
        for buffer in buffers.iter().flatten() {
            let buffer = buffer.borrow();
            if let Some(filename) = &buffer.file_name {
                file(&Value::string(filename.clone()), encoder)?;
            }
            for name in ["buffer-file-name", "buffer-file-truename"] {
                if let Some(id) = i.intern_soft(name) {
                    if let Some(value) = buffer.locals.get(&id) {
                        file(value, encoder)?;
                    }
                }
            }
        }
        drop(file);
        let mut directory = |value: &Value, encoder: &mut Encoder<'_>| -> Result<()> {
            let Value::Str(text) = value else {
                return Ok(());
            };
            let text = text.borrow().clone();
            if text.to_os_string().ok().and_then(|path| std::fs::canonicalize(path).ok()).is_some_and(|root| roots.contains(&root)) {
                if let Ref::Object(id) = encoder.value(value)? {
                    sources.directories.push(Directory {
                        old: encode_text(&text),
                        objects: vec![id],
                        trailing_slash: text.ends_with('/'),
                    });
                }
            }
            Ok(())
        };
        for name in ["load-path", "find-function-source-path"] {
            if let Some(id) = i.intern_soft(name) {
                for path in i.symbol_value(id).list_to_vec().unwrap_or_default() {
                    directory(&path, encoder)?;
                }
            }
        }
        if let Some(id) = i.q_soft("default-directory") {
            for buffer in buffers.iter().flatten() {
                let buffer = buffer.borrow();
                if let Some(value) = buffer.locals.get(&id) {
                    directory(value, encoder)?;
                }
            }
        }
        Ok(sources)
    }

    pub fn relocate(&self, nodes: &mut [Node]) -> Result<FxHashMap<Vec<u32>, Vec<u32>>> {
        let mut files = FxHashMap::default();
        let mut directory = None;
        for source in &self.files {
            let text = if source.key == "remacs-prelude" {
                crate::lisp::prelude::PRELUDE
            } else {
                crate::lisp::load::embedded(&source.key)
                    .ok_or_else(|| format!("Missing image source {}", source.key))?
            };
            let path = crate::lisp::embedded_cache::materialize(&source.key, text)
                .map_err(|error| format!("Publishing image source: {error}"))?;
            directory = path
                .parent()
                .map(|path| crate::lisp::text::EmacsText::from_os_str(path.as_os_str()));
            let text = encode_text(&crate::lisp::text::EmacsText::from_os_str(path.as_os_str()));
            Self::replace(nodes, &source.objects, &text)?;
            files.insert(source.old.clone(), text);
        }
        if let Some(directory) = directory {
            for entry in &self.directories {
                // An existing source tree may contain user edits. Keep
                // its search/default directory while it remains usable.
                if decode_text(&entry.old).to_os_string().ok().is_some_and(|path| Path::new(&path).is_dir()) {
                    continue;
                }
                let text = if entry.trailing_slash {
                    { let mut path = directory.clone(); path.push('/'); path }
                } else {
                    directory.clone()
                };
                Self::replace(nodes, &entry.objects, &encode_text(&text))?;
            }
        }
        Ok(files)
    }

    fn replace(nodes: &mut [Node], objects: &[u32], new: &[u32]) -> Result<()> {
        for id in objects {
            let Some(Node::String { text, .. }) = nodes.get_mut(*id as usize) else {
                return Err("Invalid image source reference".into());
            };
            *text = new.to_vec();
        }
        Ok(())
    }
}
