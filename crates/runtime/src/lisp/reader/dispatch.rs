//! Hash-prefixed read syntax and construction of GNU printed records/hash tables.

use super::circles::patch_in;
use super::{Reader, eof_err, plain_err, read_err, read_err_radix, read_err_sym};
use crate::lisp::{
    Interp,
    error::Flow,
    obarray::sym,
    text::EmacsText,
    value::{SymId, Value},
};
use rustc_hash::FxHashSet as HashSet;
use std::rc::Rc;

impl Reader<'_> {
    /// `#`-dispatch read.
    pub(super) fn read_dispatch(&mut self) -> Result<Value, Flow> {
        debug_assert_eq!(self.peek()?, Some('#'));
        match self.peek_at(1)? {
            Some('\'') => {
                self.pos += 2;
                if self.skip_layout()? {
                    return Err(eof_err(self.interp));
                }
                let obj = self.read_object()?;
                Ok(Value::list(vec![Value::Sym(sym::FUNCTION), obj]))
            }
            Some(':') => {
                self.pos += 2;
                let start = self.pos;
                // Uninterned symbol; GNU accepts an empty name (`#:'),
                // which reads back as `##'.
                let (tok, _) = self.read_symbol_token()?;
                let symbol = self.symbol_token(&tok, true, false)?;
                Ok(if tok.is_empty() {
                    self.interp.sym(symbol)
                } else {
                    self.located_symbol(symbol, start)
                })
            }
            Some('#') => {
                // `##' — the interned empty-name symbol (this is its
                // print representation), used e.g. in `declare-function'
                // arglists to mean "unknown signature".
                self.pos += 2;
                let symbol = self.symbol_token("", false, true)?;
                Ok(self.interp.sym(symbol))
            }
            Some('&') => {
                // `#&N"..."' — bool vector literal: N bits packed
                // LSB-first within each string byte.
                self.pos += 2;
                let mut n: usize = 0;
                while let Some(d) = self.peek()? {
                    if d.is_ascii_digit() {
                        let digit = d.to_digit(10).unwrap() as usize;
                        let Some(next) = n.checked_mul(10).and_then(|n| n.checked_add(digit))
                        else {
                            return Err(read_err(self.interp, "#&"));
                        };
                        // GNU limits bool-vector lengths to fixnums.  This
                        // also leaves room for rounding the count to bytes.
                        let max_length =
                            (crate::lisp::value::FIXNUM_MAX as u128).min(isize::MAX as u128 - 7);
                        if (next as u128) > max_length {
                            return Err(read_err(self.interp, "#&"));
                        }
                        n = next;
                        self.pos += 1;
                    } else {
                        break;
                    }
                }
                if self.next()? != Some('"') {
                    return Err(read_err(self.interp, "#&"));
                }
                let s = self.read_string()?;
                let bytes: Vec<u32> = match &s {
                    Value::Str(s) => {
                        // GNU rejects multibyte strings; raw bytes
                        // (eight-bit proxies) count as their byte value.
                        let mut v: Vec<u32> = Vec::new();
                        for c in s.borrow().chars() {
                            match crate::lisp::value::eight_bit_byte(c) {
                                Some(b) => v.push(b as u32),
                                None if c.code() < 0x80 => v.push((c).code() as u32),
                                None => {
                                    return Err(read_err(self.interp, "#&..."));
                                }
                            }
                        }
                        v
                    }
                    _ => vec![],
                };
                // GNU requires SCHARS == ceil(N/8); it also accepts the
                // Emacs 19 form where N counts the string's total bits
                // minus the last byte: N == (SCHARS-1)*8.
                let schars = bytes.len();
                if !(schars == (n + 7) / 8 || (schars >= 1 && n == (schars - 1) * 8)) {
                    return Err(read_err(self.interp, "#&..."));
                }
                let mut bits = Vec::with_capacity(n);
                for k in 0..n {
                    let byte = bytes.get(k / 8).copied().unwrap_or(0);
                    bits.push(byte >> (k % 8) & 1 != 0);
                }
                Ok(crate::lisp::builtins::misc::make_bool_vector(
                    self.interp,
                    bits,
                ))
            }
            Some('@') => {
                // `#@NNN' — used by .elc files to skip lazy doc
                // strings/byte-code.  For non-file sources GNU skips
                // to the next \037 byte (or EOF), then re-reads.
                // `#@00' skips to EOF and yields nil.
                self.pos += 2;
                let mut n: usize = 0;
                let mut digits: usize = 0;
                while let Some(c) = self.peek()? {
                    if c.is_ascii_digit() {
                        let digit = c as usize - '0' as usize;
                        let Some(next) = n.checked_mul(10).and_then(|n| n.checked_add(digit))
                        else {
                            return Err(read_err(self.interp, "#@"));
                        };
                        // GNU stores this count in ptrdiff_t, not size_t.
                        if next > isize::MAX as usize {
                            return Err(read_err(self.interp, "#@"));
                        }
                        n = next;
                        self.pos += 1;
                        digits = digits.saturating_add(1);
                    } else {
                        break;
                    }
                }
                if digits > 0 && n == 0 {
                    if matches!(&self.source, super::ReadSource::Function { .. }) {
                        while self.next()?.is_some() {}
                    } else {
                        self.pos = self.chars.len();
                    }
                    return Ok(Value::Nil);
                }
                // Function streams count N characters before scanning to the
                // record separator. If N itself consumes that separator, GNU
                // ends this read instead of skipping on to the next record.
                if matches!(&self.source, super::ReadSource::Function { .. }) {
                    for _ in 0..n {
                        let Some(c) = self.next()? else {
                            return Err(eof_err(self.interp));
                        };
                        if c == '\u{1f}' {
                            return Err(eof_err(self.interp));
                        }
                    }
                }
                while let Some(c) = self.peek()? {
                    self.pos += 1;
                    if c == '\u{1f}' {
                        break;
                    }
                }
                // In-memory readers treat the 0x1f record separator as the
                // end of this input object.  Bytes after it belong to the
                // surrounding load protocol and are not a new Lisp form.
                // GNU's function-stream reader continues this same read
                // after the separator instead of reporting EOF.
                if matches!(&self.source, super::ReadSource::Function { .. }) {
                    return match self.read_inner()? {
                        Some(value) => Ok(value),
                        None => Err(eof_err(self.interp)),
                    };
                }
                Err(eof_err(self.interp))
            }
            Some('(') => {
                // `#("str" BEG END (plist) ...)' — propertized string
                // literal; after the string each (beg end plist)
                // triple attaches properties to that char range.
                self.pos += 2;
                let items = self.read_seq_unannotated(')')?;
                match items.first() {
                    Some(Value::Str(s)) => {
                        let s = s.clone();
                        let mut rest = &items[1..];
                        while !rest.is_empty() {
                            if rest.len() < 3 {
                                return Err(read_err(self.interp, "Invalid string property list"));
                            }
                            crate::lisp::builtins::call_canonical_builtin(
                                self.interp,
                                "set-text-properties",
                                vec![
                                    rest[0].clone(),
                                    rest[1].clone(),
                                    rest[2].clone(),
                                    Value::Str(s.clone()),
                                ],
                            )?;
                            rest = &rest[3..];
                        }
                        Ok(Value::Str(s))
                    }
                    _ => Err(read_err_sym(self.interp, "#")),
                }
            }
            Some('^') => {
                // `#^[DEFALT PARENT PURPOSE ASCII S1..S64 EXTRAS...]'
                // — char-table literal (the inverse of the printer's
                // `#^' format); `#^^[DEPTH MIN-CHAR S0..SN]' is a
                // sub char-table.  Both land in the Record layout
                // used by misc::char_table_vec.
                self.pos += 2;
                let sub = self.peek()? == Some('^');
                if sub {
                    self.pos += 1;
                }
                if self.next()? != Some('[') {
                    return Err(read_err_sym(self.interp, "#^"));
                }
                let items = self.read_seq_unannotated(']')?;
                if sub {
                    // GNU validates the complete sub-table header and
                    // depth-specific slot count before constructing a trie
                    // node.  Short records cannot safely reach its printer.
                    if items.len() < 2 {
                        return Err(plain_err(self.interp, "Invalid size of sub-char-table"));
                    }
                    let slots = match items[0] {
                        Value::Int(1) => 16,
                        Value::Int(2) => 32,
                        Value::Int(3) => 128,
                        _ => return Err(plain_err(self.interp, "Invalid depth in sub-char-table")),
                    };
                    if items.len() != slots + 2 {
                        return Err(plain_err(self.interp, "Invalid size in sub-char-table"));
                    }
                    if !matches!(items[1], Value::Int(n)
                        if (0..=crate::lisp::builtins::misc::CT_MAX_CHAR as i128).contains(&n))
                    {
                        return Err(plain_err(
                            self.interp,
                            "Invalid minimum character in sub-char-table",
                        ));
                    }
                    let mut rec = Vec::with_capacity(items.len() + 1);
                    rec.push(Value::Sym(self.interp.q("sub-char-table")));
                    rec.extend(items);
                    return Ok(Value::record_with_kind(
                        rec,
                        crate::lisp::value::RecordKind::SubCharTable,
                    ));
                }
                if items.len() < 68 {
                    return Err(read_err(self.interp, "Invalid size char-table"));
                }
                let defalt = items[0].clone();
                let parent = items[1].clone();
                let purpose = items[2].clone();
                let mut slots: Vec<Value> = items.iter().skip(3).take(65).cloned().collect();
                slots.resize(65, Value::Nil);
                let mut rec = vec![
                    Value::Sym(self.interp.q("char-table")),
                    purpose,
                    Value::vector(slots),
                ];
                rec.extend(items.iter().skip(68).cloned());
                let t = Value::record_with_kind(rec, crate::lisp::value::RecordKind::CharTable);
                if !defalt.is_nil() {
                    self.interp.set_char_table_defalt(&t, defalt);
                }
                if !parent.is_nil() {
                    self.interp.set_char_table_parent(&t, parent);
                }
                Ok(t)
            }
            Some('[') => {
                // `#[...]' — a compiled/interpreted function object.
                // GNU read_bytecode() enforces the closure layout:
                // size in CLOSURE_STACK_DEPTH..=CLOSURE_INTERACTIVE+1
                // (3..=6), arglist a fixnum/cons/nil, and either
                //   (STRING code && VECTOR constants && size>3
                //    && FIXNATP stack-depth)   — byte-code, or
                //   (CONS code && (CONS||NIL) constants) — interpreted.
                self.pos += 2;
                let mut items = self.read_seq_unannotated(']')?;
                let size = items.len();
                let arglist = items.first().cloned().unwrap_or(Value::Nil);
                let arglist_ok =
                    matches!(arglist, Value::Nil | Value::Cons(_)) || arglist.fixnump();
                let code_ok = match items.get(1) {
                    Some(Value::Str(_)) => {
                        // Byte-code: constants must be a vector and
                        // STACK_DEPTH (slot 3) a natural fixnum.
                        size > 3
                            && crate::lisp::builtins::evalfn::valid_byte_code_fields(
                                &items[1], &items[2], &items[3],
                            )
                    }
                    Some(Value::Cons(_)) => {
                        // Interpreted: constants (env) cons or nil.
                        size > 2 && matches!(items.get(2), Some(Value::Nil) | Some(Value::Cons(_)))
                    }
                    _ => false,
                };
                if !(size >= 3 && size <= 6 && arglist_ok && code_ok) {
                    return Err(read_err_sym(self.interp, "Invalid byte-code object"));
                }
                if crate::lisp::builtins::strfn::string_multibyte_p(self.interp, &items[1]) {
                    items[1] = crate::lisp::builtins::strfn::f_string_as_unibyte(
                        self.interp,
                        vec![items[1].clone()].into(),
                    )?;
                }
                let body = items.get(1).cloned().unwrap_or(Value::Nil);
                let env = items.get(2).cloned().unwrap_or(Value::Nil);
                // `#[... nil]' prints with a `nil' env (plain lambda);
                // `#[... (t)]' is a top-level dynamic function.
                let plain = env.is_nil();
                crate::lisp::builtins::misc::make_interpreted_closure(
                    self.interp,
                    &arglist,
                    &body,
                    &env,
                    plain,
                    Some(items),
                )
            }
            Some('s') => {
                // `#s(...)' — record object; `#s(hash-table ...)' reads
                // a real hash table (GNU's printed-hash-table format).
                self.pos += 2;
                let next = self.peek()?;
                if next != Some('(') {
                    return Err(read_err_sym(
                        self.interp,
                        if next.is_none() { "#s" } else { "#s " },
                    ));
                }
                self.pos += 1;
                let items = self.read_seq_unannotated(')')?;
                // `#s(TYPE ...)' — the type symbol is required.
                if items.is_empty() {
                    return Err(read_err_sym(self.interp, "#s"));
                }
                if let Value::Sym(t) = &items[0] {
                    if self.interp.q_soft("hash-table") == Some(*t) {
                        if let Some(h) = read_hash_literal(self.interp, &items[1..])? {
                            return Ok(h);
                        }
                    }
                }
                crate::lisp::builtins::data::check_record_slots(self.interp, items.len() as i128)?;
                Ok(Value::record(items))
            }
            Some('_') => {
                // `#_' forces a symbol and suppresses shorthand expansion.
                self.pos += 2;
                let start = self.pos;
                let (tok, _) = self.read_symbol_token()?;
                let symbol = self.symbol_token(&tok, false, false)?;
                Ok(if tok.is_empty() {
                    self.interp.sym(symbol)
                } else {
                    self.located_symbol(symbol, start)
                })
            }
            Some('|') | Some('<') => {
                // GNU's read point is past `#' when these signal.
                self.pos += 1;
                let c = if matches!(self.peek()?, Some('|')) {
                    "|"
                } else {
                    "<"
                };
                Err(read_err_sym(self.interp, &format!("#{c}")))
            }
            Some('$') => {
                // #$ — the name of the file being loaded
                // (Emacs substitutes load-file-name, nil outside load).
                self.pos += 2;
                let name = self
                    .interp
                    .q_soft("load-file-name")
                    .filter(|id| self.interp.bound_p(*id))
                    .map(|id| self.interp.symbol_value(id));
                match name {
                    Some(Value::Str(_)) => Ok(name.unwrap()),
                    _ => Ok(Value::Nil),
                }
            }
            Some('x') | Some('X') => {
                self.pos += 2;
                let n = self.read_radix_int(16)?;
                Ok(Value::from_bigint(n))
            }
            Some('o') | Some('O') => {
                self.pos += 2;
                let n = self.read_radix_int(8)?;
                Ok(Value::from_bigint(n))
            }
            Some('b') | Some('B') => {
                self.pos += 2;
                let n = self.read_radix_int(2)?;
                Ok(Value::from_bigint(n))
            }
            Some(c) if c.is_ascii_digit() => {
                // #Nr radix, #N= label, #N# label reference.
                let start = self.pos;
                self.pos += 1;
                let mut n: u64 = 0;
                while let Some(d) = self.peek()? {
                    if d.is_ascii_digit() {
                        self.pos += 1;
                        n = match n
                            .checked_mul(10)
                            .and_then(|n| n.checked_add(d.to_digit(10).unwrap() as u64))
                        {
                            Some(n) if n <= i64::MAX as u64 => n,
                            _ => return Err(self.hash_number_error(start)),
                        };
                    } else {
                        break;
                    }
                }
                match self.next()? {
                    Some('r') | Some('R') => {
                        if n > u32::MAX as u64 {
                            return Err(read_err(self.interp, &format!("integer, radix {n}")));
                        }
                        let v = self.read_radix_int(n as u32)?;
                        Ok(Value::from_bigint(v))
                    }
                    Some('=') | Some('#')
                        if n > ((1u64 << 61) - 1)
                            || self
                                .interp
                                .q_soft("read-circle")
                                .is_some_and(|id| self.interp.symbol_value(id).is_nil()) =>
                    {
                        Err(self.hash_number_error(start))
                    }
                    Some('=') => {
                        // GNU registers a cons placeholder before reading
                        // the object, replacing any previous definition.
                        let placeholder = Value::cons(Value::Nil, Value::Nil);
                        self.labels.insert(n, placeholder.clone());
                        if self.skip_layout()? {
                            return Err(eof_err(self.interp));
                        }
                        let obj = self.read_object()?;
                        if let Value::Cons(pair) = &obj {
                            let Value::Cons(stub) = &placeholder else {
                                unreachable!()
                            };
                            if Rc::ptr_eq(pair, stub) {
                                self.pos = self.pos.saturating_sub(1);
                                return Err(read_err(self.interp, "nonsensical self-reference"));
                            }
                            // Reuse the placeholder as the actual cons.
                            // Nested/redefined labels keep GNU's identity
                            // behavior, rather than aliasing the read pair.
                            let (car, cdr) = {
                                let pair = pair.borrow();
                                (pair.car.clone(), pair.cdr.clone())
                            };
                            let mut stub = stub.borrow_mut();
                            stub.car = car;
                            stub.cdr = cdr;
                            drop(stub);
                            Ok(placeholder)
                        } else {
                            let mut seen = HashSet::default();
                            let mut changed = HashSet::default();
                            patch_in(
                                self.interp,
                                &obj,
                                &placeholder,
                                &obj,
                                &mut seen,
                                &mut changed,
                            )?;
                            self.labels.insert(n, obj.clone());
                            Ok(obj)
                        }
                    }
                    Some('#') => {
                        let object = self.labels.get(&n).cloned();
                        object.ok_or_else(|| self.hash_number_error(start))
                    }
                    _ => {
                        // GNU signals with point on the consumed char
                        // (or the stream tail at EOF) — error_pos
                        // counts through pos.  At EOF the datum is
                        // just "#N" — no phantom char.
                        Err(self.hash_number_error(start))
                    }
                }
            }
            other => {
                // GNU's read point is already past `#' when the error
                // surfaces — buffer-source errors then report the
                // column one past ours.  Advance before signalling.
                // At EOF GNU prints just "#" — no phantom char.
                self.pos += 1;
                match other {
                    Some(c) => Err(read_err_sym(self.interp, &format!("#{c}"))),
                    None => Err(read_err_sym(self.interp, "#")),
                }
            }
        }
    }

    fn hash_number_error(&mut self, start: usize) -> Flow {
        let token: EmacsText = self.chars[start..self.pos].iter().collect();
        self.pos = self.pos.saturating_sub(1);
        read_err_sym(self.interp, &format!("{token}"))
    }

    fn read_radix_int(&mut self, radix: u32) -> Result<num_bigint::BigInt, Flow> {
        if !(2..=36).contains(&radix) {
            return Err(read_err_radix(self.interp, radix));
        }
        let neg = match self.peek()? {
            Some('-') => {
                self.pos += 1;
                true
            }
            Some('+') => {
                self.pos += 1;
                false
            }
            _ => false,
        };
        let mut n = num_bigint::BigInt::from(0);
        let mut any = false;
        while let Some(c) = self.peek()? {
            if c.is_ascii_alphanumeric() {
                match c.to_digit(radix) {
                    Some(d) => {
                        n = n * radix + d;
                        any = true;
                        self.pos += 1;
                    }
                    None => return Err(read_err_radix(self.interp, radix)),
                }
            } else {
                break;
            }
        }
        if !any {
            return Err(read_err_radix(self.interp, radix));
        }
        self.unread_from(self.pos)?;
        Ok(if neg { -n } else { n })
    }
}

/// Build a real `Value::Hash' from GNU's `#s(hash-table ...)' printed
/// literal: fields are a flat list (`test SYM data (K V ...) size N
/// weakness SYM rehash-size F rehash-threshold F purecopy B').
/// Returns None when the shape doesn't match (caller falls back to a
/// plain record).
fn read_hash_literal(i: &mut Interp, fields: &[Value]) -> Result<Option<Value>, Flow> {
    use crate::lisp::builtins::hashfn::table_key;
    use crate::lisp::value::{HashTest, LispHash};
    let test_key = i.q_soft("test");
    let data_key = i.q_soft("data");
    let size_key = i.q_soft("size");
    let weakness_key = i.q_soft("weakness");
    let rehash_size_key = i.q_soft("rehash-size");
    let rehash_threshold_key = i.q_soft("rehash-threshold");
    let purecopy_key = i.q_soft("purecopy");
    let key_weakness_key = i.q_soft("key");
    let value_weakness_key = i.q_soft("value");
    let key_and_value_key = i.q_soft("key-and-value");
    let key_or_value_key = i.q_soft("key-or-value");
    let eq_test = i.q_soft("eq");
    let eql_test = i.q_soft("eql");
    let equal_test = i.q_soft("equal");
    // GNU's printed custom tables use the first occurrence of each plist
    // field, ignore size/unknown fields, and size storage from the data.
    // Built-in literal metadata uses the existing path below.
    let property = |key: Option<SymId>| {
        let Some(key) = key else {
            return Value::Nil;
        };
        fields
            .chunks(2)
            .find(|pair| matches!(&pair[0], Value::Sym(id) if *id == key))
            .and_then(|pair| pair.get(1))
            .cloned()
            .unwrap_or(Value::Nil)
    };
    let test_arg = property(test_key);
    if let Value::Sym(id) = &test_arg
        && Some(*id) != eq_test
        && Some(*id) != eql_test
        && Some(*id) != equal_test
    {
        let data = property(data_key);
        let weakness = property(weakness_key);
        if !matches!(data, Value::Nil | Value::Cons(_)) {
            return Err(i.error("Hash table data is not a list"));
        }
        let pairs = crate::lisp::builtins::want_list(i, &data)?;
        if pairs.len() % 2 != 0 {
            return Err(i.error("Hash table data length is odd"));
        }
        let test_kw = Value::Sym(i.q(":test"));
        let size_kw = Value::Sym(i.q(":size"));
        let weakness_kw = Value::Sym(i.q(":weakness"));
        let table = crate::lisp::builtins::hashfn::f_make_hash_table(
            i,
            vec![
                test_kw,
                test_arg,
                size_kw,
                Value::Int((pairs.len() / 2) as i128),
                weakness_kw,
                weakness,
            ]
            .into(),
        )?;
        for pair in pairs.chunks_exact(2) {
            crate::lisp::builtins::hashfn::f_puthash(
                i,
                vec![pair[0].clone(), pair[1].clone(), table.clone()].into(),
            )?;
        }
        return Ok(Some(table));
    }
    let mut test = HashTest::Eql;
    let mut test_seen = false;
    let mut data: Option<Vec<Value>> = None;
    let mut weakness: Option<Value> = None;
    let mut weakness_seen = false;
    let mut k = 0;
    while k + 1 < fields.len() {
        let name_id = match &fields[k] {
            Value::Sym(name_id) => *name_id,
            _ => {
                k += 2;
                continue;
            }
        };
        let val = &fields[k + 1];
        if Some(name_id) == test_key {
            if !test_seen {
                test_seen = true;
                let Some(tid) = i.sym_id(val) else {
                    return Err(i.wrong_type_mut("symbolp", val));
                };
                test = if Some(tid) == eq_test {
                    HashTest::Eq
                } else if Some(tid) == eql_test || tid == sym::NIL {
                    HashTest::Eql
                } else if Some(tid) == equal_test {
                    HashTest::Equal
                } else {
                    return Ok(None);
                };
            }
        } else if Some(name_id) == data_key {
            if data.is_none() {
                let pairs = match val {
                    Value::Nil => Vec::new(),
                    Value::Cons(_) => crate::lisp::builtins::want_list(i, val)?,
                    _ => return Err(i.error("Hash table data is not a list")),
                };
                if pairs.len() % 2 != 0 {
                    return Err(i.error("Hash table data length is odd"));
                }
                data = Some(pairs);
            }
        } else if Some(name_id) == size_key {
            // GNU ignores the legacy hint in printed tables.
        } else if Some(name_id) == weakness_key {
            if !weakness_seen {
                weakness_seen = true;
                let id = i.sym_id(val);
                if val.is_nil() {
                    weakness = None;
                } else if id == Some(sym::T) || id == key_and_value_key {
                    weakness = Some(Value::Sym(i.q("key-and-value")));
                } else if id == key_weakness_key
                    || id == value_weakness_key
                    || id == key_or_value_key
                {
                    weakness = Some(val.clone());
                } else {
                    return Err(i.signal_data(
                        sym::ERROR,
                        vec![Value::string("Invalid hash table weakness"), val.clone()],
                    ));
                }
            }
        } else if Some(name_id) == rehash_size_key
            || Some(name_id) == rehash_threshold_key
            || Some(name_id) == purecopy_key
        {
            // GNU accepts and ignores these hints in printed tables.
        } else {
            // Unknown plist keys do not affect the table's reader syntax.
        }
        k += 2;
    }
    // GNU's hash-table reader consumes complete property/value pairs and
    // ignores an unpaired trailing field. A partial unknown field must not
    // turn an otherwise valid hash-table literal into a plain record.
    let mut h = LispHash::new(test);
    // Literal data reserves one slot per pair before equivalent keys
    // coalesce, unlike lazy default construction via make-hash-table.
    h.size = data.as_ref().map_or(0, |pairs| pairs.len() / 2) as i128;
    h.gc_weakness = crate::lisp::value::HashWeakness::from_value(i, &weakness);
    h.weakness = weakness;
    let h = crate::lisp::memory::allocate(h, crate::lisp::Value::Hash);
    if let Some(pairs) = data {
        let mut it = pairs.into_iter();
        while let (Some(ky), Some(vl)) = (it.next(), it.next()) {
            let key = table_key(i, &h, &ky)?;
            let mut table = h.borrow_mut();
            table.map.insert(key.clone(), vl);
            table.put_key(key, ky);
        }
    }
    Ok(Some(Value::Hash(h)))
}
