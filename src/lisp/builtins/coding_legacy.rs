//! Metadata-driven external charset codecs. Charset maps are shared with
//! encode-char/decode-char, so runtime-defined and composite charsets work too.
use super::{charset, misc};
use crate::lisp::{
    Interp,
    value::{Value, lisp_char, lisp_char_code, mule_seq_char},
};

type Spans = Vec<(usize, usize, String)>;
type CharsetSpec = (String, usize, Vec<(i64, i64)>);
fn prop(i: &mut Interp, p: &Value, key: &str) -> Value {
    let k = i.intern(key);
    crate::lisp::eval::plist_get(p, k)
}
fn attrs(i: &mut Interp, coding: &Value) -> Option<(String, String, Value)> {
    let p = misc::f_coding_system_plist(i, vec![coding.clone()].into()).ok()?;
    let name = prop(i, &p, ":name");
    let kind = prop(i, &p, ":coding-type");
    Some((
        match name {
            Value::Sym(s) => i.symbol_name(s),
            _ => String::new(),
        },
        match kind {
            Value::Sym(s) => i.symbol_name(s),
            _ => String::new(),
        },
        p,
    ))
}
fn specs(i: &mut Interp, p: &Value) -> Vec<CharsetSpec> {
    prop(i, p, ":charset-list")
        .list_to_vec()
        .unwrap_or_default()
        .iter()
        .filter_map(|v| {
            let Value::Sym(s) = v else { return None };
            let n = i.symbol_name(*s);
            let cp = charset::effective_charset_plist(i, &n);
            let dim = prop(i, &cp, ":dimension").int().unwrap_or(1).clamp(1, 4) as usize;
            let mut ranges = charset::charset_dims(i, &cp).unwrap_or_default();
            if ranges.len() > dim {
                ranges.drain(..ranges.len() - dim);
            }
            Some((n, dim, ranges))
        })
        .collect()
}
fn scalar(c: u32) -> char {
    lisp_char(c).unwrap_or_else(|| mule_seq_char(c))
}
fn raw(b: u8) -> char {
    if b < 128 {
        b as char
    } else {
        scalar(0x3fff00 + b as u32)
    }
}
fn sjis_encode(i: &Interp, c: i64) -> Option<Vec<u8>> {
    if c < 128 {
        return Some(vec![c as u8]);
    }
    if let Some(k) = charset::charset_encode(i, "katakana-jisx0201", c) {
        return Some(vec![(k as u8) | 0x80]);
    }
    let j = charset::charset_encode(i, "japanese-jisx0208", c)?;
    let row = (j >> 8) as u8;
    let cell = j as u8;
    let mut lead = ((row - 0x21) >> 1) + 0x81;
    if lead > 0x9f {
        lead += 0x40;
    }
    let trail = if row & 1 == 1 {
        let x = cell + 0x1f;
        if x >= 0x7f { x + 1 } else { x }
    } else {
        cell + 0x7e
    };
    Some(vec![lead, trail])
}
fn encode_with(i: &Interp, sets: &[CharsetSpec], sjis: bool, c: u32) -> Option<Vec<u8>> {
    if c >= 0x3fff80 {
        return Some(vec![c as u8]);
    }
    if sjis {
        return sjis_encode(i, c as i64);
    }
    for (name, n, ranges) in sets {
        if let Some(code) = charset::charset_encode(i, name, c as i64) {
            let bytes: Vec<u8> = (0..*n).rev().map(|k| (code >> (k * 8)) as u8).collect();
            if ranges.len() == *n
                && bytes
                    .iter()
                    .zip(ranges)
                    .any(|(b, (lo, hi))| (*b as i64) < *lo || (*b as i64) > *hi)
            {
                continue;
            }
            return Some(bytes);
        }
    }
    None
}
pub(crate) fn encode(i: &mut Interp, coding: &Value, text: &str, _finish: bool) -> Option<Vec<u8>> {
    let (n, k, p) = attrs(i, coding)?;
    if n == "utf-8-emacs" {
        return Some(
            text.chars()
                .flat_map(|c| internal_encode(lisp_char_code(c) as u32))
                .collect(),
        );
    }
    if n == "utf-7" || n == "utf-7-imap" {
        return Some(utf7_encode(text, n == "utf-7-imap"));
    }
    if k != "charset" && k != "shift-jis" {
        return None;
    }
    let sets = specs(i, &p);
    let sjis = k == "shift-jis";
    let default = prop(i, &p, ":default-char").int().unwrap_or(32) as u32;
    let fallback = encode_with(i, &sets, sjis, default).unwrap_or_else(|| vec![32]);
    Some(
        text.chars()
            .flat_map(|c| {
                encode_with(i, &sets, sjis, lisp_char_code(c) as u32)
                    .unwrap_or_else(|| fallback.clone())
            })
            .collect(),
    )
}
/// The Lisp UTF-7 converter signals when base64 cannot produce bytes.
pub(crate) fn validate_decode(
    i: &mut Interp,
    coding: &Value,
    bytes: &[u8],
) -> Result<(), crate::lisp::error::Flow> {
    let Some((name, _, _)) = attrs(i, coding) else {
        return Ok(());
    };
    if name != "utf-7" && name != "utf-7-imap" {
        return Ok(());
    }
    let imap = name == "utf-7-imap";
    let esc = if imap { b'&' } else { b'+' };
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] != esc {
            at += 1;
            continue;
        }
        at += 1;
        let start = at;
        while at < bytes.len()
            && (bytes[at].is_ascii_alphanumeric()
                || bytes[at] == b'+'
                || bytes[at] == if imap { b',' } else { b'/' })
        {
            at += 1;
        }
        if (at - start) % 4 == 1 {
            return Err(i.wrong_type_mut("wholenump", &Value::Nil));
        }
    }
    Ok(())
}
pub(crate) fn decode(
    i: &mut Interp,
    coding: &Value,
    bytes: &[u8],
    force_conversion: bool,
) -> Option<(String, Spans)> {
    decode_with_native(i, coding, bytes, &[], force_conversion)
}

/// Region decoding passes native multibyte characters through without
/// mistaking them for external octets or ending a charset annotation.
pub(super) fn decode_with_native(
    i: &mut Interp,
    coding: &Value,
    bytes: &[u8],
    native: &[Option<char>],
    force_conversion: bool,
) -> Option<(String, Spans)> {
    let (n, k, p) = attrs(i, coding)?;
    if n == "utf-8-emacs" {
        return Some((internal_decode(bytes), vec![]));
    }
    if n == "utf-7" || n == "utf-7-imap" {
        return Some((utf7_decode(bytes, n == "utf-7-imap"), vec![]));
    }
    if k != "charset" && k != "shift-jis" {
        return None;
    }
    if !force_conversion
        && bytes.iter().all(|b| *b < 128 && *b != 13)
        && prop(i, &p, ":ascii-compatible-p").truthy()
    {
        return Some((
            bytes
                .iter()
                .enumerate()
                .map(|(at, b)| native.get(at).copied().flatten().unwrap_or(*b as char))
                .collect(),
            vec![],
        ));
    }
    let sets = specs(i, &p);
    let sjis = k == "shift-jis";
    let mut out = String::new();
    let mut spans: Spans = vec![];
    let mut active: Option<String> = None;
    let mut pos = 0;
    let mut at = 0;
    while at < bytes.len() {
        if let Some(c) = native.get(at).copied().flatten() {
            out.push(c);
            at += 1;
            pos += 1;
            continue;
        }
        let b = bytes[at];
        let mut found = None;
        if sjis {
            if b < 128 {
                found = Some((b as i64, 1, "ascii".to_string()));
            } else if (0xa1..=0xdf).contains(&b) {
                found = charset::charset_decode(i, "katakana-jisx0201", (b & 0x7f) as i64)
                    .map(|c| (c, 1, "katakana-jisx0201".to_string()));
            } else if ((0x81..=0x9f).contains(&b) || (0xe0..=0xef).contains(&b))
                && at + 1 < bytes.len()
            {
                let t = bytes[at + 1];
                if (0x40..=0xfc).contains(&t) && t != 0x7f {
                    let mut row = ((if b < 0xa0 { b - 0x81 } else { b - 0xc1 }) as i64) * 2 + 0x21;
                    let cell = if t >= 0x9f {
                        row += 1;
                        t as i64 - 0x7e
                    } else {
                        t as i64 - if t > 0x7f { 0x20 } else { 0x1f }
                    };
                    found = charset::charset_decode(i, "japanese-jisx0208", (row << 8) | cell)
                        .map(|c| (c, 2, "japanese-jisx0208".to_string()));
                }
            }
        } else {
            for (name, len, ranges) in &sets {
                if at + len > bytes.len()
                    || (at..at + len).any(|p| native.get(p).is_some_and(Option::is_some))
                {
                    continue;
                }
                if ranges.len() == *len
                    && bytes[at..at + len]
                        .iter()
                        .zip(ranges)
                        .any(|(b, (lo, hi))| (*b as i64) < *lo || (*b as i64) > *hi)
                {
                    continue;
                }
                let code = bytes[at..at + len]
                    .iter()
                    .fold(0i64, |v, b| (v << 8) | *b as i64);
                if let Some(c) = charset::charset_decode(i, name, code) {
                    found = Some((c, *len, name.clone()));
                    break;
                }
            }
        }
        if found.is_none() {
            let incomplete = if sjis {
                at + 1 == bytes.len() && ((0x81..=0x9f).contains(&b) || (0xe0..=0xef).contains(&b))
            } else {
                sets.iter().any(|(_, len, ranges)| {
                    *len > bytes.len() - at
                        && ranges
                            .first()
                            .is_some_and(|(lo, hi)| (b as i64) >= *lo && (b as i64) <= *hi)
                })
            };
            if incomplete && native.get(at..).unwrap_or(&[]).iter().all(Option::is_none) {
                if let Some(last) = spans.last_mut() {
                    last.1 = pos;
                }
                out.extend(bytes[at..].iter().map(|b| raw(*b)));
                return Some((out, spans));
            }
        }
        if let Some((c, len, cs)) = found {
            if cs != "ascii" && active.as_ref() != Some(&cs) {
                if let Some(last) = spans.last_mut() {
                    last.1 = pos;
                }
                spans.push((pos, pos, cs.clone()));
                active = Some(cs);
            }
            out.push(scalar(c as u32));
            at += len;
        } else {
            out.push(raw(b));
            at += 1;
        }
        pos += 1;
    }
    if let Some(last) = spans.last_mut() {
        last.1 = pos;
    }
    Some((out, spans))
}
pub(super) fn internal_encode(c: u32) -> Vec<u8> {
    if c >= 0x3fff80 {
        return vec![c as u8];
    }
    if c < 128 {
        return vec![c as u8];
    }
    let n = if c < 0x800 {
        2
    } else if c < 0x10000 {
        3
    } else if c < 0x200000 {
        4
    } else {
        5
    };
    let mut bs = vec![0; n];
    let mut x = c;
    for b in bs[1..].iter_mut().rev() {
        *b = 0x80 | (x & 63) as u8;
        x >>= 6;
    }
    bs[0] = ([0, 0, 0xc0, 0xe0, 0xf0, 0xf8][n]) | x as u8;
    bs
}
pub(super) fn internal_decode(bs: &[u8]) -> String {
    let mut out = String::new();
    let mut at = 0;
    while at < bs.len() {
        let b = bs[at];
        let n = match b {
            0..=127 => 1,
            0xc2..=0xdf => 2,
            0xe0..=0xef => 3,
            0xf0..=0xf7 => 4,
            0xf8 => 5,
            _ => 0,
        };
        if n == 0 || at + n > bs.len() || bs[at + 1..at + n].iter().any(|b| b & 0xc0 != 0x80) {
            out.push(raw(b));
            at += 1;
            continue;
        }
        let mut c = (b & [0, 127, 31, 15, 7, 3][n]) as u32;
        for b in &bs[at + 1..at + n] {
            c = (c << 6) | (b & 63) as u32;
        }
        if c < [0, 0, 0x80, 0x800, 0x10000, 0x200000][n] || (0xd800..=0xdfff).contains(&c) {
            out.push(raw(b));
            at += 1;
            continue;
        }
        out.push(scalar(c));
        at += n;
    }
    out
}
const B64: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
fn utf7_encode(s: &str, imap: bool) -> Vec<u8> {
    let esc = if imap { b'&' } else { b'+' };
    let chars: Vec<char> = s.chars().collect();
    let mut out = vec![];
    let mut at = 0;
    let direct = |c: char| {
        c == '\t'
            || c == '\n'
            || c == '\r'
            || (c >= ' ' && c <= '~' && (imap || c != '\\' && c != '~'))
    };
    while at < chars.len() {
        let c = chars[at];
        if c as u32 == esc as u32 {
            out.extend([esc, b'-']);
            at += 1;
            continue;
        }
        if direct(c) {
            out.push(c as u8);
            at += 1;
            continue;
        }
        out.push(esc);
        let mut acc = 0u32;
        let mut bits = 0;
        while at < chars.len() && !direct(chars[at]) && chars[at] as u32 != esc as u32 {
            let mut buf = [0u16; 2];
            let code = lisp_char_code(chars[at]) as u32;
            let units: &[u16] = if code >= 0x110000 {
                buf[0] = 0x20;
                &buf[..1]
            } else if (0xd800..=0xdfff).contains(&code) {
                buf[0] = code as u16;
                &buf[..1]
            } else {
                chars[at].encode_utf16(&mut buf)
            };
            for unit in units.iter() {
                for b in unit.to_be_bytes() {
                    acc = (acc << 8) | b as u32;
                    bits += 8;
                    while bits >= 6 {
                        bits -= 6;
                        let x = B64[((acc >> bits) & 63) as usize];
                        out.push(if imap && x == b'/' { b',' } else { x });
                    }
                }
            }
            at += 1;
        }
        if bits > 0 {
            let x = B64[((acc << (6 - bits)) & 63) as usize];
            out.push(if imap && x == b'/' { b',' } else { x });
        }
        if imap || at < chars.len() {
            out.push(b'-');
        }
    }
    out
}
fn utf7_decode(bs: &[u8], imap: bool) -> String {
    let esc = if imap { b'&' } else { b'+' };
    let mut out = String::new();
    let mut at = 0;
    while at < bs.len() {
        if bs[at] != esc {
            let start = at;
            while at < bs.len() && bs[at] != esc {
                at += 1;
            }
            out.push_str(&internal_decode(&bs[start..at]));
            continue;
        }
        let start = at;
        at += 1;
        if bs.get(at) == Some(&b'-') {
            out.push(esc as char);
            at += 1;
            continue;
        }
        let mut acc = 0u32;
        let mut bits = 0;
        let mut data = vec![];
        while at < bs.len() {
            if imap && bs[at] == b'/' {
                break;
            }
            let b = if imap && bs[at] == b',' { b'/' } else { bs[at] };
            let Some(v) = B64.iter().position(|x| *x == b) else {
                break;
            };
            acc = (acc << 6) | v as u32;
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                data.push((acc >> bits) as u8);
            }
            at += 1;
        }
        if at == start + 1 {
            out.extend(bs[start..at].iter().map(|b| raw(*b)));
        } else {
            // GNU UTF-16 drops a trailing high surrogate, retains a lone
            // low surrogate, and emits a rejected high unit as two Latin-1
            // characters before decoding the following complete unit.
            let mut ix = 0;
            while ix + 1 < data.len() {
                let unit = u16::from_be_bytes([data[ix], data[ix + 1]]);
                if (0xd800..=0xdbff).contains(&unit) {
                    if ix + 3 < data.len() {
                        let next = u16::from_be_bytes([data[ix + 2], data[ix + 3]]);
                        if (0xdc00..=0xdfff).contains(&next) {
                            let code =
                                0x10000 + (((unit as u32 - 0xd800) << 10) | (next as u32 - 0xdc00));
                            out.push(scalar(code));
                            ix += 4;
                            continue;
                        }
                        out.push(data[ix] as char);
                        out.push(data[ix + 1] as char);
                    }
                } else {
                    out.push(scalar(unit as u32));
                }
                ix += 2;
            }
            if data.len() % 2 == 1 {
                out.push(raw(*data.last().unwrap()));
            }
        }
        if bs.get(at) == Some(&b'-') {
            at += 1;
        }
    }
    out
}
