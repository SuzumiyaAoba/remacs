//! Logical Emacs characters and text, without Unicode proxy code points.
//!
//! Emacs accepts 22-bit characters, including surrogates and byte8 codes.
//! Keep those identities as integers; Rust Unicode is an external interface.

use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct EmacsChar(u32);

impl EmacsChar {
    pub const MAX: u32 = 0x3fffff;

    pub const fn new(code: u32) -> Option<Self> {
        if code <= Self::MAX {
            Some(Self(code))
        } else {
            None
        }
    }

    pub const fn code(self) -> u32 {
        self.0
    }

    pub const fn byte8(self) -> Option<u8> {
        if self.0 >= 0x3fff80 {
            Some(self.0 as u8)
        } else {
            None
        }
    }

    pub fn unicode(self) -> Option<char> {
        char::from_u32(self.0)
    }

    pub const fn storage_len(self) -> usize {
        match self.0 {
            0..=0x7f => 1,
            0x80..=0x7ff | 0x3fff80..=0x3fffff => 2,
            0x800..=0xffff => 3,
            0x10000..=0x1fffff => 4,
            _ => 5,
        }
    }

    pub fn encode_internal(self, output: &mut Vec<u8>) {
        let code = self.0;
        if let Some(byte) = self.byte8() {
            output.extend([0xc0 | ((byte >> 6) & 1), 0x80 | (byte & 0x3f)]);
            return;
        }
        let width = self.storage_len();
        if width == 1 {
            output.push(code as u8);
            return;
        }
        let prefix = match width {
            2 => 0xc0,
            3 => 0xe0,
            4 => 0xf0,
            _ => 0xf8,
        };
        output.push(prefix | (code >> (6 * (width - 1))) as u8);
        for shift in (0..width - 1).rev() {
            output.push(0x80 | ((code >> (6 * shift)) & 0x3f) as u8);
        }
    }

    pub fn decode_internal(bytes: &[u8]) -> Option<(Self, usize)> {
        let first = *bytes.first()?;
        if first < 0x80 {
            return Some((Self(u32::from(first)), 1));
        }
        if first == 0xc0 || first == 0xc1 {
            let second = *bytes.get(1)?;
            if second & 0xc0 != 0x80 {
                return None;
            }
            return Some((
                Self(0x3fff80 + (u32::from(first & 1) << 6) + u32::from(second & 0x3f)),
                2,
            ));
        }
        let (width, mask) = match first {
            0xc2..=0xdf => (2, 0x1f),
            0xe0..=0xef => (3, 0x0f),
            0xf0..=0xf7 => (4, 0x07),
            0xf8 => (5, 0),
            _ => return None,
        };
        if bytes.len() < width {
            return None;
        }
        let mut code = u32::from(first & mask);
        for byte in &bytes[1..width] {
            if byte & 0xc0 != 0x80 {
                return None;
            }
            code = (code << 6) | u32::from(byte & 0x3f);
        }
        let character = Self::new(code)?;
        if character.byte8().is_some() || character.storage_len() != width {
            return None;
        }
        Some((character, width))
    }
}

impl EmacsChar {
    pub fn to_string(self) -> EmacsText {
        EmacsText {
            characters: vec![self],
        }
    }
    pub fn is_ascii(self) -> bool {
        self.code() < 128
    }
    pub fn is_ascii_digit(self) -> bool {
        self.unicode().is_some_and(|c| c.is_ascii_digit())
    }
    pub fn is_ascii_alphabetic(self) -> bool {
        self.unicode().is_some_and(|c| c.is_ascii_alphabetic())
    }
    pub fn is_ascii_alphanumeric(self) -> bool {
        self.unicode().is_some_and(|c| c.is_ascii_alphanumeric())
    }
    pub fn is_ascii_hexdigit(self) -> bool {
        self.unicode().is_some_and(|c| c.is_ascii_hexdigit())
    }
    pub fn is_ascii_whitespace(self) -> bool {
        self.unicode().is_some_and(|c| c.is_ascii_whitespace())
    }
    pub fn is_whitespace(self) -> bool {
        self.unicode().is_some_and(char::is_whitespace)
    }
    pub fn is_control(self) -> bool {
        self.unicode().is_some_and(char::is_control)
    }
    pub fn is_alphanumeric(self) -> bool {
        self.unicode().is_some_and(char::is_alphanumeric)
    }
    pub fn is_alphabetic(self) -> bool {
        self.unicode().is_some_and(char::is_alphabetic)
    }
    pub fn is_uppercase(self) -> bool {
        self.unicode().is_some_and(char::is_uppercase)
    }
    pub fn is_lowercase(self) -> bool {
        self.unicode().is_some_and(char::is_lowercase)
    }
    pub fn to_digit(self, radix: u32) -> Option<u32> {
        self.unicode().and_then(|c| c.to_digit(radix))
    }
    pub fn len_utf8(self) -> usize {
        self.storage_len()
    }
    pub fn to_ascii_lowercase(self) -> Self {
        self.unicode()
            .map(|c| Self::from(c.to_ascii_lowercase()))
            .unwrap_or(self)
    }
    pub fn to_ascii_uppercase(self) -> Self {
        self.unicode()
            .map(|c| Self::from(c.to_ascii_uppercase()))
            .unwrap_or(self)
    }
    pub fn to_lowercase(self) -> std::vec::IntoIter<Self> {
        self.unicode()
            .map(|c| c.to_lowercase().map(Self::from).collect::<Vec<_>>())
            .unwrap_or_else(|| vec![self])
            .into_iter()
    }
    pub fn to_uppercase(self) -> std::vec::IntoIter<Self> {
        self.unicode()
            .map(|c| c.to_uppercase().map(Self::from).collect::<Vec<_>>())
            .unwrap_or_else(|| vec![self])
            .into_iter()
    }
}
impl PartialEq<char> for EmacsChar {
    fn eq(&self, other: &char) -> bool {
        self.code() == *other as u32
    }
}
impl PartialEq<EmacsChar> for char {
    fn eq(&self, other: &EmacsChar) -> bool {
        *self as u32 == other.code()
    }
}
impl PartialOrd<char> for EmacsChar {
    fn partial_cmp(&self, other: &char) -> Option<std::cmp::Ordering> {
        self.code().partial_cmp(&(*other as u32))
    }
}

impl From<char> for EmacsChar {
    fn from(character: char) -> Self {
        Self(character as u32)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct EmacsText {
    characters: Vec<EmacsChar>,
}

/// Borrowed logical text. Ranges are character indices, never UTF-8 byte indices.
#[repr(transparent)]
#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct EmacsStr([EmacsChar]);

impl EmacsStr {
    fn from_slice(characters: &[EmacsChar]) -> &Self {
        // repr(transparent) preserves the slice pointer and length.
        unsafe { &*(characters as *const [EmacsChar] as *const Self) }
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn chars(
        &self,
    ) -> impl DoubleEndedIterator<Item = EmacsChar> + ExactSizeIterator + Clone + '_ {
        self.0.iter().copied()
    }
    pub fn char_indices(
        &self,
    ) -> impl DoubleEndedIterator<Item = (usize, EmacsChar)> + ExactSizeIterator + Clone + '_ {
        self.chars().enumerate()
    }
    pub fn is_ascii(&self) -> bool {
        self.chars().all(EmacsChar::is_ascii)
    }
    pub fn as_str(&self) -> &Self {
        self
    }
    pub fn to_string(&self) -> EmacsText {
        self.to_owned()
    }
    pub fn to_unicode_lossy(&self) -> String {
        self.chars()
            .map(|c| c.unicode().unwrap_or('\u{fffd}'))
            .collect()
    }
    pub fn starts_with(&self, pattern: impl TextPattern) -> bool {
        self.0.starts_with(&pattern.characters())
    }
    pub fn ends_with(&self, pattern: impl TextPattern) -> bool {
        self.0.ends_with(&pattern.characters())
    }
    pub fn find(&self, pattern: impl TextPattern) -> Option<usize> {
        let pattern = pattern.characters();
        if pattern.is_empty() {
            return Some(0);
        }
        self.0
            .windows(pattern.len())
            .position(|window| window == pattern)
    }
    pub fn rfind(&self, pattern: impl TextPattern) -> Option<usize> {
        let pattern = pattern.characters();
        if pattern.is_empty() {
            return Some(self.len());
        }
        self.0
            .windows(pattern.len())
            .rposition(|window| window == pattern)
    }
    pub fn contains(&self, pattern: impl TextPattern) -> bool {
        self.find(pattern).is_some()
    }
    pub fn strip_prefix(&self, pattern: impl TextPattern) -> Option<&Self> {
        let pattern = pattern.characters();
        self.0
            .starts_with(&pattern)
            .then(|| Self::from_slice(&self.0[pattern.len()..]))
    }
    pub fn strip_suffix(&self, pattern: impl TextPattern) -> Option<&Self> {
        let pattern = pattern.characters();
        self.0
            .ends_with(&pattern)
            .then(|| Self::from_slice(&self.0[..self.len() - pattern.len()]))
    }
    pub fn trim_start(&self) -> &Self {
        let start = self
            .chars()
            .position(|c| !c.is_whitespace())
            .unwrap_or(self.len());
        Self::from_slice(&self.0[start..])
    }
    pub fn trim_end(&self) -> &Self {
        let end = self
            .chars()
            .rposition(|c| !c.is_whitespace())
            .map_or(0, |p| p + 1);
        Self::from_slice(&self.0[..end])
    }
    pub fn trim(&self) -> &Self {
        self.trim_start().trim_end()
    }
    pub fn to_lowercase(&self) -> EmacsText {
        self.chars().flat_map(EmacsChar::to_lowercase).collect()
    }
    pub fn to_uppercase(&self) -> EmacsText {
        self.chars().flat_map(EmacsChar::to_uppercase).collect()
    }
    pub fn split_once(&self, pattern: impl TextPattern) -> Option<(&Self, &Self)> {
        let pattern = pattern.characters();
        let position = self.find(pattern.as_slice())?;
        Some((
            Self::from_slice(&self.0[..position]),
            Self::from_slice(&self.0[position + pattern.len()..]),
        ))
    }
    pub fn rsplit_once(&self, pattern: impl TextPattern) -> Option<(&Self, &Self)> {
        let pattern = pattern.characters();
        let position = self.rfind(pattern.as_slice())?;
        Some((
            Self::from_slice(&self.0[..position]),
            Self::from_slice(&self.0[position + pattern.len()..]),
        ))
    }
}

pub trait TextPattern {
    fn characters(&self) -> Vec<EmacsChar>;
}
impl TextPattern for str {
    fn characters(&self) -> Vec<EmacsChar> {
        self.chars().map(EmacsChar::from).collect()
    }
}
impl TextPattern for String {
    fn characters(&self) -> Vec<EmacsChar> {
        self.as_str().characters()
    }
}
impl TextPattern for char {
    fn characters(&self) -> Vec<EmacsChar> {
        vec![(*self).into()]
    }
}
impl TextPattern for EmacsChar {
    fn characters(&self) -> Vec<EmacsChar> {
        vec![*self]
    }
}
impl TextPattern for EmacsStr {
    fn characters(&self) -> Vec<EmacsChar> {
        self.0.to_vec()
    }
}
impl TextPattern for EmacsText {
    fn characters(&self) -> Vec<EmacsChar> {
        self.characters.clone()
    }
}
impl TextPattern for [EmacsChar] {
    fn characters(&self) -> Vec<EmacsChar> {
        self.to_vec()
    }
}
impl TextPattern for std::cell::Ref<'_, EmacsText> {
    fn characters(&self) -> Vec<EmacsChar> {
        self.chars().collect()
    }
}
impl<T: TextPattern + ?Sized> TextPattern for &T {
    fn characters(&self) -> Vec<EmacsChar> {
        (**self).characters()
    }
}
impl std::ops::Deref for EmacsText {
    type Target = EmacsStr;
    fn deref(&self) -> &EmacsStr {
        EmacsStr::from_slice(&self.characters)
    }
}
impl std::borrow::ToOwned for EmacsStr {
    type Owned = EmacsText;
    fn to_owned(&self) -> EmacsText {
        EmacsText {
            characters: self.0.to_vec(),
        }
    }
}
impl std::borrow::Borrow<EmacsStr> for EmacsText {
    fn borrow(&self) -> &EmacsStr {
        self
    }
}
impl<I: std::slice::SliceIndex<[EmacsChar], Output = [EmacsChar]>> std::ops::Index<I> for EmacsStr {
    type Output = EmacsStr;
    fn index(&self, index: I) -> &EmacsStr {
        EmacsStr::from_slice(&self.0[index])
    }
}
impl From<&EmacsStr> for EmacsText {
    fn from(text: &EmacsStr) -> Self {
        text.to_owned()
    }
}
impl From<&EmacsText> for EmacsText {
    fn from(text: &EmacsText) -> Self {
        text.clone()
    }
}
impl From<&String> for EmacsText {
    fn from(text: &String) -> Self {
        text.as_str().into()
    }
}
impl PartialEq<str> for EmacsText {
    fn eq(&self, other: &str) -> bool {
        self.chars().eq(other.chars().map(EmacsChar::from))
    }
}
impl PartialEq<&str> for EmacsText {
    fn eq(&self, other: &&str) -> bool {
        self == *other
    }
}
impl PartialEq<String> for EmacsText {
    fn eq(&self, other: &String) -> bool {
        self == other.as_str()
    }
}
impl EmacsText {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            characters: Vec::with_capacity(capacity),
        }
    }
    pub fn capacity(&self) -> usize {
        self.characters.capacity() * std::mem::size_of::<EmacsChar>()
    }
    pub fn reserve(&mut self, additional: usize) {
        self.characters.reserve(additional);
    }
    pub fn clear(&mut self) {
        self.characters.clear();
    }
    pub fn truncate(&mut self, len: usize) {
        self.characters.truncate(len);
    }
    pub fn push_str(&mut self, text: impl TextPattern) {
        self.characters.extend(text.characters());
    }
    pub fn to_string(&self) -> Self {
        self.clone()
    }
}
impl fmt::Display for EmacsStr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for c in self.chars() {
            if let Some(c) = c.unicode() {
                write!(f, "{c}")?;
            } else {
                write!(f, "\\x{{{:x}}}", c.code())?;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidCharacter(pub u32);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidEncoding(pub usize);

impl EmacsText {
    pub fn from_codes(codes: impl IntoIterator<Item = u32>) -> Result<Self, InvalidCharacter> {
        let characters = codes
            .into_iter()
            .map(|code| EmacsChar::new(code).ok_or(InvalidCharacter(code)))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { characters })
    }

    pub fn from_internal_bytes(bytes: &[u8], multibyte: bool) -> Result<Self, InvalidEncoding> {
        if !multibyte {
            return Ok(Self {
                characters: bytes
                    .iter()
                    .map(|&byte| {
                        EmacsChar(if byte < 128 {
                            u32::from(byte)
                        } else {
                            0x3fff00 + u32::from(byte)
                        })
                    })
                    .collect(),
            });
        }
        let mut result = Self::default();
        let mut offset = 0;
        while offset < bytes.len() {
            let (character, width) =
                EmacsChar::decode_internal(&bytes[offset..]).ok_or(InvalidEncoding(offset))?;
            result.characters.push(character);
            offset += width;
        }
        Ok(result)
    }

    pub fn to_internal_bytes(&self, multibyte: bool) -> Result<Vec<u8>, InvalidCharacter> {
        let mut output = Vec::new();
        for character in &self.characters {
            if multibyte {
                character.encode_internal(&mut output);
            } else if let Some(byte) = character.byte8() {
                output.push(byte);
            } else if character.code() < 256 {
                output.push(character.code() as u8);
            } else {
                return Err(InvalidCharacter(character.code()));
            }
        }
        Ok(output)
    }

    pub fn len(&self) -> usize {
        self.characters.len()
    }
    pub fn is_empty(&self) -> bool {
        self.characters.is_empty()
    }
    pub fn chars(&self) -> impl DoubleEndedIterator<Item = EmacsChar> + ExactSizeIterator + '_ {
        self.characters.iter().copied()
    }
    pub fn codes(&self) -> impl DoubleEndedIterator<Item = u32> + ExactSizeIterator + '_ {
        self.characters.iter().map(|character| character.code())
    }
    pub fn push(&mut self, character: impl Into<EmacsChar>) {
        self.characters.push(character.into());
    }
    pub fn slice(&self, range: std::ops::Range<usize>) -> Option<Self> {
        self.characters.get(range).map(|characters| Self {
            characters: characters.to_vec(),
        })
    }
    pub fn append(&mut self, text: &Self) {
        self.characters.extend_from_slice(&text.characters);
    }
    pub fn to_unicode_lossy(&self) -> String {
        self.chars()
            .map(|character| character.unicode().unwrap_or('\u{fffd}'))
            .collect()
    }
}

impl From<&str> for EmacsText {
    fn from(text: &str) -> Self {
        Self {
            characters: text.chars().map(EmacsChar::from).collect(),
        }
    }
}
impl From<String> for EmacsText {
    fn from(text: String) -> Self {
        Self::from(text.as_str())
    }
}
impl FromIterator<EmacsChar> for EmacsText {
    fn from_iter<T: IntoIterator<Item = EmacsChar>>(iter: T) -> Self {
        Self {
            characters: iter.into_iter().collect(),
        }
    }
}
impl<'a> FromIterator<&'a EmacsChar> for EmacsText {
    fn from_iter<T: IntoIterator<Item = &'a EmacsChar>>(iter: T) -> Self {
        iter.into_iter().copied().collect()
    }
}
impl FromIterator<char> for EmacsText {
    fn from_iter<T: IntoIterator<Item = char>>(iter: T) -> Self {
        iter.into_iter().map(EmacsChar::from).collect()
    }
}
impl fmt::Display for EmacsText {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for character in self.chars() {
            if let Some(character) = character.unicode() {
                write!(formatter, "{character}")?;
            } else {
                write!(formatter, "\\x{{{:x}}}", character.code())?;
            }
        }
        Ok(())
    }
}

impl EmacsStr {
    pub fn split(&self, pattern: impl TextPattern) -> std::vec::IntoIter<&Self> {
        self.splitn(usize::MAX, pattern)
    }
    pub fn splitn(&self, count: usize, pattern: impl TextPattern) -> std::vec::IntoIter<&Self> {
        let pattern = pattern.characters();
        if count == 1 {
            return vec![self].into_iter();
        }
        if count == 0 {
            return Vec::new().into_iter();
        }
        let mut result = Vec::new();
        let mut remaining = self;
        if pattern.is_empty() {
            result.push(Self::from_slice(&self.0[..0]));
            for at in 0..self.len() {
                if result.len() + 1 >= count {
                    break;
                }
                result.push(Self::from_slice(&self.0[at..at + 1]));
                remaining = Self::from_slice(&self.0[at + 1..]);
            }
        } else {
            while result.len() + 1 < count {
                let Some(at) = remaining.find(pattern.as_slice()) else {
                    break;
                };
                result.push(Self::from_slice(&remaining.0[..at]));
                remaining = Self::from_slice(&remaining.0[at + pattern.len()..]);
            }
        }
        if result.len() < count {
            result.push(remaining);
        }
        result.into_iter()
    }
    pub fn rsplit(&self, pattern: impl TextPattern) -> std::vec::IntoIter<&Self> {
        let pattern = pattern.characters();
        if pattern.is_empty() {
            return self
                .split(pattern.as_slice())
                .rev()
                .collect::<Vec<_>>()
                .into_iter();
        }
        let mut result = Vec::new();
        let mut remaining = self;
        while let Some(at) = remaining.rfind(pattern.as_slice()) {
            result.push(Self::from_slice(&remaining.0[at + pattern.len()..]));
            remaining = Self::from_slice(&remaining.0[..at]);
        }
        result.push(remaining);
        result.into_iter()
    }
    pub fn split_at(&self, at: usize) -> (&Self, &Self) {
        (
            Self::from_slice(&self.0[..at]),
            Self::from_slice(&self.0[at..]),
        )
    }
    pub fn trim_start_matches(&self, pattern: impl TextPattern) -> &Self {
        let pattern = pattern.characters();
        let mut result = self;
        if !pattern.is_empty() {
            while let Some(rest) = result.strip_prefix(pattern.as_slice()) {
                result = rest;
            }
        }
        result
    }
    pub fn trim_end_matches(&self, pattern: impl TextPattern) -> &Self {
        let pattern = pattern.characters();
        let mut result = self;
        if !pattern.is_empty() {
            while let Some(rest) = result.strip_suffix(pattern.as_slice()) {
                result = rest;
            }
        }
        result
    }
    pub fn trim_matches(&self, pattern: impl TextPattern) -> &Self {
        let pattern = pattern.characters();
        self.trim_start_matches(pattern.as_slice())
            .trim_end_matches(pattern.as_slice())
    }
    pub fn replace(&self, pattern: impl TextPattern, replacement: impl TextPattern) -> EmacsText {
        let parts = self.split(pattern).collect::<Vec<_>>();
        let replacement = replacement.characters();
        let mut result = EmacsText::new();
        for (index, part) in parts.into_iter().enumerate() {
            if index != 0 {
                result.characters.extend_from_slice(&replacement);
            }
            result.characters.extend_from_slice(&part.0);
        }
        result
    }
    pub fn eq_ignore_ascii_case(&self, other: impl TextPattern) -> bool {
        self.chars().map(EmacsChar::to_ascii_lowercase).eq(other
            .characters()
            .into_iter()
            .map(EmacsChar::to_ascii_lowercase))
    }
    pub fn parse<T: std::str::FromStr>(&self) -> Result<T, T::Err> {
        self.to_unicode_lossy().parse()
    }
}
impl EmacsText {
    pub fn pop(&mut self) -> Option<EmacsChar> {
        self.characters.pop()
    }
    pub fn extend<C: Into<EmacsChar>>(&mut self, characters: impl IntoIterator<Item = C>) {
        self.characters
            .extend(characters.into_iter().map(Into::into));
    }
}
impl PartialEq<str> for EmacsStr {
    fn eq(&self, other: &str) -> bool {
        self.chars().eq(other.chars().map(EmacsChar::from))
    }
}
impl PartialEq<&str> for EmacsStr {
    fn eq(&self, other: &&str) -> bool {
        self == *other
    }
}
impl PartialEq<EmacsText> for str {
    fn eq(&self, other: &EmacsText) -> bool {
        other == self
    }
}
impl PartialEq<EmacsText> for String {
    fn eq(&self, other: &EmacsText) -> bool {
        other == self.as_str()
    }
}

/// Borrow Unicode input as logical text only at the boundary that needs it.
pub trait TextSource {
    fn with_text<R>(&self, action: impl FnOnce(&EmacsStr) -> R) -> R;
}
impl TextSource for str {
    fn with_text<R>(&self, action: impl FnOnce(&EmacsStr) -> R) -> R {
        action(&EmacsText::from(self))
    }
}
impl TextSource for String {
    fn with_text<R>(&self, action: impl FnOnce(&EmacsStr) -> R) -> R {
        self.as_str().with_text(action)
    }
}
impl TextSource for EmacsText {
    fn with_text<R>(&self, action: impl FnOnce(&EmacsStr) -> R) -> R {
        action(self)
    }
}
impl TextSource for EmacsStr {
    fn with_text<R>(&self, action: impl FnOnce(&EmacsStr) -> R) -> R {
        action(self)
    }
}
impl<T: TextSource + ?Sized> TextSource for &T {
    fn with_text<R>(&self, action: impl FnOnce(&EmacsStr) -> R) -> R {
        (**self).with_text(action)
    }
}
impl TextSource for std::cell::Ref<'_, EmacsText> {
    fn with_text<R>(&self, action: impl FnOnce(&EmacsStr) -> R) -> R {
        action(self)
    }
}

pub trait CharSequence {
    fn with_chars<R>(&self, action: impl FnOnce(&[EmacsChar]) -> R) -> R;
}
impl CharSequence for [EmacsChar] {
    fn with_chars<R>(&self, action: impl FnOnce(&[EmacsChar]) -> R) -> R {
        action(self)
    }
}
impl CharSequence for Vec<EmacsChar> {
    fn with_chars<R>(&self, action: impl FnOnce(&[EmacsChar]) -> R) -> R {
        action(self)
    }
}
impl CharSequence for [char] {
    fn with_chars<R>(&self, action: impl FnOnce(&[EmacsChar]) -> R) -> R {
        action(
            &self
                .iter()
                .copied()
                .map(EmacsChar::from)
                .collect::<Vec<_>>(),
        )
    }
}
impl CharSequence for Vec<char> {
    fn with_chars<R>(&self, action: impl FnOnce(&[EmacsChar]) -> R) -> R {
        self.as_slice().with_chars(action)
    }
}
impl CharSequence for std::cell::RefMut<'_, [EmacsChar]> {
    fn with_chars<R>(&self, action: impl FnOnce(&[EmacsChar]) -> R) -> R {
        action(self)
    }
}
impl EmacsChar {
    pub fn is_ascii_punctuation(self) -> bool {
        self.unicode().is_some_and(|c| c.is_ascii_punctuation())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reserved_unicode_values_remain_distinct_from_extensions() {
        let codes = [0xf0080, 0x3fff80, 0xf0100, 0xd800, 0xf0b00, 0x110000];
        let text = EmacsText::from_codes(codes).unwrap();
        assert_eq!(text.codes().collect::<Vec<_>>(), codes);
        assert_eq!(
            text.chars().map(EmacsChar::storage_len).collect::<Vec<_>>(),
            [4, 2, 4, 3, 4, 4]
        );
        let encoded = text.to_internal_bytes(true).unwrap();
        assert_eq!(
            EmacsText::from_internal_bytes(&encoded, true).unwrap(),
            text
        );
        assert_eq!(
            EmacsText::from("\u{f0080}\u{f0100}")
                .codes()
                .collect::<Vec<_>>(),
            [0xf0080, 0xf0100]
        );
    }

    #[test]
    fn gnu_internal_byte_boundaries() {
        for (code, bytes) in [
            (0x7f, vec![0x7f]),
            (0x80, vec![0xc2, 0x80]),
            (0xd800, vec![0xed, 0xa0, 0x80]),
            (0xf0080, vec![0xf3, 0xb0, 0x82, 0x80]),
            (0x110000, vec![0xf4, 0x90, 0x80, 0x80]),
            (0x200000, vec![0xf8, 0x88, 0x80, 0x80, 0x80]),
            (0x3fff7f, vec![0xf8, 0x8f, 0xbf, 0xbd, 0xbf]),
            (0x3fff80, vec![0xc0, 0x80]),
            (0x3fffff, vec![0xc1, 0xbf]),
        ] {
            let mut encoded = Vec::new();
            EmacsChar::new(code).unwrap().encode_internal(&mut encoded);
            assert_eq!(encoded, bytes, "{code:x}");
            assert_eq!(
                EmacsChar::decode_internal(&bytes),
                Some((EmacsChar(code), bytes.len()))
            );
        }
    }

    #[test]
    fn every_emacs_character_has_a_canonical_lossless_encoding() {
        let mut bytes = Vec::with_capacity(5);
        for code in 0..=EmacsChar::MAX {
            bytes.clear();
            let character = EmacsChar::new(code).unwrap();
            character.encode_internal(&mut bytes);
            assert_eq!(bytes.len(), character.storage_len());
            assert_eq!(
                EmacsChar::decode_internal(&bytes),
                Some((character, bytes.len())),
                "{code:x}"
            );
        }
        assert_eq!(EmacsChar::new(0x400000), None);
    }

    #[test]
    fn malformed_storage_is_not_reinterpreted_as_unicode() {
        for bytes in [
            vec![0x80],
            vec![0xc0],
            vec![0xe0, 0x80, 0x80],
            vec![0xf0, 0x80, 0x80, 0x80],
            vec![0xf8, 0x80, 0x80, 0x80, 0x80],
            vec![0xf8, 0x8f, 0xbf, 0xbe, 0x80],
            vec![0xff],
        ] {
            assert_eq!(EmacsChar::decode_internal(&bytes), None);
        }
    }

    #[test]
    fn search_split_and_replacement_preserve_non_unicode_identity() {
        let original =
            EmacsText::from_codes([0xf0080, 0x3fff80, 0xd800, 0xf0100, 0x110000]).unwrap();
        assert_eq!(original.find(EmacsChar::new(0x3fff80).unwrap()), Some(1));
        assert_eq!(original.find('\u{f0080}'), Some(0));
        let parts: Vec<_> = original.split(EmacsChar::new(0xd800).unwrap()).collect();
        assert_eq!(
            parts[0].chars().map(EmacsChar::code).collect::<Vec<_>>(),
            [0xf0080, 0x3fff80]
        );
        assert_eq!(
            parts[1].chars().map(EmacsChar::code).collect::<Vec<_>>(),
            [0xf0100, 0x110000]
        );
        let replacement = EmacsText::from_codes([0x200000]).unwrap();
        let result = original.replace(EmacsChar::new(0xd800).unwrap(), &replacement);
        assert_eq!(
            result.codes().collect::<Vec<_>>(),
            [0xf0080, 0x3fff80, 0x200000, 0xf0100, 0x110000]
        );
        assert_eq!(original.splitn(1, "").next().unwrap(), &*original);
        assert_eq!(original.splitn(0, "").count(), 0);
        let pieces: Vec<_> = original.split("").collect();
        assert_eq!(pieces.len(), 7);
        assert!(pieces[0].is_empty() && pieces[6].is_empty());
        let restored: EmacsText = pieces.into_iter().flat_map(EmacsStr::chars).collect();
        assert_eq!(restored, original);
    }

    #[test]
    fn byte_storage_and_text_slices_keep_original_codes() {
        let bytes: Vec<u8> = (0..=255).collect();
        let text = EmacsText::from_internal_bytes(&bytes, false).unwrap();
        assert_eq!(text.to_internal_bytes(false).unwrap(), bytes);
        let mut joined = text.slice(127..130).unwrap();
        joined.append(&EmacsText::from_codes([0xf0080, 0x110000]).unwrap());
        assert_eq!(
            joined.codes().collect::<Vec<_>>(),
            [127, 0x3fff80, 0x3fff81, 0xf0080, 0x110000]
        );
        assert_eq!(
            joined.to_internal_bytes(false),
            Err(InvalidCharacter(0xf0080))
        );
        assert_eq!(
            EmacsText::from_codes([0x400000]),
            Err(InvalidCharacter(0x400000))
        );
    }
}

impl AsRef<EmacsStr> for EmacsText {
    fn as_ref(&self) -> &EmacsStr {
        self
    }
}

impl EmacsStr {
    /// UTF-8 filesystem/process boundary: byte8 characters are raw octets.
    /// Unlike internal string storage, this never emits C0/C1 byte8 pairs.
    pub fn to_external_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        for c in self.chars() {
            if let Some(byte) = c.byte8() {
                bytes.push(byte);
            } else {
                c.encode_internal(&mut bytes);
            }
        }
        bytes
    }

    pub fn to_os_string(&self) -> Result<std::ffi::OsString, InvalidCharacter> {
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStringExt;
            Ok(std::ffi::OsString::from_vec(self.to_external_bytes()))
        }
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStringExt;
            let mut units = Vec::new();
            for c in self.chars() {
                if let Some(unicode) = c.unicode() {
                    units.extend_from_slice(unicode.encode_utf16(&mut [0; 2]));
                } else if (0xd800..=0xdfff).contains(&c.code()) {
                    units.push(c.code() as u16);
                } else {
                    return Err(InvalidCharacter(c.code()));
                }
            }
            Ok(std::ffi::OsString::from_wide(&units))
        }
        #[cfg(not(any(unix, windows)))]
        {
            let mut text = String::new();
            for c in self.chars() {
                text.push(c.unicode().ok_or(InvalidCharacter(c.code()))?);
            }
            Ok(text.into())
        }
    }
}
impl EmacsText {
    /// Decode external UTF-8 without mistaking an invalid overlong sequence
    /// for internal byte8 storage. Every unrecognized octet remains separate.
    pub fn from_external_bytes(bytes: &[u8]) -> Self {
        let mut text = Self::new();
        let mut at = 0;
        while at < bytes.len() {
            if let Some((c, width)) =
                EmacsChar::decode_internal(&bytes[at..]).filter(|(c, _)| c.byte8().is_none())
            {
                text.push(c);
                at += width;
            } else {
                text.push(EmacsChar::new(0x3fff00 + bytes[at] as u32).unwrap());
                at += 1;
            }
        }
        text
    }
    pub fn from_os_str(text: &std::ffi::OsStr) -> Self {
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            Self::from_external_bytes(text.as_bytes())
        }
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            std::char::decode_utf16(text.encode_wide())
                .map(|c| match c {
                    Ok(c) => EmacsChar::from(c),
                    Err(error) => EmacsChar::new(error.unpaired_surrogate() as u32).unwrap(),
                })
                .collect()
        }
        #[cfg(not(any(unix, windows)))]
        {
            Self::from_external_bytes(text.as_encoded_bytes())
        }
    }
}

#[cfg(test)]
mod external_text_tests {
    use super::*;
    #[test]
    fn external_octets_roundtrip_without_aliasing_internal_byte8_encoding() {
        let mut bytes: Vec<u8> = (0..=255).collect();
        bytes.extend([0xc0, 0x80, 0xc1, 0xbf, 0xf3, 0xb0, 0x82, 0x80]);
        let text = EmacsText::from_external_bytes(&bytes);
        assert_eq!(text.to_external_bytes(), bytes);
        let last = text.codes().last().unwrap();
        assert_eq!(last, 0xf0080);
        let raw = EmacsText::from_external_bytes(&[0xc0, 0x80]);
        assert_eq!(raw.codes().collect::<Vec<_>>(), [0x3fffc0, 0x3fff80]);
    }
    #[test]
    fn every_two_octet_external_sequence_preserves_its_bytes() {
        for pair in 0..=u16::MAX {
            let bytes = pair.to_be_bytes();
            let text = EmacsText::from_external_bytes(&bytes);
            assert_eq!(text.to_external_bytes(), bytes, "{pair:04x}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn unix_paths_retain_private_use_raw_bytes_and_extended_codes() {
        let codes = [b'/' as u32, 0xf0080, 0x3fff80, 0xd800, 0x110000, 0x200000];
        let original = EmacsText::from_codes(codes).unwrap();
        let native = original.to_os_string().unwrap();
        assert_eq!(EmacsText::from_os_str(&native), original);
    }
}

impl Default for &EmacsStr {
    fn default() -> Self {
        EmacsStr::from_slice(&[])
    }
}

impl TextSource for std::borrow::Cow<'_, EmacsStr> {
    fn with_text<R>(&self, f: impl FnOnce(&EmacsStr) -> R) -> R {
        f(self.as_ref())
    }
}

impl From<&EmacsChar> for EmacsChar {
    fn from(c: &EmacsChar) -> Self {
        *c
    }
}
impl EmacsText {
    pub fn join(parts: &[Self], separator: impl TextPattern) -> Self {
        let separator = separator.characters();
        let mut result = Self::new();
        for (index, part) in parts.iter().enumerate() {
            if index != 0 {
                result.characters.extend_from_slice(&separator);
            }
            result.append(part);
        }
        result
    }
}
impl EmacsStr {
    pub fn split_inclusive(&self, separator: impl Into<EmacsChar>) -> std::vec::IntoIter<&Self> {
        let separator = separator.into();
        let mut parts = Vec::new();
        let mut start = 0;
        for (index, c) in self.chars().enumerate() {
            if c == separator {
                parts.push(Self::from_slice(&self.0[start..index + 1]));
                start = index + 1;
            }
        }
        if start < self.len() {
            parts.push(Self::from_slice(&self.0[start..]));
        }
        parts.into_iter()
    }
}

impl From<std::borrow::Cow<'_, str>> for EmacsText {
    fn from(text: std::borrow::Cow<'_, str>) -> Self { Self::from(text.as_ref()) }
}
impl TextSource for std::borrow::Cow<'_, str> {
    fn with_text<R>(&self, f: impl FnOnce(&EmacsStr) -> R) -> R { self.as_ref().with_text(f) }
}
impl PartialEq<&EmacsStr> for EmacsText {
    fn eq(&self, other: &&EmacsStr) -> bool { self.as_str() == *other }
}
impl PartialEq<EmacsText> for &EmacsStr {
    fn eq(&self, other: &EmacsText) -> bool { *self == other.as_str() }
}
impl<const N: usize> TextPattern for [char; N] {
    fn characters(&self) -> Vec<EmacsChar> { self.iter().copied().map(EmacsChar::from).collect() }
}

impl EmacsStr { pub(crate) fn as_ptr(&self) -> *const EmacsChar { self.0.as_ptr() } }
