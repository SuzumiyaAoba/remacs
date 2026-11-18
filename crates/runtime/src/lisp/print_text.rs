//! Transient printer output. Instructions are never encoded as Unicode characters.
use super::text::{EmacsChar, EmacsText, TextSource};
use super::value::HashKey;
use std::{fmt, ops::Range};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputToken {
    Character(EmacsChar),
    PrintedByte(u8),
    WrittenByte(u8),
    RawByte(u8),
}

/// A print-number-table transition whose output prefix ends after this token.
/// Function output streams can fail while consuming the prefix; in that case
/// GNU leaves the corresponding negative table entry unchanged.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PrintNumberEvent {
    pub after_token: usize,
    pub identity: usize,
    pub number: usize,
    pub entry_key: Option<HashKey>,
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
    number_events: Vec<PrintNumberEvent>,
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
    pub(crate) fn push_token(&mut self, token: OutputToken) {
        self.tokens.push(token);
    }
    pub(crate) fn push_number_event(
        &mut self,
        identity: usize,
        number: usize,
        entry_key: Option<HashKey>,
    ) {
        self.number_events.push(PrintNumberEvent {
            after_token: self.len(),
            identity,
            number,
            entry_key,
        });
    }
    pub(crate) fn number_events(&self) -> &[PrintNumberEvent] {
        &self.number_events
    }
    pub(crate) fn previously_numbered(&self, key: &HashKey) -> Option<usize> {
        self.number_events
            .iter()
            .rev()
            .find(|event| event.entry_key.as_ref() == Some(key))
            .map(|event| event.number)
    }
    pub(crate) fn slice_tokens(&self, range: Range<usize>) -> Self {
        Self {
            tokens: self.tokens[range].to_vec(),
            number_events: Vec::new(),
        }
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
        let offset = self.len();
        self.tokens.extend_from_slice(&other.tokens);
        self.number_events
            .extend(other.number_events.iter().cloned().map(|mut event| {
                event.after_token += offset;
                event
            }));
    }
    /// Emulate GNU print.c's shared output buffer when `print' recurs from
    /// `print-unreadable-function': nested output replaces the buffer, then
    /// the saved prefix is copied back without restoring the nested cursor.
    pub(crate) fn restore_recursive_print(&mut self, nested: &Self, saved: &Self) {
        self.tokens.clone_from(&nested.tokens);
        let copied = self.tokens.len().min(saved.tokens.len());
        self.tokens[..copied].copy_from_slice(&saved.tokens[..copied]);
        self.number_events = saved
            .number_events
            .iter()
            .filter(|event| event.after_token <= copied)
            .cloned()
            .collect();
    }
    pub fn replace_range<T: TextSource + ?Sized>(&mut self, range: Range<usize>, replacement: &T) {
        replacement.with_text(|text| {
            let start = range.start;
            let end = range.end;
            let old_len = end - start;
            let new_len = text.chars().count();
            self.tokens
                .splice(range, text.chars().map(OutputToken::Character));
            self.number_events.retain_mut(|event| {
                if event.after_token <= start {
                    true
                } else if event.after_token >= end {
                    event.after_token = if new_len >= old_len {
                        event.after_token + (new_len - old_len)
                    } else {
                        event.after_token - (old_len - new_len)
                    };
                    true
                } else {
                    false
                }
            });
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
