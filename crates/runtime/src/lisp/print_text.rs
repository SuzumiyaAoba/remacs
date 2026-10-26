//! Transient printer output. Instructions are never encoded as Unicode characters.
use super::text::{EmacsChar, EmacsText, TextSource};
use std::{fmt, ops::Range};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputToken {
    Character(EmacsChar),
    PrintedByte(u8),
    WrittenByte(u8),
    RawByte(u8),
}
impl OutputToken {
    pub fn function_code(self) -> i128 {
        match self {
            Self::Character(c) => c.byte8().map_or(c.code() as i128, i128::from),
            Self::PrintedByte(b) | Self::WrittenByte(b) => {
                if b < 128 {
                    b as i128
                } else {
                    0x3fff00 + b as i128
                }
            }
            Self::RawByte(b) => b as i128,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PrintText {
    tokens: Vec<OutputToken>,
}
impl PrintText {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn len(&self) -> usize {
        self.tokens.len()
    }
    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }
    pub fn tokens(&self) -> impl Iterator<Item = OutputToken> + '_ {
        self.tokens.iter().copied()
    }
    pub fn push(&mut self, c: impl Into<EmacsChar>) {
        self.tokens.push(OutputToken::Character(c.into()));
    }
    pub fn push_printed_byte(&mut self, byte: u8) {
        self.tokens.push(OutputToken::PrintedByte(byte));
    }
    pub fn push_written_byte(&mut self, byte: u8) {
        self.tokens.push(OutputToken::WrittenByte(byte));
    }
    pub fn push_raw_byte(&mut self, byte: u8) {
        self.tokens.push(OutputToken::RawByte(byte));
    }
    pub fn push_str<T: TextSource + ?Sized>(&mut self, text: &T) {
        text.with_text(|text| self.tokens.extend(text.chars().map(OutputToken::Character)));
    }
    pub fn append(&mut self, other: &Self) {
        self.tokens.extend_from_slice(&other.tokens);
    }
    pub fn replace_range<T: TextSource + ?Sized>(&mut self, range: Range<usize>, replacement: &T) {
        replacement.with_text(|text| {
            self.tokens
                .splice(range, text.chars().map(OutputToken::Character));
        });
    }
    pub fn to_text(&self, multibyte: bool) -> EmacsText {
        let mut text = EmacsText::with_capacity(self.len());
        for token in self.tokens() {
            match token {
                OutputToken::Character(c) => text.push(c),
                OutputToken::PrintedByte(b) if multibyte => text.push_str(format!("\\{b:03o}")),
                OutputToken::PrintedByte(b)
                | OutputToken::WrittenByte(b)
                | OutputToken::RawByte(b) => {
                    text.push(if b < 128 {
                        EmacsChar::from(b as char)
                    } else {
                        EmacsChar::new(0x3fff00 + b as u32).unwrap()
                    });
                }
            }
        }
        text
    }
    pub fn last_character(&self) -> Option<EmacsChar> {
        self.tokens.last().map(|token| match *token {
            OutputToken::Character(c) => c,
            OutputToken::PrintedByte(b) | OutputToken::WrittenByte(b) | OutputToken::RawByte(b) => {
                if b < 128 {
                    EmacsChar::from(b as char)
                } else {
                    EmacsChar::new(0x3fff00 + b as u32).unwrap()
                }
            }
        })
    }
    pub fn span_is_unreadable(&self, start: usize) -> bool {
        let span = &self.tokens[start..];
        span.len() > 2
            && span[0] == OutputToken::Character('#'.into())
            && span[1] == OutputToken::Character('<'.into())
            && span.last() == Some(&OutputToken::Character('>'.into()))
    }
    pub fn stdout_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        for token in self.tokens() {
            match token {
                OutputToken::RawByte(b) => bytes.push(b),
                OutputToken::Character(c) => c.encode_internal(&mut bytes),
                OutputToken::PrintedByte(b) | OutputToken::WrittenByte(b) => {
                    if b < 128 {
                        bytes.push(b);
                    } else {
                        EmacsChar::new(0x3fff00 + b as u32)
                            .unwrap()
                            .encode_internal(&mut bytes);
                    }
                }
            }
        }
        bytes
    }
}
impl fmt::Write for PrintText {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        self.push_str(text);
        Ok(())
    }
}
impl fmt::Display for PrintText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.to_text(true), f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn private_use_characters_never_become_output_instructions() {
        let source =
            EmacsText::from_codes([0xf0080, 0xf0a80, 0xf0c80, 0xf0d80, 0xf0100, 0x110000]).unwrap();
        let mut output = PrintText::new();
        output.push_str(&source);
        assert_eq!(output.to_text(true), source);
        assert_eq!(output.to_text(false), source);
        assert_eq!(
            output
                .tokens()
                .map(OutputToken::function_code)
                .collect::<Vec<_>>(),
            source.codes().map(i128::from).collect::<Vec<_>>()
        );
        assert_eq!(
            output.stdout_bytes(),
            source.to_internal_bytes(true).unwrap()
        );
    }
    #[test]
    fn sinks_apply_byte_instructions_without_changing_literal_characters() {
        let mut output = PrintText::new();
        output.push('\u{f0c80}');
        output.push_printed_byte(0x80);
        output.push_written_byte(0x81);
        output.push_raw_byte(0x82);
        output.push(EmacsChar::new(0x3fff83).unwrap());
        assert_eq!(
            output
                .tokens()
                .map(OutputToken::function_code)
                .collect::<Vec<_>>(),
            [0xf0c80, 0x3fff80, 0x3fff81, 0x82, 0x83]
        );
        let text = output.to_text(true);
        assert_eq!(
            text.codes().collect::<Vec<_>>(),
            [0xf0c80, 92, 50, 48, 48, 0x3fff81, 0x3fff82, 0x3fff83]
        );
        assert_eq!(
            output.to_text(false).codes().collect::<Vec<_>>(),
            [0xf0c80, 0x3fff80, 0x3fff81, 0x3fff82, 0x3fff83]
        );
        assert_eq!(
            output.stdout_bytes(),
            [
                0xf3, 0xb0, 0xb2, 0x80, 0xc0, 0x80, 0xc0, 0x81, 0x82, 0xc0, 0x83
            ]
        );
    }
    #[test]
    fn callback_splicing_uses_character_positions_and_keeps_other_tokens() {
        let mut output = PrintText::new();
        output.push('\u{f0080}');
        output.push_str("#<object>");
        output.push_written_byte(0xff);
        let replacement = EmacsText::from_codes([0xd800, 0x200000]).unwrap();
        output.replace_range(1..10, &replacement);
        assert_eq!(
            output.tokens().collect::<Vec<_>>(),
            [
                OutputToken::Character(EmacsChar::from('\u{f0080}')),
                OutputToken::Character(EmacsChar::new(0xd800).unwrap()),
                OutputToken::Character(EmacsChar::new(0x200000).unwrap()),
                OutputToken::WrittenByte(0xff)
            ]
        );
    }
}

pub trait OutputSource {
    fn with_output<R>(&self, f: impl FnOnce(&PrintText) -> R) -> R;
}
impl OutputSource for PrintText {
    fn with_output<R>(&self, f: impl FnOnce(&PrintText) -> R) -> R {
        f(self)
    }
}
macro_rules! text_output_source {
    ($($ty:ty),*) => { $(impl OutputSource for $ty {
        fn with_output<R>(&self, f: impl FnOnce(&PrintText) -> R) -> R {
            let mut output = PrintText::new(); output.push_str(self); f(&output)
        }
    })* };
}
text_output_source!(
    str,
    String,
    super::text::EmacsStr,
    EmacsText,
    std::cell::Ref<'_, EmacsText>
);
impl<T: OutputSource + ?Sized> OutputSource for &T {
    fn with_output<R>(&self, f: impl FnOnce(&PrintText) -> R) -> R {
        (**self).with_output(f)
    }
}

impl From<char> for OutputToken {
    fn from(c: char) -> Self {
        Self::Character(c.into())
    }
}
impl From<EmacsChar> for OutputToken {
    fn from(c: EmacsChar) -> Self {
        Self::Character(c)
    }
}
