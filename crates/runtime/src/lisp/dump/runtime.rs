//! Persistent Rust-side metadata that is not already a symbol value cell.
//! Execution stacks, OS resources, and derived caches are deliberately reset
//! at a fresh startup; semantic tables and their references are restored.

use std::collections::BTreeMap;
use std::rc::Rc;

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value as Json;

use super::graph::{Decoder, Encoder, Ref, Result, decode_text, encode_text};
use crate::lisp::value::*;
use crate::lisp::{Interp, eval::MatchData};

/// JSON is used only as an intermediate typed record. This tagged tree
/// keeps strings in Emacs character space, including in map keys, without
/// reserving a user-visible string or confusing strings with arrays.
#[derive(Serialize, Deserialize)]
enum Data {
    Null,
    Bool(bool),
    Number(String),
    Text(Vec<u32>),
    Array(Vec<Data>),
    Object(Vec<(Vec<u32>, Data)>),
}
fn decode_native_text(codes: &[u32]) -> Result<String> {
    codes.iter().map(|code| char::from_u32(*code)
        .ok_or_else(|| "Non-Unicode character in native dump metadata".to_owned())).collect()
}

impl Data {
    fn encode(value: Json) -> Self {
        match value {
            Json::Null => Self::Null,
            Json::Bool(v) => Self::Bool(v),
            Json::Number(v) => Self::Number(v.to_string()),
            Json::String(v) => Self::Text(encode_text(&v)),
            Json::Array(v) => Self::Array(v.into_iter().map(Self::encode).collect()),
            Json::Object(v) => Self::Object(
                v.into_iter()
                    .map(|(k, v)| (encode_text(&k), Self::encode(v)))
                    .collect(),
            ),
        }
    }
    fn decode(&self) -> Result<Json> {
        Ok(match self {
            Self::Null => Json::Null,
            Self::Bool(v) => Json::Bool(*v),
            Self::Number(v) => {
                Json::Number(v.parse().map_err(|_| "Invalid runtime number in dump")?)
            }
            Self::Text(v) => Json::String(decode_native_text(v)?),
            Self::Array(v) => Json::Array(v.iter().map(Self::decode).collect::<Result<_>>()?),
            Self::Object(v) => Json::Object(
                v.iter()
                    .map(|(k, v)| Ok((decode_native_text(k)?, v.decode()?)))
                    .collect::<Result<_>>()?,
            ),
        })
    }
}

#[derive(Serialize, Deserialize)]
pub(super) struct Runtime(BTreeMap<String, Data>);

impl Runtime {
    fn put<T: Serialize>(&mut self, name: &str, value: T) -> Result<()> {
        let value = serde_json::to_value(value).map_err(|e| e.to_string())?;
        self.0.insert(name.to_owned(), Data::encode(value));
        Ok(())
    }
    fn get<T: DeserializeOwned>(&self, name: &str) -> Result<T> {
        let value = self
            .0
            .get(name)
            .ok_or_else(|| format!("Missing runtime dump field {name}"))?
            .decode()?;
        serde_json::from_value(value).map_err(|e| format!("Invalid runtime dump field {name}: {e}"))
    }
    pub fn capture(i: &Interp, e: &mut Encoder<'_>) -> Result<Self> {
        let mut saved = Self(BTreeMap::new());
        saved.put("fixed_symbols", i.fixed_symbols.iter()
            .map(|(name, symbol)| Ok((name.clone(), e.symbol(*symbol)?)))
            .collect::<Result<Vec<_>>>()?)?;
        saved.put("native_lisp_literals", i.native_lisp_literals.iter()
            .map(|(key, value)| Ok((key.clone(), e.value(value)?)))
            .collect::<Result<Vec<_>>>()?)?;
        macro_rules! scalar { ($($field:ident),* $(,)?) => { $(saved.put(stringify!($field), &i.$field)?;)* }; }
        scalar!(
            max_lisp_eval_depth,
            native_trampolines,
            next_frame_name,
            tz_rule,
            charset_aliases,
            iso_charset_overrides,
            extra_coding_systems,
            extra_coding_aliases,
            coding_detection_members,
            kbd_macro_mark,
            mtwlb_phase,
            ccl_program_count,
            code_conv_map_count,
            color_db_init
        );
        saved.put("charset_order", &*i.charset_order.borrow())?;
        let maps = i.charset_maps.borrow();
        saved.put(
            "charset_maps",
            maps.iter()
                .map(|(k, v)| (k, v.as_ref()))
                .collect::<Vec<_>>(),
        )?;
        macro_rules! value { ($($field:ident),* $(,)?) => { $(saved.put(stringify!($field), e.value(&i.$field)?)?;)* }; }
        value!(
            read_expression_history,
            terminal_coding,
            keyboard_coding,
            thread_last_error,
            zero_vector
        );
        macro_rules! symbols { ($($field:ident),* $(,)?) => { $(saved.put(stringify!($field), e.symbol(i.$field)?)?;)* }; }
        symbols!(
            standard_output_sym,
            undo_inhibit_sym,
            bfn_sym,
            var_alias_prop
        );
        saved.put(
            "features",
            i.features
                .iter()
                .map(|s| e.symbol(*s))
                .collect::<Result<Vec<_>>>()?,
        )?;
        saved.put(
            "file_declared",
            i.file_declared
                .borrow()
                .iter()
                .map(|s| e.symbol(*s))
                .collect::<Result<Vec<_>>>()?,
        )?;
        saved.put("kbd_macro_events", e.values(&i.kbd_macro_events)?)?;
        macro_rules! optional { ($($field:ident),* $(,)?) => { $(
            saved.put(stringify!($field), i.$field.as_ref().map(|v| e.value(v)).transpose()?)?;
        )* }; }
        optional!(standard_case_table, standard_category_table, terminal);
        saved.put(
            "default_obarray",
            i.default_obarray
                .as_ref()
                .map(|v| e.value(&Value::Record(v.clone())))
                .transpose()?,
        )?;
        saved.put(
            "special_records",
            i.special_records
                .iter()
                .filter_map(|(weak, kind)| weak.upgrade().map(|record| (record, kind)))
                .map(|(record, kind)| {
                    Ok((
                        e.value(&Value::Record(record))?,
                        match kind {
                            crate::lisp::eval::SpecialRecordKind::Obarray => 0u8,
                            crate::lisp::eval::SpecialRecordKind::FontSpec => 1u8,
                            crate::lisp::eval::SpecialRecordKind::SymbolWithPos => 2u8,
                        },
                    ))
                })
                .collect::<Result<Vec<_>>>()?,
        )?;
        macro_rules! named { ($($field:ident),* $(,)?) => { $(
            saved.put(stringify!($field), i.$field.iter().map(|(name, value)| Ok((name.clone(), e.value(value)?))).collect::<Result<Vec<_>>>()?)?;
        )* }; }
        named!(face_table, face_t_table, charsets, char_code_prop_tables);
        saved.put(
            "char_code_props",
            i.char_code_props
                .iter()
                .map(|(name, values)| {
                    Ok((
                        name.clone(),
                        values
                            .iter()
                            .map(|(ch, value)| Ok((*ch, e.value(value)?)))
                            .collect::<Result<Vec<_>>>()?,
                    ))
                })
                .collect::<Result<Vec<_>>>()?,
        )?;
        saved.put(
            "var_watchers",
            i.var_watchers
                .iter()
                .map(|(s, v)| Ok((e.symbol(*s)?, e.value(v)?)))
                .collect::<Result<Vec<_>>>()?,
        )?;
        saved.put(
            "advice_bases",
            i.advice_bases
                .iter()
                .map(|(s, v)| Ok((e.symbol(*s)?, e.value(v)?)))
                .collect::<Result<Vec<_>>>()?,
        )?;
        saved.put(
            "advices",
            i.advices
                .iter()
                .map(|(s, entries)| {
                    Ok((
                        e.symbol(*s)?,
                        entries
                            .iter()
                            .map(|(where_, fun, name)| {
                                Ok((e.symbol(*where_)?, e.value(fun)?, e.value(name)?))
                            })
                            .collect::<Result<Vec<_>>>()?,
                    ))
                })
                .collect::<Result<Vec<_>>>()?,
        )?;
        saved.put(
            "advice_links",
            i.advice_links
                .iter()
                .map(|(a, b, c, d)| Ok((e.value(a)?, e.value(b)?, e.value(c)?, e.value(d)?)))
                .collect::<Result<Vec<_>>>()?,
        )?;
        saved.put(
            "byte_op_subrs",
            i.byte_op_subrs
                .as_ref()
                .map(|map| {
                    map.iter()
                        .map(|(s, subr)| Ok((e.symbol(*s)?, e.value(&Value::Subr(subr))?)))
                        .collect::<Result<Vec<_>>>()
                })
                .transpose()?,
        )?;
        saved.put(
            "terminal_params",
            i.terminal_params
                .iter()
                .map(|(k, v)| Ok((e.value(k)?, e.value(v)?)))
                .collect::<Result<Vec<_>>>()?,
        )?;
        saved.put(
            "match_data",
            i.match_data
                .as_ref()
                .map(|m| -> Result<_> {
                    Ok((
                        m.regs.clone(),
                        m.in_buffer,
                        m.base,
                        m.buffer
                            .as_ref()
                            .map(|v| e.value(&Value::Buffer(v.clone())))
                            .transpose()?,
                    ))
                })
                .transpose()?,
        )?;
        Ok(saved)
    }

    /// Weak side tables do not introduce roots for otherwise dead objects.
    /// Parent/default relations may in turn make other char tables reachable.
    pub fn capture_weak(&mut self, i: &Interp, e: &mut Encoder<'_>) -> Result<()> {
        loop {
            let before = e.node_count();
            e.drain()?;
            for entries in [&i.char_table_parents, &i.char_table_defalts] {
                for (_, weak, parent) in entries {
                    if let Some(table) = weak.upgrade() {
                        if e.knows(&Value::Record(table)) {
                            e.value(parent)?;
                        }
                    }
                }
            }
            if e.node_count() == before {
                break;
            }
        }
        macro_rules! tables {
            ($field:ident) => {{
                let mut rows = Vec::new();
                for (_, weak, parent) in &i.$field {
                    if let Some(table) = weak.upgrade() {
                        let table = Value::Record(table);
                        if e.knows(&table) {
                            rows.push((e.value(&table)?, e.value(parent)?));
                        }
                    }
                }
                self.put(stringify!($field), rows)?;
            }};
        }
        tables!(char_table_parents);
        tables!(char_table_defalts);
        let mut names = Vec::new();
        for (weak, name) in i.compiled_fn_names.borrow().iter() {
            if let Some(function) = weak.upgrade() {
                let function = Value::Lambda(function);
                if e.knows(&function) {
                    names.push((e.value(&function)?, encode_text(name)));
                }
            }
        }
        self.put("compiled_fn_names", names)?;
        Ok(())
    }

    pub fn restore(&self, i: &mut Interp, d: &mut Decoder<'_>) -> Result<()> {
        if self.0.contains_key("fixed_symbols") {
            let symbols: Vec<(String, Ref)> = self.get("fixed_symbols")?;
            i.fixed_symbols = symbols.into_iter()
                .map(|(name, reference)| Ok((name, d.symbol(&reference)?)))
                .collect::<Result<_>>()?;
        }
        i.native_lisp_literals.clear();
        if self.0.contains_key("native_lisp_literals") {
            let literals: Vec<(String, Ref)> = self.get("native_lisp_literals")?;
            for (key, reference) in literals {
                let value = d.value(i, &reference)?;
                i.native_lisp_literals.insert(key, value);
            }
        }
        macro_rules! scalar { ($($field:ident),* $(,)?) => { $(i.$field = self.get(stringify!($field))?;)* }; }
        scalar!(
            max_lisp_eval_depth,
            native_trampolines,
            next_frame_name,
            tz_rule,
            charset_aliases,
            iso_charset_overrides,
            extra_coding_systems,
            extra_coding_aliases,
            coding_detection_members,
            kbd_macro_mark,
            mtwlb_phase,
            ccl_program_count,
            code_conv_map_count,
            color_db_init
        );
        *i.charset_order.borrow_mut() = self.get("charset_order")?;
        i.char_charset_cache.borrow_mut().clear();
        let maps: Vec<(String, crate::lisp::builtins::charset::CharsetTable)> =
            self.get("charset_maps")?;
        *i.charset_maps.borrow_mut() = maps.into_iter().map(|(k, v)| (k, Rc::new(v))).collect();
        macro_rules! value { ($($field:ident),* $(,)?) => { $(
            let reference: Ref = self.get(stringify!($field))?; i.$field = d.value(i,&reference)?;
        )* }; }
        value!(
            read_expression_history,
            terminal_coding,
            keyboard_coding,
            thread_last_error,
            zero_vector
        );
        macro_rules! symbols { ($($field:ident),* $(,)?) => { $(i.$field = d.symbol(&self.get(stringify!($field))?)?;)* }; }
        symbols!(
            standard_output_sym,
            undo_inhibit_sym,
            bfn_sym,
            var_alias_prop
        );
        i.features = self
            .get::<Vec<Ref>>("features")?
            .iter()
            .map(|r| d.symbol(r))
            .collect::<Result<_>>()?;
        *i.file_declared.borrow_mut() = self
            .get::<Vec<Ref>>("file_declared")?
            .iter()
            .map(|r| d.symbol(r))
            .collect::<Result<_>>()?;
        i.kbd_macro_events = d.values(i, &self.get::<Vec<Ref>>("kbd_macro_events")?)?;
        macro_rules! optional { ($($field:ident),* $(,)?) => { $(
            i.$field = self.get::<Option<Ref>>(stringify!($field))?.as_ref().map(|r| d.value(i,r)).transpose()?;
        )* }; }
        optional!(standard_case_table, standard_category_table, terminal);
        i.default_obarray = self
            .get::<Option<Ref>>("default_obarray")?
            .as_ref()
            .map(|r| match d.value(i, r)? {
                Value::Record(v) | Value::Vec(v) => Ok(v),
                _ => Err("Invalid dumped default obarray".to_owned()),
            })
            .transpose()?;
        i.special_records.clear();
        if self.0.contains_key("special_records") {
            let records: Vec<(Ref, u8)> = self.get("special_records")?;
            for (reference, kind) in records {
                let Value::Record(record) = d.value(i, &reference)? else {
                    return Err("Invalid dumped special record".to_owned());
                };
                let kind = match kind {
                    0 => crate::lisp::eval::SpecialRecordKind::Obarray,
                    1 => crate::lisp::eval::SpecialRecordKind::FontSpec,
                    2 => crate::lisp::eval::SpecialRecordKind::SymbolWithPos,
                    _ => return Err("Invalid dumped special record kind".to_owned()),
                };
                i.special_records.push((Rc::downgrade(&record), kind));
            }
        }
        macro_rules! named { ($($field:ident),* $(,)?) => { $(
            i.$field = self.get::<Vec<(String,Ref)>>(stringify!($field))?.into_iter().map(|(name,r)| Ok((name,d.value(i,&r)?))).collect::<Result<_>>()?;
        )* }; }
        named!(face_table, face_t_table, charsets, char_code_prop_tables);
        i.char_code_props = self
            .get::<Vec<(String, Vec<(i64, Ref)>)>>("char_code_props")?
            .into_iter()
            .map(|(name, values)| {
                Ok((
                    name,
                    values
                        .into_iter()
                        .map(|(ch, r)| Ok((ch, d.value(i, &r)?)))
                        .collect::<Result<_>>()?,
                ))
            })
            .collect::<Result<_>>()?;
        i.var_watchers = self
            .get::<Vec<(Ref, Ref)>>("var_watchers")?
            .into_iter()
            .map(|(s, v)| Ok((d.symbol(&s)?, d.value(i, &v)?)))
            .collect::<Result<_>>()?;
        i.advice_bases = self
            .get::<Vec<(Ref, Ref)>>("advice_bases")?
            .into_iter()
            .map(|(s, v)| Ok((d.symbol(&s)?, d.value(i, &v)?)))
            .collect::<Result<_>>()?;
        i.advices = self
            .get::<Vec<(Ref, Vec<(Ref, Ref, Ref)>)>>("advices")?
            .into_iter()
            .map(|(s, entries)| {
                Ok((
                    d.symbol(&s)?,
                    entries
                        .into_iter()
                        .map(|(where_, fun, name)| {
                            Ok((d.symbol(&where_)?, d.value(i, &fun)?, d.value(i, &name)?))
                        })
                        .collect::<Result<_>>()?,
                ))
            })
            .collect::<Result<_>>()?;
        i.advice_links = self
            .get::<Vec<(Ref, Ref, Ref, Ref)>>("advice_links")?
            .into_iter()
            .map(|(a, b, c, e)| {
                Ok((
                    d.value(i, &a)?,
                    d.value(i, &b)?,
                    d.value(i, &c)?,
                    d.value(i, &e)?,
                ))
            })
            .collect::<Result<_>>()?;
        i.byte_op_subrs = self
            .get::<Option<Vec<(Ref, Ref)>>>("byte_op_subrs")?
            .map(|entries| {
                entries
                    .into_iter()
                    .map(|(s, r)| match d.value(i, &r)? {
                        Value::Subr(subr) => Ok((d.symbol(&s)?, subr)),
                        _ => Err("Invalid dumped byte-op primitive".to_owned()),
                    })
                    .collect::<Result<_>>()
            })
            .transpose()?;
        i.byte_op_subrs_by_name = None;
        i.terminal_params = self
            .get::<Vec<(Ref, Ref)>>("terminal_params")?
            .into_iter()
            .map(|(k, v)| Ok((d.value(i, &k)?, d.value(i, &v)?)))
            .collect::<Result<_>>()?;
        // Asynchronous callbacks are execution state, never resumed by an image.
        i.running_asynch_code = false;
        i.saved_async_match_data = None;
        i.signaling_function = Value::Nil;
        i.match_data = self
            .get::<Option<(Vec<Option<usize>>, bool, usize, Option<Ref>)>>("match_data")?
            .map(|(regs, in_buffer, base, buffer)| -> Result<MatchData> {
                Ok(MatchData {
                    regs,
                    in_buffer,
                    base,
                    buffer: buffer
                        .map(|r| match d.value(i, &r)? {
                            Value::Buffer(b) => Ok(b),
                            _ => Err("Invalid dumped match buffer".to_owned()),
                        })
                        .transpose()?,
                })
            })
            .transpose()?;
        macro_rules! tables {
            ($field:ident) => {
                i.$field = self
                    .get::<Vec<(Ref, Ref)>>(stringify!($field))?
                    .into_iter()
                    .map(|(table, parent)| {
                        let Value::Record(table) = d.value(i, &table)? else {
                            return Err("Invalid dumped char-table metadata".into());
                        };
                        Ok((
                            Rc::as_ptr(&table) as usize,
                            Rc::downgrade(&table),
                            d.value(i, &parent)?,
                        ))
                    })
                    .collect::<Result<_>>()?;
            };
        }
        tables!(char_table_parents);
        tables!(char_table_defalts);
        *i.compiled_fn_names.borrow_mut() = self
            .get::<Vec<(Ref, Vec<u32>)>>("compiled_fn_names")?
            .into_iter()
            .map(|(function, name)| {
                let Value::Lambda(function) = d.value(i, &function)? else {
                    return Err("Invalid dumped function name".into());
                };
                Ok((Rc::downgrade(&function), decode_text(&name)))
            })
            .collect::<Result<_>>()?;
        Ok(())
    }
}

#[cfg(test)]
mod wide_metadata_tests {
    use super::*;
    #[test]
    fn native_private_use_strings_and_map_keys_roundtrip_without_aliases() {
        let original = serde_json::json!({"\u{f0080}": ["\u{f0100}", "\u{f0b00}"]});
        let data = Data::encode(original.clone());
        let wire = serde_json::to_vec(&data).unwrap();
        let restored: Data = serde_json::from_slice(&wire).unwrap();
        assert_eq!(restored.decode().unwrap(), original);
    }
    #[test]
    fn wide_function_name_rows_use_code_arrays_instead_of_json_strings() {
        let codes = vec![0xf0080u32, 0x3fff80, 0xd800, 0x110000, 0x200000];
        let original = vec![(7u32, codes.clone())];
        let data = Data::encode(serde_json::to_value(&original).unwrap());
        let restored: Vec<(u32, Vec<u32>)> = serde_json::from_value(data.decode().unwrap()).unwrap();
        assert_eq!(restored, original);
        assert_eq!(encode_text(&decode_text(&codes)), codes);
    }
    #[test]
    fn native_metadata_rejects_non_unicode_instead_of_inventing_proxies() {
        for code in [0xd800, 0x110000, 0x3fff80, 0x400000] {
            assert!(Data::Text(vec![code]).decode().is_err());
            assert!(Data::Object(vec![(vec![code], Data::Null)]).decode().is_err());
        }
    }
}
