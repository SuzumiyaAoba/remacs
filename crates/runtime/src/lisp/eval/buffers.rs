//! Interpreter-facing buffer selection, buffer initialization, and standard character tables.

use super::Interp;
use crate::lisp::value::Value;
use std::{cell::RefCell, rc::Rc};

impl Interp {
    // ---------- buffers ----------

    /// The current buffer's shared handle.
    pub fn current_buffer_ref(&self) -> Option<Rc<RefCell<crate::buffer::Buffer>>> {
        self.buffers.get(self.current_buffer)
    }

    /// Buffer id for a `Value::Buffer` or `Value::Str` (name).
    pub fn buffer_id_of(&self, v: &Value) -> Option<usize> {
        match v {
            Value::Buffer(b) => Some(b.borrow().id),
            Value::Str(s) => self.buffers.by_name(&s.borrow()),
            _ => None,
        }
    }

    /// `Value` for a live buffer id.
    pub fn buffer_value(&self, id: usize) -> Option<Value> {
        self.buffers.get(id).map(|r| Value::Buffer(r.clone()))
    }

    pub fn buffer_name(&self, id: usize) -> Option<crate::lisp::text::EmacsText> {
        self.buffers.get(id).map(|b| b.borrow().name.clone())
    }

    /// Initialize the fixed GNU buffer slots separately from mutable
    /// default values.  In particular mode-name defaults to nil, but a
    /// fresh buffer's own slot is always "Fundamental".
    fn initialize_buffer_slots(&mut self, id: usize) {
        let directory = self.q("default-directory");
        let dir = self.symbol_value(directory);
        let multibyte = self.q("enable-multibyte-characters");
        let multi = self.obarray.symbol(multibyte).value.clone();
        let fundamental = self.q("fundamental-mode");
        let buffer = self.buffers.get(id).expect("new buffer");
        let undo = buffer.borrow().undo_list();
        for name in crate::buffer::primitives::ALWAYS_LOCAL_VARIABLES {
            let symbol = self.intern(name);
            let value = match *name {
                "default-directory" => dir.clone(),
                "enable-multibyte-characters" => multi.clone(),
                "major-mode" => Value::Sym(fundamental),
                "mode-name" => Value::string("Fundamental"),
                "buffer-undo-list" => undo.clone(),
                "buffer-auto-save-file-format" | "buffer-invisibility-spec" => Value::t(),
                "buffer-saved-size" | "buffer-display-count" => Value::Int(0),
                _ => Value::Nil,
            };
            buffer.borrow_mut().locals.insert(symbol, value);
        }
    }

    pub fn create_buffer<T: crate::lisp::text::TextSource + ?Sized>(&mut self, name: &T) -> usize {
        let id = self.buffers.create(name);
        self.initialize_buffer_slots(id);
        id
    }

    pub fn create_buffer_exact<T: crate::lisp::text::TextSource + ?Sized>(
        &mut self,
        name: &T,
    ) -> usize {
        let id = self.buffers.create_exact(name);
        self.initialize_buffer_slots(id);
        id
    }

    pub(super) fn create_buffer_nolast<T: crate::lisp::text::TextSource + ?Sized>(
        &mut self,
        name: &T,
    ) -> usize {
        let id = self.buffers.create_exact_nolast(name);
        self.initialize_buffer_slots(id);
        id
    }

    pub fn set_current_buffer(&mut self, id: usize) {
        self.set_current_buffer_position_inner(id);
        self.sync_symbol_position_view();
    }

    fn set_current_buffer_position_inner(&mut self, id: usize) {
        // Emacs: `set-buffer'/`with-current-buffer' do not update
        // buffer-list recency — only window selection/display does.
        if self.buffers.get(id).is_none() {
            return;
        }
        // GNU set_buffer_internal_2 (Bug#18180): the undo list is
        // parked in the base buffer — an indirect buffer's record
        // moves back to its base when we switch away, and a switch
        // INTO an indirect buffer picks the base's list up.
        if let Some(old) = self.buffers.get(self.current_buffer) {
            let (base, list) = {
                let ob = old.borrow();
                (ob.base_buffer.clone(), ob.undo_list())
            };
            if let Some(base) = base {
                base.borrow_mut().set_undo_list(list);
            }
        }
        self.current_buffer = id;
        if let Some(new) = self.buffers.get(id) {
            let base = new.borrow().base_buffer.clone();
            if let Some(base) = base {
                let shared = base.borrow().undo_list();
                new.borrow_mut().set_undo_list(shared);
            }
        }
    }

    /// With `set-buffer` semantics — no error for dead/missing.
    pub fn current_buffer_is_live(&self) -> bool {
        self.buffers.get(self.current_buffer).is_some()
    }

    /// `current-case-table': the buffer's local case table if set,
    /// else `standard-case-table'.
    pub fn current_case_table(&mut self) -> Value {
        let local = self
            .buffers
            .get(self.current_buffer)
            .and_then(|b| b.try_borrow().ok().and_then(|bb| bb.case_table.clone()));
        match local {
            Some(t) => t,
            None => self.standard_case_table(),
        }
    }

    /// `standard-case-table': the shared case-table char-table.
    /// GNU's layout: contents = downcase map, extra slots =
    /// {upcase, canonicalize, equivalency} char-tables.  All four are
    /// replayed from GNU Emacs 31's printed tries (see ctdata.rs),
    /// giving exact >255 contents and trie structure.
    pub fn standard_case_table(&mut self) -> Value {
        if let Some(v) = &self.standard_case_table {
            return v.clone();
        }
        use crate::lisp::builtins::misc::{ct_replay, make_ct};
        let case_tag = Value::Sym(self.q("case-table"));
        // GNU's case tables physically carry 3 extra slots each
        // (`char-table-extra-slots' = 3); most stay nil.
        let build = |i: &mut Self, ops: &str| {
            let t = make_ct(
                i,
                case_tag.clone(),
                Value::Nil,
                vec![Value::Nil, Value::Nil, Value::Nil],
            );
            ct_replay(i, &t, ops);
            t
        };
        use crate::lisp::ctdata::*;
        let up = build(self, CASE_UP_OPS);
        let canon = build(self, CASE_CANON_OPS);
        let equiv = build(self, CASE_EQUIV_OPS);
        let down = build(self, CASE_OPS);
        if let (Value::Record(d), Value::Record(c)) = (&down, &canon) {
            let mut rr = d.borrow_mut();
            rr[3] = up;
            rr[4] = canon.clone();
            rr[5] = equiv.clone();
            // GNU's canon table shares the equivalencies table in its
            // own third extra slot.
            c.borrow_mut()[5] = equiv;
        }
        self.standard_case_table = Some(down.clone());
        down
    }

    /// Char-table parent accessor (record identity → parent value).
    pub fn char_table_parent(&self, table: &Value) -> Value {
        let r = match table {
            Value::Record(r) => r,
            _ => return Value::Nil,
        };
        let id = std::rc::Rc::as_ptr(r) as usize;
        self.char_table_parents
            .iter()
            .find(|(k, w, _)| *k == id && w.upgrade().is_some_and(|u| std::rc::Rc::ptr_eq(&u, r)))
            .map(|(_, _, v)| v.clone())
            .unwrap_or(Value::Nil)
    }

    /// Set a char-table's parent (side-table; record layout untouched).
    pub fn set_char_table_parent(&mut self, table: &Value, parent: Value) {
        if let Value::Record(r) = table {
            let id = std::rc::Rc::as_ptr(r) as usize;
            self.char_table_parents
                .retain(|(k, w, _)| *k != id && w.upgrade().is_some());
            if !parent.is_nil() {
                self.char_table_parents
                    .push((id, std::rc::Rc::downgrade(r), parent));
            }
        }
    }

    /// Char-table defalt accessor (record identity → defalt value).
    pub fn char_table_defalt(&self, table: &Value) -> Value {
        let r = match table {
            Value::Record(r) => r,
            _ => return Value::Nil,
        };
        let id = std::rc::Rc::as_ptr(r) as usize;
        self.char_table_defalts
            .iter()
            .find(|(k, w, _)| *k == id && w.upgrade().is_some_and(|u| std::rc::Rc::ptr_eq(&u, r)))
            .map(|(_, _, v)| v.clone())
            .unwrap_or(Value::Nil)
    }

    /// Set a char-table's defalt (side-table; record layout untouched).
    pub fn set_char_table_defalt(&mut self, table: &Value, defalt: Value) {
        if let Value::Record(r) = table {
            let id = std::rc::Rc::as_ptr(r) as usize;
            self.char_table_defalts
                .retain(|(k, w, _)| *k != id && w.upgrade().is_some());
            if !defalt.is_nil() {
                self.char_table_defalts
                    .push((id, std::rc::Rc::downgrade(r), defalt));
            }
        }
    }

    /// `char-code-property-alist': GNU's `charprop.el' registers each
    /// property with its data file name; the file lazily loads into a
    /// real char-table on first access (GNU's dump materializes twelve
    /// of them — our `characters.el' run does the same through
    /// `unicode-property-table-internal').
    pub fn seed_char_code_property_alist(&mut self) -> Value {
        // (prop . FILE) in GNU's dumped order.
        let specs: &[(&str, &str)] = &[
            ("bracket-type", "uni-brackets.el"),
            ("paired-bracket", "uni-brackets.el"),
            ("special-titlecase", "uni-special-titlecase.el"),
            ("special-lowercase", "uni-special-lowercase.el"),
            ("special-uppercase", "uni-special-uppercase.el"),
            ("titlecase", "uni-titlecase.el"),
            ("lowercase", "uni-lowercase.el"),
            ("uppercase", "uni-uppercase.el"),
            ("iso-10646-comment", "uni-comment.el"),
            ("old-name", "uni-old-name.el"),
            ("mirroring", "uni-mirrored.el"),
            ("mirrored", "uni-mirrored.el"),
            ("numeric-value", "uni-numeric.el"),
            ("digit-value", "uni-digit.el"),
            ("decimal-digit-value", "uni-decimal.el"),
            ("decomposition", "uni-decomposition.el"),
            ("bidi-class", "uni-bidi.el"),
            ("canonical-combining-class", "uni-combining.el"),
            ("general-category", "uni-category.el"),
            ("name", "uni-name.el"),
        ];
        let mut alist = Vec::with_capacity(specs.len());
        for &(prop, file) in specs {
            let psym = Value::Sym(self.intern(prop));
            let backing = Value::string(file);
            self.char_code_prop_tables
                .push((prop.to_string(), backing.clone()));
            alist.push(Value::cons(psym, backing));
        }
        Value::list(alist)
    }

    /// `standard-category-table': the shared category table.  Like
    /// GNU's temacs it starts empty — `international/characters.el'
    /// (loaded during init, loadup.el:215) fills in the category
    /// docstrings and per-char membership via `define-category' /
    /// `modify-category-entry'.
    pub fn standard_category_table(&mut self) -> Value {
        if let Some(v) = &self.standard_category_table {
            return v.clone();
        }
        let t = crate::lisp::builtins::misc::make_category_table_value(self, false);
        self.standard_category_table = Some(t.clone());
        t
    }

    /// GNU `BUFFER_LIVE_P` on a buffer id.
    pub fn buffer_live(&self, id: usize) -> bool {
        self.buffers
            .get(id)
            .map(|b| b.borrow().live)
            .unwrap_or(false)
    }
}
