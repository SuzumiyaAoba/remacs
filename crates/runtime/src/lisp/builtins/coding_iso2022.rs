//! ISO-2022 designation/invocation codec.  The coding and charset plists
//! supply the repertoires, initial graphic sets, dimensions and final bytes.
use crate::lisp::text::{EmacsChar, EmacsStr, EmacsText};
use super::{charset, misc};
use crate::lisp::{
    Interp,
    value::{Value, eight_bit_byte, lisp_char, lisp_char_code, mule_seq_char},
};

fn prop(i: &Interp, pl: &Value, key: &str) -> Value {
    i.intern_soft(key)
        .map(|s| crate::lisp::eval::plist_get(pl, s))
        .unwrap_or(Value::Nil)
}
fn name(i: &Interp, v: &Value) -> Option<String> {
    match v {
        Value::Sym(s) => Some(i.symbol_name(*s).to_string()),
        _ => None,
    }
}
#[derive(Clone)]
struct Set {
    name: String,
    dim: usize,
    final_byte: u8,
    chars96: bool,
}
fn set(i: &mut Interp, n: &str) -> Option<Set> {
    let pl = charset::effective_charset_plist(i, n);
    let final_byte = prop(i, &pl, ":iso-final-char").int()? as u8;
    let dim = prop(i, &pl, ":dimension").int().unwrap_or(1) as usize;
    let dims = charset::charset_dims(i, &pl).unwrap_or_default();
    Some(Set {
        name: n.into(),
        dim: dim.clamp(1, 4),
        final_byte,
        chars96: dims.iter().any(|&(lo, hi)| hi - lo == 95),
    })
}
struct Spec {
    sets: Vec<Set>,
    initial: [Option<String>; 4],
    flags: Vec<String>,
    default_char: i64,
    tw: bool,
}
impl Spec {
    fn flag(&self, f: &str) -> bool {
        self.flags.iter().any(|x| x == f)
    }
    fn get(&self, n: &str) -> Option<&Set> {
        self.sets.iter().find(|s| s.name == n)
    }
}
fn spec(i: &mut Interp, coding: &Value) -> Option<Spec> {
    let pl = misc::f_coding_system_plist(i, vec![coding.clone()].into()).ok()?;
    if name(i, &prop(i, &pl, ":coding-type")).as_deref() != Some("iso-2022") {
        return None;
    }
    let flags = prop(i, &pl, ":flags")
        .list_to_vec()
        .unwrap_or_default()
        .iter()
        .filter_map(|v| name(i, v))
        .collect();
    let mut initial: [Option<String>; 4] = Default::default();
    if let Value::Vec(v) = prop(i, &pl, ":designation") {
        for (j, v) in v.borrow().iter().take(4).enumerate() {
            initial[j] = if let Value::Cons(c) = v {
                name(i, &c.borrow().car)
            } else {
                name(i, v)
            };
        }
    }
    let names = charset::coding_system_charset_names(i, coding)
        .list_to_vec()
        .unwrap_or_default();
    let mut sets: Vec<Set> = names
        .iter()
        .filter_map(|v| name(i, v))
        .collect::<Vec<_>>()
        .into_iter()
        .filter_map(|n| set(i, &n))
        .collect();
    for n in initial.iter().flatten() {
        if !sets.iter().any(|s| &s.name == n) {
            if let Some(s) = set(i, n) {
                sets.push(s);
            }
        }
    }
    Some(Spec {
        sets,
        initial,
        flags,
        default_char: prop(i, &pl, ":default-char").int().unwrap_or(32) as i64,
        tw: name(i, &prop(i, &pl, ":mime-charset")).as_deref() == Some("euc-tw"),
    })
}
fn designate(out: &mut Vec<u8>, s: &Set, reg: usize, short: bool) {
    out.push(27);
    if s.dim > 1 {
        out.push(b'$');
    }
    if !(short && s.dim == 2 && reg == 0 && !s.chars96 && (b'@'..=b'B').contains(&s.final_byte)) {
        out.push((if s.chars96 { b',' } else { b'(' }) + reg as u8);
    }
    out.push(s.final_byte);
}
fn reset(out: &mut Vec<u8>, sp: &Spec, state: &mut [Option<String>; 4], gl: &mut usize) {
    // Designation precedes invocation, including the final ASCII reset.
    if state[0].as_deref() != sp.initial[0].as_deref() {
        if let Some(s) = sp.initial[0].as_ref().and_then(|n| sp.get(n)) {
            designate(out, s, 0, sp.flag("short"));
            state[0] = Some(s.name.clone());
        }
    }
    if *gl != 0 {
        out.push(15);
        *gl = 0;
    }
}

pub(super) fn encode_with_preferences(
    i: &mut Interp,
    coding: &Value,
    text: &EmacsStr,
    finish: bool,
    preferred: &[Option<String>],
) -> Option<Vec<u8>> {
    encode_stream(
        i,
        coding,
        text,
        finish,
        preferred,
        &mut EncoderState::default(),
    )
}

#[derive(Default)]
pub(crate) struct EncoderState {
    registers: Option<[Option<String>; 4]>,
    gl: usize,
}

pub(super) fn encode_stream(
    i: &mut Interp,
    coding: &Value,
    text: &EmacsStr,
    finish: bool,
    preferred: &[Option<String>],
    stream: &mut EncoderState,
) -> Option<Vec<u8>> {
    let sp = spec(i, coding)?;
    let mut state = stream
        .registers
        .take()
        .unwrap_or_else(|| sp.initial.clone());
    let mut gl = stream.gl;
    let mut out = Vec::new();
    for (index, ch) in text.chars().enumerate() {
        if let Some(b) = eight_bit_byte(ch) {
            out.push(b);
            continue;
        }
        let mut code = lisp_char_code(ch) as i64;
        let mut selected = preferred
            .get(index)
            .and_then(|n| n.as_ref())
            .and_then(|n| sp.get(n))
            .and_then(|s| charset::charset_encode(i, &s.name, code).map(|c| (s, c)))
            .or_else(|| {
                sp.sets
                    .iter()
                    .find_map(|s| charset::charset_encode(i, &s.name, code).map(|c| (s, c)))
            });
        if selected.is_none() && code >= 128 {
            code = sp.default_char;
            selected = sp
                .sets
                .iter()
                .find_map(|s| charset::charset_encode(i, &s.name, code).map(|c| (s, c)));
        }
        if code < 128 {
            if (sp.flag("designation") && code >= 32)
                || (code < 32 || code == 127)
                    && (sp.flag("ascii-at-cntl") || code == 10 && sp.flag("ascii-at-eol"))
            {
                if (code < 32 || code == 127) && gl != 0 {
                    out.push(15);
                    gl = 0;
                }
                reset(&mut out, &sp, &mut state, &mut gl);
            }
            out.push(code as u8);
            if code == 10 && sp.flag("init-at-bol") {
                state = sp.initial.clone();
            }
            continue;
        }
        let Some((s, packed)) = selected else {
            out.push(b' ');
            continue;
        };
        let fixed = sp.initial.iter().position(|n| n.as_ref() == Some(&s.name));
        let reg = if let Some(reg) = state.iter().position(|n| n.as_ref() == Some(&s.name)) {
            reg
        } else if !sp.flag("designation") {
            if sp.tw && s.name.starts_with("chinese-cns11643-") && fixed.is_none() {
                2
            } else {
                fixed.unwrap_or(0)
            }
        } else if s.chars96 && sp.flag("single-shift") {
            2
        } else if s.chars96 && sp.flag("locking-shift") {
            1
        } else {
            0
        };
        if (sp.flag("designation") || fixed.is_none()) && state[reg].as_ref() != Some(&s.name) {
            designate(&mut out, s, reg, sp.flag("short") || !sp.flag("long-form"));
            state[reg] = Some(s.name.clone());
        }
        let high = if reg >= 2 {
            if sp.flag("7-bit") {
                out.extend_from_slice(&[27, b'N' + (reg - 2) as u8]);
                false
            } else {
                out.push(0x8e + (reg - 2) as u8);
                true
            }
        } else if reg == 1 && !sp.flag("7-bit") {
            true
        } else {
            if gl != reg {
                out.push(if reg == 1 { 14 } else { 15 });
                gl = reg;
            }
            false
        };
        if sp.tw && reg == 2 {
            let plane = s
                .name
                .rsplit('-')
                .next()
                .and_then(|x| x.parse::<u8>().ok())
                .unwrap_or(1);
            out.push(0xa0 + plane);
        }
        for j in (0..s.dim).rev() {
            out.push(((packed >> (j * 8)) as u8 & 0x7f) | if high { 0x80 } else { 0 });
        }
    }
    if finish {
        if gl != 0 {
            out.push(15);
            gl = 0;
        }
        reset(&mut out, &sp, &mut state, &mut gl);
    }
    stream.registers = Some(state);
    stream.gl = gl;
    Some(out)
}

fn push_raw(out: &mut EmacsText, b: u8) {
    out.push(if b < 128 {
        EmacsChar::from(b as char)
    } else {
        lisp_char(0x3fff00 + b as u32).unwrap()
    });
}
fn push_char(out: &mut EmacsText, n: i64) {
    out.push(lisp_char(n as u32).unwrap_or_else(|| mule_seq_char(n as u32)));
}

pub(super) fn decode(
    i: &mut Interp,
    coding: &Value,
    bytes: &[u8],
) -> Option<(EmacsText, Vec<(usize, usize, String)>)> {
    decode_with_native(i, coding, bytes, &[], bytes.len())
}

pub(super) fn decode_with_native(
    i: &mut Interp,
    coding: &Value,
    bytes: &[u8],
    native: &[Option<EmacsChar>],
    storage_bytes: usize,
) -> Option<(EmacsText, Vec<(usize, usize, String)>)> {
    decode_inner(
        i,
        coding,
        bytes,
        native,
        storage_bytes,
        &mut DecoderState::default(),
        true,
    )
}

#[derive(Default)]
pub(crate) struct DecoderState {
    registers: Option<[Option<String>; 4]>,
    gl: usize,
    gr: Option<usize>,
    pending: Vec<u8>,
}

impl DecoderState {
    pub(crate) fn has_pending(&self) -> bool { !self.pending.is_empty() }
}

pub(super) fn decode_stream(
    i: &mut Interp,
    coding: &Value,
    bytes: &[u8],
    stream: &mut DecoderState,
    finish: bool,
) -> Option<(EmacsText, Vec<(usize, usize, String)>)> {
    // Check the codec before consuming pending input: other codec families
    // use their own stream state.
    spec(i, coding)?;
    let mut input = std::mem::take(&mut stream.pending);
    input.extend_from_slice(bytes);
    decode_inner(i, coding, &input, &[], input.len(), stream, finish)
}

fn decode_inner(
    i: &mut Interp,
    coding: &Value,
    bytes: &[u8],
    native: &[Option<EmacsChar>],
    storage_bytes: usize,
    stream: &mut DecoderState,
    finish: bool,
) -> Option<(EmacsText, Vec<(usize, usize, String)>)> {
    let sp = spec(i, coding)?;
    let mut state = stream
        .registers
        .take()
        .unwrap_or_else(|| sp.initial.clone());
    let mut gl = stream.gl;
    let mut gr = stream.gr.unwrap_or(1);
    let mut p = 0;
    let mut out = EmacsText::new();
    let mut spans: Vec<(usize, usize, String)> = Vec::new();
    let mut chars = 0;
    // GNU records charset runs in its bounded integer charbuf. ASCII
    // inherits the last non-ASCII run, until the buffer is flushed.
    let limit = storage_bytes.saturating_add(16).min(0x4000) - 15;
    let mut units = 0usize;
    let mut run: Option<usize> = None;
    while p < bytes.len() {
        if units >= limit {
            units = 0;
            run = None;
        }
        if let Some(c) = native.get(p).copied().flatten() {
            out.push(c);
            // Negative charbuf input follows GNU's invalid_code arm:
            // retain the character, but resynchronize graphic plane 0.
            gl = 0;
            state[0] = Some("ascii".into());
            p += 1;
            chars += 1;
            units += 1;
            if let Some(r) = run {
                spans[r].1 = chars;
            }
            continue;
        }
        let b = bytes[p];
        if b == 27 && sp.flag("designation") {
            if !finish {
                let mut end = p + 1;
                if end == bytes.len() {
                    break;
                }
                if bytes[end] == b'$' {
                    end += 1;
                }
                if end == bytes.len() {
                    break;
                }
                if (b'('..=b'/').contains(&bytes[end]) {
                    end += 1;
                }
                if end == bytes.len() {
                    break;
                }
            }
            let mut q = p + 1;
            let mut dim = 1;
            if bytes.get(q) == Some(&b'$') {
                dim = 2;
                q += 1;
            }
            let mut reg = 0;
            let mut chars96 = false;
            let mut designation = false;
            if let Some(&x) = bytes.get(q) {
                if (b'('..=b'+').contains(&x) {
                    reg = (x - b'(') as usize;
                    designation = true;
                    q += 1;
                } else if (b','..=b'/').contains(&x) {
                    reg = (x - b',') as usize;
                    chars96 = true;
                    designation = true;
                    q += 1;
                } else if dim == 2 && (b'@'..=b'B').contains(&x) {
                    designation = true;
                }
            }
            if designation {
                if let Some(&f) = bytes.get(q) {
                    // Restrict designation to this coding's repertoire, but
                    // accept designation into any graphic register.
                    let found = sp
                        .sets
                        .iter()
                        .find(|s| s.dim == dim && s.chars96 == chars96 && s.final_byte == f);
                    if let Some(s) = found {
                        state[reg] = Some(s.name.clone());
                        p = q + 1;
                        continue;
                    }
                }
            }
            if sp.flag("locking-shift") {
                match bytes.get(p + 1) {
                    Some(b'n') => {
                        gl = 2;
                        p += 2;
                        continue;
                    }
                    Some(b'o') => {
                        gl = 3;
                        p += 2;
                        continue;
                    }
                    Some(b'~') => {
                        gr = 1;
                        p += 2;
                        continue;
                    }
                    Some(b'}') => {
                        gr = 2;
                        p += 2;
                        continue;
                    }
                    Some(b'|') => {
                        gr = 3;
                        p += 2;
                        continue;
                    }
                    _ => {}
                }
            }
        }
        if sp.flag("locking-shift") && (b == 14 || b == 15) {
            if b == 15 || state[1].is_some() {
                gl = if b == 14 { 1 } else { 0 };
                p += 1;
                continue;
            }
        }
        let start = p;
        let mut reg = if b >= 128 { gr } else { gl };
        let mut single = false;
        if sp.flag("single-shift") || sp.tw {
            if b == 0x8e || b == 0x8f {
                reg = (b - 0x8e + 2) as usize;
                p += 1;
                single = true;
            } else if b == 27 && matches!(bytes.get(p + 1), Some(b'N' | b'O')) {
                reg = (bytes[p + 1] - b'N' + 2) as usize;
                p += 2;
                single = true;
            }
        }
        if !finish && single && p == bytes.len() {
            p = start;
            break;
        }
        let tw_name = if sp.tw && single && reg == 2 {
            if let Some(&plane) = bytes.get(p) {
                if (0xa1..=0xa7).contains(&plane) {
                    p += 1;
                    Some(format!("chinese-cns11643-{}", plane - 0xa0))
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };
        if !finish && single && p == bytes.len() {
            p = start;
            break;
        }
        let active = tw_name.as_ref().or(state[reg].as_ref());
        // Controls and SPACE/DEL bypass a 94-character graphic set.
        let ordinary_ascii = !single
            && b < 128
            && ((b < 32 && !matches!(b, 14 | 15 | 27))
                || matches!(b, 32 | 127)
                    && !active
                        .and_then(|n| sp.get(n))
                        .map(|s| s.chars96)
                        .unwrap_or(false));
        if ordinary_ascii {
            out.push(b as char);
            p += 1;
            chars += 1;
            units += 1;
            if let Some(r) = run {
                spans[r].1 = chars;
            }
            continue;
        }
        if let Some(s) = active.and_then(|n| sp.get(n)) {
            let end = p + s.dim;
            if !finish && end > bytes.len() {
                let high = bytes[p] >= 128;
                let graphic = bytes[p] & 0x7f;
                let valid = (!single || sp.flag("7-bit") || high)
                    && if s.chars96 {
                        (32..=127).contains(&graphic)
                    } else {
                        (33..=126).contains(&graphic) || s.name == "ascii" && !high
                    };
                if valid
                    && bytes[p + 1..]
                        .iter()
                        .all(|b| (*b >= 128) == high && (32..=127).contains(&(*b & 0x7f)))
                {
                    p = start;
                    break;
                }
            }
            if end <= bytes.len() {
                let hi = bytes[p] >= 128;
                let mut packed = 0i64;
                let y = bytes[p] & 0x7f;
                let first_valid = (!single || sp.flag("7-bit") || hi)
                    && if s.chars96 {
                        (32..=127).contains(&y)
                    } else {
                        (33..=126).contains(&y) || s.name == "ascii" && !hi
                    };
                // GNU starts a charset annotation as soon as the first
                // graphic byte selects a set, before validating its trail.
                if first_valid
                    && s.name != "ascii"
                    && run.map(|r| spans[r].2 != s.name).unwrap_or(true)
                {
                    if run.is_some() {
                        units += 4;
                    }
                    spans.push((chars, chars, s.name.clone()));
                    run = Some(spans.len() - 1);
                }
                let valid = first_valid
                    && bytes[p..end]
                        .iter()
                        .skip(1)
                        .all(|&x| (x >= 128) == hi && (32..=127).contains(&(x & 0x7f)));
                if valid {
                    for &x in &bytes[p..end] {
                        packed = (packed << 8) | (x & 0x7f) as i64;
                    }
                    if let Some(ch) = charset::charset_decode(i, &s.name, packed) {
                        push_char(&mut out, ch);
                        if let Some(r) = run {
                            spans[r].1 = chars + 1;
                        }
                        units += 1;
                        chars += 1;
                        p = end;
                        continue;
                    }
                    // A complete graphic code outside the charset's code
                    // space is emitted verbatim without losing designation.
                    for &raw in &bytes[start..end] {
                        push_raw(&mut out, raw);
                    }
                    let count = end - start;
                    chars += count;
                    units += count;
                    p = end;
                    if let Some(r) = run {
                        spans[r].1 = chars;
                    }
                    continue;
                }
            }
        }
        // Failed sequences retain every original byte; consumed prefixes
        // are not allowed to hide truncated single shifts.
        p = start + 1;
        push_raw(&mut out, bytes[start]);
        chars += 1;
        units += 1;
        if let Some(r) = run {
            spans[r].1 = chars;
        }
        // GNU resynchronizes malformed input by invoking ASCII in G0.
        gl = 0;
        state[0] = Some("ascii".into());
    }
    stream.registers = Some(state);
    stream.gl = gl;
    stream.gr = Some(gr);
    stream.pending = bytes[p..].to_vec();
    Some((out, spans))
}
