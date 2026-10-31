//! XBM file decoding for one-bit face background patterns.

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Bitmap {
    pub width: u16,
    pub height: u16,
    pub data: Vec<u8>,
}

#[derive(Debug, PartialEq, Eq)]
enum Token<'a> {
    Ident(&'a [u8]),
    Number(u32),
    Punct(u8),
}

struct Lexer<'a> {
    source: &'a [u8],
    position: usize,
}

impl<'a> Lexer<'a> {
    fn next(&mut self) -> Option<Token<'a>> {
        loop {
            while self
                .source
                .get(self.position)
                .is_some_and(|byte| matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | 11 | 12))
            {
                self.position += 1;
            }
            if self.source.get(self.position..self.position + 2) != Some(b"/*") {
                break;
            }
            self.position += 2;
            let end = self.source[self.position..]
                .windows(2)
                .position(|pair| pair == b"*/")?;
            self.position += end + 2;
        }

        let start = self.position;
        let byte = *self.source.get(start)?;
        self.position += 1;
        if byte.is_ascii_alphabetic() || byte == b'_' {
            while self
                .source
                .get(self.position)
                .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
            {
                self.position += 1;
            }
            return Some(Token::Ident(&self.source[start..self.position]));
        }
        if byte == b'\'' {
            if self.source.get(self.position..self.position + 2) != Some(b"\\x") {
                return None;
            }
            self.position += 2;
            let number = self.number(16, 0, true)?;
            if self.source.get(self.position) != Some(&b'\'') {
                return None;
            }
            self.position += 1;
            return Some(Token::Number(number));
        }
        if byte.is_ascii_digit() {
            let number = if byte == b'0' {
                if matches!(self.source.get(self.position), Some(b'x' | b'X')) {
                    self.position += 1;
                    self.number(16, 0, true)?
                } else {
                    self.number(8, 0, false)?
                }
            } else {
                self.number(10, u32::from(byte - b'0'), false)?
            };
            return Some(Token::Number(number));
        }
        Some(Token::Punct(byte))
    }

    fn number(&mut self, radix: u32, mut value: u32, require_digit: bool) -> Option<u32> {
        let start = self.position;
        while let Some(digit) = self
            .source
            .get(self.position)
            .and_then(|byte| char::from(*byte).to_digit(radix))
        {
            value = value.checked_mul(radix)?.checked_add(digit)?;
            if value > i32::MAX as u32 {
                return None;
            }
            self.position += 1;
        }
        (!require_digit || self.position > start).then_some(value)
    }

    fn ident(&mut self, expected: &[u8]) -> Option<()> {
        (self.next()? == Token::Ident(expected)).then_some(())
    }

    fn punct(&mut self, expected: u8) -> Option<()> {
        (self.next()? == Token::Punct(expected)).then_some(())
    }
}

/// Decode the byte and legacy short-array forms accepted by GNU's XBM reader.
/// Rows are packed LSB-first, without the legacy format's 16-bit row padding.
pub(super) fn parse_xbm(source: &[u8]) -> Option<Bitmap> {
    let mut lexer = Lexer {
        source,
        position: 0,
    };
    let mut width = None;
    let mut height = None;
    let mut token = lexer.next()?;
    while token == Token::Punct(b'#') {
        lexer.ident(b"define")?;
        let Token::Ident(name) = lexer.next()? else {
            return None;
        };
        let Token::Number(number) = lexer.next()? else {
            return None;
        };
        match name.rsplit(|byte| *byte == b'_').next()? {
            b"width" => width = Some(number),
            b"height" => height = Some(number),
            _ => {}
        }
        token = lexer.next()?;
    }
    let width = u16::try_from(width?).ok().filter(|width| *width > 0)?;
    let height = u16::try_from(height?).ok().filter(|height| *height > 0)?;
    if token != Token::Ident(b"static") {
        return None;
    }
    let unit_bytes = match lexer.next()? {
        Token::Ident(b"unsigned") => {
            lexer.ident(b"char")?;
            1
        }
        Token::Ident(b"char") => 1,
        Token::Ident(b"short") => 2,
        _ => return None,
    };
    if !matches!(lexer.next()?, Token::Ident(_)) {
        return None;
    }
    for punctuation in b"[]={" {
        lexer.punct(*punctuation)?;
    }

    let row_bytes = usize::from(width).div_ceil(8);
    let row_units = row_bytes.div_ceil(unit_bytes);
    let mut data = Vec::new();
    // Grow only as actual data is decoded; bogus dimensions with a short body
    // must not cause a large allocation before the missing bytes are noticed.
    for _ in 0..height {
        for column in 0..row_units {
            let Token::Number(number) = lexer.next()? else {
                return None;
            };
            data.push(number as u8);
            if unit_bytes == 2 && column * 2 + 1 < row_bytes {
                data.push((number >> 8) as u8);
            }
            if !matches!(lexer.next()?, Token::Punct(b',' | b'}')) {
                return None;
            }
        }
    }
    Some(Bitmap {
        width,
        height,
        data,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modern_bitmap_accepts_comments_numbers_and_hex_character_literals() {
        let source = br#"/* an ordinary X11 bitmap */
          #define pattern_width 9
          #define pattern_height 2
          #define pattern_x_hot 0
          #define pattern_y_hot 1
          static unsigned char pattern_bits[] = {
            0x81, 01, /* first row */ 85, '\x01'
          };"#;
        assert_eq!(
            parse_xbm(source),
            Some(Bitmap {
                width: 9,
                height: 2,
                data: vec![0x81, 1, 85, 1],
            })
        );
    }

    #[test]
    fn legacy_short_rows_drop_padding_and_keep_low_byte_first() {
        let source = b"#define old_width 17\n#define old_height 2\n\
          static short old_bits[] = { 0x0201, 0xff03, 0x0504, 0xee06 };";
        assert_eq!(
            parse_xbm(source),
            Some(Bitmap {
                width: 17,
                height: 2,
                data: vec![1, 2, 3, 4, 5, 6],
            })
        );
        let source = b"#define old_width 8\n#define old_height 2\n\
          static short old_bits[] = { 0xff81, 0xee42 };";
        assert_eq!(parse_xbm(source).unwrap().data, [0x81, 0x42]);
    }

    #[test]
    fn invalid_or_truncated_bitmaps_are_rejected() {
        for source in [
            "/* missing end",
            "#define x_width 0\n#define x_height 1\nstatic char x[] = { 0 };",
            "#define x_width 65536\n#define x_height 1\nstatic char x[] = { 0 };",
            "#define x_width 65535\n#define x_height 65535\nstatic char x[] = { 0 };",
            "#define x_width 8\n#define x_height 1\nstatic char x[] = { 0x80000000 };",
            "#define x_width 8\n#define x_height 1\nstatic char x[] = { 08 };",
            "#define x_width 8\n#define x_height 1\nstatic char x[] = { '\\xgg' };",
        ] {
            assert!(parse_xbm(source.as_bytes()).is_none(), "{source}");
        }
    }
}
