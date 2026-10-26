//! Quoted strings, character literals, and GNU modifier/escape decoding.

use super::{
    CHAR_ALT, CHAR_CTL, CHAR_HYPER, CHAR_META, CHAR_SHIFT, CHAR_SUPER, Reader, eof_err,
    non_unicode_err, plain_err, read_err,
};
use crate::lisp::{
    error::Flow,
    text::{EmacsChar, EmacsText},
    value::Value,
};

impl Reader<'_> {
    pub(super) fn read_string(&mut self) -> Result<Value, Flow> {
        let mut out = EmacsText::new();
        let mut multibyte = false;
        loop {
            match self.next()? {
                None => return Err(eof_err(self.interp)),
                Some('"') => {
                    if out.is_empty() {
                        return Ok(self.interp.empty_string(multibyte));
                    }
                    let out = if multibyte {
                        out
                    } else {
                        // GNU keeps the encoded bytes when no original input
                        // character forced multibyte storage. Byte8 sequences
                        // collapse to their single-byte representation.
                        EmacsText::from_internal_bytes(&out.to_external_bytes(), false).unwrap()
                    };
                    let v = Value::string_with_kind(out, multibyte);
                    // GNU: only real non-ASCII characters force multibyte
                    // storage. Byte8 characters alone remain raw bytes,
                    // independently of the source's representation.
                    if multibyte {
                        if let Value::Str(r) = &v {
                            self.interp.mark_multibyte(r);
                        }
                    } else if let Value::Str(r) = &v {
                        self.interp.mark_unibyte(r);
                    }
                    return Ok(v);
                }
                Some('\\') => {
                    if let Some(c) = self.read_string_escape()? {
                        if c.code() >= 0x80 && crate::lisp::value::eight_bit_byte(c).is_none() {
                            multibyte = true;
                        }
                        out.push(c);
                    }
                }
                Some(_) => {
                    let c = self.last_character();
                    if self.last_character_code() <= 0x3fffff
                        && c.code() >= 0x80
                        && crate::lisp::value::eight_bit_byte(c).is_none()
                    {
                        multibyte = true;
                    }
                    out.push(c);
                }
            }
        }
    }

    /// String-specific rules around GNU's shared character-escape parser.
    fn read_string_escape(&mut self) -> Result<Option<EmacsChar>, Flow> {
        match self.peek()? {
            Some(' ' | '\n') => {
                self.pos += 1;
                return Ok(None);
            }
            Some('s') => {
                self.pos += 1;
                return Ok(Some(' '.into()));
            }
            _ => {}
        }
        let escaped = self.read_char_escape()?;
        const MODIFIERS: i128 =
            CHAR_CTL | CHAR_META | CHAR_SHIFT | CHAR_HYPER | CHAR_SUPER | CHAR_ALT;
        let mut modifiers = escaped & MODIFIERS;
        let mut character = escaped & !MODIFIERS;
        if modifiers == CHAR_CTL && character == 32 {
            character = 0;
            modifiers = 0;
        }
        if character < 128 {
            if modifiers & CHAR_SHIFT != 0 {
                if (b'A' as i128..=b'Z' as i128).contains(&character) {
                    modifiers &= !CHAR_SHIFT;
                } else if (b'a' as i128..=b'z' as i128).contains(&character) {
                    character -= 32;
                    modifiers &= !CHAR_SHIFT;
                }
            }
            if modifiers & CHAR_META != 0 {
                modifiers &= !CHAR_META;
                character = 0x3fff00 + (character | 0x80);
            }
        }
        if modifiers != 0 {
            return Err(read_err(self.interp, "Invalid modifier in string"));
        }
        Ok(Some(
            crate::lisp::value::lisp_char(character as u32)
                .unwrap_or_else(|| crate::lisp::value::mule_seq_char(character as u32)),
        ))
    }

    /// Read a char literal after `?` or `\` inside a string modifier.
    /// Returns the char code with modifier bits.
    pub(super) fn read_char_literal(&mut self) -> Result<i128, Flow> {
        match self.next()? {
            None => Err(eof_err(self.interp)),
            Some('\\') => self.read_char_escape(),
            Some(_) => Ok(self.last_character_code()),
        }
    }

    /// GNU requires the char after a `?' literal to terminate it:
    /// EOF, a control/space char (code ≤ 32), or one of
    /// `"' ; ( ) [ ] # ? ` , .'.
    pub(super) fn check_char_delim(&mut self) -> Result<(), Flow> {
        let delimiter = self.peek()?;
        // GNU returns this character even when it makes the literal invalid.
        self.unread_from(self.pos)?;
        match delimiter {
            None => Ok(()),
            Some(c) if (c as u32) <= 32 => Ok(()),
            Some('"' | '\'' | ';' | '(' | ')' | '[' | ']' | '#' | '?' | '`' | ',' | '.') => Ok(()),
            _ => Err(read_err(self.interp, "?")),
        }
    }

    /// GNU `\x' escape: one or more hex digits. With 1–2 digits,
    /// values ≥ 0x80 become byte8 chars (BYTE8_TO_CHAR → 0x3FFFxx).
    /// Larger values may carry modifier bits.
    fn read_hex_char_escape(&mut self) -> Result<(i128, usize), Flow> {
        let mut val: i128 = 0;
        let mut count = 0usize;
        while let Some(c) = self.peek()? {
            match c.to_digit(16) {
                Some(d) => {
                    val = val
                        .checked_mul(16)
                        .and_then(|v| v.checked_add(d as i128))
                        .unwrap_or(0x1000_0000);
                    // GNU caps at CHAR_META | (CHAR_META - 1) = 0xFFFFFFF.
                    if val > CHAR_META | (CHAR_META - 1) {
                        return Err(plain_err(
                            self.interp,
                            &format!("Hex character out of range: \\x{val:x}"),
                        ));
                    }
                    self.pos += 1;
                    if count < 3 {
                        count += 1;
                    }
                }
                None => break,
            }
        }
        self.unread_from(self.pos)?;
        if count == 0 {
            return Err(plain_err(
                self.interp,
                "Invalid escape char syntax: \\x not followed by hex digit",
            ));
        }
        if count < 3 && val >= 0x80 {
            val = 0x3F_FF00 + val;
        }
        Ok((val, count))
    }

    /// GNU `\u'/`\U' escapes need exactly N hex digits; a non-hex
    /// character or EOF is an error.
    fn read_exact_hex(&mut self, n: usize) -> Result<i128, Flow> {
        let mut v: i128 = 0;
        for _ in 0..n {
            match self.next()? {
                None => {
                    return Err(plain_err(
                        self.interp,
                        &format!("Malformed Unicode escape: \\u{v:x}"),
                    ));
                }
                Some(c) => match c.to_digit(16) {
                    Some(d) => v = v * 16 + d as i128,
                    None => {
                        return Err(plain_err(
                            self.interp,
                            &format!(
                                "Non-hex character used for Unicode escape: {c} ({})",
                                c as u32
                            ),
                        ));
                    }
                },
            }
        }
        Ok(v)
    }

    /// GNU `\N{name}' — Unicode char name (whitespace normalized to a
    /// single space) or `U+XXXX'.
    fn read_named_char(&mut self) -> Result<i128, Flow> {
        if self.next()? != Some('{') {
            return Err(read_err(self.interp, "Expected opening brace after \\N"));
        }
        let mut name = String::new();
        let mut ws = false;
        loop {
            match self.next()? {
                None => return Err(eof_err(self.interp)),
                Some('}') => break,
                Some(c) if (c as u32) >= 0x80 => {
                    return Err(read_err(
                        self.interp,
                        &format!("Invalid character U+{:04X} in character name", c as u32),
                    ));
                }
                Some(c) if matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0b' | '\x0c') => {
                    if !ws {
                        name.push(' ');
                    }
                    ws = true;
                }
                Some(c) => {
                    ws = false;
                    name.push(c);
                }
            }
        }
        if name.is_empty() {
            return Err(read_err(self.interp, "Empty character name"));
        }
        let code: Option<i128> = if name.len() > 2 && name[..2].eq_ignore_ascii_case("u+") {
            i128::from_str_radix(&name[2..], 16).ok()
        } else {
            unicode_names2::character(&name).map(|c| c as i128)
        };
        match code {
            Some(n) if (0..=0x10_ffff).contains(&n) && !(0xd800..0xe000).contains(&n) => Ok(n),
            _ => Err(read_err(self.interp, &format!("\\N{{{name}}}"))),
        }
    }

    /// Char literal after `?\`.
    pub(super) fn read_char_escape(&mut self) -> Result<i128, Flow> {
        match self.next()? {
            None => Err(eof_err(self.interp)),
            // Modifier prefixes must come first: `?\C-a`, `?\M-x`, `?\s-k`.
            Some(c @ ('C' | 'M' | 'S' | 'H' | 'A' | 's')) if self.peek()? == Some('-') => {
                self.pos += 1;
                let bit = match c {
                    'C' => CHAR_CTL,
                    'M' => CHAR_META,
                    'S' => CHAR_SHIFT,
                    'H' => CHAR_HYPER,
                    'A' => CHAR_ALT,
                    's' => CHAR_SUPER,
                    _ => unreachable!(),
                };
                let inner = self.read_char_literal()?;
                // `C-' folds the base char to a control character when
                // possible instead of setting the control bit:
                // ?\C-a == 1, ?\C-@ == 0, ?\C-? == 127. Chars that can't
                // fold (e.g. digits, multibyte) keep CHAR_CTL.
                if bit == CHAR_CTL {
                    const ALL_MODS: i128 =
                        CHAR_CTL | CHAR_META | CHAR_SHIFT | CHAR_HYPER | CHAR_SUPER | CHAR_ALT;
                    let base = inner & !ALL_MODS;
                    let mods = inner & (ALL_MODS & !CHAR_CTL);
                    return Ok(match ctrl_fold(base) {
                        Some(c) => mods | c,
                        None => mods | CHAR_CTL | base,
                    });
                }
                Ok(inner | bit)
            }
            Some('a') => Ok(7),
            Some('b') => Ok(8),
            Some('d') => Ok(127),
            Some('e') => Ok(27),
            Some('f') => Ok(12),
            Some('n') => Ok(10),
            Some('r') => Ok(13),
            // `\s' not followed by `-' is just a space (GNU).
            Some('s') => {
                self.unread_from(self.pos)?;
                Ok(32)
            }
            Some('t') => Ok(9),
            Some('v') => Ok(11),
            Some('x') => self.read_hex_char_escape().map(|(n, _)| n),
            Some('u') => {
                let n = self.read_exact_hex(4)?;
                if n > 0x10_ffff {
                    return Err(non_unicode_err(self.interp, n));
                }
                self.unread_from(self.pos)?;
                Ok(n)
            }
            Some('U') => {
                let n = self.read_exact_hex(8)?;
                if n > 0x10_ffff {
                    return Err(non_unicode_err(self.interp, n));
                }
                self.unread_from(self.pos)?;
                Ok(n)
            }
            Some('N') => self.read_named_char(),
            Some('^') => {
                let inner = match self.next()? {
                    None => return Err(eof_err(self.interp)),
                    Some('\\') => self.read_char_escape()?,
                    Some(_) => self.last_character_code(),
                };
                const ALL_MODS: i128 =
                    CHAR_CTL | CHAR_META | CHAR_SHIFT | CHAR_HYPER | CHAR_SUPER | CHAR_ALT;
                let mods = inner & (ALL_MODS & !CHAR_CTL);
                let base = inner & !ALL_MODS;
                Ok(mods | ctrl_fold(base).unwrap_or(CHAR_CTL | base))
            }
            Some('\n') => Err(plain_err(
                self.interp,
                "Invalid escape char syntax: \\<newline>",
            )),
            Some(c @ ('C' | 'M' | 'S' | 'H' | 'A')) => Err(plain_err(
                self.interp,
                &format!("Invalid escape char syntax: \\{c} not followed by -"),
            )),
            Some(c) if c.is_digit(8) => {
                let mut n = (c as i128) - ('0' as i128);
                for _ in 0..2 {
                    match self.peek()? {
                        Some(d) if d.is_digit(8) => {
                            n = n * 8 + (d as i128 - '0' as i128);
                            self.pos += 1;
                        }
                        _ => break,
                    }
                }
                // GNU: octal escapes in 0x80..0xFF are raw bytes.
                if (0x80..0x100).contains(&n) {
                    n = 0x3F_FF00 + n;
                }
                self.unread_from(self.pos)?;
                Ok(n)
            }
            Some(_) => Ok(self.last_character_code()),
        }
    }
}

/// GNU's control-char fold: `@'..`_' and `a'..'z' fold to &0x1f,
/// `?' folds to 127. Anything else keeps the CHAR_CTL modifier.
fn ctrl_fold(base: i128) -> Option<i128> {
    match base {
        63 => Some(127),
        b if (64..96).contains(&b) || (97..123).contains(&b) => Some(b & 0x1f),
        _ => None,
    }
}
