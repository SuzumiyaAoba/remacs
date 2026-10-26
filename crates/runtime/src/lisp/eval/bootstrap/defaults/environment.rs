//! Host-derived startup values. Each probe runs at its position in the
//! eager default list, rather than changing process environment or Lisp state.

use super::super::paths::{doc_dir_of_emacs, find_emacs_on_path, info_dir_of_emacs};
use crate::lisp::value::Value;

pub(super) fn shell() -> Value {
    Value::string(std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into()))
}

pub(super) fn temporary_directory() -> Value {
    Value::string({
        let d = std::env::var("TMPDIR").unwrap_or_else(|_| "/tmp".into());
        if d.ends_with('/') { d } else { format!("{d}/") }
    })
}

pub(super) fn documentation_directory() -> Value {
    Value::string(
        find_emacs_on_path(&|exe| doc_dir_of_emacs(exe, 0))
            .unwrap_or_else(|| "/usr/share/emacs/".into()),
    )
}

pub(super) fn info_directory() -> Value {
    Value::string(
        find_emacs_on_path(&|exe| info_dir_of_emacs(exe, 0))
            .unwrap_or_else(|| "/usr/local/share/info/".into()),
    )
}
