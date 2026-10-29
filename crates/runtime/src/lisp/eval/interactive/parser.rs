//! Streaming scan of interactive strings, without Lisp evaluation or input.
//! Keep unknown control letters intact: the executor formats their prompts
//! before deciding whether the letter is valid.

use crate::lisp::text::{EmacsChar, EmacsText};

pub(super) enum SpecPart {
    Reserved,
    Writable,
    Ignored,
    ShiftSelection,
    Argument { code: EmacsChar, prompt: EmacsText },
}

pub(super) struct SpecReader {
    chars: Vec<EmacsChar>,
    position: usize,
}

impl SpecReader {
    pub fn new(chars: Vec<EmacsChar>) -> Self {
        Self { chars, position: 0 }
    }

    pub fn next(&mut self) -> Option<SpecPart> {
        let code = *self.chars.get(self.position)?;
        let initial = self.position == 0;
        self.position += 1;
        if initial {
            match code.unicode().unwrap_or('\u{fffd}') {
                '+' => return Some(SpecPart::Reserved),
                '*' => return Some(SpecPart::Writable),
                '-' | '@' => return Some(SpecPart::Ignored),
                '^' => return Some(SpecPart::ShiftSelection),
                _ => {}
            }
        }
        let mut prompt = EmacsText::new();
        while self.position < self.chars.len() && self.chars[self.position] != '\n' {
            prompt.push(self.chars[self.position]);
            self.position += 1;
        }
        if self.position < self.chars.len() {
            self.position += 1;
        }
        Some(SpecPart::Argument { code, prompt })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reader(text: &str) -> SpecReader {
        SpecReader::new(text.chars().map(Into::into).collect())
    }

    #[test]
    fn only_offset_zero_is_a_prefix_directive() {
        let mut r = reader("*@ignored\npPrompt");
        assert!(matches!(r.next(), Some(SpecPart::Writable)));
        let Some(SpecPart::Argument { code, prompt }) = r.next() else {
            panic!("missing argument")
        };
        assert_eq!(code, '@');
        assert_eq!(prompt, "ignored");
        assert!(matches!(r.next(), Some(SpecPart::Argument {code, ..}) if code == 'p'));
        assert!(r.next().is_none());
    }

    #[test]
    fn prompts_keep_unicode_percent_sequences_and_empty_lines() {
        let mut r = reader("i日本語 %s\n\np");
        let Some(SpecPart::Argument { code, prompt }) = r.next() else {
            panic!("missing argument")
        };
        assert_eq!(code, 'i');
        assert_eq!(prompt, "日本語 %s");
        assert!(
            matches!(r.next(), Some(SpecPart::Argument {code, prompt}) if code == '\n' && prompt == "p")
        );
        assert!(r.next().is_none());
    }

    #[test]
    fn reserved_prefix_and_unknown_letters_remain_distinct() {
        assert!(matches!(
            reader("+ignored").next(),
            Some(SpecPart::Reserved)
        ));
        assert!(
            matches!(reader("~%s").next(), Some(SpecPart::Argument {code, prompt}) if code == '~' && prompt == "%s")
        );
    }
}
