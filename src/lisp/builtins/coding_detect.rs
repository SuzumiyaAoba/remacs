//! Byte-structure detectors corresponding to GNU Emacs 31.1's coding.c.
//!
//! A negative input item is the negated codepoint of a genuine multibyte
//! character, which ONE_MORE_BYTE skips rather than treating its UTF-8
//! representation as data. Retaining the codepoint lets Mule composition
//! detection measure its original storage length.
//! The caller owns ASCII/ISO escape handling, category priorities, configurable
//! charset/CCL detectors, and selection of the unrejected fallback category.

pub(crate) const UTF8_AUTO: u32 = 1 << 6;
pub(crate) const UTF8: u32 = 1 << 7;
pub(crate) const UTF8_SIG: u32 = 1 << 8;
pub(crate) const UTF16_AUTO: u32 = 1 << 9;
pub(crate) const UTF16_BE: u32 = 1 << 10;
pub(crate) const UTF16_LE: u32 = 1 << 11;
pub(crate) const UTF16_BE_NOSIG: u32 = 1 << 12;
pub(crate) const UTF16_LE_NOSIG: u32 = 1 << 13;
pub(crate) const SJIS: u32 = 1 << 15;
pub(crate) const BIG5: u32 = 1 << 16;
pub(crate) const EMACS_MULE: u32 = 1 << 18;
pub(crate) const UTF8_MASK: u32 = UTF8_AUTO | UTF8 | UTF8_SIG;
pub(crate) const UTF16_MASK: u32 =
    UTF16_AUTO | UTF16_BE | UTF16_LE | UTF16_BE_NOSIG | UTF16_LE_NOSIG;

/// Initial table from init_coding_once, before define-charset registers IDs.
pub(crate) const fn initial_mule_lengths() -> [u8; 256] {
    let mut lengths = [1; 256];
    lengths[0x9a] = 3;
    lengths[0x9b] = 3;
    lengths[0x9c] = 4;
    lengths[0x9d] = 4;
    lengths
}

/// GNU's standard charset definitions; callers supporting define-charset
/// should build this table from their current charset registry instead.
#[cfg(test)]
fn default_mule_lengths() -> [u8; 256] {
    let mut lengths = initial_mule_lengths();
    lengths[0] = 2; // ASCII has dimension 1 and emacs-mule-id 0.
    for id in 0x81..=0x8a {
        lengths[id] = 2;
    }
    for id in 0x8c..=0x8f {
        lengths[id] = 2;
    }
    for id in 0x90..=0x99 {
        lengths[id] = 3;
    }
    for id in [0xa0, 0xa1, 0xa2, 0xa3, 0xa4, 0xa5, 0xa7, 0xe0, 0xe1] {
        lengths[id] = 3;
    }
    for id in 0xf0..=0xfc {
        lengths[id] = 4;
    }
    lengths[0xfe] = 4;
    lengths
}

/// MULTIBYTE describes the source string's storage, not merely whether INPUT
/// contains negative items. Raw 8-bit characters in a multibyte string have
/// escaped internal bytes, so they cannot form the initial literal UTF-8 BOM.
/// No charset or CCL category bits are set by this module.
pub(crate) fn detect_non_iso_with_tables(
    input: &[i32],
    mule_lengths: &[u8; 256],
    sjis_max_lead: u8,
    multibyte: bool,
    head_ascii: usize,
) -> (u32, u32) {
    debug_assert!(input.iter().all(|&c| c <= 255));
    let mut found = 0;
    let mut rejected = 0;
    for (f, r) in [
        detect_utf8(input, multibyte),
        detect_utf16(input),
        detect_sjis(input, sjis_max_lead),
        detect_big5(input),
        detect_mule(input, mule_lengths, multibyte, head_ascii),
    ] {
        found |= f;
        rejected |= r;
    }
    (found, rejected)
}

fn detect_utf8(input: &[i32], multibyte: bool) -> (u32, u32) {
    // GNU deliberately requires another byte after the three-byte BOM.
    let bom_len = if multibyte && input.len() > 1 && input[0] == -0xfeff {
        // A genuine U+FEFF has literal EF BB BF internal bytes, unlike
        // three escaped raw-byte characters in a multibyte string.
        1
    } else if !multibyte && input.len() > 3 && input.starts_with(&[0xef, 0xbb, 0xbf]) {
        3
    } else {
        0
    };
    let bom = bom_len != 0;
    let mut pos = bom_len;
    let mut non_ascii = bom;
    while let Some(&c) = input.get(pos) {
        pos += 1;
        if c < 0x80 {
            non_ascii |= c < 0;
            continue;
        }
        // Emacs detection checks structure, not Unicode scalar validity.
        // This accepts overlong C0/C1 and F5..F7 sequences, as coding.c does.
        let continuation_count = match c {
            0xc0..=0xdf => 1,
            0xe0..=0xef => 2,
            0xf0..=0xf7 => 3,
            _ => return (0, UTF8_MASK),
        };
        for _ in 0..continuation_count {
            match input.get(pos) {
                Some(0x80..=0xbf) => pos += 1,
                _ => return (0, UTF8_MASK),
            }
        }
        non_ascii = true;
    }
    if bom {
        (UTF8_MASK, 0)
    } else {
        (if non_ascii { UTF8 | UTF8_AUTO } else { 0 }, UTF8_SIG)
    }
}

/// TWO_MORE_BYTES skips genuine multibyte characters in its first position,
/// but reports one in its second position to the detector as a negative item.
fn next_utf16_pair(input: &[i32], pos: &mut usize) -> Option<(i32, i32)> {
    while input.get(*pos).is_some_and(|&c| c < 0) {
        *pos += 1;
    }
    let c1 = *input.get(*pos)?;
    *pos += 1;
    let c2 = *input.get(*pos)?;
    *pos += 1;
    Some((c1, c2))
}

fn detect_utf16(input: &[i32]) -> (u32, u32) {
    if input.len() & 1 != 0 {
        return (0, UTF16_MASK);
    }
    let mut pos = 0;
    let Some((c1, c2)) = next_utf16_pair(input, &mut pos) else {
        return (0, 0);
    };
    if (c1, c2) == (0xff, 0xfe) {
        return (
            UTF16_LE | UTF16_AUTO,
            UTF16_BE | UTF16_BE_NOSIG | UTF16_LE_NOSIG,
        );
    }
    if (c1, c2) == (0xfe, 0xff) {
        return (
            UTF16_BE | UTF16_AUTO,
            UTF16_LE | UTF16_BE_NOSIG | UTF16_LE_NOSIG,
        );
    }
    if c2 < 0 {
        return (0, UTF16_MASK);
    }
    let mut rejected = UTF16_AUTO | UTF16_BE | UTF16_LE;
    let mut even = [false; 256];
    let mut odd = [false; 256];
    even[c1 as usize] = true;
    odd[c2 as usize] = true;
    let mut even_count = 1;
    let mut odd_count = 1;
    while let Some((c1, c2)) = next_utf16_pair(input, &mut pos) {
        if c2 < 0 {
            break;
        }
        if !even[c1 as usize] {
            even[c1 as usize] = true;
            even_count += 1;
            if even_count >= 128 {
                rejected |= UTF16_BE_NOSIG;
            }
        }
        if !odd[c2 as usize] {
            odd[c2 as usize] = true;
            odd_count += 1;
            if odd_count >= 128 {
                rejected |= UTF16_LE_NOSIG;
            }
        }
        if rejected == UTF16_MASK {
            break;
        }
    }
    (0, rejected)
}

fn detect_sjis(input: &[i32], max_lead: u8) -> (u32, u32) {
    let mut pos = 0;
    let mut found = 0;
    while let Some(&c) = input.get(pos) {
        pos += 1;
        if c < 0x80 {
            continue;
        }
        if (0x81..=0x9f).contains(&c) || (0xe0..=i32::from(max_lead)).contains(&c) {
            let Some(&trail) = input.get(pos) else {
                return (0, SJIS);
            };
            pos += 1;
            if !(0x40..=0xfc).contains(&trail) || trail == 0x7f {
                return (0, SJIS);
            }
            found = SJIS;
        } else if (0xa0..0xe0).contains(&c) {
            found = SJIS;
        } else {
            return (0, SJIS);
        }
    }
    (found, 0)
}

fn detect_big5(input: &[i32]) -> (u32, u32) {
    let mut pos = 0;
    let mut found = 0;
    while let Some(&c) = input.get(pos) {
        pos += 1;
        if c < 0x80 {
            continue;
        }
        if c < 0xa1 {
            return (0, BIG5);
        }
        let Some(&trail) = input.get(pos) else {
            return (0, BIG5);
        };
        pos += 1;
        if trail < 0x40 || (0x7f..=0xa0).contains(&trail) {
            // coding.c returns directly here without recording rejection.
            // Keeping this neutral matters for the final fallback candidate.
            return (0, 0);
        }
        found = BIG5;
    }
    (found, 0)
}

fn source_item_bytes(c: i32, multibyte: bool) -> usize {
    if !multibyte || (0..0x80).contains(&c) {
        1
    } else if c >= -0x7ff {
        2
    } else if c >= -0xffff {
        3
    } else if c >= -0x1fffff {
        4
    } else {
        5
    }
}

fn detect_mule(
    input: &[i32],
    lengths: &[u8; 256],
    multibyte: bool,
    head_ascii: usize,
) -> (u32, u32) {
    // A successful early ISO scan can stop the ASCII prefix at ESC/SI/SO.
    // Those controls must then be seen (and rejected) by the Mule detector.
    let mut pos = head_ascii;
    let mut found = 0;
    while let Some(&initial) = input.get(pos) {
        pos += 1;
        let mut c = initial;
        if c < 0 {
            continue;
        }
        while c == 0x80 {
            // GNU measures the physical source byte span, including the
            // terminating character. Raw bytes and Unicode characters have
            // different widths when the source string is multibyte.
            let mut bytes = 0;
            loop {
                let Some(&next) = input.get(pos) else {
                    return (0, EMACS_MULE);
                };
                pos += 1;
                c = next;
                bytes += source_item_bytes(c, multibyte);
                if c < 0xa0 {
                    break;
                }
            }
            if bytes <= 4 {
                return (0, EMACS_MULE);
            }
            found = EMACS_MULE;
        }
        if c < 0x80 {
            if matches!(c, 0x1b | 0x0e | 0x0f) {
                return (0, EMACS_MULE);
            }
            continue;
        }
        for _ in 1..lengths[c as usize] {
            match input.get(pos) {
                Some(0xa0..=0xff) => pos += 1,
                _ => return (0, EMACS_MULE),
            }
        }
        found = EMACS_MULE;
    }
    (found, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combines_only_the_requested_non_iso_category_states() {
        let lengths = default_mule_lengths();
        let (found, rejected) = detect_non_iso_with_tables(&[0x81, 0xa0], &lengths, 0xef, false, 0);
        assert_eq!(found, SJIS | EMACS_MULE);
        assert_eq!(rejected & (UTF8_MASK | BIG5), UTF8_MASK | BIG5);
        assert_eq!((found | rejected) & ((1 << 14) | (1 << 17)), 0);
    }

    #[test]
    fn mule_preserves_the_early_iso_prefix_boundary() {
        let lengths = default_mule_lengths();
        assert_eq!(detect_mule(&[27, 255], &lengths, false, 0), (0, EMACS_MULE));
        assert_eq!(detect_mule(&[27, 255], &lengths, false, 1), (EMACS_MULE, 0));
        assert_eq!(
            detect_mule(&[65, 27, 255], &lengths, false, 1),
            (0, EMACS_MULE)
        );
    }

    #[test]
    fn utf8_checks_byte_structure_and_the_complete_final_block() {
        assert_eq!(
            detect_utf8(&[0xc0, 0x80], false),
            (UTF8 | UTF8_AUTO, UTF8_SIG)
        );
        assert_eq!(
            detect_utf8(&[0xf7, 0xbf, 0xbf, 0xbf], false),
            (UTF8 | UTF8_AUTO, UTF8_SIG)
        );
        assert_eq!(detect_utf8(&[0xe3, 0x81], false), (0, UTF8_MASK));
        assert_eq!(detect_utf8(&[0xe3, -1, 0x81], true), (0, UTF8_MASK));
        assert_eq!(detect_utf8(&[-1], true), (UTF8 | UTF8_AUTO, UTF8_SIG));
    }

    #[test]
    fn utf8_bom_distinguishes_literal_bytes_from_escaped_raw_bytes() {
        assert_eq!(
            detect_utf8(&[0xef, 0xbb, 0xbf], false),
            (UTF8 | UTF8_AUTO, UTF8_SIG)
        );
        assert_eq!(detect_utf8(&[0xef, 0xbb, 0xbf, 65], false), (UTF8_MASK, 0));
        assert_eq!(
            detect_utf8(&[0xef, 0xbb, 0xbf, 65], true),
            (UTF8 | UTF8_AUTO, UTF8_SIG)
        );
        assert_eq!(detect_utf8(&[-0xfeff, 65], true), (UTF8_MASK, 0));
        assert_eq!(detect_utf8(&[-0xfeff], true), (UTF8 | UTF8_AUTO, UTF8_SIG));
    }

    #[test]
    fn utf16_bom_odd_lengths_and_dispersion() {
        assert_eq!(detect_utf16(&[0xff, 0xfe, 65]), (0, UTF16_MASK));
        assert_eq!(
            detect_utf16(&[0xff, 0xfe, 0, 0xd8]),
            (
                UTF16_AUTO | UTF16_LE,
                UTF16_BE | UTF16_BE_NOSIG | UTF16_LE_NOSIG
            )
        );
        let bytes: Vec<i32> = (0..128).flat_map(|n| [n, 65]).collect();
        assert_eq!(
            detect_utf16(&bytes),
            (0, UTF16_AUTO | UTF16_BE | UTF16_LE | UTF16_BE_NOSIG)
        );
        assert_eq!(detect_utf16(&[-1, 65]), (0, 0));
        assert_eq!(detect_utf16(&[65, -1]), (0, UTF16_MASK));
    }

    #[test]
    fn legacy_detection_consumes_the_whole_input() {
        assert_eq!(detect_sjis(&[0x82, 0xa0], 0xef), (SJIS, 0));
        assert_eq!(detect_sjis(&[0x82, 0xa0, 0x80], 0xef), (0, SJIS));
        assert_eq!(detect_sjis(&[0xa0], 0xef), (SJIS, 0));
        assert_eq!(detect_big5(&[0xff, 0xff]), (BIG5, 0));
        assert_eq!(detect_big5(&[0xa1, 0x20]), (0, 0));
        assert_eq!(detect_big5(&[0xa1]), (0, BIG5));
    }

    #[test]
    fn mule_checks_charset_lengths_and_composite_termination() {
        let lengths = default_mule_lengths();
        assert_eq!(
            detect_mule(&[0x81, 0xa0], &lengths, false, 0),
            (EMACS_MULE, 0)
        );
        assert_eq!(detect_mule(&[0x81], &lengths, false, 0), (0, EMACS_MULE));
        assert_eq!(
            detect_mule(&[0xe1, 0xa0, 0xa0], &lengths, false, 0),
            (EMACS_MULE, 0)
        );
        assert_eq!(
            detect_mule(&[0x80, 0xa0, 0xa0, 0xa0, 0xa0, 65], &lengths, false, 0),
            (EMACS_MULE, 0)
        );
        assert_eq!(
            detect_mule(&[0x80, 0xa0, 0xa0, 0xa0, 0xa0], &lengths, false, 0),
            (0, EMACS_MULE)
        );
    }

    #[test]
    fn mule_compositions_measure_multibyte_storage_widths() {
        let lengths = default_mule_lengths();
        assert_eq!(
            detect_mule(&[0x80, 0xaa, -128512], &lengths, true, 0),
            (EMACS_MULE, 0)
        );
        assert_eq!(
            detect_mule(&[0x80, 0xaa, -233], &lengths, true, 0),
            (0, EMACS_MULE)
        );
        assert_eq!(
            detect_mule(&[0x80, 0xaa, -12354], &lengths, true, 0),
            (EMACS_MULE, 0)
        );
    }
}
