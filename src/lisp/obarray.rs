//! Symbol internment (the obarray).
//!
//! Every interned symbol gets a `SymId` index. Symbol data (value cell,
//! function cell, plist, flags) lives in a single vector so `Value::Sym`
//! stays small and comparisons are cheap integer compares.

use std::collections::HashMap;

use super::value::{SymId, Value};

/// Per-symbol storage.
pub struct Symbol {
    pub name: String,
    /// Global value cell. Dynamic bindings save/restore this slot
    /// (the specbind stack), so lookup is always O(1).
    pub value: Value,
    /// Function cell: a Subr, Lambda, macro Lambda, or symbol (alias chain).
    pub function: Value,
    /// Property list (proper list of alternating key/value).
    pub plist: Value,
    /// `defvar`/`defconst` marks the symbol special (dynamically bound
    /// even under `lexical-binding`).
    pub special: bool,
    /// Constants (nil, t, keywords, defconst) can't be rebound or set.
    pub constant: bool,
    /// `make-variable-buffer-local`: setting this var auto-creates a
    /// buffer-local binding in the current buffer.
    pub make_local_if_set: bool,
    /// GNU `DEFVAR_PER_BUFFER': every buffer has a slot for this var;
    /// `local-variable-p' is always t for it and `setq' writes the
    /// buffer-local slot (e.g. `post-command-hook').
    pub always_local: bool,
    /// A C-backed variable (GNU `DEFVAR_LISP'/`DEFVAR_PER_BUFFER'):
    /// `makunbound' refuses with "Built-in variable may not be unbound".
    pub builtin_variable: bool,
    /// Whether this symbol names a defined variable (boundp / defvar'd).
    pub variable_documentation: Option<String>,
    /// Uninterned symbols (make-symbol/gensym) print with a `#:' prefix.
    pub uninterned: bool,
}

/// Well-known symbol ids, interned at startup. Adding a new entry here
/// requires interning it in `Obarray::new` in the same order.
pub mod sym {
    use super::SymId;
    pub const NIL: SymId = 0;
    pub const T: SymId = 1;
    pub const QUOTE: SymId = 2;
    pub const FUNCTION: SymId = 3;
    pub const IF: SymId = 4;
    pub const COND: SymId = 5;
    pub const PROGN: SymId = 6;
    pub const PROG1: SymId = 7;
    pub const PROG2: SymId = 8;
    pub const LET: SymId = 9;
    pub const LET_STAR: SymId = 10;
    pub const SETQ: SymId = 11;
    pub const SETQ_DEFAULT: SymId = 12;
    pub const DEFVAR: SymId = 13;
    pub const DEFCONST: SymId = 14;
    pub const DEFUN: SymId = 15;
    pub const DEFMACRO: SymId = 16;
    pub const LAMBDA: SymId = 17;
    pub const WHILE: SymId = 18;
    pub const CATCH: SymId = 19;
    pub const UNWIND_PROTECT: SymId = 20;
    pub const CONDITION_CASE: SymId = 21;
    pub const AND: SymId = 22;
    pub const OR: SymId = 23;
    pub const INTERACTIVE: SymId = 24;
    pub const SAVE_EXCURSION: SymId = 25;
    pub const SAVE_CURRENT_BUFFER: SymId = 26;
    pub const WITH_CURRENT_BUFFER: SymId = 27;
    pub const SAVE_RESTRICTION: SymId = 28;
    pub const TRACK_MOUSE: SymId = 29;
    pub const OPTIONAL: SymId = 30; // &optional
    pub const REST: SymId = 31; // &rest
    pub const BACKQUOTE: SymId = 32;
    pub const COMMA: SymId = 33;
    pub const COMMA_AT: SymId = 34;
    pub const COMMA_DOT: SymId = 35;
    pub const MACRO: SymId = 36;
    pub const UNBOUND: SymId = 37;
    pub const QUOTE_FUNCTION: SymId = 38;
    pub const SET: SymId = 39;
    pub const WHILE_NO_INPUT: SymId = 40;
    pub const AND_LET_STAR: SymId = 41;
    pub const INTERACTIVE_FORM: SymId = 42;
    pub const ERROR: SymId = 43;
    pub const VOID_FUNCTION: SymId = 44;
    pub const VOID_VARIABLE: SymId = 45;
    pub const WRONG_TYPE_ARGUMENT: SymId = 46;
    pub const WRONG_NUMBER_OF_ARGUMENTS: SymId = 47;
    pub const ARGS_OUT_OF_RANGE: SymId = 48;
    pub const QUIT: SymId = 49;
    pub const ARITH_ERROR: SymId = 50;
    pub const BEGINNING_OF_BUFFER: SymId = 51;
    pub const END_OF_BUFFER: SymId = 52;
    pub const BUFFER_READ_ONLY: SymId = 53;
    pub const MARK_INACTIVE: SymId = 54;
    pub const TEXT_READ_ONLY: SymId = 55;
    pub const FILE_ERROR: SymId = 56;
    pub const FILE_MISSING: SymId = 57;
    pub const CIRCULAR_LIST: SymId = 58;
    pub const INVALID_READ_SYNTAX: SymId = 59;
    pub const END_OF_FILE: SymId = 60;
    pub const INVALID_FUNCTION: SymId = 61;
    pub const SETTING_CONSTANT: SymId = 62;
    pub const SEARCH_FAILED: SymId = 63;
    pub const USER_ERROR: SymId = 64;
    pub const NO_CATCH: SymId = 65;
    pub const FEATUREP_TEST: SymId = 66;
    pub const MARK_SET: SymId = 67;
    pub const MARK_ACTIVE: SymId = 68;
    pub const REST2: SymId = 69;
    pub const TOP_LEVEL: SymId = 70;
    pub const EXIT_RECURSIVE_EDIT: SymId = 71;
    pub const SAVE_MARK_AND_EXCURSION: SymId = 72;
    pub const PROGV: SymId = 73;
}

const CORE_SYMBOLS: &[&str] = &[
    "nil",
    "t",
    "quote",
    "function",
    "if",
    "cond",
    "progn",
    "prog1",
    "prog2",
    "let",
    "let*",
    "setq",
    "setq-default",
    "defvar",
    "defconst",
    "defun",
    "defmacro",
    "lambda",
    "while",
    "catch",
    "unwind-protect",
    "condition-case",
    "and",
    "or",
    "interactive",
    "save-excursion",
    "save-current-buffer",
    "with-current-buffer",
    "save-restriction",
    "track-mouse",
    "&optional",
    "&rest",
    "`",
    ",",
    ",@",
    ",.",
    "macro",
    "unbound",
    "quote-function",
    "set",
    "while-no-input",
    "and-let*",
    "interactive-form",
    "error",
    "void-function",
    "void-variable",
    "wrong-type-argument",
    "wrong-number-of-arguments",
    "args-out-of-range",
    "quit",
    "arith-error",
    "beginning-of-buffer",
    "end-of-buffer",
    "buffer-read-only",
    "mark-inactive",
    "text-read-only",
    "file-error",
    "file-missing",
    "circular-list",
    "invalid-read-syntax",
    "end-of-file",
    "invalid-function",
    "setting-constant",
    "search-failed",
    "user-error",
    "no-catch",
    "featurep",
    "mark-set",
    "mark-active",
    "rest",
    "top-level",
    "exit-recursive-edit",
    "save-mark-and-excursion",
    "progv",
];

/// The obarray: interned symbol table.
pub struct Obarray {
    map: HashMap<String, SymId>,
    symbols: Vec<Symbol>,
    /// `nil`, `t` and `nil` again for a stable `[nil t ...]` test ordering.
    gensym_counter: u64,
    gensym_n: u64,
}

impl Obarray {
    pub fn new() -> Self {
        let mut ob = Obarray {
            map: HashMap::with_capacity(4096),
            symbols: Vec::with_capacity(4096),
            gensym_counter: 0,
            gensym_n: 0,
        };
        for name in CORE_SYMBOLS {
            ob.intern(name);
        }
        // The unbound marker must not be reachable via `intern`: in Emacs,
        // `unbound` is an ordinary interned symbol and `boundp` doesn't
        // treat it specially — the real marker is internal.
        ob.map.remove("unbound");
        ob.symbols[sym::UNBOUND as usize].uninterned = true;
        // nil and t are self-evaluating constants.
        ob.symbols[sym::NIL as usize].constant = true;
        ob.symbols[sym::NIL as usize].special = true;
        ob.symbols[sym::NIL as usize].value = Value::Nil;
        ob.symbols[sym::T as usize].constant = true;
        ob.symbols[sym::T as usize].special = true;
        ob.symbols[sym::T as usize].value = Value::Sym(sym::T);
        ob
    }

    /// Intern `name`, creating the symbol if necessary.
    pub fn intern(&mut self, name: &str) -> SymId {
        if let Some(&id) = self.map.get(name) {
            return id;
        }
        let id = self.symbols.len() as SymId;
        self.map.insert(name.to_string(), id);
        self.symbols.push(Symbol {
            name: name.to_string(),
            value: Value::Sym(sym::UNBOUND),
            function: Value::Sym(sym::UNBOUND),
            plist: Value::Nil,
            special: false,
            constant: name.starts_with(':'),
            make_local_if_set: false,
            always_local: false,
            builtin_variable: false,
            variable_documentation: None,
            uninterned: false,
        });
        if name.starts_with(':') {
            self.symbols[id as usize].value = Value::Sym(id);
        }
        id
    }

    /// Look up a symbol without interning.
    pub fn intern_soft(&self, name: &str) -> Option<SymId> {
        self.map.get(name).copied()
    }

    /// Fresh uninterned symbol (for `make-symbol`/`gensym`).
    pub fn make_symbol(&mut self, name: &str) -> SymId {
        // Uninterned symbols get unique internal names so the map key
        // never collides with real symbols.
        self.gensym_counter += 1;
        let key = format!("#uninterned#{}\x00{}", name, self.gensym_counter);
        let id = self.symbols.len() as SymId;
        self.map.insert(key, id);
        self.symbols.push(Symbol {
            name: name.to_string(),
            value: Value::Sym(sym::UNBOUND),
            function: Value::Sym(sym::UNBOUND),
            plist: Value::Nil,
            special: false,
            constant: false,
            make_local_if_set: false,
            always_local: false,
            builtin_variable: false,
            variable_documentation: None,
            uninterned: true,
        });
        id
    }

    pub fn symbol(&self, id: SymId) -> &Symbol {
        &self.symbols[id as usize]
    }

    pub fn symbol_mut(&mut self, id: SymId) -> &mut Symbol {
        &mut self.symbols[id as usize]
    }

    pub fn name(&self, id: SymId) -> &str {
        &self.symbols[id as usize].name
    }

    /// `gensym`: uninterned symbol named `<prefix><n>`.
    /// Emacs keeps a dedicated counter (the `gensym-counter`
    /// variable) — make-symbol doesn't consume it.
    pub fn gensym(&mut self, prefix: &str) -> SymId {
        let n = self.gensym_n;
        self.gensym_n += 1;
        self.make_symbol(&format!("{}{}", prefix, n))
    }

    /// Remove a name mapping (for `unintern`). The symbol object itself
    /// stays — indices are stable — but `intern` on the name will create
    /// a fresh symbol.
    pub fn unintern_by_name(&mut self, name: &str) {
        self.map.remove(name);
    }

    /// Every interned symbol id (for `mapatoms`); uninterned symbols
    /// (gensyms, the `unbound` marker) are skipped like in Emacs.
    pub fn all_ids(&self) -> Vec<SymId> {
        (0..self.symbols.len() as SymId)
            .filter(|id| !self.symbols[*id as usize].uninterned)
            .collect()
    }

    pub fn len(&self) -> usize {
        self.symbols.len()
    }
}

impl Default for Obarray {
    fn default() -> Self {
        Self::new()
    }
}
