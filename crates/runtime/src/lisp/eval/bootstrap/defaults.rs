//! Register built-in conditions and variable defaults before evaluating Lisp.

mod conditions;
mod environment;
mod policy;
mod values;
use crate::lisp::{eval::Interp, value::Value};

impl Interp {
    pub(super) fn define_error_conditions(&mut self) {
        let ec = self.q("error-conditions");
        let put = |interp: &mut Interp, name: &str, conds: &[&str]| {
            let s = interp.intern(name);
            let list = Value::list(conds.iter().map(|c| Value::Sym(interp.intern(c))).collect());
            interp.put_prop(s, ec, list);
        };
        for &(name, conditions) in conditions::CORE {
            put(self, name, conditions);
        }
        // Remaining GNU error symbols (fileio.c, keyboard.c, module.c,
        // comp.c, alloc.c, thread.c, xdisp.c, search.c, eval.c, ...).
        for &(name, conds) in conditions::ADDITIONAL {
            put(self, name, conds);
        }
        // `target-idx' properties from syms_of_coding (coding.c):
        // `find-operation-coding-system' picks its TARGET argument via
        // the operation symbol's `target-idx' plist entry.
        {
            let tidx = self.q("target-idx");
            for (op, n) in [
                ("insert-file-contents", 0i128),
                ("write-region", 2),
                ("call-process", 0),
                ("call-process-region", 2),
                ("start-process", 2),
                ("open-network-stream", 3),
            ] {
                let s = self.intern(op);
                self.put_prop(s, tidx, Value::Int(n));
            }
        }
        // Tree-sitter errors (treesit.c): a `treesit-error' parent so
        // `ignore-errors'/`condition-case error' catches them — GNU's
        // `treesit-query-valid-p' relies on that to probe grammars.
        let em = self.q("error-message");
        put(self, "treesit-error", &["treesit-error", "error"]);
        let te = self.q("treesit-error");
        self.put_prop(te, em, Value::string("Generic tree-sitter error"));
        for &(name, msg) in conditions::TREE_SITTER {
            put(self, name, &[name, "treesit-error", "error"]);
            let s = self.intern(name);
            self.put_prop(s, em, Value::string(msg));
        }
        for (name, msg) in conditions::MESSAGES {
            let s = self.intern(name);
            self.put_prop(s, em, Value::string(*msg));
        }
    }

    pub(super) fn define_special_variables(&mut self) {
        self.declare_dynamic_variables();
        self.seed_early_variable_values();
        self.declare_buffer_variables();
        // The `obarray' variable's record; its Rc identity is stored on
        // the interpreter so primitives can tell the default symbol
        // table from a custom `obarray-make' record.
        let default_obarray_rec = crate::lisp::Value::vector_ref(vec![
            Value::Sym(self.q("obarray")),
            Value::vector(vec![Value::Nil; 4]),
        ]);
        self.default_obarray = Some(default_obarray_rec.clone());
        Value::Record(default_obarray_rec.clone())
            .set_record_kind(crate::lisp::value::RecordKind::Obarray);

        let defs = self.collect_variable_defaults(&default_obarray_rec);
        for (name, val) in &defs {
            let id = self.intern(name);
            self.obarray.symbol_mut(id).value = val.clone();
        }
        self.initialize_runtime_tables();
    }
    fn declare_dynamic_variables(&mut self) {
        // Variables that are always dynamically bound even under
        // lexical-binding. Real Emacs has hundreds; these cover startup.

        for name in policy::SPECIAL {
            let id = self.intern(name);
            self.obarray.symbol_mut(id).special = true;
        }
    }

    fn seed_early_variable_values(&mut self) {
        // Printer variables with their Emacs defaults.
        for name in ["print-quoted", "print-unreadable"] {
            let id = self.intern(name);
            self.obarray.symbol_mut(id).value = Value::t();
        }
        // GNU simple.el defvars these to function names.
        for (name, val) in [
            (
                "filter-buffer-substring-function",
                "buffer-substring--filter",
            ),
            ("revert-buffer-function", "revert-buffer--default"),
            ("forward-sexp-function", "forward-sexp-default-function"),
            (
                "hack-read-symbol-shorthands-function",
                "hack-read-symbol-shorthands",
            ),
        ] {
            let id = self.intern(name);
            self.obarray.symbol_mut(id).value = Value::Sym(self.intern(val));
        }
        for name in ["print-gensym", "print-escape-newlines", "print-circle"] {
            let id = self.intern(name);
            self.obarray.symbol_mut(id).value = Value::Nil;
        }
    }

    fn declare_buffer_variables(&mut self) {
        // Variables that are automatically buffer-local when set
        // (`make-variable-buffer-local' / DEFVAR_PER_BUFFER semantics in
        // GNU).  Set verified against GNU 31.1 `-Q --batch' via
        // `local-variable-if-set-p' in a fresh buffer.

        for name in policy::AUTO_LOCAL {
            let id = self.intern(name);
            self.obarray.symbol_mut(id).make_local_if_set = true;
            self.obarray.symbol_mut(id).special = true;
        }

        // GNU `DEFVAR_PER_BUFFER' variables: a real slot in every
        // buffer, so `local-variable-p' is t even before any `setq'
        // (set verified against GNU 31.1 in a fresh buffer).
        let per_buffer = crate::buffer::primitives::ALWAYS_LOCAL_VARIABLES;
        for name in per_buffer {
            let id = self.intern(name);
            self.obarray.symbol_mut(id).always_local = true;
            self.obarray.symbol_mut(id).special = true;
        }

        // GNU's C-backed variables: `makunbound' refuses on them
        // (the "Built-in variable may not be unbound" set, verified
        // against GNU 31.1).

        for name in policy::BUILTIN_LOCAL {
            let id = self.intern(name);
            self.obarray.symbol_mut(id).builtin_variable = true;
        }
    }

    fn initialize_runtime_tables(&mut self) {
        // GNU DEFVAR_LISP specials (plain dynamic vars): the
        // lookup_char_property chain vars and the read-only bypass.
        for name in ["char-property-alias-alist", "default-text-properties"] {
            let id = self.intern(name);
            self.obarray.symbol_mut(id).special = true;
        }

        // GNU binds `main-thread' to the main thread object at -Q
        // (threads.c); our thread table's slot 0 is that object.
        {
            let id = self.q("main-thread");
            self.obarray.symbol_mut(id).value =
                Value::Thread(self.threads[self.current_thread].clone());
        }
        // GNU binds `keyboard-translate-table' to a fresh char-table in
        // keyboard.c's syms_of_keyboard; it starts nil.
        {
            let id = self.q("keyboard-translate-table");
            let ktt =
                crate::lisp::builtins::seq::make_char_table_raw(self, Value::Sym(id), Value::Nil);
            self.obarray.symbol_mut(id).value = ktt;
        }

        // GNU seeds auto-coding tables at dump (files.el/mule.el).
        // Quoted literals — identical to a -Q build's contents.
        let _ = self.eval_str(
            "(progn \
               (setq auto-coding-functions \
                     '(sgml-xml-auto-coding-function \
                       sgml-html-meta-auto-coding-function)) \
               (setq auto-coding-alist \
                     '((\"\\\\.\\\\(arc\\\\|zip\\\\|lzh\\\\|lha\\\\|zoo\\\\|[jew]ar\\\\|xpi\\\\|rar\\\\|7z\\\\|squashfs\\\\|ARC\\\\|ZIP\\\\|LZH\\\\|LHA\\\\|ZOO\\\\|[JEW]AR\\\\|XPI\\\\|RAR\\\\|7Z\\\\|SQUASHFS\\\\)\\\\'\" . no-conversion-multibyte) \
                       (\"\\\\.\\\\(exe\\\\|EXE\\\\)\\\\'\" . no-conversion) \
                       (\"\\\\.\\\\(sx[dmicw]\\\\|odt\\\\|tar\\\\|t[bg]z\\\\)\\\\'\" . no-conversion) \
                       (\"\\\\.\\\\(gz\\\\|Z\\\\|bz\\\\|bz2\\\\|xz\\\\|gpg\\\\)\\\\'\" . no-conversion) \
                       (\"\\\\.\\\\(jpe?g\\\\|png\\\\|gif\\\\|tiff?\\\\|p[bpgn]m\\\\)\\\\'\" . no-conversion) \
                       (\"\\\\.pdf\\\\'\" . no-conversion) \
                       (\"/\\\\.editorconfig\\\\'\" . utf-8) \
                       (\"/#[^/]+#\\\\'\" . utf-8-emacs-unix))) \
               (setq auto-coding-regexp-alist \
                     '((\"\\\\`BABYL OPTIONS:[ \t]*-\\\\*-[ \t]*rmail[ \t]*-\\\\*-\" . no-conversion) \
                       (\"\\\\`\\376\\377\" . utf-16be-with-signature) \
                       (\"\\\\`\\377\\376\" . utf-16le-with-signature) \
                       (\"\\\\`\\357\\273\\277\" . utf-8-with-signature) \
                       (\"\\\\`;ELC\\024\\000\\000\\000\" . emacs-mule))))",
        );
    }
}
