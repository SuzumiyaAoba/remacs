//! Coding category detection and priority assembly (GNU coding.c).
//!
//! The input alphabet distinguishes raw bytes from genuine multibyte
//! characters (negative code points), just as ONE_MORE_BYTE does in GNU. Rust's UTF-8 string
//! storage is not an external encoding and must never be detected as one.
use super::{charset, coding_detect, misc};
use crate::lisp::Interp;
use crate::lisp::error::EvalResult;
use crate::lisp::value::Value;

const ISO7: u32 = 1;
const ISO7_TIGHT: u32 = 1 << 1;
const ISO7_ELSE: u32 = 1 << 2;
const ISO8_1: u32 = 1 << 3;
const ISO8_2: u32 = 1 << 4;
const ISO8_ELSE: u32 = 1 << 5;
const ISO7BIT: u32 = ISO7 | ISO7_TIGHT;
const ISO8BIT: u32 = ISO8_1 | ISO8_2;
const ISO: u32 = ISO7BIT | ISO7_ELSE | ISO8BIT | ISO8_ELSE;
const ISO_ESCAPE: u32 = ISO7BIT | ISO7_ELSE | ISO8_ELSE;
const CHARSET: u32 = 1 << 14;
const CCL: u32 = 1 << 17;
const RAW: u32 = 1 << 19;
const UNDECIDED: u32 = 1 << 20;
const ANY: u32 = RAW - 1;

fn category(name: &str) -> u32 {
    let names = [
        "iso-7",
        "iso-7-tight",
        "iso-7-else",
        "iso-8-1",
        "iso-8-2",
        "iso-8-else",
        "utf-8-auto",
        "utf-8",
        "utf-8-sig",
        "utf-16-auto",
        "utf-16-be",
        "utf-16-le",
        "utf-16-be-nosig",
        "utf-16-le-nosig",
        "charset",
        "sjis",
        "big5",
        "ccl",
        "emacs-mule",
        "raw-text",
        "undecided",
    ];
    name.strip_prefix("coding-category-")
        .and_then(|n| names.iter().position(|v| *v == n))
        .map(|n| 1 << n)
        .unwrap_or(0)
}
fn prop(i: &mut Interp, plist: &Value, name: &str) -> Value {
    let key = i.intern(name);
    crate::lisp::eval::plist_get(plist, key)
}
fn symbol_names(i: &Interp, list: &Value) -> Vec<String> {
    list.list_to_vec()
        .unwrap_or_default()
        .iter()
        .filter_map(|v| match v {
            Value::Sym(s) => Some(i.symbol_name(*s).to_string()),
            _ => None,
        })
        .collect()
}
struct Candidate {
    bit: u32,
    name: String,
    eol_base: String,
    fixed_eol: bool,
    flags: Vec<String>,
    charsets: Vec<String>,
    plist: Value,
}
impl Candidate {
    fn flag(&self, name: &str) -> bool {
        self.flags.iter().any(|f| f == name)
    }
}
fn variable(i: &mut Interp, name: &str) -> Value {
    let s = i.intern(name);
    i.symbol_value(s)
}
fn latin_extra(i: &mut Interp) -> Vec<bool> {
    match variable(i, "latin-extra-code-table") {
        Value::Vec(v) => v.borrow().iter().map(Value::truthy).collect(),
        _ => vec![false; 256],
    }
}

/// ISO-2022's evidence and rejection masks. A designation is tested against
/// each category's current safe charset set, including runtime overrides.
fn detect_iso(
    i: &mut Interp,
    input: &[i32],
    candidates: &[Candidate],
    extra: &[bool],
    inhibit: bool,
) -> (u32, u32) {
    let mut found = 0;
    let mut rejected = 0;
    let mut single_shift = false;
    let mut composition: Option<usize> = None;
    let mut pos = 0;
    macro_rules! next {
        () => {{
            if pos == input.len() {
                return (found & !rejected, rejected);
            }
            let c = input[pos];
            pos += 1;
            c
        }};
    }
    while pos < input.len() && rejected != ISO {
        let c = next!();
        match c {
            27 if !inhibit => {
                single_shift = false;
                let c = next!();
                if c == 78 || c == 79 {
                    single_shift = true;
                    rejected |= ISO7BIT | ISO8BIT;
                    continue;
                }
                if c == 49 {
                    if composition.is_some_and(|n| n <= 16) {
                        composition = None;
                        found |= ISO;
                    }
                    continue;
                }
                if (48..=52).contains(&c) {
                    composition = Some(0);
                    continue;
                }
                let (dim, chars96, final_char) = if (40..=47).contains(&c) {
                    (1, c >= 44, next!())
                } else if c == 36 {
                    let c = next!();
                    if (64..=66).contains(&c) {
                        (2, false, c)
                    } else if (40..=47).contains(&c) {
                        (2, c >= 44, next!())
                    } else {
                        if c >= 128 {
                            rejected |= ISO7BIT | ISO7_ELSE;
                        }
                        continue;
                    }
                } else {
                    if c >= 128 {
                        rejected |= ISO7BIT | ISO7_ELSE;
                    }
                    continue;
                };
                let charset = if (32..128).contains(&final_char) {
                    charset::iso_charset_lookup(i, dim, chars96, final_char as i64)
                } else {
                    None
                };
                let Some(charset) = charset else {
                    if final_char >= 128 {
                        rejected |= ISO7BIT | ISO7_ELSE;
                    }
                    continue;
                };
                rejected |= ISO8BIT;
                for candidate in candidates.iter().filter(|c| c.bit & ISO_ESCAPE != 0) {
                    if candidate.charsets.contains(&charset) {
                        found |= candidate.bit;
                    } else {
                        rejected |= candidate.bit;
                    }
                }
            }
            14 | 15 if !inhibit => {
                single_shift = false;
                rejected |= ISO7BIT | ISO8BIT;
            }
            142 | 143 if inhibit => continue,
            _ => {
                if c < 0 {
                    continue;
                }
                if c == 155 {
                    single_shift = false;
                    rejected |= ISO7BIT | ISO7_ELSE;
                    found |= ISO8_ELSE;
                } else if (c == 142 || c == 143) && !inhibit {
                    single_shift = false;
                    rejected |= ISO7BIT | ISO7_ELSE;
                    for candidate in candidates.iter().filter(|c| c.bit & ISO8BIT != 0) {
                        if candidate.flag("single-shift") {
                            found |= candidate.bit;
                            single_shift = true;
                        }
                    }
                    if single_shift {
                        continue;
                    }
                } else if c < 128 {
                    if let Some(n) = &mut composition {
                        *n += 1;
                    }
                    single_shift = false;
                    continue;
                } else {
                    rejected |= ISO7BIT | ISO7_ELSE;
                    if c >= 160 {
                        found |= ISO8_1;
                        if !single_shift && rejected & ISO8_2 == 0 {
                            let mut len = 1;
                            while pos < input.len() && input[pos] >= 160 {
                                len += 1;
                                pos += 1;
                            }
                            if len % 2 != 0 && pos < input.len() {
                                rejected |= ISO8_2;
                                if let Some(n) = &mut composition {
                                    *n += len;
                                }
                            } else {
                                found |= ISO8_2;
                                if let Some(n) = &mut composition {
                                    *n += len / 2;
                                }
                            }
                        }
                        continue;
                    }
                }
                {
                    if !extra.get(c as usize).copied().unwrap_or(false) {
                        rejected = ISO;
                        break;
                    }
                    if candidates
                        .iter()
                        .any(|c| c.bit == ISO8_1 && c.flag("latin-extra"))
                    {
                        found |= ISO8_1;
                    } else {
                        rejected |= ISO8_1;
                    }
                    rejected |= ISO8_2;
                }
            }
        }
    }
    (found & !rejected, rejected)
}

fn detect_charset(
    i: &mut Interp,
    input: &[i32],
    candidate: &Candidate,
    extra: &[bool],
    head_ascii: usize,
) -> (u32, u32) {
    let mut dimensions = Vec::new();
    for name in &candidate.charsets {
        let plist = charset::effective_charset_plist(i, name);
        if let Some(mut dims) = charset::charset_dims(i, &plist) {
            let dim = charset::charset_dimension_of(i, name);
            if dims.len() > dim {
                dims.drain(..dims.len() - dim);
            }
            dimensions.push(dims);
        }
    }
    // Shorter encodings precede longer alternatives sharing a lead byte.
    // Keep the declared order for equal dimensions.
    dimensions.sort_by_key(Vec::len);
    let check_extra =
        candidate.name.starts_with("iso-latin-") || candidate.name.starts_with("iso-8859-");
    let mut found = 0;
    let mut pos = if prop(i, &candidate.plist, ":ascii-compatible-p").truthy() {
        head_ascii
    } else {
        0
    };
    while pos < input.len() {
        let c = input[pos];
        if c < 0 {
            pos += 1;
            continue;
        }
        if (128..160).contains(&c)
            && check_extra
            && !extra.get(c as usize).copied().unwrap_or(false)
        {
            return (0, CHARSET);
        }
        // Match GNU's ordered first-byte validity table. Alternatives share
        // the consumption cursor: failed continuation bytes are not rewound.
        let choices: Vec<_> = dimensions
            .iter()
            .filter(|dims| {
                dims.first()
                    .is_some_and(|(lo, hi)| (*lo..=*hi).contains(&(c as i64)))
            })
            .collect();
        if choices.is_empty() {
            return (0, CHARSET);
        }
        if c >= 128 {
            found = CHARSET;
        }
        pos += 1;
        let mut idx = 1;
        for dims in &choices {
            while idx < dims.len() {
                let Some(&byte) = input.get(pos) else {
                    return (0, CHARSET);
                };
                pos += 1;
                let (lo, hi) = dims[idx];
                if !(lo..=hi).contains(&(byte as i64)) {
                    break;
                }
                idx += 1;
            }
            if idx == dims.len() {
                break;
            }
            if choices.len() == 1 {
                return (0, CHARSET);
            }
        }
    }
    (found, 0)
}

fn eol(input: &[i32], bit: u32, multibyte: bool) -> Option<&'static str> {
    let mut chars = Vec::new();
    if bit & ((1 << 10) | (1 << 11) | (1 << 12) | (1 << 13)) != 0 {
        // GNU 31.1 detect_eol compares the category with the bitwise OR
        // of its LE enum values, so its LE detection also scans BE units.
        let mut bytes = Vec::new();
        for &c in input {
            if c < 0 {
                let code = (-c) as u32;
                let (len, lead) = if code < 0x800 {
                    (2, 0xc0)
                } else if code < 0x10000 {
                    (3, 0xe0)
                } else if code < 0x200000 {
                    (4, 0xf0)
                } else {
                    (5, 0xf8)
                };
                bytes.push(lead | (code >> (6 * (len - 1))) as u8);
                for shift in (0..len - 1).rev() {
                    bytes.push(0x80 | ((code >> (6 * shift)) & 0x3f) as u8);
                }
            } else if multibyte && c >= 128 {
                bytes.push(0xc0 | ((c >> 6) & 1) as u8);
                bytes.push(0x80 | (c & 0x3f) as u8);
            } else {
                bytes.push(c as u8);
            }
        }
        chars.extend(
            bytes
                .chunks_exact(2)
                .map(|c| ((c[0] as i32) << 8) | c[1] as i32),
        );
    } else {
        chars.extend_from_slice(input);
    }
    let mut seen = 0;
    let mut total = 0;
    let mut pos = 0;
    while pos < chars.len() {
        let this = if chars[pos] == 13 {
            if chars.get(pos + 1) == Some(&10) {
                pos += 1;
                2
            } else {
                4
            }
        } else if chars[pos] == 10 {
            1
        } else {
            0
        };
        pos += 1;
        if this == 0 {
            continue;
        }
        if seen == 0 {
            seen = this;
        } else if seen != this {
            if seen | this == 6 {
                seen = 2;
            } else {
                seen = 1;
                break;
            }
        }
        total += 1;
        if total == 3 {
            break;
        }
    }
    match seen {
        1 => Some("unix"),
        2 => Some("dos"),
        4 => Some("mac"),
        _ => None,
    }
}

pub(crate) fn detect(i: &mut Interp, input: &[i32], multibyte: bool, highest: bool) -> EvalResult {
    let priorities = variable(i, "remacs-coding-system-priorities")
        .list_to_vec()
        .unwrap_or_default();
    let mut candidates = Vec::new();
    for value in priorities {
        let Some((cat, name)) = charset::coding_category_member(i, &value) else {
            continue;
        };
        let eol_base = name.clone();
        let name = i
            .coding_detection_members
            .get(&cat)
            .cloned()
            .unwrap_or(name);
        let bit = category(&cat);
        if bit == 0 {
            continue;
        }
        let plist = misc::f_coding_system_plist(i, vec![value.clone()].into())?;
        let selected = Value::Sym(i.intern(&name));
        let resolved = misc::coding_alias_group(i, &selected)
            .and_then(|group| group.first().cloned())
            .unwrap_or_else(|| name.clone());
        let fixed_eol = !prop(i, &plist, ":eol-type").is_nil()
            || ["-unix", "-dos", "-mac"]
                .iter()
                .any(|suffix| resolved.ends_with(suffix));
        let flags = prop(i, &plist, ":flags");
        let charsets = charset::coding_system_charset_names(i, &value);
        candidates.push(Candidate {
            bit,
            name,
            eol_base,
            fixed_eol,
            flags: symbol_names(i, &flags),
            charsets: symbol_names(i, &charsets)
                .iter()
                .map(|name| charset::canonical_charset(i, name))
                .collect(),
            plist,
        });
    }
    let undecided = candidates
        .iter()
        .find(|c| c.bit == UNDECIDED)
        .map(|c| c.name.clone())
        .unwrap_or_else(|| "undecided".into());
    // GNU scans the first coding_category_raw_text priority slots. Raw
    // and undecided entries can themselves occupy one of those slots.
    let active = &candidates[..candidates.len().min(19)];
    let inhibit_iso = variable(i, "inhibit-iso-escape-detection").truthy();
    let inhibit_null = variable(i, "inhibit-null-byte-detection").truthy();
    let extra = latin_extra(i);
    let high = input.iter().any(|c| *c < 0 || *c >= 128);
    let trigger = !inhibit_iso && input.iter().any(|c| matches!(c, 14 | 15 | 27));
    let (mut found, mut rejected) = (0, 0);
    let mut null = false;
    let mut iso_only = false;
    let mut early_iso_success = false;
    // GNU probes ISO controls before scanning high bytes. A valid ISO scan
    // restricts fallback to escape-based categories, even for an incomplete
    // designation. High bytes still allow the other detectors to run.
    if trigger {
        let (f, r) = detect_iso(i, input, &candidates, &extra, inhibit_iso);
        found |= f;
        rejected |= r;
        if r != ISO {
            early_iso_success = true;
            iso_only = r & ISO7_ELSE == 0;
            rejected |= !ISO_ESCAPE;
            let first = input
                .iter()
                .position(|c| matches!(c, 14 | 15 | 27))
                .unwrap();
            null = !inhibit_null && input[..first].contains(&0);
        }
    }
    if !early_iso_success {
        null = !inhibit_null && input.contains(&0);
    }
    let mut names: Vec<(String, u32)> = Vec::new();
    if null {
        names.push(("no-conversion".into(), RAW));
    } else if !high && !trigger {
        names.push((undecided.clone(), UNDECIDED));
    } else {
        if !iso_only {
            let mut mule_lengths = coding_detect::initial_mule_lengths();
            let charset_names = variable(i, "charset-list");
            for name in symbol_names(i, &charset_names) {
                let plist = charset::effective_charset_plist(i, &name);
                if let Some(id) = prop(i, &plist, ":emacs-mule-id").int() {
                    if (0..256).contains(&id) {
                        let dim = charset::charset_dimension_of(i, &name);
                        mule_lengths[id as usize] = (dim + if id < 160 { 1 } else { 2 }) as u8;
                    }
                }
            }
            // GNU's SJIS detector reads the original undecided coding's
            // charset list, not the currently selected SJIS representative.
            let head_ascii = input
                .iter()
                .position(|c| {
                    !(0..128).contains(c) || (early_iso_success && matches!(c, 14 | 15 | 27))
                })
                .unwrap_or(input.len());
            let (f, r) = coding_detect::detect_non_iso_with_tables(
                input,
                &mule_lengths,
                0xef,
                multibyte,
                head_ascii,
            );
            let groups = [
                coding_detect::UTF8_MASK,
                coding_detect::UTF16_MASK,
                coding_detect::SJIS,
                coding_detect::BIG5,
                coding_detect::EMACS_MULE,
            ];
            let checked = groups
                .iter()
                .filter(|mask| active.iter().any(|c| c.bit & **mask != 0))
                .fold(0, |bits, mask| bits | mask);
            found |= f & checked;
            rejected |= r & checked;
            let (f, r) = detect_iso(i, input, &candidates, &extra, inhibit_iso);
            if active.iter().any(|c| c.bit & ISO != 0) {
                found |= f;
                rejected |= r;
            }
            for candidate in active.iter().filter(|c| c.bit == CHARSET) {
                let (f, r) = detect_charset(i, input, candidate, &extra, head_ascii);
                found |= f;
                rejected |= r;
            }
            for candidate in active.iter().filter(|c| c.bit == CCL) {
                let valids = prop(i, &candidate.plist, ":valids")
                    .list_to_vec()
                    .unwrap_or_default();
                let mut allowed = [false; 256];
                for range in valids {
                    let bounds = match range {
                        Value::Int(n) => Some((n, n)),
                        Value::Cons(c) => {
                            let c = c.borrow();
                            c.car.int().zip(c.cdr.int())
                        }
                        _ => None,
                    };
                    if let Some((lo, hi)) = bounds {
                        for c in lo.max(0)..=hi.min(255) {
                            allowed[c as usize] = true;
                        }
                    }
                }
                let start = if prop(i, &candidate.plist, ":ascii-compatible-p").truthy() {
                    head_ascii
                } else {
                    0
                };
                if input[start..]
                    .iter()
                    .any(|c| *c < 0 || !allowed[*c as usize])
                {
                    rejected |= CCL;
                }
            }
        }
        // Early ISO success rejects non-ISO fallback categories without
        // marking their detectors checked. High bytes still cause those
        // detectors to run, and their positive evidence must survive that
        // fallback rejection mask (GNU keeps found and rejected separate).
        if rejected & ANY == ANY {
            names.push(("no-conversion".into(), RAW));
        } else if found == 0 && rejected == 0 {
            names.push((undecided.clone(), UNDECIDED));
        } else {
            for c in active {
                if c.bit & found != 0 {
                    names.push((c.name.clone(), c.bit));
                }
            }
            if let Some(c) = active.iter().find(|c| (found | rejected) & c.bit == 0) {
                names.push((c.name.clone(), c.bit));
            }
        }
    }
    // GNU's HIGHEST fast path retains the selected auto UTF-16 coding's
    // identity. Its mask does not select either normal or endian EOL
    // detection, so that auto identity has no suffix, regardless of which
    // UTF-16 category performed detection first.
    let highest_utf16_auto = highest
        && names
            .first()
            .is_some_and(|(_, bit)| *bit == coding_detect::UTF16_AUTO);
    let symbols: Vec<Value> = names
        .iter()
        .map(|(name, bit)| {
            let candidate = candidates.iter().find(|c| c.name == *name);
            let fixed_eol = candidate.is_some_and(|c| c.fixed_eol)
                || (highest_utf16_auto && *bit == coding_detect::UTF16_AUTO);
            let name = if name != "no-conversion" && !fixed_eol {
                match eol(input, *bit, multibyte) {
                    Some(suffix) => format!(
                        "{}-{suffix}",
                        candidate.map(|c| c.eol_base.as_str()).unwrap_or(name)
                    ),
                    None => name.clone(),
                }
            } else {
                name.clone()
            };
            Value::Sym(i.intern(&name))
        })
        .collect();
    Ok(if highest {
        symbols.into_iter().next().unwrap_or(Value::Nil)
    } else {
        Value::list(symbols)
    })
}
