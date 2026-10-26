//! Bounded stream state over the shared string/file codecs.
use super::{coding_iso2022, coding_legacy, misc, strfn};
use crate::lisp::{Flow, Interp, Value};

fn symbol(i: &mut Interp, name: &str) -> Value {
    if name == "nil" {
        Value::Nil
    } else {
        Value::Sym(i.intern(name))
    }
}

fn utf8_prefix(bytes: &[u8], internal: bool) -> usize {
    let mut at = 0;
    while at < bytes.len() {
        let b = bytes[at];
        let width = match b {
            0xc2..=0xdf => 2,
            0xe0..=0xef => 3,
            0xf0..=0xf4 => 4,
            0xf5..=0xf7 if internal => 4,
            0xf8 if internal => 5,
            _ => 1,
        };
        if at + width > bytes.len() {
            let tail = &bytes[at + 1..];
            let continuation = tail.iter().all(|b| (0x80..=0xbf).contains(b));
            let valid_second = tail.first().is_none_or(|b| match bytes[at] {
                0xe0 => *b >= 0xa0,
                0xed if !internal => *b <= 0x9f,
                0xf0 => *b >= 0x90,
                0xf4 if !internal => *b <= 0x8f,
                _ => true,
            });
            if continuation && valid_second {
                return at;
            }
        }
        at += if at + width <= bytes.len()
            && bytes[at + 1..at + width]
                .iter()
                .all(|b| (0x80..=0xbf).contains(b))
        {
            width
        } else {
            1
        };
    }
    at
}

#[derive(Default)]
pub(crate) struct Decoder {
    name: String,
    canonical: String,
    pending: Vec<u8>,
    started: bool,
    iso: coding_iso2022::DecoderState,
}

impl Decoder {
    pub(crate) fn has_pending(&self) -> bool {
        !self.pending.is_empty() || self.iso.has_pending()
    }

    /// Pure codec work. User post-read functions run after the process has
    /// stored this state and released its RefCell borrow.
    pub(crate) fn decode(
        &mut self,
        i: &mut Interp,
        name: &str,
        bytes: &[u8],
        finish: bool,
        raw_destination: bool,
    ) -> Result<(Value, String), Flow> {
        let requested = symbol(i, name);
        let original = misc::coding_canonical(i, &requested);
        if self.name != name || self.canonical != original {
            *self = Self::default();
            self.name = name.to_owned();
            self.canonical = original;
        }
        let coding = if raw_destination {
            misc::raw_file_coding(i, &requested)?
        } else {
            requested.clone()
        };
        let canonical = misc::coding_canonical(i, &coding);
        let eol = misc::coding_fixed_eol(i, &coding);
        let mut input = std::mem::take(&mut self.pending);
        input.extend_from_slice(bytes);
        let mut end = input.len();
        if !finish {
            if canonical.starts_with("utf-16") {
                end &= !1;
                let little = canonical.starts_with("utf-16le")
                    || !self.started && input.starts_with(&[255, 254]);
                let unit = |at: usize| {
                    if little {
                        u16::from_le_bytes([input[at], input[at + 1]])
                    } else {
                        u16::from_be_bytes([input[at], input[at + 1]])
                    }
                };
                if end >= 2 && (0xd800..=0xdbff).contains(&unit(end - 2)) {
                    end -= 2;
                }
                if end >= 2 && matches!(eol, None | Some(1)) && unit(end - 2) == 13 {
                    end -= 2;
                }
            } else {
                // UTF-7 and HZ are Lisp conversion hooks over UTF-8. As in
                // GNU, only the underlying UTF-8 character can span reads.
                if canonical.starts_with("utf-8")
                    || matches!(
                        canonical.as_str(),
                        "undecided" | "utf-7" | "utf-7-imap" | "chinese-hz"
                    )
                {
                    end = utf8_prefix(&input, canonical == "utf-8-emacs");
                } else if let Some(prefix) = coding_legacy::complete_prefix(i, &coding, &input) {
                    end = prefix;
                }
                if end == input.len()
                    && end > 0
                    && input[end - 1] == 13
                    && matches!(eol, None | Some(1))
                {
                    end -= 1;
                }
            }
        }
        self.pending = input.split_off(end);
        if input.is_empty() && !finish {
            return Ok((Value::Nil, name.into()));
        }
        let (value, used) =
            misc::process_decode_bytes(i, &input, &coding, &mut self.iso, !self.started, finish)?;
        self.started |= !input.is_empty();
        let mut used = if let Value::Sym(id) = used {
            i.symbol_name(id)
        } else {
            "nil".into()
        };
        if name == "nil" || raw_destination {
            used = name.into();
        }
        self.name = used.clone();
        let resolved = symbol(i, &used);
        self.canonical = misc::coding_canonical(i, &resolved);
        let empty = matches!(&value, Value::Str(s) if s.borrow().is_empty());
        Ok((if empty { Value::Nil } else { value }, used))
    }
}

#[derive(Default)]
pub(crate) struct Encoder {
    name: String,
    signature_written: bool,
    iso: coding_iso2022::EncoderState,
}

impl Encoder {
    pub(crate) fn encode(
        &mut self,
        i: &mut Interp,
        name: &str,
        text: Value,
    ) -> Result<Vec<u8>, Flow> {
        if self.name != name {
            *self = Self::default();
            self.name = name.into();
        }
        let requested = symbol(i, name);
        let multibyte = strfn::string_multibyte_p(i, &text);
        let coding = if multibyte {
            requested
        } else {
            misc::raw_file_coding(i, &requested)?
        };
        if !multibyte {
            // Switching to raw text resets the original encoder, including
            // on an empty unibyte send. A later multibyte send starts fresh.
            self.iso = Default::default();
            self.signature_written = false;
        }
        let encoded = misc::encode_process_text(
            i,
            vec![text, coding].into(),
            multibyte.then_some(&mut self.iso),
            self.signature_written,
        )?;
        self.signature_written |= multibyte;
        Ok(misc::lisp_string_bytes(i, &encoded))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn large_stack(test: impl FnOnce() + Send + 'static) {
        std::thread::Builder::new()
            .stack_size(64 << 20)
            .spawn(test)
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn every_byte_boundary_preserves_text_and_bounds_pending_input() {
        large_stack(|| {
            let mut i = Interp::new();
            let cases: &[(&str, &[u8], &str)] = &[
                (
                    "utf-8-dos",
                    b"A\xc3\xa9\xe3\x81\x82\xf0\x9f\x98\x80\r\nZ",
                    "Aéあ😀\nZ",
                ),
                (
                    "utf-16le-with-signature-dos",
                    b"\xff\xfeA\0\xe9\0\x3d\xd8\0\xde\r\0\n\0Z\0",
                    "Aé😀\nZ",
                ),
                ("japanese-shift-jis", b"A\x82\xa0Z", "AあZ"),
                ("iso-2022-jp", b"A\x1b$B$\x22$$\x1b(BZ", "AあいZ"),
            ];
            for &(coding, input, expected) in cases {
                for size in 1..=input.len() {
                    let mut decoder = Decoder::default();
                    let mut name = coding.to_owned();
                    let mut output = String::new();
                    for chunk in input.chunks(size) {
                        let (value, used) =
                            decoder.decode(&mut i, &name, chunk, false, false).unwrap();
                        name = used;
                        if let Value::Str(text) = value {
                            output.push_str(&text.borrow().to_unicode_lossy());
                        }
                        assert!(
                            decoder.pending.len() <= 8,
                            "unbounded partial input for {coding}"
                        );
                    }
                    let (value, _) = decoder.decode(&mut i, &name, &[], true, false).unwrap();
                    if let Value::Str(text) = value {
                        output.push_str(&text.borrow().to_unicode_lossy());
                    }
                    assert_eq!(output, expected, "{coding}, chunk size {size}");
                    assert!(!decoder.has_pending());
                }
            }
        });
    }

    #[test]
    fn encoder_keeps_state_until_a_raw_send_resets_it() {
        large_stack(|| {
            let mut i = Interp::new();
            let mut encoder = Encoder::default();
            let name = "iso-2022-jp-unix";
            let mut bytes = encoder.encode(&mut i, name, Value::string("あ")).unwrap();
            bytes.extend(encoder.encode(&mut i, name, Value::string("い")).unwrap());
            assert_eq!(bytes, b"\x1b$B$\x22$$");
            bytes = encoder.encode(&mut i, name, Value::string("X")).unwrap();
            bytes.extend(encoder.encode(&mut i, name, Value::string("あ")).unwrap());
            assert_eq!(bytes, b"X\x1b$B$\x22");

            let mut encoder = Encoder::default();
            let name = "utf-16le-with-signature-unix";
            let mut bytes = encoder.encode(&mut i, name, Value::string("é")).unwrap();
            bytes.extend(encoder.encode(&mut i, name, Value::string("é")).unwrap());
            assert_eq!(bytes, b"\xff\xfe\xe9\0\xe9\0");
            assert!(
                encoder
                    .encode(&mut i, name, Value::string(""))
                    .unwrap()
                    .is_empty()
            );
            assert_eq!(
                encoder.encode(&mut i, name, Value::string("é")).unwrap(),
                b"\xff\xfe\xe9\0"
            );
        });
    }
}
