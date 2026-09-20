//! Gap buffer: Emacs's classic text storage. Contiguous text with a
//! movable gap so insert/delete near point is O(gap move + insert size).

/// A buffer of `char`s with a gap at the editing position.
pub struct GapBuffer {
    buf: Vec<char>,
    gap_start: usize,
    gap_end: usize, // exclusive
}

const GAP_INIT: usize = 64;

impl GapBuffer {
    pub fn new() -> Self {
        GapBuffer {
            buf: vec![0 as char; GAP_INIT],
            gap_start: 0,
            gap_end: GAP_INIT,
        }
    }

    /// Total character count (text length).
    pub fn len(&self) -> usize {
        self.buf.len() - (self.gap_end - self.gap_start)
    }

    /// Char at logical index `i` (0-based).
    pub fn char_at(&self, i: usize) -> char {
        debug_assert!(i < self.len());
        if i < self.gap_start {
            self.buf[i]
        } else {
            self.buf[i + (self.gap_end - self.gap_start)]
        }
    }

    fn gap_size(&self) -> usize {
        self.gap_end - self.gap_start
    }

    /// Move the gap so it starts at logical position `pos`.
    fn move_gap(&mut self, pos: usize) {
        let pos = pos.min(self.len());
        if pos == self.gap_start {
            return;
        }
        let gsize = self.gap_size();
        if pos < self.gap_start {
            // Shift text right into the gap.
            let n = self.gap_start - pos;
            self.buf.copy_within(pos..self.gap_start, self.gap_end - n);
            self.gap_start = pos;
            self.gap_end -= n;
        } else {
            // Shift text left into the gap.
            let n = pos - self.gap_start;
            self.buf
                .copy_within(self.gap_end..self.gap_end + n, self.gap_start);
            self.gap_start = pos;
            self.gap_end += n;
        }
        let _ = gsize;
    }

    /// Grow the gap so at least `need` chars fit.
    fn ensure_gap(&mut self, need: usize) {
        if self.gap_size() >= need {
            return;
        }
        let grow = need.max(64);
        let old_len = self.buf.len();
        self.buf.resize(old_len + grow, 0 as char);
        // Move post-gap text to the new end.
        let _post = old_len - self.gap_end;
        self.buf
            .copy_within(self.gap_end..old_len, self.gap_end + grow);
        self.gap_end += grow;
    }

    /// Insert `text` at logical position `pos`.
    pub fn insert(&mut self, pos: usize, text: &str) {
        self.move_gap(pos);
        let n = text.chars().count();
        self.ensure_gap(n);
        for c in text.chars() {
            self.buf[self.gap_start] = c;
            self.gap_start += 1;
        }
    }

    /// Delete `[start, end)`.
    pub fn delete(&mut self, start: usize, end: usize) {
        let end = end.min(self.len());
        if start >= end {
            return;
        }
        self.move_gap(start);
        self.gap_end = (self.gap_end + (end - start)).min(self.buf.len());
    }

    /// Extract `[start, end)` as a String.
    pub fn substring(&self, start: usize, end: usize) -> String {
        let end = end.min(self.len());
        let start = start.min(end);
        let mut s = String::with_capacity(end - start);
        for i in start..end {
            s.push(self.char_at(i));
        }
        s
    }

    /// Whole contents.
    pub fn text(&self) -> String {
        self.substring(0, self.len())
    }

    /// Replace whole contents.
    pub fn set_text(&mut self, s: &str) {
        self.buf.clear();
        self.buf
            .resize(GAP_INIT.max(s.chars().count() * 2), 0 as char);
        self.gap_start = 0;
        self.gap_end = self.buf.len();
        self.insert(0, s);
    }

    /// Position of the `n`th line's start (0-based char index).
    /// Line `n` = n newlines before it. `line_of_pos` returns which line
    /// `pos` is on (0-based).
    pub fn line_of_pos(&self, pos: usize) -> usize {
        let mut line = 0;
        let end = pos.min(self.len());
        for i in 0..end {
            if self.char_at(i) == '\n' {
                line += 1;
            }
        }
        line
    }

    /// Start position of line `line` (0-based). Past-the-end returns len.
    pub fn line_start(&self, line: usize) -> usize {
        if line == 0 {
            return 0;
        }
        let mut l = 0;
        let len = self.len();
        for i in 0..len {
            if self.char_at(i) == '\n' {
                l += 1;
                if l == line {
                    return i + 1;
                }
            }
        }
        len
    }

    /// Char position of end of the line containing `pos` (index of the
    /// newline, or len).
    pub fn line_end(&self, pos: usize) -> usize {
        let len = self.len();
        let mut i = pos.min(len);
        while i < len {
            if self.char_at(i) == '\n' {
                return i;
            }
            i += 1;
        }
        len
    }
}

impl Default for GapBuffer {
    fn default() -> Self {
        Self::new()
    }
}
