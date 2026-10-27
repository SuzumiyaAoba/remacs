//! ANSI output for the runtime's device-independent screen grid.
use crossterm::{
    cursor, queue,
    style::{Attribute, SetAttribute},
    terminal,
};
use remacs_runtime::frontend::{CellColor, CellStyle, Grid};
use std::io::{self, Write};

/// Emit the SGR state for one cell style: reset, then foreground /
/// background colors, then the attribute bits.  `:inverse-video'
/// swaps the two colors; when only one side is set the terminal's
/// own swap does the right thing via SGR 7, so the swap is done in
/// the emitted colors only when both are explicit.
fn emit_style(out: &mut dyn io::Write, st: CellStyle) -> io::Result<()> {
    write!(out, "\x1b[0m")?;
    let (fg, bg, rev) = if st.inverse && st.fg.is_some() && st.bg.is_some() {
        (st.bg, st.fg, false)
    } else {
        (st.fg, st.bg, st.inverse)
    };
    if let Some(CellColor(r, g, b)) = fg {
        write!(out, "\x1b[38;2;{r};{g};{b}m")?;
    }
    if let Some(CellColor(r, g, b)) = bg {
        write!(out, "\x1b[48;2;{r};{g};{b}m")?;
    }
    if st.bold {
        write!(out, "\x1b[1m")?;
    }
    if st.italic {
        write!(out, "\x1b[3m")?;
    }
    if st.underline {
        write!(out, "\x1b[4m")?;
    }
    if st.overline {
        write!(out, "\x1b[53m")?;
    }
    if st.strike {
        write!(out, "\x1b[9m")?;
    }
    if rev {
        write!(out, "\x1b[7m")?;
    }
    Ok(())
}

pub(super) fn draw_grid(out: &mut impl Write, grid: &Grid) -> io::Result<()> {
    queue!(out, cursor::MoveTo(0, 0))?;
    let default = CellStyle::default();
    for (y, row) in grid.rows.iter().enumerate() {
        queue!(out, cursor::MoveTo(0, y as u16))?;
        let cells = grid.attrs.get(y).map(|v| v.as_slice()).unwrap_or(&[]);
        // Emit contiguous same-style runs; SGR state resets at
        // each boundary so only the style's own escapes are live.
        let mut cur = None;
        let mut run = String::new();
        for (x, ch) in row.chars().enumerate() {
            let st = cells.get(x).copied().unwrap_or(default);
            if Some(st) != cur {
                if !run.is_empty() {
                    emit_style(out, cur.unwrap())?;
                    write!(out, "{run}")?;
                    run.clear();
                }
                cur = Some(st);
            }
            run.push(ch);
        }
        if !run.is_empty() || cur.is_some() {
            emit_style(out, cur.unwrap_or(default))?;
            write!(out, "{run}")?;
        }
        // Reset trailing state so nothing leaks past the row.
        queue!(out, SetAttribute(Attribute::Reset))?;
    }
    if let Some((x, y)) = grid.cursor {
        queue!(out, cursor::MoveTo(x as u16, y as u16), cursor::Show)?;
    }
    out.flush()
}

pub(super) fn draw_echo(
    out: &mut impl Write,
    width: usize,
    height: usize,
    text: &str,
) -> io::Result<()> {
    let y = height.saturating_sub(1) as u16;
    queue!(
        out,
        cursor::MoveTo(0, y),
        terminal::Clear(terminal::ClearType::CurrentLine)
    )?;
    write!(out, "{:<width$}", text, width = width)?;
    let cx = text.chars().count().min(width.saturating_sub(1));
    queue!(out, cursor::MoveTo(cx as u16, y), cursor::Show)?;
    out.flush()
}
