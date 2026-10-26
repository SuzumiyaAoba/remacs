//! Symbol internment (the obarray).
//!
//! Every interned symbol gets a `SymId` index. Symbol data (value cell,
//! function cell, plist, flags) lives in a single vector so `Value::Sym`
//! stays small and comparisons are cheap integer compares.

use rustc_hash::FxHashMap as HashMap;

use super::value::{SymId, Value};

/// lread.c compares both the stored bytes and character count of names.
/// In particular, an unibyte byte and a multibyte byte8 character differ.
#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub(crate) struct InternKey {
    pub bytes: Vec<u8>,
    pub characters: usize,
}

impl InternKey {
    pub(crate) fn from_text<T: super::text::TextSource + ?Sized>(text: &T) -> Self {
        text.with_text(|text| {
            let multibyte = text.chars().any(|c| c.code() >= 128 && c.byte8().is_none());
            Self::from_text_kind(text, multibyte)
        })
    }

    pub(crate) fn from_text_kind<T: super::text::TextSource + ?Sized>(text: &T, multibyte: bool) -> Self {
        text.with_text(|text| {
            let mut bytes = Vec::with_capacity(text.len());
            for c in text.chars() {
                if multibyte { c.encode_internal(&mut bytes); }
                else { bytes.push(c.byte8().unwrap_or(c.code() as u8)); }
            }
            Self { bytes, characters: text.len() }
        })
    }
}

/// Per-symbol storage.
pub struct Symbol {
    pub name: String,
    /// Lisp-visible name, retained by identity when supplied to intern/
    /// make-symbol. Internal names allocate this lazily on symbol-name.
    pub name_string: Option<super::value::StrRef>,
    pub(crate) internment_key: Option<InternKey>,
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
    /// Read-only symbols (nil, t, keywords and their aliases).
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
    /// GNU SYMBOL_FORWARDED, as distinct from a C-backed variable that
    /// has become SYMBOL_LOCALIZED. `defvaralias' rejects each differently.
    pub forwarded_variable: bool,
    /// Whether this symbol names a defined variable (boundp / defvar'd).
    pub variable_documentation: Option<super::text::EmacsText>,
    /// Uninterned symbols (make-symbol/gensym) print with a `#:' prefix.
    pub uninterned: bool,
    /// Cached `variable-alias' property target (set by
    /// `defvaralias'/`put'/`setplist' through `Interp::put_prop').
    /// Avoids a plist walk on every variable reference.
    pub var_alias: Option<SymId>,
}

impl Drop for Symbol {
    fn drop(&mut self) {
        super::memory::symbol_freed();
    }
}

impl Drop for Obarray {
    fn drop(&mut self) {
        super::memory::symbol_capacity(self.symbols.capacity(), 0);
    }
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
    map: HashMap<InternKey, Vec<SymId>>,
    /// Derived index for the frequent internal ASCII lookups. Borrowing &str
    /// here avoids allocating a storage-byte key at every evaluator poll.
    ascii: HashMap<String, SymId>,
    symbols: Vec<Symbol>,
    /// `nil`, `t` and `nil` again for a stable `[nil t ...]` test ordering.
    gensym_counter: u64,
    gensym_n: u64,
}

impl Obarray {
    /// No symbols or reserved capacity: only for an unevaluated runtime slot.
    pub(crate) fn empty() -> Self {
        Self {
            map: HashMap::default(),
            ascii: HashMap::default(),
            symbols: Vec::new(),
            gensym_counter: 0,
            gensym_n: 0,
        }
    }

    pub fn new() -> Self {
        let mut ob = Obarray {
            map: HashMap::with_capacity_and_hasher(4096, rustc_hash::FxBuildHasher::default()),
            ascii: HashMap::with_capacity_and_hasher(4096, rustc_hash::FxBuildHasher::default()),
            symbols: Vec::with_capacity(4096),
            gensym_counter: 0,
            gensym_n: 0,
        };
        super::memory::symbol_capacity(0, ob.symbols.capacity());
        for name in CORE_SYMBOLS {
            ob.intern(name);
        }
        // The unbound marker must not be reachable via `intern`: in Emacs,
        // `unbound` is an ordinary interned symbol and `boundp` doesn't
        // treat it specially — the real marker is internal.
        ob.map.remove(&InternKey::from_text("unbound"));
        ob.ascii.remove("unbound");
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
        if name.is_ascii() {
            if let Some(&symbol) = self.ascii.get(name) { return symbol; }
        }
        self.intern_with_key(name, InternKey::from_text(name))
    }

    pub(crate) fn intern_with_key<T: super::text::TextSource + ?Sized>(&mut self, name: &T, key: InternKey) -> SymId {
        if let Some(id) = self.lookup_key(&key) {
            return id;
        }
        self.intern_new_with_key(name, key)
    }

    pub(crate) fn intern_new_with_key<T: super::text::TextSource + ?Sized>(&mut self, name: &T, key: InternKey) -> SymId {
        name.with_text(|name| {
        let id = self.symbols.len() as SymId;
        self.map.entry(key.clone()).or_default().push(id);
        self.refresh_ascii(&key);
        let old_capacity = self.symbols.capacity();
        self.symbols.push(Symbol {
            name: format!("{name}"),
            name_string: (!name.is_ascii()).then(|| Value::string_ref(name)),
            internment_key: Some(key),
            value: Value::Sym(sym::UNBOUND),
            function: Value::Sym(sym::UNBOUND),
            plist: Value::Nil,
            special: false,
            constant: name.starts_with(':'),
            make_local_if_set: false,
            always_local: false,
            builtin_variable: false,
            forwarded_variable: false,
            variable_documentation: None,
            uninterned: false,
            var_alias: None,
        });
        super::memory::symbol_allocated();
        super::memory::symbol_capacity(old_capacity, self.symbols.capacity());
        if name.starts_with(':') {
            self.symbols[id as usize].value = Value::Sym(id);
        }
        id

        })
    }

    /// Look up a symbol without interning.
    pub fn intern_soft(&self, name: &str) -> Option<SymId> {
        if name.is_ascii() { return self.ascii.get(name).copied(); }
        self.lookup_key(&InternKey::from_text(name))
    }

    fn refresh_ascii(&mut self, key: &InternKey) {
        if key.characters == key.bytes.len() && key.bytes.is_ascii() {
            let name = std::str::from_utf8(&key.bytes).unwrap();
            if let Some(symbol) = self.lookup_key(key) {
                self.ascii.insert(name.to_owned(), symbol);
            } else {
                self.ascii.remove(name);
            }
        }
    }

    pub(crate) fn lookup_key(&self, key: &InternKey) -> Option<SymId> {
        self.map.get(key).and_then(|ids| ids.last()).copied()
    }

    pub(crate) fn key_candidates(&self, key: &InternKey) -> &[SymId] {
        self.map.get(key).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Fresh uninterned symbol (for `make-symbol`/`gensym`).
    pub fn make_symbol<T: super::text::TextSource + ?Sized>(&mut self, name: &T) -> SymId {
        name.with_text(|name| {
        self.gensym_counter += 1;
        let id = self.symbols.len() as SymId;
        let old_capacity = self.symbols.capacity();
        self.symbols.push(Symbol {
            name: format!("{name}"),
            name_string: (!name.is_ascii()).then(|| Value::string_ref(name)),
            internment_key: None,
            value: Value::Sym(sym::UNBOUND),
            function: Value::Sym(sym::UNBOUND),
            plist: Value::Nil,
            special: false,
            constant: false,
            make_local_if_set: false,
            always_local: false,
            builtin_variable: false,
            forwarded_variable: false,
            variable_documentation: None,
            uninterned: true,
            var_alias: None,
        });
        super::memory::symbol_allocated();
        super::memory::symbol_capacity(old_capacity, self.symbols.capacity());
        id

        })
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

    /// Unintern the `g<N>' strays interned while reading boot source
    /// (`let' bindings named g0..): GNU's `.elc' boot never interns
    /// them, so `gensym' would skip names it should hand out.
    pub fn unintern_boot_gensyms(&mut self) {
        let stray: Vec<InternKey> = self
            .map
            .iter()
            .filter(|(_, ids)| {
                let name = &self.symbols[*ids.last().unwrap() as usize].name;
                name.len() > 1 && name.starts_with('g') && name[1..].chars().all(|c| c.is_ascii_digit())
            })
            .map(|(key, _)| key.clone())
            .collect();
        for name in stray {
            if let Some(ids) = self.map.remove(&name) {
                for id in ids { self.symbols[id as usize].uninterned = true; }
            }
            self.refresh_ascii(&name);
        }
    }

    /// Remove a name mapping (for `unintern`). The symbol object itself
    /// stays — indices are stable — but `intern` on the name will create
    /// a fresh symbol.
    pub fn unintern_by_name(&mut self, name: &str) {
        if let Some(id) = self.lookup_key(&InternKey::from_text(name)) {
            self.unintern_symbol(id);
        }
    }

    pub(crate) fn unintern_symbol(&mut self, id: SymId) -> bool {
        let Some(key) = self.symbols[id as usize].internment_key.clone() else { return false; };
        let Some(ids) = self.map.get_mut(&key) else { return false; };
        let Some(position) = ids.iter().position(|&member| member == id) else { return false; };
        ids.remove(position);
        if ids.is_empty() { self.map.remove(&key); }
        self.refresh_ascii(&key);
        self.symbols[id as usize].uninterned = true;
        true
    }

    pub(crate) fn clear_namespace(&mut self) {
        // GNU obarray-clear leaves the symbols' interned flags and cells
        // alone, but releases every bucket in the table.
        self.map.clear();
        self.ascii.clear();
    }

    /// Every interned symbol id (for `mapatoms`); uninterned symbols
    /// (gensyms, the `unbound` marker) are skipped like in Emacs.
    pub fn all_ids(&self) -> Vec<SymId> {
        (0..self.symbols.len() as SymId)
            .filter(|id| self.is_initial_symbol(*id))
            .collect()
    }

    pub(crate) fn is_core_symbol(&self, id: SymId) -> bool {
        (id as usize) < CORE_SYMBOLS.len()
    }

    pub(crate) fn dump_counters(&self) -> (u64, u64) {
        (self.gensym_counter, self.gensym_n)
    }

    pub(crate) fn is_initial_symbol(&self, id: SymId) -> bool {
        self.symbols[id as usize].internment_key.as_ref()
            .is_some_and(|key| self.key_candidates(key).contains(&id))
    }

    pub(crate) fn prepare_dump_core(&mut self, id: SymId, uninterned: bool) -> Result<(), String> {
        if !self.is_core_symbol(id) {
            return Err("Invalid core symbol id".into());
        }
        if uninterned {
            self.unintern_symbol(id);
            self.symbols[id as usize].uninterned = true;
        }
        Ok(())
    }

    pub(crate) fn restore_dump_membership(
        &mut self,
        interned: &[SymId],
        retained: &[SymId],
        counters: (u64, u64),
    ) -> Result<(), String> {
        let mut map: HashMap<InternKey, Vec<SymId>> = HashMap::default();
        for id in interned {
            let symbol = self
                .symbols
                .get(*id as usize)
                .ok_or("Invalid dumped interned symbol")?;
            let key = symbol.internment_key.clone()
                .unwrap_or_else(|| InternKey::from_text(&symbol.name));
            if symbol.uninterned || map.get(&key).is_some_and(|ids| ids.contains(id)) {
                return Err("Invalid dumped symbol membership".into());
            }
            map.entry(key).or_default().push(*id);
        }
        let retained: std::collections::HashSet<_> = retained.iter().copied().collect();
        for (id, symbol) in self.symbols.iter_mut().enumerate() {
            if !retained.contains(&(id as SymId)) {
                symbol.uninterned = true;
            }
        }
        self.map = map;
        self.ascii = self.map.iter().filter_map(|(key, symbols)| {
            (key.characters == key.bytes.len() && key.bytes.is_ascii()).then(|| (
                String::from_utf8(key.bytes.clone()).unwrap(), *symbols.last().unwrap(),
            ))
        }).collect();
        self.gensym_counter = self.gensym_counter.max(counters.0);
        self.gensym_n = counters.1;
        Ok(())
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
