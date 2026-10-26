//! Base64 operates on octets, independently of Lisp's Unicode representation.
use crate::lisp::text::{EmacsChar, EmacsStr, EmacsText};
use base64::{Engine, engine::general_purpose as engines};

pub(crate) fn encode(
    text: &EmacsStr,
    unibyte: bool,
    url: bool,
    padding: bool,
    wrap: bool,
) -> Result<EmacsText, &'static str> {
    let mut bytes = Vec::with_capacity(text.len());
    for c in text.chars() {
        if let Some(byte) = c.byte8() {
            bytes.push(byte);
        } else if c.code() < 128 || (unibyte && c.code() <= 255) {
            bytes.push(c.code() as u8);
        } else {
            return Err("Multibyte character in data for base64 encoding");
        }
    }
    let engine = match (url, padding) {
        (false, true) => &engines::STANDARD,
        (false, false) => &engines::STANDARD_NO_PAD,
        (true, true) => &engines::URL_SAFE,
        (true, false) => &engines::URL_SAFE_NO_PAD,
    };
    let encoded = engine.encode(bytes);
    let mut result = EmacsText::new();
    for (index, chunk) in encoded.as_bytes().chunks(76).enumerate() {
        if wrap && index != 0 {
            result.push('\n');
        }
        result.extend(chunk.iter().map(|byte| *byte as char));
    }
    Ok(result)
}

pub(crate) fn decode(text: &EmacsStr, unibyte: bool) -> Result<EmacsText, &'static str> {
    let cleaned: EmacsText = text.chars().filter(|c| !c.is_whitespace()).collect();
    let bytes = engines::STANDARD
        .decode(cleaned.to_external_bytes())
        .map_err(|_| "Invalid base64 data")?;
    Ok(bytes
        .into_iter()
        .map(|byte| {
            if byte < 128 || unibyte {
                EmacsChar::from(byte as char)
            } else {
                EmacsChar::new(0x3fff00 + byte as u32).unwrap()
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn standard_line_break_control_keeps_padding() {
        assert_eq!(
            encode(&EmacsText::from("f"), false, false, true, false).unwrap(),
            "Zg=="
        );
        for n in [0usize, 1, 56, 57, 58, 114, 115] {
            let input = EmacsText::from("f".repeat(n));
            let flat = encode(&input, false, false, true, false).unwrap();
            let wrapped = encode(&input, false, false, true, true).unwrap();
            assert_eq!(
                wrapped.chars().filter(|c| *c == '\n').count(),
                flat.len().saturating_sub(1) / 76
            );
            assert_eq!(
                wrapped
                    .chars()
                    .filter(|c| *c != '\n')
                    .collect::<EmacsText>(),
                flat
            );
            assert!(!wrapped.ends_with('\n'));
        }
    }
    #[test]
    fn url_padding_is_controlled_separately_from_line_breaks() {
        let input = EmacsText::from("f");
        assert_eq!(encode(&input, false, true, true, false).unwrap(), "Zg==");
        assert_eq!(encode(&input, false, true, false, false).unwrap(), "Zg");
    }
    #[test]
    fn binary_decode_never_merges_utf8_octets_or_replaces_invalid_ones() {
        assert_eq!(
            decode(&EmacsText::from("/w=="), false)
                .unwrap()
                .codes()
                .collect::<Vec<_>>(),
            [0x3fffff]
        );
        assert_eq!(
            decode(&EmacsText::from("wqA="), false)
                .unwrap()
                .codes()
                .collect::<Vec<_>>(),
            [0x3fffc2, 0x3fffa0]
        );
        assert_eq!(
            decode(&EmacsText::from("wqA="), true)
                .unwrap()
                .codes()
                .collect::<Vec<_>>(),
            [194, 160]
        );
        assert!(decode(&EmacsText::from("Zg"), false).is_err());
    }
    #[test]
    fn encoder_rejects_unicode_but_accepts_all_binary_octets() {
        for code in [128, 0xf0080, 0xd800, 0x110000] {
            assert!(
                encode(
                    &EmacsText::from_codes([code]).unwrap(),
                    false,
                    false,
                    true,
                    false
                )
                .is_err()
            );
        }
        for unibyte in [false, true] {
            let original = EmacsText::from_codes((0..=255).map(|byte| {
                if !unibyte && byte >= 128 {
                    0x3fff00 + byte
                } else {
                    byte
                }
            }))
            .unwrap();
            let encoded = encode(&original, unibyte, false, true, true).unwrap();
            assert_eq!(decode(&encoded, unibyte).unwrap(), original);
        }
    }
}
