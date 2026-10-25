//! Unicode coding systems share their byte representation between string,
//! region and file conversion. Keep GNU's raw-byte and surrogate behavior.
use crate::lisp::value::{lisp_char, lisp_char_code, mule_seq_char};

fn scalar(code: u32) -> char {
    lisp_char(code).unwrap_or_else(|| mule_seq_char(code))
}
fn raw(byte: u8) -> char {
    scalar(if byte < 128 {
        byte as u32
    } else {
        0x3fff00 + byte as u32
    })
}

pub(super) fn encode(name: &str, text: &str, default: u32) -> Option<Vec<u8>> {
    let utf16 = name.starts_with("utf-16");
    if !utf16 && !name.starts_with("utf-8") {
        return None;
    }
    let little = name.starts_with("utf-16le");
    let signature = matches!(name, "utf-16" | "utf-8-auto" | "utf-8-with-signature")
        || name.ends_with("-with-signature");
    let mut out = Vec::new();
    if signature {
        out.extend_from_slice(if !utf16 {
            &[239, 187, 191]
        } else if little {
            &[255, 254]
        } else {
            &[254, 255]
        });
    }
    for c in text.chars() {
        let mut code = lisp_char_code(c) as u32;
        if utf16 {
            if code > 0x10ffff {
                code = default;
            }
            let units = if code < 0x10000 {
                vec![code as u16]
            } else {
                let n = code - 0x10000;
                vec![0xd800 | (n >> 10) as u16, 0xdc00 | (n & 1023) as u16]
            };
            for unit in units {
                out.extend_from_slice(&if little {
                    unit.to_le_bytes()
                } else {
                    unit.to_be_bytes()
                });
            }
        } else {
            out.extend(super::coding_legacy::internal_encode(code));
        }
    }
    Some(out)
}

/// Resolve the auto coding without confusing escaped bytes in a multibyte
/// source with the literal initial signature. Invalid input keeps -auto.
pub(super) fn resolve(name: &str, input: &[i32], multibyte: bool) -> String {
    use super::coding_detect::*;
    if name == "utf-8-auto" {
        let (found, rejected) = detect_utf8(input, multibyte);
        if found & UTF8_SIG != 0 {
            "utf-8-with-signature".into()
        } else if rejected & UTF8 == 0 {
            "utf-8".into()
        } else {
            name.into()
        }
    } else if name == "utf-16" {
        let (found, _) = detect_utf16(input);
        if found & UTF16_LE != 0 {
            "utf-16le-with-signature".into()
        } else if found & UTF16_BE != 0 {
            "utf-16be-with-signature".into()
        } else {
            name.into()
        }
    } else {
        name.into()
    }
}

pub(super) fn decode_utf16(name: &str, input: &[i32]) -> String {
    let little = name.starts_with("utf-16le");
    let signature = name.ends_with("-with-signature");
    let mut at = if signature && input.starts_with(if little { &[255, 254] } else { &[254, 255] }) {
        2
    } else {
        0
    };
    let mut out = String::new();
    let mut surrogate = 0u32;
    while at < input.len() {
        let first = input[at];
        at += 1;
        if first < 0 {
            out.push(scalar((-first) as u32));
            continue;
        }
        let Some(&second) = input.get(at) else {
            out.push(raw(first as u8));
            break;
        };
        at += 1;
        if second < 0 {
            out.push(raw(first as u8));
            out.push(scalar((-second) as u32));
            continue;
        }
        let code = if little {
            ((second as u32) << 8) | first as u32
        } else {
            ((first as u32) << 8) | second as u32
        };
        if surrogate != 0 {
            if (0xdc00..=0xdfff).contains(&code) {
                out.push(scalar(
                    0x10000 + ((surrogate - 0xd800) << 10) + code - 0xdc00,
                ));
                surrogate = 0;
            } else {
                // GNU emits the pending high surrogate's octets as
                // character codes, and keeps it pending until replaced.
                let bytes = if little {
                    (surrogate as u16).to_le_bytes()
                } else {
                    (surrogate as u16).to_be_bytes()
                };
                out.extend(bytes.map(|b| b as char));
                if (0xd800..=0xdbff).contains(&code) {
                    surrogate = code;
                } else {
                    out.push(scalar(code));
                }
            }
        } else if (0xd800..=0xdbff).contains(&code) {
            surrogate = code;
        } else {
            out.push(scalar(code));
        }
    }
    // GNU leaves an unmatched final high surrogate pending, rather than
    // replacing it with U+FFFD or the original two bytes.
    out
}

pub(super) fn normalize_eol(text: &str, fixed: Option<usize>) -> (String, Option<usize>) {
    let chars: Vec<char> = text.chars().collect();
    let mut seen = [false; 3];
    let mut at = 0;
    while at < chars.len() {
        match chars[at] {
            '\r' if chars.get(at + 1) == Some(&'\n') => {
                seen[1] = true;
                at += 1;
            }
            '\r' => seen[2] = true,
            '\n' => seen[0] = true,
            _ => {}
        }
        at += 1;
    }
    let eol = fixed.or_else(|| match seen.iter().filter(|s| **s).count() {
        0 => None,
        1 => seen.iter().position(|s| *s),
        _ => Some(0),
    });
    let mut out = String::new();
    at = 0;
    while at < chars.len() {
        if eol == Some(1) && chars[at..].starts_with(&['\r', '\n']) {
            out.push('\n');
            at += 2;
        } else {
            out.push(if eol == Some(2) && chars[at] == '\r' {
                '\n'
            } else {
                chars[at]
            });
            at += 1;
        }
    }
    (out, eol)
}
