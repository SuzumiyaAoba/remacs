//! Terminal front-end (crossterm): raw-mode input, frame rendering,
//! and the editor command loop.
//!
//! Non-blocking design: the loop always `poll`s input with a timeout,
//! renders the frame, and executes commands. Long Lisp operations can
//! interleave with repaints because `sit-for`/`sleep-for` hand control
//! back to this loop rather than blocking inside the evaluator.

use std::collections::HashMap;
use std::io::{self, Write};
use std::time::{Duration, Instant};

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    queue,
    style::{Attribute, SetAttribute},
    terminal::{self},
};

use crate::buffer::Buffer;
use crate::editor::{CHAR_CTL, CHAR_HYPER, CHAR_META, CHAR_SHIFT, CHAR_SUPER};
use crate::lisp::Interp;
use crate::lisp::value::Value;

/// Frontend-agnostic key/render interface used by the command loop
/// and the nested minibuffer/isearch loops. The crossterm `Terminal`
/// implements it directly; the gpui front-end implements it over a
/// channel to the UI thread.
pub trait KeyIo {
    /// Poll for a key event, waiting at most `dur`. Returns an Emacs
    /// key code (char + modifier bits).
    fn poll_key(&mut self, dur: Duration) -> io::Result<Option<i128>>;
    /// Push a key back onto the pending queue.
    fn unread(&mut self, k: i128);
    /// Repaint the whole frame.
    fn render(&mut self, i: &Interp) -> io::Result<()>;
    /// Show TEXT in the echo area (minibuffer prompt/status).
    fn draw_echo(&mut self, text: &str) -> io::Result<()>;
    /// Current frame size in text cells.
    fn size(&self) -> (usize, usize);
}

/// A terminal color: RGB. Face color specs (names, `#rgb' forms) are
/// resolved to this at render time; the crossterm front-end emits
/// true-color SGR, the gpui front-end paints it directly.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct CellColor(pub u8, pub u8, pub u8);

/// The merged face attributes for one screen cell.  `Option' fields
/// are tri-state during merging (None = unspecified, Some(false) =
/// explicitly cleared) and collapsed to plain bools once merged.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct CellStyle {
    pub fg: Option<CellColor>,
    pub bg: Option<CellColor>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub overline: bool,
    pub strike: bool,
    /// `:inverse-video': swap foreground/background.  Front-ends invert
    /// themselves (so a colored inverse cell keeps its colors swapped
    /// rather than emitting SGR 7 on top of them).
    pub inverse: bool,
    /// The face's `:extend' flag: the face's background covers the
    /// empty cells past the end of the line (region-face semantics).
    pub extend: bool,
}

/// Builder state for CellStyle while face specs merge — each field is
/// Option so an explicitly-nil attribute clears a lower layer.
#[derive(Clone, Copy, Default)]
struct StyleMix {
    fg: Option<CellColor>,
    bg: Option<CellColor>,
    bold: Option<bool>,
    italic: Option<bool>,
    underline: Option<bool>,
    overline: Option<bool>,
    strike: Option<bool>,
    inverse: Option<bool>,
    extend: Option<bool>,
}

impl StyleMix {
    /// Later (higher-priority) layer wins wherever it specifies a value.
    fn merge(&mut self, other: &StyleMix) {
        macro_rules! m {
            ($($f:ident),*) => {
                $(if let Some(v) = other.$f { self.$f = Some(v); })*
            };
        }
        m!(fg, bg, bold, italic, underline, overline, strike, inverse, extend);
    }

    fn finish(&self) -> CellStyle {
        CellStyle {
            fg: self.fg,
            bg: self.bg,
            bold: self.bold.unwrap_or(false),
            italic: self.italic.unwrap_or(false),
            underline: self.underline.unwrap_or(false),
            overline: self.overline.unwrap_or(false),
            strike: self.strike.unwrap_or(false),
            inverse: self.inverse.unwrap_or(false),
            extend: self.extend.unwrap_or(false),
        }
    }
}

impl PartialEq for StyleMix {
    fn eq(&self, o: &Self) -> bool {
        self.fg == o.fg
            && self.bg == o.bg
            && self.bold == o.bold
            && self.italic == o.italic
            && self.underline == o.underline
            && self.overline == o.overline
            && self.strike == o.strike
            && self.inverse == o.inverse
            && self.extend == o.extend
    }
}

/// A text-mode screen snapshot produced by `render_grid`: one string
/// per row, the per-cell face attributes (same shape as `rows`), which
/// rows are mode lines, and the hardware cursor position.  Both
/// front-ends share it — the terminal writes it, gpui draws it.
#[derive(Clone)]
pub struct Grid {
    pub rows: Vec<String>,
    /// Cell styles, `attrs[y][x]` alongside `rows[y]`'s chars.
    pub attrs: Vec<Vec<CellStyle>>,
    pub mode_rows: Vec<usize>,
    pub cursor: Option<(usize, usize)>,
}

impl Grid {
    /// An all-default grid of WIDTH × HEIGHT cells.
    pub fn blank(width: usize, height: usize) -> Grid {
        Grid {
            rows: vec![String::new(); height],
            attrs: vec![vec![CellStyle::default(); width]; height],
            mode_rows: Vec::new(),
            cursor: None,
        }
    }
}
// ---------- face resolution ----------
//
// GNU realizes faces in xfaces.c (`realize_face' +
// `compute_char_face'): the `default' face seeds every cell, the
// active region and `face'/`font-lock-face' text properties overlay
// it, and overlays stack on top by ascending `priority'.  The face
// attribute layers here are the same: defface specs land in
// `Interp::face_table' via `internal-set-lisp-face-attribute'/the
// prelude's `face-spec-set', `(set-face-attribute FACE t ...)' goes to
// `face_t_table', and `face-alias' links resolve like GNU's
// `lface_from_face_name'.

/// Per-render face computation: symbol/keyword ids looked up once,
/// the color name table from `color-name-rgb-alist', and a cache of
/// already-resolved named faces.
struct FaceCtx {
    // Symbol ids for property names (SymId::MAX = missing).
    face: u32,
    font_lock_face: u32,
    priority: u32,
    window: u32,
    mark_active: u32,
    inherit: u32,
    /// `color-name-rgb-alist' folded into (canonical name -> rgb).
    colors: HashMap<String, (u8, u8, u8)>,
    /// Resolved face name -> merged StyleMix.
    cache: HashMap<String, StyleMix>,
}

impl FaceCtx {
    fn new(i: &Interp) -> FaceCtx {
        let sym = |n: &str| i.intern_soft(n).unwrap_or(u32::MAX);
        let mut colors: HashMap<String, (u8, u8, u8)> = HashMap::new();
        // The eight standard colors come from tty-standard-colors so
        // they resolve even if color-name-rgb-alist never loads.
        for (n, r, g, b) in [
            ("black", 0u32, 0u32, 0u32),
            ("red", 65535, 0, 0),
            ("green", 0, 65535, 0),
            ("yellow", 65535, 65535, 0),
            ("blue", 0, 0, 65535),
            ("magenta", 65535, 0, 65535),
            ("cyan", 0, 65535, 65535),
            ("white", 65535, 65535, 65535),
        ] {
            colors.insert(n.to_string(), ((r / 256) as u8, (g / 256) as u8, (b / 256) as u8));
        }
        if let Value::Cons(_) = i.symbol_value(sym("color-name-rgb-alist")) {
            let alist = i.symbol_value(sym("color-name-rgb-alist"));
            if let Ok(entries) = alist.list_to_vec() {
                for e in entries {
                    if let Value::Cons(c) = &e {
                        let (k, v) = {
                            let cb = c.borrow();
                            (cb.car.clone(), cb.cdr.clone())
                        };
                        if let Value::Str(name) = &k {
                            if let Ok(rgb) = v.list_to_vec() {
                                let mut c8 = [0u8; 3];
                                let mut ok = rgb.len() >= 3;
                                for (n, x) in rgb.iter().take(3).enumerate() {
                                    match x {
                                        Value::Int(v) => {
                                            c8[n] = ((*v).clamp(0, 65535) / 256) as u8;
                                        }
                                        _ => ok = false,
                                    }
                                }
                                if ok {
                                    colors.insert(
                                        name.borrow().to_lowercase().replace(' ', ""),
                                        (c8[0], c8[1], c8[2]),
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
        FaceCtx {
            face: sym("face"),
            font_lock_face: sym("font-lock-face"),
            priority: sym("priority"),
            window: sym("window"),
            mark_active: sym("mark-active"),
            inherit: sym(":inherit"),
            colors,
            cache: HashMap::new(),
        }
    }

    /// GNU `tty_color_standard_values' + `tty-color-canonicalize':
    /// `#rgb'/`#rrggbb' parse directly, else the canonicalized name is
    /// looked up in the color-name alist.
    fn color(&self, v: &Value) -> Option<CellColor> {
        let Value::Str(s) = v else {
            return None;
        };
        let t = s.borrow();
        if let Some(hex) = t.strip_prefix('#') {
            let parse = |h: &str| u8::from_str_radix(h, 16).ok();
            return match hex.len() {
                6 => Some(CellColor(
                    parse(&hex[0..2])?,
                    parse(&hex[2..4])?,
                    parse(&hex[4..6])?,
                )),
                3 => {
                    let mut it = hex.chars().filter_map(|c| c.to_digit(16).map(|d| d as u8));
                    let (r, g, b) = (it.next()?, it.next()?, it.next()?);
                    Some(CellColor(r * 17, g * 17, b * 17))
                }
                _ => None,
            };
        }
        let canon = t.to_lowercase().replace(' ', "");
        self.colors
            .get(&canon)
            .map(|&(r, g, b)| CellColor(r, g, b))
    }

    /// Merge one named face's attributes (spec layer first, then the
    /// new-frame defaults layer like GNU's realize_default_face, then
    /// the compiled-in basic-face fallbacks).
    fn named(&mut self, i: &Interp, name: &str, depth: u32) -> StyleMix {
        if let Some(s) = self.cache.get(name) {
            return *s;
        }
        // The realized value: face_table plist (defface/theme layer)
        // over the new-frame defaults layer over built-in defaults.
        let mut mix = builtin_face_defaults(i, name);
        if let Some((_, plist)) = i.face_t_table.iter().find(|(n, _)| n == name) {
            mix.merge(&self.face_plist(i, plist, depth));
        }
        if let Some((_, plist)) = i.face_table.iter().find(|(n, _)| n == name) {
            mix.merge(&self.face_plist(i, plist, depth));
        }
        self.cache.insert(name.to_string(), mix);
        mix
    }

    /// One face's attribute plist: :inherit expands first (GNU merge
    /// order), then the face's own attributes override it.
    fn face_plist(&mut self, i: &Interp, plist: &Value, depth: u32) -> StyleMix {
        let mut mix = StyleMix::default();
        if depth >= 16 {
            return mix;
        }
        let inherit = crate::lisp::eval::plist_get(plist, self.inherit);
        if inherit.truthy() {
            mix.merge(&self.face_value(i, &inherit, depth + 1));
        }
        mix.merge(&self.attrs(i, plist));
        mix
    }

    /// The attribute keywords of a plist, without :inherit.
    /// The attribute keywords of a plist, without :inherit.
    fn attrs(&mut self, i: &Interp, plist: &Value) -> StyleMix {
        let mut mix = StyleMix::default();
        let mut distant_fg = None;
        let Ok(items) = plist.list_to_vec() else {
            return mix;
        };
        for kv in items.chunks(2) {
            let (Some(k), Some(v)) = (i.sym_id(&kv[0]), kv.get(1)) else {
                continue;
            };
            match i.symbol_name(k).as_str() {
                ":foreground" => {
                    if let Some(c) = self.color(v) {
                        mix.fg = Some(c);
                    }
                }
                ":distant-foreground" => distant_fg = self.color(v),
                ":background" => {
                    if let Some(c) = self.color(v) {
                        mix.bg = Some(c);
                    }
                }
                ":weight" => mix.bold = Some(weight_bold(i, v)),
                ":slant" => mix.italic = Some(slant_italic(i, v)),
                ":underline" => mix.underline = Some(v.truthy()),
                ":overline" => mix.overline = Some(v.truthy()),
                ":strike-through" => mix.strike = Some(v.truthy()),
                ":inverse-video" => mix.inverse = Some(v.truthy()),
                ":extend" => mix.extend = Some(v.truthy()),
                _ => {}
            }
        }
        // :distant-foreground is only used when it contrasts with the
        // background; on this coarse model it fills fg when the face
        // sets no real :foreground (GNU drops it when it blends in).
        if mix.fg.is_none() {
            mix.fg = distant_fg;
        }
        mix
    }

    /// A `face' property value: a symbol/string names a face, a list
    /// whose car is a keyword is an anonymous face plist, any other
    /// list merges each element in order, and
    /// (foreground-color . C)/(background-color . C) pairs set colors.
    fn face_value(&mut self, i: &Interp, v: &Value, depth: u32) -> StyleMix {
        if depth >= 16 {
            return StyleMix::default();
        }
        match v {
            Value::Sym(s) => {
                let name = i.symbol_name(*s);
                match crate::editor::face_resolve(i, &name) {
                    Some(t) => self.named(i, &t, depth + 1),
                    None => StyleMix::default(),
                }
            }
            Value::Str(s) => {
                let name = s.borrow().clone();
                match crate::editor::face_resolve(i, &name) {
                    Some(t) => self.named(i, &t, depth + 1),
                    None => StyleMix::default(),
                }
            }
            Value::Cons(_) => {
                let Ok(items) = v.list_to_vec() else {
                    // Dotted pair: (foreground-color . COLOR) /
                    // (background-color . COLOR).
                    let (car, cdr) = {
                        let Value::Cons(c) = v else {
                            return StyleMix::default();
                        };
                        let cb = c.borrow();
                        (cb.car.clone(), cb.cdr.clone())
                    };
                    let mut mix = StyleMix::default();
                    if let Some(cs) = i.sym_id(&car).map(|s| i.symbol_name(s)) {
                        if let Some(c) = self.color(&cdr) {
                            match cs.as_str() {
                                "foreground-color" => mix.fg = Some(c),
                                "background-color" => mix.bg = Some(c),
                                _ => {}
                            }
                        }
                    }
                    return mix;
                };
                let is_plist = i
                    .sym_id(&items[0])
                    .map(|s| i.symbol_name(s).starts_with(':'))
                    .unwrap_or(false);
                if is_plist {
                    // Anonymous face: a plist of attributes.
                    self.face_plist(i, v, depth + 1)
                } else {
                    let mut mix = StyleMix::default();
                    for it in items {
                        mix.merge(&self.face_value(i, &it, depth + 1));
                    }
                    mix
                }
            }
            _ => StyleMix::default(),
        }
    }
}

/// GNU weight mapping (xfaces.c `face_numeric_weight'): semi-bold and
/// heavier render as bold on a tty.
fn weight_bold(i: &Interp, v: &Value) -> bool {
    let n: i128 = match v {
        Value::Int(n) => *n,
        Value::Sym(s) => match i.symbol_name(*s).as_str() {
            "thin" => 200,
            "ultra-light" | "extra-light" => 275,
            "light" => 300,
            "semi-light" | "book" => 380,
            "semi-bold" | "demi-bold" | "demibold" => 600,
            "bold" => 700,
            "extra-bold" | "ultra-bold" | "extrabold" | "ultrabold" => 800,
            "black" | "heavy" => 900,
            _ => 400,
        },
        _ => 400,
    };
    n >= 600
}

/// GNU slant mapping: italic/oblique slant the text.
fn slant_italic(i: &Interp, v: &Value) -> bool {
    match v {
        Value::Sym(s) => !matches!(
            i.symbol_name(*s).as_str(),
            "normal" | "roman" | "unspecified" | "reversed-oblique" | "reversed-italic"
        ),
        _ => false,
    }
}

/// The attributes a face has even with no face_table row.  These
/// mirror the `(t ...)'/`((type tty) (class mono) ...)' fallbacks in
/// GNU's defface specs — what `face-spec-set' would leave on a
/// monochrome tty — for the faces `lisp/faces.el'/`lisp/isearch.el'
/// define but that aren't booted into `face_table'.
fn builtin_face_defaults(i: &Interp, name: &str) -> StyleMix {
    let mut mix = StyleMix::default();
    match name {
        "default" => {
            mix.bold = Some(false);
            mix.italic = Some(false);
            mix.underline = Some(false);
            mix.overline = Some(false);
            mix.strike = Some(false);
            mix.inverse = Some(false);
            mix.extend = Some(false);
        }
        "bold" => mix.bold = Some(true),
        "italic" => mix.italic = Some(true),
        "bold-italic" => {
            mix.bold = Some(true);
            mix.italic = Some(true);
        }
        "underline" => mix.underline = Some(true),
        // faces.el/isearch.el `(t :inverse-video t)' and
        // `((type tty) (class mono)) :inverse-video t' entries.
        "isearch" | "isearch-fail" | "highlight" | "secondary-selection"
        | "trailing-whitespace" | "match" | "query-replace" | "mode-line"
        | "mode-line-active" | "mode-line-inactive" => mix.inverse = Some(true),
        "region" => {
            // GNU's last-resort `(t :background "gray" :extend t)'.
            mix.bg = Some(CellColor(190, 190, 190));
            mix.extend = Some(true);
        }
        "lazy-highlight" => mix.underline = Some(true),
        "isearch-group-1" | "isearch-group-2" => {
            mix.merge(&builtin_face_defaults(i, "isearch"))
        }
        // `(t (:weight bold))' entries.
        "mode-line-emphasis" | "mode-line-buffer-id" => mix.bold = Some(true),
        _ => {}
    }
    mix
}

/// One window's face context for render_grid: the buffer's overlay
/// faces and `face'/`font-lock-face' text-property runs collected
/// once per render, and the active-region range.
struct WindowFaces {
    /// Overlay (start, end, face-value, priority, creation index),
    /// applied low→high: ascending priority, oldest first.
    overlays: Vec<(usize, usize, Value, i128, usize)>,
    /// 'face/'font-lock-face' text-property runs: (start, end, value,
    /// insertion index).  Later-inserted props merge on top (GNU's
    /// interval "last write wins").
    prop_faces: Vec<(usize, usize, Value, usize)>,
    /// Active region as a half-open range when the mark is active.
    region: Option<(usize, usize)>,
}

fn window_faces(
    i: &Interp,
    ctx: &FaceCtx,
    b: &std::rc::Rc<std::cell::RefCell<Buffer>>,
    win: &Value,
) -> WindowFaces {
    let bb = b.borrow();
    // Region: GNU shows it between the mark and the buffer's point
    // while `mark-active' is non-nil.
    let region = {
        let active = i
            .buffer_local_toplevel_value(ctx.mark_active, bb.id)
            .or_else(|| i.default_toplevel_value(ctx.mark_active))
            .or_else(|| bb.locals.get(&ctx.mark_active).cloned())
            .map(|v| v.truthy())
            .unwrap_or(false);
        let mark = bb.mark.as_ref().and_then(|m| {
            let mm = m.borrow();
            (mm.buffer == Some(bb.id)).then_some(mm.position)
        });
        match (active, mark) {
            (true, Some(mk)) if mk != bb.point() => {
                Some((mk.min(bb.point()), mk.max(bb.point())))
            }
            _ => None,
        }
    };
    // Overlays covering this buffer, for this window (a `window'
    // property restricts an overlay to one window — lazy-highlight
    // uses it).  Merge order: ascending priority, oldest first —
    // GNU's overlays_at sorted order reversed.
    let mut overlays: Vec<(usize, usize, Value, i128, usize)> = bb
        .overlays
        .iter()
        .enumerate()
        .filter(|(_, ov)| {
            if ov.buffer != Some(bb.id) || ov.start >= ov.end {
                return false;
            }
            let wv = crate::lisp::eval::plist_get(&ov.plist, ctx.window);
            // nil = all windows; non-nil must be this window object.
            wv.is_nil() || crate::lisp::builtins::eq_values(&wv, win)
        })
        .filter_map(|(idx, ov)| {
            let fv = crate::lisp::eval::plist_get(&ov.plist, ctx.face);
            if fv.is_nil() {
                return None;
            }
            Some((
                ov.start,
                ov.end,
                fv,
                crate::lisp::eval::plist_get(&ov.plist, ctx.priority)
                    .int()
                    .unwrap_or(0),
                idx,
            ))
        })
        .collect();
    overlays.sort_by_key(|(_, _, _, p, idx)| (*p, *idx));
    let prop_faces: Vec<(usize, usize, Value, usize)> = bb
        .text
        .props
        .borrow()
        .iter()
        .enumerate()
        .filter(|(_, tp)| tp.prop == ctx.face || tp.prop == ctx.font_lock_face)
        .map(|(idx, tp)| (tp.start, tp.end, tp.value.clone(), idx))
        .collect();
    WindowFaces {
        overlays,
        prop_faces,
        region,
    }
}

/// The merged face style for buffer position POS under WF.  GNU
/// `compute_char_face' order: `default' seeds, the active `region'
/// face, then text-property faces (insertion order — last write
/// wins), then overlays in ascending priority.
fn style_at_pos(
    i: &Interp,
    ctx: &mut FaceCtx,
    wf: &WindowFaces,
    pos: usize,
) -> CellStyle {
    let mut mix = ctx.named(i, "default", 0);
    if let Some((rs, re)) = wf.region {
        if pos >= rs && pos < re {
            mix.merge(&ctx.named(i, "region", 0));
        }
    }
    for (s, e, v, _) in &wf.prop_faces {
        if pos >= *s && pos < *e {
            let v = v.clone();
            mix.merge(&ctx.face_value(i, &v, 0));
        }
    }
    for (s, e, fv, _, _) in &wf.overlays {
        if pos >= *s && pos < *e {
            let fv = fv.clone();
            mix.merge(&ctx.face_value(i, &fv, 0));
        }
    }
    mix.finish()
}

/// Compute the screen contents for the selected frame at `width` ×
/// `height` text cells.  Like GNU's `redisplay_window' it also keeps
/// each window's point on screen: a window's `start' is scrolled the
/// minimal amount to show point, and recentered when point sits more
/// than a window-height away (scrolling can't reach it cheaply).
pub fn render_grid(i: &Interp, width: usize, height: usize) -> Grid {
    let mut rows = vec![String::new(); height];
    let mut attrs = vec![vec![CellStyle::default(); width]; height];
    let mut mode_rows = Vec::new();
    let mut cursor_pos: Option<(usize, usize)> = None;
    let frame = match &i.selected_frame {
        Some(f) => f.clone(),
        None => {
            return Grid {
                rows,
                attrs,
                mode_rows,
                cursor: cursor_pos,
            };
        }
    };
    let mut fctx = FaceCtx::new(i);
    let fb = frame.borrow();
    let n_windows = fb.windows.len();
    let mini_height = 1usize;
    let body_height = height.saturating_sub(mini_height);

    for (wi, w) in fb.windows.iter().enumerate() {
        let per = if n_windows == 0 {
            0
        } else {
            body_height / n_windows
        };
        let top = wi * per;
        let wh = if wi == n_windows - 1 {
            body_height.saturating_sub(per * wi)
        } else {
            per
        };
        let wh = wh.max(2);
        let (buf_id, start, hscroll) = {
            let wb = w.borrow();
            (wb.buffer, wb.start, wb.hscroll)
        };
        let text_rows = wh.saturating_sub(1);
        // Keep point inside the window: scroll `start' the minimal
        // amount, or recenter when point is more than a window-height
        // away, and likewise `hscroll' horizontally (GNU
        // `redisplay_window' semantics — the window's start/hscroll
        // are redisplay state, so updating them here is correct).
        let (start, hscroll) = match i.buffers.get(buf_id) {
            Some(b) if text_rows > 0 => {
                let (suspend_hs, old_pt, min_hs) = {
                    let wb = w.borrow();
                    (wb.suspend_hscroll, wb.old_point, wb.min_hscroll)
                };
                let bb = b.borrow();
                let pt = if std::rc::Rc::ptr_eq(w, &fb.selected) {
                    bb.point()
                } else {
                    w.borrow().point
                };
                // GNU xdisp.c: point movement since the last redisplay
                // lifts an hscroll suspension; then record it as the
                // new old_point.
                let mut wb_mut = w.borrow_mut();
                if wb_mut.suspend_hscroll && pt != wb_mut.old_point {
                    wb_mut.suspend_hscroll = false;
                }
                wb_mut.old_point = pt;
                drop(wb_mut);
                let suspend_hs = suspend_hs && pt == old_pt;
                let pt_line = bb.text.line_of_pos(pt);
                let start_line = bb.text.line_of_pos(start);
                let new_start = if pt_line < start_line {
                    let up = start_line - pt_line;
                    if up > text_rows {
                        bb.text.line_start(pt_line.saturating_sub(text_rows / 2))
                    } else {
                        bb.text.line_start(pt_line)
                    }
                } else if pt_line >= start_line + text_rows {
                    let down = pt_line - (start_line + text_rows - 1);
                    if down > text_rows {
                        bb.text.line_start(pt_line.saturating_sub(text_rows / 2))
                    } else {
                        bb.text.line_start(pt_line + 1 - text_rows)
                    }
                } else {
                    start
                };
                // Auto-hscroll only when `auto-hscroll-mode' allows it
                // for this window's buffer and not suspended by an
                // explicit `scroll-left'/`-right'/`set-window-hscroll'.
                // GNU's min_hscroll floors the automatic value.
                let ahs_id = i.intern_soft("auto-hscroll-mode").unwrap_or(u32::MAX);
                let ahs_on = i
                    .buffer_local_toplevel_value(ahs_id, buf_id)
                    .or_else(|| i.default_toplevel_value(ahs_id))
                    .map(|v| v.truthy())
                    .unwrap_or(true);
                let new_hscroll = if width > 0 && ahs_on && !suspend_hs {
                    let col = pt - bb.text.line_start(pt_line);
                    if col < hscroll {
                        col
                    } else if col >= hscroll + width {
                        col + 1 - width
                    } else {
                        hscroll
                    }
                    .max(min_hs)
                } else {
                    hscroll
                };
                {
                    let mut wb = w.borrow_mut();
                    if new_start != start {
                        wb.start = new_start;
                    }
                    if new_hscroll != hscroll {
                        wb.hscroll = new_hscroll;
                    }
                }
                (new_start, new_hscroll)
            }
            _ => (start, hscroll),
        };
        // Per-window face context (overlays, text props, region).
        let wf = i
            .buffers
            .get(buf_id)
            .map(|b| window_faces(i, &fctx, &b, &Value::Window(w.clone())));
        for row in 0..text_rows {
            let y = top + row;
            if y >= body_height {
                break;
            }
            // Line-aligned rendering: `start' is a line start because
            // the scroll code above always lands it on line_start().
            let (line, line_pos) = match i.buffers.get(buf_id) {
                Some(b) => {
                    let bb = b.borrow();
                    let ls = bb
                        .text
                        .line_start(bb.text.line_of_pos(start) + row);
                    let le = bb.text.line_end(ls);
                    (bb.text.substring(ls, le), ls)
                }
                None => (String::new(), 0),
            };
            let nvis = line.chars().count().saturating_sub(hscroll).min(width);
            let mut vis = String::with_capacity(nvis);
            // Characters visible in this row and their cells' styles.
            for (x, ch) in line.chars().skip(hscroll).take(width).enumerate() {
                vis.push(ch);
                let pos = line_pos + hscroll + x;
                if let Some(wf) = &wf {
                    attrs[y][x] = style_at_pos(i, &mut fctx, wf, pos);
                }
            }
            // Empty tail of the row: the line-end face extends its
            // background over it iff the face's :extend holds (GNU
            // extend_face_to_end_of_line).
            if let Some(wf) = &wf {
                let tail_pos = (line_pos + line.chars().count()).min(line_pos + hscroll + nvis);
                let end_pos = line_pos + line.chars().count();
                let tail = style_at_pos(i, &mut fctx, wf, tail_pos.max(end_pos));
                if tail.extend {
                    for x in nvis..width {
                        let mut t = tail;
                        t.fg = None;
                        t.bold = false;
                        t.italic = false;
                        t.underline = false;
                        t.overline = false;
                        t.strike = false;
                        t.inverse = false;
                        t.extend = false;
                        attrs[y][x] = t;
                    }
                }
            }
            rows[y] = format!("{vis:<width$}", width = width);
        }
        // Mode line.
        let my = top + text_rows;
        if my < body_height {
            let (name, modified, point_line, point_col) = match i.buffers.get(buf_id) {
                Some(b) => {
                    let bb = b.borrow();
                    (
                        bb.name.clone(),
                        bb.is_modified(),
                        bb.text.line_of_pos(bb.point()) + 1,
                        bb.point() - bb.text.line_start(bb.text.line_of_pos(bb.point())),
                    )
                }
                None => ("???".into(), false, 0, 0),
            };
            let mode = format!(
                "--{}- {}   L{} C{}{}",
                if modified { "**" } else { "--" },
                name,
                point_line,
                point_col,
                if std::rc::Rc::ptr_eq(w, &fb.selected) {
                    "  [sel]"
                } else {
                    ""
                }
            );
            // GNU draws mode lines under `mode-line-active' on the
            // selected window, `mode-line-inactive' elsewhere — both
            // resolve through `mode-line' (inverse on a mono tty).
            // An unresolved (pre-faces.el) mode-line face falls back
            // to plain inverse so the row still reads as a divider.
            let ml_name = if std::rc::Rc::ptr_eq(w, &fb.selected) {
                "mode-line-active"
            } else {
                "mode-line-inactive"
            };
            let mut ml = fctx.named(i, ml_name, 0).finish();
            if ml == CellStyle::default() {
                ml.inverse = true;
            }
            rows[my] = format!("{mode:<width$}", width = width);
            for cell in attrs[my].iter_mut() {
                *cell = ml;
            }
            mode_rows.push(my);
        }
        // Cursor for the selected window.
        if std::rc::Rc::ptr_eq(w, &fb.selected) {
            if let Some(b) = i.buffers.get(buf_id) {
                let bb = b.borrow();
                let p = bb.point();
                let line = bb.text.line_of_pos(p);
                let ls = bb.text.line_start(line);
                let start_line = bb.text.line_of_pos(start);
                let row = line.saturating_sub(start_line);
                // GNU clamps an hscroll-hidden cursor to the window
                // edge (left edge when scrolled past on the left,
                // right edge when past the right) instead of hiding
                // it — needed while `suspend_hscroll' holds the view.
                let pcol = p - ls;
                let col = if pcol < hscroll {
                    0
                } else {
                    (pcol - hscroll).min(width.saturating_sub(1))
                };
                if row < text_rows && width > 0 {
                    cursor_pos = Some((col, top + row));
                }
            }
        }
    }
    // Minibuffer / echo area (last row).  The minibuffer's own text
    // properties apply (minibuffer-prompt's `face' prop colors the
    // prompt); a plain echo message renders under `default'.
    let mini_buf = fb
        .minibuffer
        .as_ref()
        .and_then(|w| i.buffers.get(w.borrow().buffer));
    let mini_text = match &mini_buf {
        Some(b) => b.borrow().text.text(),
        None => String::new(),
    };
    let (msg, styled) = if mini_text.is_empty() {
        (i.echo_message.clone(), None)
    } else {
        (mini_text, mini_buf.as_ref())
    };
    if let Some(y) = rows.len().checked_sub(1) {
        let vis: String = msg.chars().take(width).collect();
        rows[y] = format!("{vis:<width$}", width = width);
        if let Some(b) = styled {
            let wf = window_faces(i, &fctx, &b, &Value::Nil);
            for (x, _) in msg.chars().take(width).enumerate() {
                attrs[y][x] = style_at_pos(i, &mut fctx, &wf, x);
            }
        }
    }
    Grid {
        rows,
        attrs,
        mode_rows,
        cursor: cursor_pos,
    }
}

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

/// The terminal screen.
pub struct Terminal {
    pub width: usize,
    pub height: usize,
    out: Box<dyn io::Write>,
    /// Pending key events read but not yet consumed (for unread-command-events).
    pending: Vec<i128>,
}

impl Terminal {
    /// Enter raw mode + alternate screen.
    pub fn enter() -> io::Result<Terminal> {
        let (w, h) = terminal::size().unwrap_or((80, 24));
        let w = w.max(10);
        let h = h.max(3);
        terminal::enable_raw_mode()?;
        let mut out = io::stdout();
        queue!(out, terminal::EnterAlternateScreen, cursor::Hide)?;
        out.flush()?;
        Ok(Terminal {
            width: w as usize,
            height: h as usize,
            out: Box::new(out),
            pending: Vec::new(),
        })
    }

    /// Restore the terminal. Always call before exit (RAII via Drop).
    pub fn leave(&mut self) {
        let _ = queue!(
            self.out,
            SetAttribute(Attribute::Reset),
            cursor::Show,
            terminal::LeaveAlternateScreen
        );
        let _ = self.out.flush();
        let _ = terminal::disable_raw_mode();
    }
}

impl KeyIo for Terminal {
    /// Poll for a key event, waiting at most `dur`.
    /// Returns an Emacs key code (char + modifier bits).
    fn poll_key(&mut self, dur: Duration) -> io::Result<Option<i128>> {
        if let Some(k) = self.pending.pop() {
            return Ok(Some(k));
        }
        if !event::poll(dur)? {
            return Ok(None);
        }
        match event::read()? {
            Event::Key(k) => Ok(key_event_to_code(k)),
            Event::Resize(w, h) => {
                self.width = (w as usize).max(10);
                self.height = (h as usize).max(3);
                Ok(None)
            }
            _ => Ok(None),
        }
    }

    /// Push a key back to the pending queue.
    fn unread(&mut self, k: i128) {
        self.pending.push(k);
    }

    /// Draw the frame: each window's buffer, mode lines, minibuffer line.
    fn render(&mut self, i: &Interp) -> io::Result<()> {
        let (width, height) = (self.width, self.height);
        let grid = render_grid(i, width, height);
        queue!(self.out, cursor::MoveTo(0, 0))?;
        let default = CellStyle::default();
        for (y, row) in grid.rows.iter().enumerate() {
            queue!(self.out, cursor::MoveTo(0, y as u16))?;
            let cells = grid
                .attrs
                .get(y)
                .map(|v| v.as_slice())
                .unwrap_or(&[]);
            // Emit contiguous same-style runs; SGR state resets at
            // each boundary so only the style's own escapes are live.
            let mut cur = None;
            let mut run = String::new();
            for (x, ch) in row.chars().enumerate() {
                let st = cells.get(x).copied().unwrap_or(default);
                if Some(st) != cur {
                    if !run.is_empty() {
                        emit_style(&mut self.out, cur.unwrap())?;
                        let _ = write!(self.out, "{run}");
                        run.clear();
                    }
                    cur = Some(st);
                }
                run.push(ch);
            }
            if !run.is_empty() || cur.is_some() {
                emit_style(&mut self.out, cur.unwrap_or(default))?;
                let _ = write!(self.out, "{run}");
            }
            // Reset trailing state so nothing leaks past the row.
            queue!(self.out, SetAttribute(Attribute::Reset))?;
        }
        if let Some((x, y)) = grid.cursor {
            queue!(self.out, cursor::MoveTo(x as u16, y as u16), cursor::Show)?;
        }
        self.out.flush()
    }

    /// Draw the echo-area line with TEXT, cursor at its end.
    fn draw_echo(&mut self, text: &str) -> io::Result<()> {
        let y = self.height.saturating_sub(1) as u16;
        queue!(
            self.out,
            cursor::MoveTo(0, y),
            terminal::Clear(terminal::ClearType::CurrentLine)
        )?;
        let width = self.width;
        let _ = write!(self.out, "{:<width$}", text, width = width);
        let cx = text.chars().count().min(width.saturating_sub(1));
        queue!(self.out, cursor::MoveTo(cx as u16, y), cursor::Show)?;
        self.out.flush()
    }

    fn size(&self) -> (usize, usize) {
        (self.width, self.height)
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        self.leave();
    }
}

/// Translate a crossterm key event to an Emacs event code.
fn key_event_to_code(k: KeyEvent) -> Option<i128> {
    let mut mods = 0i128;
    if k.modifiers.contains(KeyModifiers::CONTROL) {
        mods |= CHAR_CTL;
    }
    if k.modifiers.contains(KeyModifiers::ALT) {
        mods |= CHAR_META;
    }
    if k.modifiers.contains(KeyModifiers::SHIFT) {
        // Shift on printable chars arrives pre-shifted in `code`.
        mods |= CHAR_SHIFT;
    }
    if k.modifiers.contains(KeyModifiers::SUPER) {
        mods |= CHAR_SUPER;
    }
    if k.modifiers.contains(KeyModifiers::HYPER) {
        mods |= CHAR_HYPER;
    }
    // ALT is sometimes delivered as META; treat as M-.
    let base = match k.code {
        KeyCode::Char(c) => {
            if c.is_ascii_uppercase() && k.modifiers.contains(KeyModifiers::SHIFT) {
                mods &= !CHAR_SHIFT; // shift folded into the char
            }
            if mods & CHAR_CTL != 0 && (c as i128) < 128 {
                // Emacs folds C-<ascii> to the control char (C-u → 21).
                mods &= !CHAR_CTL;
                if c == '?' { 127 } else { (c as i128) & 0x1f }
            } else {
                c as i128
            }
        }
        KeyCode::Enter => b'\r' as i128,
        KeyCode::Tab => b'\t' as i128,
        KeyCode::Backspace => 127,
        KeyCode::Esc => 27,
        KeyCode::Up => named_code("up"),
        KeyCode::Down => named_code("down"),
        KeyCode::Left => named_code("left"),
        KeyCode::Right => named_code("right"),
        KeyCode::Home => named_code("home"),
        KeyCode::End => named_code("end"),
        KeyCode::PageUp => named_code("prior"),
        KeyCode::PageDown => named_code("next"),
        KeyCode::Delete => named_code("delete"),
        KeyCode::Insert => named_code("insert"),
        KeyCode::F(n) => named_code(&format!("f{}", n)),
        _ => return None,
    };
    Some(base | mods)
}

fn named_code(name: &str) -> i128 {
    crate::editor::event_code_for(name)
}

// ---------- command loop ----------

/// Run the editor until `C-x C-c` (or `kill-emacs`).
pub fn run_editor(i: &mut Interp) -> io::Result<()> {
    run_editor_with(Terminal::enter()?, i)
}

/// The editor command loop, parameterized over a front-end. Also
/// used by the gpui front-end, which supplies a channel-backed KeyIo
/// and runs this on a dedicated logic thread (Interp is !Send).
pub fn run_editor_with<T: KeyIo + 'static>(frontend: T, i: &mut Interp) -> io::Result<()> {
    let term = std::rc::Rc::new(std::cell::RefCell::new(frontend));
    // Install the interactive input hook so read-from-minibuffer,
    // M-x, y-or-n-p, read-char, and `interactive' spec codes can read
    // input from inside the evaluator.
    {
        let t = term.clone();
        i.minibuf_reader = Some(std::rc::Rc::new(move |interp, prompt, single| {
            minibuf_loop::<T>(&t, interp, prompt, single)
        }));
    }
    // Sync frame geometry.
    {
        let (w, h) = term.borrow().size();
        if let Some(f) = &i.selected_frame {
            f.borrow_mut().width = w;
            f.borrow_mut().height = h;
        }
    }
    let mut keys: Vec<i128> = Vec::new();
    // Set while a universal/digit-argument sequence is being entered:
    // digits, `-', and further C-u keep building `prefix-arg' (Emacs's
    // `universal-argument' continuation behavior).
    let mut arg_mode = false;
    loop {
        if i.quit_editor {
            break;
        }
        term.borrow_mut().render(i)?;
        // Replayed macro events take priority over real input;
        // `executing-kbd-macro` clears once the queue drains.
        let key = match i.macro_replay.pop_front() {
            Some(k) => {
                i.macro_replaying = true;
                k
            }
            None => {
                let ek = i.intern("executing-kbd-macro");
                if i.symbol_value(ek).truthy() {
                    let _ = i.set_symbol(ek, Value::Nil);
                }
                // Poll input. Sleep deadlines keep the UI responsive.
                let timeout = Duration::from_millis(50);
                match term.borrow_mut().poll_key(timeout)? {
                    Some(k) => k,
                    None => continue,
                }
            }
        };
        dispatch_key(&term, i, key, &mut keys, &mut arg_mode)?;
        i.macro_replaying = false;
        // A command error while replaying a macro aborts the rest of
        // the replay; the queue was already cleared.
        if let Some(f) = i.macro_pending_flow.take() {
            report_flow(i, f);
        }
    }
    Ok(())
}

/// Process one key event: extend the key sequence, look up the
/// binding, and execute the command (or self-insert / enter isearch).
/// Shared by every front-end via `run_editor_with`.
/// Dispatch one key event through the active keymaps — the shared
/// core for the front-end key loop and `minibuf_read`'s scripted
/// input path.  `isearch` intercepts the four incremental-search
/// commands (TUI passes a closure that re-reads keys; the canned
/// reader supplies `None` so isearch runs as an ordinary command).
pub fn dispatch_key_core(
    i: &mut Interp,
    key: i128,
    keys: &mut Vec<i128>,
    arg_mode: &mut bool,
    mut isearch: Option<&mut dyn FnMut(&mut Interp, bool, bool) -> io::Result<()>>,
) -> io::Result<()> {
    {
        if *arg_mode {
            let digit = (48..58).contains(&key);
            let minus = key == 45;
            let cu = key == 21;
            if digit || minus || cu {
                let lce = i.intern("last-command-event");
                let _ = i.set_symbol(lce, Value::Int(key));
                let name = if cu {
                    "universal-argument"
                } else {
                    "digit-argument"
                };
                let id = i.intern(name);
                if i.command_execute(&Value::Sym(id)).is_ok() {
                    post_command_undo_boundary(i);
                }
                return Ok(());
            }
            *arg_mode = false;
        }
        keys.push(key);
        // Record the event when defining a keyboard macro (replayed
        // keys are not re-recorded, matching GNU).
        if !i.macro_replaying {
            let dm = i.intern("defining-kbd-macro");
            if i.symbol_value(dm).truthy() {
                i.kbd_macro_events.push(Value::Int(key));
            }
        }
        let seq = Value::Vec(std::rc::Rc::new(std::cell::RefCell::new(
            keys.iter().map(|k| Value::Int(*k)).collect(),
        )));
        // Look up the key sequence.
        let binding = lookup_command(i, &seq);
        match binding {
            LookupResult::Command(raw_cmd) => {
                // GNU command_loop_1: a symbol binding is checked
                // against `[remap CMD]' in the active maps first —
                // `self-insert-command' under `y-or-n-p-map' becomes
                // `y-or-n-p-insert-y'.
                let cmd = match &raw_cmd {
                    Value::Sym(_) => crate::editor::command_remapping_pub(i, &raw_cmd)
                        .ok()
                        .filter(|v| v.truthy())
                        .unwrap_or_else(|| raw_cmd.clone()),
                    _ => raw_cmd.clone(),
                };
                keys.clear();
                let lce = i.intern("last-command-event");
                let _ = i.set_symbol(lce, Value::Int(key));
                let this_cmd = i.intern("this-command");
                let prev = i.symbol_value(this_cmd);
                let lc = i.intern("last-command");
                let _ = i.set_symbol(lc, prev);
                let _ = i.set_symbol(this_cmd, cmd.clone());
                let pa = i.intern("prefix-arg");
                let prefix = i.symbol_value(pa);
                let cpa = i.intern("current-prefix-arg");
                let _ = i.set_symbol(cpa, prefix);
                // Consume prefix-arg BEFORE executing so commands that
                // set it (C-u, digit-argument) affect the NEXT command.
                let _ = i.set_symbol(pa, Value::Nil);
                // isearch commands enter a dedicated incremental loop
                // rather than running once through command_execute.
                let isearch_cmd = if let Value::Sym(id) = &cmd {
                    match i.symbol_name(*id).as_str() {
                        "isearch-forward" => Some((false, false)),
                        "isearch-backward" => Some((true, false)),
                        "isearch-forward-regexp" => Some((false, true)),
                        "isearch-backward-regexp" => Some((true, true)),
                        _ => None,
                    }
                } else {
                    None
                };
                if let Some((back, re)) = isearch_cmd {
                    if let Some(hook) = isearch.as_mut() {
                        hook(i, back, re)?;
                        post_command_undo_boundary(i);
                        return Ok(());
                    }
                }
                // GNU command_loop_1: `pre-command-hook' runs before
                // the command, `post-command-hook' after it — even on
                // error (both via safe_run_hooks).
                let _ = crate::lisp::builtins::evalfn::safe_call_hook(i, "pre-command-hook");
                let mut boundary = false;
                match i.command_execute(&cmd) {
                    Ok(_) => {
                        boundary = true;
                    }
                    Err(crate::lisp::error::Flow::Quit) => {
                        if i.minibuf_catching_exit {
                            // Inside read_minibuf's catch: a quit aborts
                            // the read — hand the flow to minibuf_loop.
                            i.minibuf_pending_flow = Some(crate::lisp::error::Flow::Quit);
                        } else if i.macro_replaying || i.macro_strict {
                            // GNU: a quit inside execute-kbd-macro aborts
                            // the macro and propagates.
                            i.macro_replay.clear();
                            i.macro_pending_flow = Some(crate::lisp::error::Flow::Quit);
                        } else {
                            i.message("Quit");
                        }
                    }
                    Err(e @ crate::lisp::error::Flow::Signal(..)) => {
                        if i.macro_replaying || i.macro_strict {
                            // GNU `command_loop_2': an error during a
                            // macro iteration terminates the macro and
                            // propagates out of `execute-kbd-macro'.
                            i.macro_replay.clear();
                            i.macro_pending_flow = Some(e);
                        } else {
                            let crate::lisp::error::Flow::Signal(sym, data, _) = e else {
                                unreachable!()
                            };
                            let name = match &sym {
                                Value::Sym(id) => i.symbol_name(*id),
                                _ => "error".into(),
                            };
                            let msg = format!(
                                "{}: {}",
                                name,
                                data.list_to_vec()
                                    .unwrap_or_default()
                                    .iter()
                                    .map(|v| i.princ_to_string(v))
                                    .collect::<Vec<_>>()
                                    .join(" ")
                            );
                            i.message(&msg);
                        }
                    }
                    Err(crate::lisp::error::Flow::Throw(tag, v)) => {
                        let is_exit = matches!(&tag, Value::Sym(s) if i.symbol_name(*s) == "exit");
                        if i.minibuf_catching_exit && is_exit {
                            // `exit' reached read_minibuf's catch —
                            // GNU recursive_edit_1's value protocol:
                            // nil → contents, t → quit, string →
                            // `error' signal, function → called after
                            // unwinding (minibuffer-quit-recursive-edit).
                            match &v {
                                Value::Nil => i.minibuf_exited = true,
                                Value::Str(s) => {
                                    let msg = Value::string(s.borrow().clone());
                                    let es = Value::Sym(i.intern("error"));
                                    i.minibuf_pending_flow =
                                        Some(crate::lisp::error::Flow::Signal(
                                            es,
                                            Value::list(vec![msg]),
                                            false,
                                        ));
                                }
                                Value::Sym(_) if v.truthy() => {
                                    i.minibuf_pending_flow = Some(crate::lisp::error::Flow::Quit);
                                }
                                _ => {
                                    // Functionp values are called after
                                    // the read unwinds.
                                    i.minibuf_exit_fn = Some(v.clone());
                                    i.minibuf_exited = true;
                                }
                            }
                        } else if i.minibuf_catching_exit {
                            // A throw for another tag unwinds past the
                            // `exit' catch — abort the read with it.
                            i.minibuf_pending_flow = Some(crate::lisp::error::Flow::Throw(tag, v));
                        } else if i.macro_replaying || i.macro_strict {
                            // Uncaught throw aborts the macro (GNU
                            // command_loop_2 → `no-catch' signal).
                            i.macro_replay.clear();
                            i.macro_pending_flow = Some(crate::lisp::error::Flow::Throw(tag, v));
                        } else {
                            i.message(&format!("No catch for tag: {}", i.princ_to_string(&tag)));
                        }
                    }
                    Err(crate::lisp::error::Flow::Exit(_)) => {
                        // `kill-emacs' requested termination; the flag is
                        // already set, so the loop exits at the top.
                        i.quit_editor = true;
                    }
                }
                if boundary {
                    post_command_undo_boundary(i);
                }
                let _ = crate::lisp::builtins::evalfn::safe_call_hook(i, "post-command-hook");
                // Continue arg entry after C-u / M-digit / M--.
                if let Value::Sym(id) = &cmd {
                    let n = i.symbol_name(*id);
                    *arg_mode = matches!(
                        n.as_str(),
                        "universal-argument" | "digit-argument" | "negative-argument"
                    );
                }
                // Command boundary: `cancel-kbd-macro-events` truncates
                // back to this mark.
                i.kbd_macro_mark = i.kbd_macro_events.len();
            }
            LookupResult::Prefix => {
                // keep reading keys
            }
            LookupResult::None => {
                // Plain char → self-insert.  GNU treats these as
                // `self-insert-command'/`newline'/`delete-backward-char'
                // commands: this-command is set and the command hooks run.
                if keys.len() == 1 {
                    if let Some(&k) = keys.first() {
                        let name = if k < CHAR_CTL && k >= 32 && k != 127 {
                            Some("self-insert-command")
                        } else if k == b'\r' as i128 {
                            Some("newline")
                        } else if k == 127 {
                            Some("delete-backward-char")
                        } else {
                            None
                        };
                        if let Some(n) = name {
                            let _ = crate::lisp::builtins::evalfn::safe_call_hook(
                                i,
                                "pre-command-hook",
                            );
                            let this_cmd = i.intern("this-command");
                            let prev = i.symbol_value(this_cmd);
                            let lc = i.intern("last-command");
                            let _ = i.set_symbol(lc, prev);
                            let cmd_sym = Value::Sym(i.intern(n));
                            let _ = i.set_symbol(this_cmd, cmd_sym);
                            if k == 127 {
                                if let Some(b) = i.current_buffer_ref() {
                                    let p = b.borrow().point();
                                    if p > 0 {
                                        let _ = crate::buffer::primitives::chg_delete(i, p - 1, p);
                                    }
                                }
                            } else if i.current_buffer_ref().is_some() {
                                let s = if k == b'\r' as i128 {
                                    "\n".to_string()
                                } else {
                                    char::from_u32(k as u32).unwrap_or('?').to_string()
                                };
                                let _ = crate::buffer::primitives::chg_insert_pt(i, &s, false);
                            }
                            keys.clear();
                            post_command_undo_boundary(i);
                            let _ = crate::lisp::builtins::evalfn::safe_call_hook(
                                i,
                                "post-command-hook",
                            );
                            return Ok(());
                        }
                    }
                }
                i.message(&format!(
                    "{} is undefined",
                    keys.iter()
                        .map(|k| crate::editor::describe_key_pub(*k))
                        .collect::<Vec<_>>()
                        .join(" ")
                ));
                keys.clear();
            }
        }
    }
    Ok(())
}

/// Drain the replay queue synchronously, dispatching each queued event
/// through `dispatch_key_core` — the batch/nested path of
/// `execute-kbd-macro` (GNU `command_loop_2' inside `Fexecute_kbd_macro').
/// `stop` is the queue length at which to stop: a nested
/// `execute-kbd-macro' call passes the length it found on entry so the
/// outer call's still-queued events are left for the outer drain.
/// Stops early when a command error set `macro_pending_flow'.
pub fn replay_macro_events(i: &mut Interp, stop: usize) {
    let mut keys: Vec<i128> = Vec::new();
    let mut arg_mode = false;
    // Events consumed in this drain — GNU's executing_kbd_macro_index,
    // the number already read from the current iteration.
    let mut idx: i128 = 0;
    while i.macro_replay.len() > stop {
        let Some(k) = i.macro_replay.pop_front() else {
            break;
        };
        let eki = i.intern("executing-kbd-macro-index");
        let _ = i.set_symbol(eki, Value::Int(idx));
        idx += 1;
        i.macro_replaying = true;
        if dispatch_key_core(i, k, &mut keys, &mut arg_mode, None).is_err() {
            i.macro_replaying = false;
            return;
        }
        i.macro_replaying = false;
        if i.macro_pending_flow.is_some() || i.quit_editor {
            return;
        }
    }
}

/// Echo a deferred command flow (abort/uncaught-throw from a macro
/// replay) the same way `dispatch_key_core' reports live errors.
fn report_flow(i: &mut Interp, f: crate::lisp::error::Flow) {
    match f {
        crate::lisp::error::Flow::Quit => i.message("Quit"),
        crate::lisp::error::Flow::Signal(sym, data, _) => {
            let name = match &sym {
                Value::Sym(id) => i.symbol_name(*id),
                _ => "error".into(),
            };
            let msg = format!(
                "{}: {}",
                name,
                data.list_to_vec()
                    .unwrap_or_default()
                    .iter()
                    .map(|v| i.princ_to_string(v))
                    .collect::<Vec<_>>()
                    .join(" ")
            );
            i.message(&msg);
        }
        crate::lisp::error::Flow::Throw(tag, _) => {
            i.message(&format!("No catch for tag: {}", i.princ_to_string(&tag)));
        }
        crate::lisp::error::Flow::Exit(_) => i.quit_editor = true,
    }
}

fn dispatch_key<T: KeyIo>(
    term: &std::rc::Rc<std::cell::RefCell<T>>,
    i: &mut Interp,
    key: i128,
    keys: &mut Vec<i128>,
    arg_mode: &mut bool,
) -> io::Result<()> {
    let mut hook = |i: &mut Interp, back: bool, re: bool| isearch_loop(term, i, back, re);
    dispatch_key_core(i, key, keys, arg_mode, Some(&mut hook))
}

/// The front-end-independent half of `minibuf_loop' for a canned/
/// scripted input source: dispatch one `MinibufInput::Key' through
/// the minibuffer's real keymaps.  `minibuf_read' owns `keys'/
/// `arg_mode' across calls, then does minibuf_loop's post-dispatch
/// checks (minibuf_exit_fn / minibuf_exited / minibuf_pending_flow).
pub(crate) fn minibuf_dispatch_key(
    i: &mut Interp,
    key: i128,
    keys: &mut Vec<i128>,
    arg_mode: &mut bool,
) -> Result<(), crate::lisp::error::Flow> {
    dispatch_key_core(i, key, keys, arg_mode, None)
        .map_err(|e| i.error(format!("minibuffer input: {e}")))
}

/// Incremental search loop (C-s / C-r). Reads keys directly until the
/// search exits via RET, C-g, or a non-search key (which is then
/// re-dispatched as a command, like Emacs).
///
/// `point` semantics follow Emacs: forward searches leave point after
/// the match, backward searches leave it at the match start. C-g
/// restores the entry position; RET exits and sets the mark there.
/// Live state of an incremental search.
struct Isearch {
    query: String,
    /// History of match positions; last is the current match.
    positions: Vec<usize>,
    /// Match spans (beg, end) parallel to `positions`.
    spans: Vec<(usize, usize)>,
    failing: bool,
    start: usize,
    backward: bool,
    regexp: bool,
    /// The `isearch' overlay on the current match (handle or None).
    match_ov: Option<Value>,
    /// `lazy-highlight' overlays on the other visible matches.
    lazy_ovs: Vec<Value>,
}

/// Run one search; returns (point, match-beg, match-end) — all
/// 0-based — or None when the query doesn't match from `from`.
fn isearch_search(
    i: &mut Interp,
    query: &str,
    backward: bool,
    regexp: bool,
    from: usize,
) -> Option<(usize, usize, usize)> {
    if query.is_empty() {
        return Some((from, from, from));
    }
    let mut esc = String::new();
    for c in query.chars() {
        if c == '\\' || c == '"' {
            esc.push('\\');
        }
        esc.push(c);
    }
    let fn_name = match (backward, regexp) {
        (false, false) => "search-forward",
        (true, false) => "search-backward",
        (false, true) => "re-search-forward",
        (true, true) => "re-search-backward",
    };
    // +1 for the point→char offset used by these primitives.
    let src = format!(
        "(progn (goto-char (min {} (point-max))) \
           (and ({} \"{}\" nil t) \
                (list (1- (point)) (1- (match-beginning 0)) (1- (match-end 0)))))",
        from + 1,
        fn_name,
        esc
    );
    match i.eval_str(&src) {
        Ok(v) => {
            let items = v.list_to_vec().ok()?;
            let p = items.first()?.int()? as usize;
            let b = items.get(1)?.int()? as usize;
            let e = items.get(2)?.int()? as usize;
            Some((p, b, e))
        }
        _ => None,
    }
}

/// What the isearch driver should do after a key.
enum IsearchAction {
    /// Keep reading keys.
    Continue,
    /// Exit, restoring point to the entry position (C-g).
    Abort,
    /// Exit, keeping point and pushing the mark at entry (RET).
    Done,
    /// Exit and re-dispatch this key as a command.
    ReDispatch(i128),
}

impl Isearch {
    fn new(i: &Interp, backward: bool, regexp: bool) -> Isearch {
        let start = i
            .current_buffer_ref()
            .map(|b| b.borrow().point())
            .unwrap_or(0);
        Isearch {
            query: String::new(),
            positions: vec![start],
            spans: vec![(start, start)],
            failing: false,
            start,
            backward,
            regexp,
            match_ov: None,
            lazy_ovs: Vec::new(),
        }
    }

    fn cur(&self) -> usize {
        *self.positions.last().unwrap_or(&self.start)
    }

    fn go_to(&self, i: &Interp, p: usize) {
        if let Some(b) = i.current_buffer_ref() {
            b.borrow_mut().set_point(p);
        }
    }

    /// Handle one key event; updates query/point/failing state.
    fn step(&mut self, i: &mut Interp, key: i128) -> IsearchAction {
        let cur = self.cur();
        match key {
            // C-g: abort, restore entry position.
            7 => {
                self.go_to(i, self.start);
                IsearchAction::Abort
            }
            // RET: accept; mark goes at the entry position.
            13 => {
                if let Some(b) = i.current_buffer_ref() {
                    b.borrow_mut().set_mark(self.start);
                }
                IsearchAction::Done
            }
            // C-s / C-r: repeat search in that direction from just
            // past (before) the current match.
            19 | 18 => {
                let back = key == 18;
                let from = if back { cur.saturating_sub(1) } else { cur + 1 };
                match isearch_search(i, &self.query, back, self.regexp, from) {
                    Some((p, b, e)) => {
                        self.positions.push(p);
                        self.spans.push((b, e));
                        self.failing = false;
                        self.go_to(i, p);
                    }
                    None => self.failing = true,
                }
                IsearchAction::Continue
            }
            // DEL: unwind one step of the search.
            127 => {
                if self.positions.len() > 1 {
                    self.positions.pop();
                    self.spans.pop();
                    // If the tail of query grew since the last repeat,
                    // pop a char too.
                    if !self.query.is_empty() {
                        self.query.pop();
                    }
                    self.go_to(i, self.cur());
                    self.failing = false;
                } else if !self.query.is_empty() {
                    self.query.pop();
                    self.failing = false;
                }
                IsearchAction::Continue
            }
            c if c >= 32
                && c < 0x110000
                && c & (CHAR_META | CHAR_CTL | CHAR_SHIFT | CHAR_SUPER | CHAR_HYPER) == 0 =>
            {
                if let Some(ch) = char::from_u32(c as u32) {
                    self.query.push(ch);
                }
                // Extend the current match like GNU's
                // `isearch-search-and-update': forward searches
                // restart at the match's START (`isearch-other-end')
                // so the match grows in place; backward searches try
                // `looking-at' at the match start first (the new
                // match must not reach past the entry point), else
                // fall back to a real backward search.
                let hit = if self.positions.len() > 1 {
                    let (mb, me) = *self.spans.last().unwrap_or(&(cur, cur));
                    if self.backward {
                        match isearch_search(i, &self.query, false, self.regexp, mb) {
                            Some((_, b, e)) if b == mb && e <= self.start => Some((b, b, e)),
                            _ => isearch_search(
                                i,
                                &self.query,
                                true,
                                self.regexp,
                                self.start.min(me + 1),
                            ),
                        }
                    } else {
                        isearch_search(i, &self.query, false, self.regexp, mb)
                    }
                } else {
                    isearch_search(i, &self.query, self.backward, self.regexp, self.start)
                };
                match hit {
                    Some((p, b, e)) => {
                        self.positions.push(p);
                        self.spans.push((b, e));
                        self.failing = false;
                        self.go_to(i, p);
                    }
                    None => self.failing = true,
                }
                IsearchAction::Continue
            }
            other => {
                if let Some(b) = i.current_buffer_ref() {
                    b.borrow_mut().set_mark(self.start);
                }
                IsearchAction::ReDispatch(other)
            }
        }
    }

    /// Echo-area label, e.g. `Failing I-search backward: foo`.
    fn label(&self) -> String {
        let dir = if self.backward { " backward" } else { "" };
        let re = if self.regexp { " regexp" } else { "" };
        let status = if self.failing { "Failing " } else { "" };
        format!("{}I-search{}{}: {}", status, dir, re, self.query)
    }

    /// One `apply' of a Lisp helper; errors are ignored (highlighting
    /// must never break the search itself).
    fn call(i: &mut Interp, name: &str, args: Vec<Value>) {
        let f = match i.intern_soft(name) {
            Some(s) => Value::Sym(s),
            None => return,
        };
        let _ = i.apply(&f, args);
    }

    /// Delete the highlight overlays (`isearch-dehighlight').
    fn clear(&mut self, i: &mut Interp) {
        if let Some(ov) = self.match_ov.take() {
            Self::call(i, "delete-overlay", vec![ov]);
        }
        for ov in self.lazy_ovs.drain(..) {
            Self::call(i, "delete-overlay", vec![ov]);
        }
    }

    /// Rebuild the highlight overlays after each key like GNU's
    /// `isearch-highlight'/`isearch-lazy-highlight-search':
    /// `isearch' face + priority 1001 on the current match,
    /// `lazy-highlight' + priority 1000 + `window' on the other
    /// matches visible in the selected window.  `isearch.el' isn't
    /// booted, so the faces resolve through render_grid's
    /// compiled-in fallbacks.
    fn refresh_overlays(&mut self, i: &mut Interp) {
        // Where the last key left point (an abort may have moved it
        // off the match); the lazy scan below must restore exactly
        // this afterwards.
        let restore = i
            .current_buffer_ref()
            .map(|b| b.borrow().point())
            .unwrap_or(self.cur());
        for ov in self.lazy_ovs.drain(..) {
            Self::call(i, "delete-overlay", vec![ov]);
        }
        let span = *self.spans.last().unwrap_or(&(self.start, self.start));
        // No highlight while the query is empty (GNU shows nothing
        // until the first char); a failing search keeps the last
        // match highlighted like GNU does.  `search-highlight' gates
        // the current match, `isearch-lazy-highlight' the rest —
        // both unbound → default t.
        let hl_on = i
            .intern_soft("search-highlight")
            .map(|s| i.symbol_value(s).truthy())
            .unwrap_or(true);
        let lazy_on = i
            .intern_soft("isearch-lazy-highlight")
            .map(|s| i.symbol_value(s).truthy())
            .unwrap_or(true);
        let active = !self.query.is_empty() && span.0 < span.1 && hl_on;
        match (self.match_ov.clone(), active) {
            (Some(ov), true) => {
                Self::call(
                    i,
                    "move-overlay",
                    vec![
                        ov,
                        Value::Int(span.0 as i128 + 1),
                        Value::Int(span.1 as i128 + 1),
                    ],
                );
            }
            (None, true) => {
                let f = Value::Sym(i.intern("make-overlay"));
                if let Ok(ov) = i.apply(
                    &f,
                    vec![
                        Value::Int(span.0 as i128 + 1),
                        Value::Int(span.1 as i128 + 1),
                    ],
                ) {
                    let pri = Value::Sym(i.intern("priority"));
                    let face = Value::Sym(i.intern("face"));
                    let isearch = Value::Sym(i.intern("isearch"));
                    Self::call(i, "overlay-put", vec![ov.clone(), pri, Value::Int(1001)]);
                    Self::call(i, "overlay-put", vec![ov.clone(), face, isearch]);
                    self.match_ov = Some(ov);
                }
            }
            (Some(ov), false) => {
                Self::call(i, "delete-overlay", vec![ov]);
                self.match_ov = None;
            }
            (None, false) => {}
        }
        if !active || self.failing || !lazy_on {
            return;
        }
        let sw = Value::Sym(i.intern("selected-window"));
        let wv = match i.apply(&sw, vec![]) {
            Ok(v) => v,
            Err(_) => return,
        };
        let ws_sym = i.intern("window-start");
        let we_sym = i.intern("window-end");
        let wstart = i
            .apply(&Value::Sym(ws_sym), vec![wv.clone()])
            .ok()
            .and_then(|v| v.int())
            .unwrap_or(1)
            .max(1) as usize;
        let wend = i
            .apply(&Value::Sym(we_sym), vec![wv.clone(), Value::t()])
            .ok()
            .and_then(|v| v.int())
            .map(|n| n.max(1) as usize)
            .unwrap_or(usize::MAX);
        let mut pos = wstart;
        let mut count = 0usize;
        let pri = Value::Sym(i.intern("priority"));
        let face = Value::Sym(i.intern("face"));
        let lazy = Value::Sym(i.intern("lazy-highlight"));
        let winprop = Value::Sym(i.intern("window"));
        while count < 200 {
            let Some((_, b, e)) = isearch_search(i, &self.query, false, self.regexp, pos) else {
                break;
            };
            if b >= wend || e <= b {
                break;
            }
            pos = e;
            if (b, e) == span {
                continue;
            }
            let f = Value::Sym(i.intern("make-overlay"));
            let Ok(ov) = i.apply(
                &f,
                vec![Value::Int(b as i128 + 1), Value::Int(e as i128 + 1)],
            ) else {
                break;
            };
            Self::call(i, "overlay-put", vec![ov.clone(), pri.clone(), Value::Int(1000)]);
            Self::call(i, "overlay-put", vec![ov.clone(), face.clone(), lazy.clone()]);
            Self::call(i, "overlay-put", vec![ov.clone(), winprop.clone(), wv.clone()]);
            self.lazy_ovs.push(ov);
            count += 1;
        }
        // The scan's searches moved point to the last visible match;
        // put it back where the last key left it (GNU does the same
        // through save-excursion).
        self.go_to(i, restore);
    }
}

/// Incremental search loop (C-s / C-r). Reads keys directly until the
/// search exits via RET, C-g, or a non-search key (which is then
/// re-dispatched as a command, like Emacs).
fn isearch_loop<T: KeyIo>(
    term: &std::rc::Rc<std::cell::RefCell<T>>,
    i: &mut Interp,
    backward: bool,
    regexp: bool,
) -> io::Result<()> {
    let mut st = Isearch::new(i, backward, regexp);
    loop {
        {
            let mut t = term.borrow_mut();
            let _ = t.render(i);
            let _ = t.draw_echo(&st.label());
        }
        let key = term
            .borrow_mut()
            .poll_key(Duration::from_secs(86400))?
            .unwrap_or(0);
        let action = st.step(i, key);
        // Rebuild the isearch/lazy-highlight overlays so the next
        // render paints the matches (GNU: isearch-update →
        // isearch-highlight + lazy-highlight).
        st.refresh_overlays(i);
        match action {
            IsearchAction::Continue => {}
            IsearchAction::Abort | IsearchAction::Done => break,
            IsearchAction::ReDispatch(other) => {
                st.clear(i);
                let seq = Value::Vec(std::rc::Rc::new(std::cell::RefCell::new(vec![Value::Int(
                    other,
                )])));
                if let LookupResult::Command(cmd) = lookup_command(i, &seq) {
                    if i.command_execute(&cmd).is_ok() {
                        post_command_undo_boundary(i);
                    }
                }
                return Ok(());
            }
        }
    }
    st.clear(i);
    Ok(())
}

/// GNU `command_loop_1' calls `undo-boundary' after each successfully
/// executed command; this pushes a nil boundary into the current
/// buffer's `buffer-undo-list' unless one is already there.
fn post_command_undo_boundary(i: &mut Interp) {
    if let Some(b) = i.current_buffer_ref() {
        b.borrow_mut().undo_boundary();
    }
}

enum LookupResult {
    Command(Value),
    Prefix,
    None,
}

fn lookup_command(i: &mut Interp, seq: &Value) -> LookupResult {
    let keys = match seq {
        Value::Vec(v) => v.borrow().clone(),
        _ => return LookupResult::None,
    };
    if keys.is_empty() {
        return LookupResult::None;
    }
    // Convert key vector to string for keymap lookup if all plain chars.
    // Our define-key accepts vectors directly.
    let binding = match crate::editor::lookup_command_in_maps(i, &keys) {
        Ok(b) => b,
        // A `:filter' (or other keyelt evaluation) error counts as no
        // binding — GNU treats an unfound command the same.
        Err(_) => None,
    };
    match binding {
        Some(v) => {
            // is it a keymap (prefix)?
            if let Value::Cons(c) = &v {
                if i.sym_is(&c.borrow().car, i.intern_soft("keymap").unwrap_or(u32::MAX)) {
                    return LookupResult::Prefix;
                }
            }
            if v.truthy() {
                LookupResult::Command(v)
            } else {
                LookupResult::None
            }
        }
        None => {
            // If no binding and the last key extends a prefix → undefined.
            LookupResult::None
        }
    }
}

/// Blocking `sleep-for` replacement at the loop level: repaint during the wait.
pub fn nonblocking_sleep(i: &mut Interp, dur: Duration) -> io::Result<()> {
    let _ = i;
    let deadline = Instant::now() + dur;
    while Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    Ok(())
}

/// Nested input loop for minibuffer reads. Runs while the outer
/// command loop is suspended inside `command_execute`.
fn minibuf_loop<T: KeyIo>(
    term: &std::rc::Rc<std::cell::RefCell<T>>,
    i: &mut Interp,
    prompt: &str,
    single: bool,
) -> Result<crate::lisp::eval::MinibufInput, crate::lisp::error::Flow> {
    use crate::lisp::eval::MinibufInput;
    // GNU's `read_minibuf' runs the ordinary command loop inside the
    // `exit' catch: keys dispatch through the minibuffer's keymaps,
    // `self-insert-command'/`delete-backward-char' edit the real
    // buffer (the prompt's `read-only'/`field' props protect it),
    // `pre-command-hook'/`post-command-hook' run per command (what
    // icomplete & friends hang on), and `exit-minibuffer' throws
    // `exit' to end the read.
    let mut keys: Vec<i128> = Vec::new();
    let mut arg_mode = false;
    loop {
        {
            let mut t = term.borrow_mut();
            let shown = if single {
                prompt.to_string()
            } else {
                minibuf_echo_text(i, prompt)
            };
            let _ = t.draw_echo(&shown);
        }
        // Propagate io errors — swallowing them as None spins an
        // infinite poll loop on a dead input source.
        let code = match term.borrow_mut().poll_key(Duration::from_secs(86400)) {
            Ok(Some(k)) => k,
            Ok(None) => continue,
            Err(e) => return Err(i.error(format!("minibuffer input: {e}"))),
        };
        if single {
            return Ok(MinibufInput::Key(code));
        }
        if let Err(e) = dispatch_key(term, i, code, &mut keys, &mut arg_mode) {
            return Err(i.error(format!("minibuffer input: {e}")));
        }
        if let Some(f) = i.minibuf_exit_fn.take() {
            return Ok(MinibufInput::Call(f));
        }
        if i.minibuf_exited {
            return Ok(MinibufInput::Text(i.minibuf_contents()));
        }
        if let Some(f) = i.minibuf_pending_flow.take() {
            return Err(f);
        }
    }
}

/// The minibuffer's display text for the echo area: buffer text plus
/// overlay `before-string'/`after-string' contributions, like GNU's
/// redisplay (icomplete shows its prospects through an `after-string'
/// on `icomplete-overlay').
fn minibuf_echo_text(i: &mut Interp, prompt: &str) -> String {
    let Some(b) = i.buffers.get(i.current_buffer) else {
        return prompt.to_string();
    };
    let bb = b.borrow();
    let text = bb.text.text();
    // Collect (pos, string) splices; after-string goes at the
    // overlay's end, before-string at its start.
    let before_sym = i.intern("before-string");
    let after_sym = i.intern("after-string");
    let mut splices: Vec<(usize, usize, String)> = Vec::new();
    for ov in &bb.overlays {
        if ov.buffer.is_none() {
            continue;
        }
        let bs = crate::lisp::eval::plist_get(&ov.plist, before_sym);
        let as_ = crate::lisp::eval::plist_get(&ov.plist, after_sym);
        for (v, at_start) in [(bs, true), (as_, false)] {
            if let Value::Str(s) = &v {
                let pos = if at_start { ov.start } else { ov.end };
                splices.push((pos, usize::from(!at_start), s.borrow().clone()));
            }
        }
    }
    if splices.is_empty() {
        return text;
    }
    // Splice right-to-left so positions stay valid; later-positioned
    // strings at the same spot come after earlier ones.
    splices.sort_by_key(|&(pos, is_after, ref _s)| (pos, is_after));
    let mut chars: Vec<char> = text.chars().collect();
    for (pos, _, s) in splices.into_iter().rev() {
        let p = pos.min(chars.len());
        let tail: Vec<char> = chars.split_off(p);
        chars.extend(s.chars());
        chars.extend(tail);
    }
    chars.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
    use std::cell::RefCell;
    use std::rc::Rc;

    /// A `Write` sink over a shared buffer so tests can inspect what
    /// `render`/`draw_echo` emitted.
    struct SharedBuf(Rc<RefCell<Vec<u8>>>);
    impl io::Write for SharedBuf {
        fn write(&mut self, b: &[u8]) -> io::Result<usize> {
            self.0.borrow_mut().extend_from_slice(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn test_term(w: usize, h: usize) -> (Terminal, Rc<RefCell<Vec<u8>>>) {
        let buf = Rc::new(RefCell::new(Vec::new()));
        (
            Terminal {
                width: w,
                height: h,
                out: Box::new(SharedBuf(buf.clone())),
                pending: Vec::new(),
            },
            buf,
        )
    }

    /// An Interp with a tty frame displaying a buffer holding `text`.
    fn interp_with_frame(text: &str) -> crate::lisp::Interp {
        let mut i = crate::lisp::Interp::new();
        let buf_id = i.create_buffer("test-buf");
        {
            let b = i.buffers.get(buf_id).unwrap();
            b.borrow_mut().insert(text);
            b.borrow_mut().set_point(0);
        }
        let mb_id = i.create_buffer(" *Minibuf-0*");
        let frame = crate::editor::Frame::new_tty(buf_id, mb_id, 80, 25);
        i.selected_frame = Some(frame.clone());
        i.frames.push(frame);
        i.set_current_buffer(buf_id);
        i
    }

    #[test]
    fn render_draws_buffer_and_mode_line() {
        let (mut t, out) = test_term(80, 25);
        let i = interp_with_frame("hello\nworld");
        t.render(&i).unwrap();
        let s = String::from_utf8_lossy(&out.borrow()).to_string();
        assert!(s.contains("hello"), "render should show buffer text");
        assert!(s.contains("test-buf"), "mode line shows buffer name");
        assert!(s.contains("\x1b[7m"), "mode line is reverse-video");
        let _ = i;
    }

    #[test]
    fn render_shows_echo_message_when_minibuf_empty() {
        let (mut t, out) = test_term(40, 10);
        let mut i = interp_with_frame("x");
        i.message("a message");
        t.render(&i).unwrap();
        let s = String::from_utf8_lossy(&out.borrow()).to_string();
        assert!(s.contains("a message"));
    }

    #[test]
    fn render_minibuffer_text_wins_over_echo() {
        let (mut t, out) = test_term(40, 10);
        let mut i = interp_with_frame("x");
        i.message("echo-msg");
        let mb = {
            let f = i.selected_frame.as_ref().unwrap().borrow();
            f.minibuffer.clone().unwrap().borrow().buffer
        };
        i.buffers.get(mb).unwrap().borrow_mut().insert("mini-text");
        t.render(&i).unwrap();
        let s = String::from_utf8_lossy(&out.borrow()).to_string();
        assert!(s.contains("mini-text"));
        assert!(!s.contains("echo-msg"));
    }

    #[test]
    fn draw_echo_pads_and_positions_cursor() {
        let (mut t, out) = test_term(20, 6);
        t.draw_echo("Prompt: abc").unwrap();
        let s = String::from_utf8_lossy(&out.borrow()).to_string();
        assert!(s.contains("Prompt: abc"));
        assert!(s.contains("\x1b["), "uses cursor-move escapes");
    }

    #[test]
    fn isearch_extends_match_in_place_and_overlays_paint() {
        let mut i = interp_with_frame("xxx yyy xxx\n");
        let mut st = Isearch::new(&i, false, false);
        for c in "xxx".chars() {
            st.step(&mut i, c as i128);
        }
        // GNU semantics: each char re-searches from the match's
        // START (`isearch-other-end'), so the span grows in place.
        assert_eq!(st.spans, vec![(0, 0), (0, 1), (0, 2), (0, 3)]);
        assert_eq!(st.positions, vec![0, 1, 2, 3]);
        st.refresh_overlays(&mut i);
        let grid = render_grid(&i, 80, 24);
        // Current match 0..3 inverse (isearch), the second xxx at
        // 8..11 lazy-highlighted (underline).
        assert!(
            grid.attrs[0][..3].iter().all(|c| c.inverse),
            "{:?}",
            &grid.attrs[0][..4]
        );
        assert!(
            grid.attrs[0][8..11].iter().all(|c| c.underline),
            "{:?}",
            &grid.attrs[0][8..11]
        );
        assert!(!grid.attrs[0][4].inverse && !grid.attrs[0][4].underline);
    }

    #[test]
    fn unread_key_is_returned_by_poll() {
        let (mut t, _) = test_term(10, 4);
        t.unread(97);
        assert_eq!(t.pending.pop(), Some(97));
    }

    #[test]
    fn render_handles_multi_window_frame() {
        let (mut t, out) = test_term(80, 10);
        let mut i = interp_with_frame("w1");
        // Split the frame's window in two.
        let f = i.selected_frame.as_ref().unwrap().clone();
        let buf2 = i.create_buffer("buf2");
        i.buffers.get(buf2).unwrap().borrow_mut().insert("w2");
        let w2 = crate::editor::Window::new(buf2);
        {
            let mut fb = f.borrow_mut();
            fb.windows.push(w2);
        }
        t.render(&i).unwrap();
        let s = String::from_utf8_lossy(&out.borrow()).to_string();
        assert!(s.contains("w1") && s.contains("w2"));
    }

    /// Interp whose current buffer holds `text`, point at start.
    fn isearch_interp(text: &str) -> crate::lisp::Interp {
        let mut i = crate::lisp::Interp::new();
        let buf_id = i.create_buffer("s");
        i.set_current_buffer(buf_id);
        i.buffers.get(buf_id).unwrap().borrow_mut().insert(text);
        i.buffers.get(buf_id).unwrap().borrow_mut().set_point(0);
        i
    }

    fn point(i: &Interp) -> usize {
        i.current_buffer_ref().unwrap().borrow().point()
    }

    #[test]
    fn isearch_chars_move_point_to_match() {
        let mut i = isearch_interp("one two one\n");
        let mut st = Isearch::new(&i, false, false);
        assert!(matches!(
            st.step(&mut i, 't' as i128),
            IsearchAction::Continue
        ));
        assert!(matches!(
            st.step(&mut i, 'w' as i128),
            IsearchAction::Continue
        ));
        assert!(matches!(
            st.step(&mut i, 'o' as i128),
            IsearchAction::Continue
        ));
        // "two" ends at index 7.
        assert_eq!(point(&i), 7);
        assert!(!st.failing);
        assert!(st.label().contains("I-search: two"));
    }

    #[test]
    fn isearch_cs_repeats_and_del_unwinds() {
        let mut i = isearch_interp("aa aa\n");
        let mut st = Isearch::new(&i, false, false);
        st.step(&mut i, 'a' as i128);
        let first = point(&i);
        st.step(&mut i, 19); // C-s → next match
        assert!(point(&i) > first);
        st.step(&mut i, 19); // no third match → failing
        assert!(st.failing);
        st.step(&mut i, 127); // DEL unwinds
        assert_eq!(point(&i), first);
    }

    #[test]
    fn isearch_cg_restores_entry() {
        let mut i = isearch_interp("xx yy\n");
        let mut st = Isearch::new(&i, false, false);
        st.step(&mut i, 'y' as i128);
        assert!(point(&i) > 0);
        assert!(matches!(st.step(&mut i, 7), IsearchAction::Abort));
        assert_eq!(point(&i), 0);
    }

    #[test]
    fn isearch_ret_sets_mark() {
        let mut i = isearch_interp("xx yy\n");
        let mut st = Isearch::new(&i, false, false);
        st.step(&mut i, 'y' as i128);
        assert!(matches!(st.step(&mut i, 13), IsearchAction::Done));
        let b = i.current_buffer_ref().unwrap();
        let bb = b.borrow();
        assert_eq!(bb.mark_position(), Some(0));
        assert!(bb.point() > 0);
    }

    #[test]
    fn isearch_backward_searches_backward() {
        let mut i = isearch_interp("aa bb aa\n");
        // Point at end.
        i.current_buffer_ref().unwrap().borrow_mut().set_point(8);
        let mut st = Isearch::new(&i, true, false);
        st.step(&mut i, 'a' as i128);
        // Backward match puts point at match start (index 7).
        assert_eq!(point(&i), 7);
        assert!(st.label().contains("backward"));
    }

    #[test]
    fn isearch_other_key_redispatches() {
        let mut i = isearch_interp("text\n");
        let mut st = Isearch::new(&i, false, false);
        st.step(&mut i, 'x' as i128);
        match st.step(&mut i, 24) {
            IsearchAction::ReDispatch(k) => assert_eq!(k, 24),
            _ => panic!("C-x should re-dispatch"),
        }
    }

    #[test]
    fn isearch_failing_then_empty_query() {
        let mut i = isearch_interp("abc\n");
        let mut st = Isearch::new(&i, false, false);
        st.step(&mut i, 'z' as i128);
        st.step(&mut i, 'z' as i128);
        assert!(st.failing);
        assert!(st.label().starts_with("Failing"));
        st.step(&mut i, 127); // DEL pops one char
        st.step(&mut i, 127);
        assert_eq!(st.query, "");
    }

    // ---------- key translation ----------

    #[test]
    fn key_event_to_code_plain_and_modified_chars() {
        assert_eq!(
            key_event_to_code(ev(KeyCode::Char('a'), KeyModifiers::NONE)),
            Some('a' as i128)
        );
        // C-u folds to 21.
        assert_eq!(
            key_event_to_code(ev(KeyCode::Char('u'), KeyModifiers::CONTROL)),
            Some(21)
        );
        // C-? folds to 127 (DEL).
        assert_eq!(
            key_event_to_code(ev(KeyCode::Char('?'), KeyModifiers::CONTROL)),
            Some(127)
        );
        // Meta sets CHAR_META on the char.
        let m = key_event_to_code(ev(KeyCode::Char('x'), KeyModifiers::ALT)).unwrap();
        assert_eq!(m & !0x3f_ffff, CHAR_META);
        // Shift on a printable char is folded into the character.
        let s = key_event_to_code(ev(KeyCode::Char('A'), KeyModifiers::SHIFT)).unwrap();
        assert_eq!(s, 'A' as i128);
        // C-M-a keeps meta + folds control.
        let cm = key_event_to_code(ev(
            KeyCode::Char('a'),
            KeyModifiers::CONTROL | KeyModifiers::ALT,
        ))
        .unwrap();
        assert_eq!(cm & !0x3f_ffff, CHAR_META);
        assert_eq!(cm & 0x3f_ffff, 1);
        // Super/hyper pass through as modifier bits.
        let sup = key_event_to_code(ev(KeyCode::Char('a'), KeyModifiers::SUPER)).unwrap();
        assert_eq!(sup & !0x3f_ffff, CHAR_SUPER);
        let hyp = key_event_to_code(ev(KeyCode::Char('a'), KeyModifiers::HYPER)).unwrap();
        assert_eq!(hyp & !0x3f_ffff, CHAR_HYPER);
    }

    #[test]
    fn key_event_to_code_named_keys() {
        assert_eq!(
            key_event_to_code(ev(KeyCode::Enter, KeyModifiers::NONE)),
            Some(13)
        );
        assert_eq!(
            key_event_to_code(ev(KeyCode::Tab, KeyModifiers::NONE)),
            Some(9)
        );
        assert_eq!(
            key_event_to_code(ev(KeyCode::Backspace, KeyModifiers::NONE)),
            Some(127)
        );
        assert_eq!(
            key_event_to_code(ev(KeyCode::Esc, KeyModifiers::NONE)),
            Some(27)
        );
        for code in [
            KeyCode::Up,
            KeyCode::Down,
            KeyCode::Left,
            KeyCode::Right,
            KeyCode::Home,
            KeyCode::End,
            KeyCode::PageUp,
            KeyCode::PageDown,
            KeyCode::Delete,
            KeyCode::Insert,
        ] {
            assert!(key_event_to_code(ev(code, KeyModifiers::NONE)).is_some());
        }
        assert!(key_event_to_code(ev(KeyCode::F(5), KeyModifiers::NONE)).is_some());
        // Unmapped codes yield None.
        assert_eq!(
            key_event_to_code(ev(KeyCode::Null, KeyModifiers::NONE)),
            None
        );
    }

    // ---------- input loops ----------

    /// An Interp whose current buffer is a minibuffer that already
    /// holds `prompt` (what `minibuf_read' leaves for the reader):
    /// the prompt, the local map, the `exit' catch, and the prompt
    /// stack entry `minibuf_contents' reads.
    fn interp_with_minibuffer(prompt: &str) -> crate::lisp::Interp {
        let mut i = crate::lisp::Interp::new();
        let mb = i.create_buffer(" *Minibuf-1*");
        let map = i
            .intern_soft("minibuffer-local-map")
            .map(|id| i.symbol_value(id))
            .unwrap_or(Value::Nil);
        let km = i.intern("local-keymap");
        {
            let b = i.buffers.get(mb).unwrap();
            let mut bb = b.borrow_mut();
            bb.insert(prompt);
            bb.locals.insert(km, map);
        }
        i.set_current_buffer(mb);
        i.minibuf_prompts.push(prompt.to_string());
        i.minibuf_level = 1;
        i.minibuf_catching_exit = true;
        let exit_sym = Value::Sym(i.intern("exit"));
        i.catch_tags.push(exit_sym);
        i
    }

    #[test]
    fn minibuf_loop_reads_text() {
        let (t, _out) = test_term(20, 5);
        let term = Rc::new(RefCell::new(t));
        for k in [b'a' as i128, b'b' as i128, 13] {
            term.borrow_mut().unread(k);
        }
        // pending is a stack — reverse order.
        term.borrow_mut().pending.reverse();
        let mut i = interp_with_minibuffer("P: ");
        match minibuf_loop(&term, &mut i, "P: ", false) {
            Ok(crate::lisp::eval::MinibufInput::Text(s)) => assert_eq!(s, "ab"),
            other => panic!("expected Text, got {:?}", other.map(|_| ())),
        }
        // The typed text lives in the minibuffer buffer.
        let b = i.current_buffer_ref().unwrap();
        assert_eq!(b.borrow().text.text(), "P: ab");
    }

    #[test]
    fn minibuf_loop_del_and_cg() {
        let (t, _out) = test_term(20, 5);
        let term = Rc::new(RefCell::new(t));
        // type 'x', DEL, then C-g abort.
        for k in [b'x' as i128, 127, 7] {
            term.borrow_mut().unread(k);
        }
        term.borrow_mut().pending.reverse();
        let mut i = interp_with_minibuffer("");
        // GNU: `abort-minibuffers' throws `exit' with a function that
        // signals `minibuffer-quit' after unwind.
        match minibuf_loop(&term, &mut i, "", false) {
            Ok(crate::lisp::eval::MinibufInput::Call(f)) => {
                let r = i.apply(&f, vec![]);
                assert!(
                    matches!(r, Err(crate::lisp::Flow::Signal(..))),
                    "expected minibuffer-quit signal, got {:?}",
                    r.map(|_| ())
                );
            }
            other => panic!("expected Call, got {:?}", other.map(|_| ())),
        }
    }

    #[test]
    fn minibuf_loop_icomplete_exhibits() {
        // GNU icomplete: `icomplete-minibuffer-setup' runs via
        // `minibuffer-setup-hook' inside `minibuf_read', installing a
        // buffer-local `post-command-hook' that exhibits completion
        // prospects through an overlay `after-string'.
        let (t, out) = test_term(40, 5);
        let term = Rc::new(RefCell::new(t));
        for k in [b'a' as i128, 13] {
            term.borrow_mut().unread(k);
        }
        term.borrow_mut().pending.reverse();
        let mut i = crate::lisp::Interp::new();
        i.eval_str("(require 'icomplete) (icomplete-mode 1)")
            .unwrap();
        // `completing-read''s dynamic context (the specbind f_completing_read
        // installs around minibuf_read).
        let mark = i.specbind_depth();
        let mct = i.intern("minibuffer-completion-table");
        let table = i.eval_str("(list \"alpha\" \"alpine\" \"beta\")").unwrap();
        let _ = i.specbind(mct, table);
        let t2 = term.clone();
        i.minibuf_reader = Some(Rc::new(move |interp, prompt, single| {
            minibuf_loop(&t2, interp, prompt, single)
        }));
        let r = i.minibuf_read("Pick: ", Default::default());
        let _ = i.unbind_to(mark);
        assert_eq!(r.unwrap(), "a");
        let drawn = String::from_utf8_lossy(&out.borrow()).into_owned();
        // GNU renders `a[lp]{ha | ine}' for input `a' against
        // (alpha alpine beta): [lp] is the common completion,
        // {ha | ine} the remaining prospects.
        assert!(
            drawn.contains("a[lp]{ha | ine}"),
            "expected prospects in echo, got: {drawn:?}"
        );
    }

    #[test]
    fn minibuf_loop_fido_ret() {
        // fido-mode: RET runs `icomplete-fido-ret' →
        // `icomplete-force-complete-and-exit' → completes to the
        // first flex-sorted match and exits with it.
        let (t, out) = test_term(40, 5);
        let term = Rc::new(RefCell::new(t));
        for k in [b'a' as i128, b'p' as i128, 13] {
            term.borrow_mut().unread(k);
        }
        term.borrow_mut().pending.reverse();
        let mut i = crate::lisp::Interp::new();
        i.eval_str("(require 'icomplete) (fido-mode 1)").unwrap();
        let mark = i.specbind_depth();
        let mct = i.intern("minibuffer-completion-table");
        let table = i.eval_str("(list \"alpha\" \"alpine\" \"beta\")").unwrap();
        let _ = i.specbind(mct, table);
        let t2 = term.clone();
        i.minibuf_reader = Some(Rc::new(move |interp, prompt, single| {
            minibuf_loop(&t2, interp, prompt, single)
        }));
        let r = i.minibuf_read("Pick: ", Default::default());
        let _ = i.unbind_to(mark);
        let got = match r {
            Ok(v) => v,
            Err(f) => {
                let d = match &f {
                    crate::lisp::error::Flow::Signal(s, data, _) => format!(
                        "signal {} {}",
                        i.prin1_to_string(s),
                        i.prin1_to_string(data)
                    ),
                    other => format!("{other:?}"),
                };
                let msgs = i
                    .buffers
                    .by_name("*Messages*")
                    .and_then(|id| i.buffers.get(id))
                    .map(|b| {
                        let t = b.borrow().text.text();
                        t.chars()
                            .rev()
                            .take(800)
                            .collect::<String>()
                            .chars()
                            .rev()
                            .collect::<String>()
                    })
                    .unwrap_or_default();
                panic!(
                    "minibuf_read failed: {d}\necho: {}\nmessages-tail: {msgs}",
                    String::from_utf8_lossy(&out.borrow())
                );
            }
        };
        assert!(
            got == "alpha" || got == "alpine",
            "expected flex match, got: {got:?}"
        );
        let drawn = String::from_utf8_lossy(&out.borrow()).into_owned();
        // fido shows full prospect names: `ap{alpha | alpine}'.
        assert!(drawn.contains("ap{alpha | alpine}"), "echo: {drawn:?}");
    }

    #[test]
    fn minibuf_loop_icomplete_vertical() {
        // icomplete-vertical-mode renders prospects one-per-line via
        // `icomplete--render-vertical' (exercises the loop rewrites).
        let (t, out) = test_term(40, 8);
        let term = Rc::new(RefCell::new(t));
        for k in [b'a' as i128, 13] {
            term.borrow_mut().unread(k);
        }
        term.borrow_mut().pending.reverse();
        let mut i = crate::lisp::Interp::new();
        i.eval_str("(require 'icomplete) (icomplete-mode 1) (icomplete-vertical-mode 1)")
            .unwrap();
        let mark = i.specbind_depth();
        let mct = i.intern("minibuffer-completion-table");
        let table = i.eval_str("(list \"alpha\" \"alpine\" \"beta\")").unwrap();
        let _ = i.specbind(mct, table);
        let t2 = term.clone();
        i.minibuf_reader = Some(Rc::new(move |interp, prompt, single| {
            minibuf_loop(&t2, interp, prompt, single)
        }));
        let r = i.minibuf_read("Pick: ", Default::default());
        let _ = i.unbind_to(mark);
        assert_eq!(r.unwrap(), "a");
        let drawn = String::from_utf8_lossy(&out.borrow()).into_owned();
        // Vertical layout: each prospect on its own line.
        assert!(
            drawn.contains("alpha") || drawn.contains("alpine"),
            "echo: {drawn:?}"
        );
    }

    #[test]
    fn minibuf_loop_mb_depth_nested() {
        // GNU mb-depth: `minibuffer-depth-setup' (on
        // `minibuffer-setup-hook') puts a `[N] ' `before-string'
        // overlay on the prompt when `minibuffer-depth' > 1.
        let (t, _out) = test_term(30, 5);
        let term = Rc::new(RefCell::new(t));
        for k in [b'a' as i128, 13] {
            term.borrow_mut().unread(k);
        }
        term.borrow_mut().pending.reverse();
        let mut i = crate::lisp::Interp::new();
        i.eval_str(
            "(require 'mb-depth)
             (minibuffer-depth-indicate-mode 1)
             (defvar mb-probe-depth nil)
             (defvar mb-probe-ind nil)
             ;; Record depth + the indicator overlay inside the
             ;; nested read (appended so it runs after
             ;; `minibuffer-depth-setup').
             (add-hook 'minibuffer-setup-hook
                       (lambda ()
                         (setq mb-probe-depth (minibuffer-depth))
                         (dolist (ov (overlays-in (point-min) (point-max)))
                           (let ((s (overlay-get ov 'before-string)))
                             (when s (setq mb-probe-ind s)))))
                       t)
             ;; One-shot: fire a nested read from inside the outer
             ;; read's command loop (GNU reaches this via e.g. M-x
             ;; in the minibuffer).
             (defun mb-probe-cmd ()
               (remove-hook 'post-command-hook #'mb-probe-cmd)
               (let ((enable-recursive-minibuffers t))
                 (read-string \"L2: \")))
             (add-hook 'post-command-hook #'mb-probe-cmd)",
        )
        .unwrap();
        let t2 = term.clone();
        i.minibuf_reader = Some(Rc::new(move |interp, prompt, single| {
            // Nested reads (level ≥ 2) get canned input; the outer
            // read (level 1) uses the real command loop.
            if interp.minibuf_level > 1 {
                return Ok(crate::lisp::eval::MinibufInput::Text("inner".into()));
            }
            minibuf_loop(&t2, interp, prompt, single)
        }));
        let r = i.minibuf_read("L1: ", Default::default());
        assert_eq!(r.unwrap(), "a");
        // The nested read ran at depth 2 with the depth indicator.
        let d = i.eval_str("mb-probe-depth").unwrap();
        assert!(matches!(&d, crate::lisp::Value::Int(n) if *n == 2));
        let ind = i.eval_str("mb-probe-ind").unwrap();
        let s = match &ind {
            crate::lisp::Value::Str(s) => s.borrow().clone(),
            other => panic!("expected indicator string, got {other:?}"),
        };
        assert_eq!(s, "[2] ");
    }

    #[test]
    fn minibuf_loop_single_key_mode() {
        let (t, _out) = test_term(20, 5);
        let term = Rc::new(RefCell::new(t));
        term.borrow_mut().unread(b'y' as i128);
        let mut i = crate::lisp::Interp::new();
        match minibuf_loop(&term, &mut i, "y/n ", true) {
            Ok(crate::lisp::eval::MinibufInput::Key(k)) => assert_eq!(k, b'y' as i128),
            _ => panic!("expected Key"),
        }
    }

    #[test]
    fn isearch_loop_end_to_end() {
        let (t, _out) = test_term(20, 5);
        let term = Rc::new(RefCell::new(t));
        // Type 'y', then RET to finish.
        for k in [b'y' as i128, 13] {
            term.borrow_mut().unread(k);
        }
        term.borrow_mut().pending.reverse();
        let mut i = interp_with_frame("xx yy\n");
        isearch_loop(&term, &mut i, false, false).unwrap();
        let b = i.current_buffer_ref().unwrap();
        assert_eq!(b.borrow().point(), 4); // after the first "y" match
    }

    #[test]
    fn isearch_loop_abort_restores() {
        let (t, _out) = test_term(20, 5);
        let term = Rc::new(RefCell::new(t));
        for k in [b'y' as i128, 7] {
            term.borrow_mut().unread(k);
        }
        term.borrow_mut().pending.reverse();
        let mut i = interp_with_frame("xx yy\n");
        isearch_loop(&term, &mut i, false, false).unwrap();
        assert_eq!(i.current_buffer_ref().unwrap().borrow().point(), 0);
    }

    #[test]
    fn nonblocking_sleep_short() {
        let mut i = crate::lisp::Interp::new();
        nonblocking_sleep(&mut i, Duration::from_millis(1)).unwrap();
    }

    #[test]
    fn dispatch_inserts_undo_boundaries_per_command() {
        let (t, _out) = test_term(20, 5);
        let term = Rc::new(RefCell::new(t));
        let mut i = interp_with_frame("");
        let mut keys = Vec::new();
        let mut arg_mode = false;
        // Each self-insert char is one command → one undo boundary.
        dispatch_key(&term, &mut i, b'a' as i128, &mut keys, &mut arg_mode).unwrap();
        dispatch_key(&term, &mut i, b'b' as i128, &mut keys, &mut arg_mode).unwrap();
        let b = i.current_buffer_ref().unwrap();
        assert_eq!(b.borrow().text.text(), "ab");
        // C-/ runs `undo' → removes the last command's "b" insertion.
        dispatch_key(&term, &mut i, 31, &mut keys, &mut arg_mode).unwrap();
        assert_eq!(b.borrow().text.text(), "a");
    }

    #[test]
    fn dispatch_self_insert_expands_abbrev() {
        let (t, _out) = test_term(20, 5);
        let term = Rc::new(RefCell::new(t));
        let mut i = interp_with_frame("");
        i.eval_str("(define-abbrev global-abbrev-table \"tq\" \"quick\") (abbrev-mode 1)")
            .unwrap();
        let mut keys = Vec::new();
        let mut arg_mode = false;
        dispatch_key(&term, &mut i, b't' as i128, &mut keys, &mut arg_mode).unwrap();
        dispatch_key(&term, &mut i, b'q' as i128, &mut keys, &mut arg_mode).unwrap();
        dispatch_key(&term, &mut i, b' ' as i128, &mut keys, &mut arg_mode).unwrap();
        let b = i.current_buffer_ref().unwrap();
        assert_eq!(b.borrow().text.text(), "quick ");
        // With abbrev-mode off, no expansion happens.
        i.eval_str("(abbrev-mode 0)").unwrap();
        let mut keys = Vec::new();
        dispatch_key(&term, &mut i, b't' as i128, &mut keys, &mut arg_mode).unwrap();
        dispatch_key(&term, &mut i, b'q' as i128, &mut keys, &mut arg_mode).unwrap();
        dispatch_key(&term, &mut i, b' ' as i128, &mut keys, &mut arg_mode).unwrap();
        assert_eq!(b.borrow().text.text(), "quick tq ");
    }

    fn ev(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: mods,
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        }
    }

    #[test]
    fn ctrl_chars_fold_to_control_codes() {
        // C-u -> 21, C-x -> 24, C-a -> 1 (Emacs event representation).
        assert_eq!(
            key_event_to_code(ev(KeyCode::Char('u'), KeyModifiers::CONTROL)),
            Some(21)
        );
        assert_eq!(
            key_event_to_code(ev(KeyCode::Char('x'), KeyModifiers::CONTROL)),
            Some(24)
        );
        assert_eq!(
            key_event_to_code(ev(KeyCode::Char('a'), KeyModifiers::CONTROL)),
            Some(1)
        );
        // C-? -> DEL
        assert_eq!(
            key_event_to_code(ev(KeyCode::Char('?'), KeyModifiers::CONTROL)),
            Some(127)
        );
        // C-@ -> 0 (crossterm may deliver it as C-2 or C-Space variant)
        assert_eq!(
            key_event_to_code(ev(KeyCode::Char('@'), KeyModifiers::CONTROL)),
            Some(0)
        );
    }

    #[test]
    fn meta_sets_bit() {
        assert_eq!(
            key_event_to_code(ev(KeyCode::Char('f'), KeyModifiers::ALT)),
            Some(b'f' as i128 | CHAR_META)
        );
        // M-C-f -> meta | control-folded char
        assert_eq!(
            key_event_to_code(ev(
                KeyCode::Char('f'),
                KeyModifiers::ALT | KeyModifiers::CONTROL
            )),
            Some(6 | CHAR_META)
        );
    }

    #[test]
    fn named_keys_get_symbol_codes() {
        assert_eq!(
            key_event_to_code(ev(KeyCode::Up, KeyModifiers::empty())),
            Some(named_code("up"))
        );
        assert_eq!(
            key_event_to_code(ev(KeyCode::PageDown, KeyModifiers::empty())),
            Some(named_code("next"))
        );
        assert_eq!(
            key_event_to_code(ev(KeyCode::F(3), KeyModifiers::empty())),
            Some(named_code("f3"))
        );
        // M-<up>
        assert_eq!(
            key_event_to_code(ev(KeyCode::Up, KeyModifiers::ALT)),
            Some(named_code("up") | CHAR_META)
        );
    }

    #[test]
    fn plain_chars_and_whitespace() {
        assert_eq!(
            key_event_to_code(ev(KeyCode::Char('a'), KeyModifiers::empty())),
            Some(97)
        );
        assert_eq!(
            key_event_to_code(ev(KeyCode::Enter, KeyModifiers::empty())),
            Some(13)
        );
        assert_eq!(
            key_event_to_code(ev(KeyCode::Tab, KeyModifiers::empty())),
            Some(9)
        );
        assert_eq!(
            key_event_to_code(ev(KeyCode::Backspace, KeyModifiers::empty())),
            Some(127)
        );
        assert_eq!(
            key_event_to_code(ev(KeyCode::Esc, KeyModifiers::empty())),
            Some(27)
        );
    }

    #[test]
    fn lookup_routes_to_bindings() {
        let mut i = crate::lisp::Interp::new();
        let seq = |k: i128| {
            Value::Vec(std::rc::Rc::new(std::cell::RefCell::new(vec![Value::Int(
                k,
            )])))
        };
        // 'a' -> t default -> self-insert-command
        match lookup_command(&mut i, &seq(97)) {
            LookupResult::Command(v) => {
                assert_eq!(i.princ_to_string(&v), "self-insert-command")
            }
            _ => panic!("'a' should resolve to self-insert-command"),
        }
        // C-u -> universal-argument
        match lookup_command(&mut i, &seq(21)) {
            LookupResult::Command(v) => {
                assert_eq!(i.princ_to_string(&v), "universal-argument")
            }
            _ => panic!("C-u should resolve to universal-argument"),
        }
        // C-x alone -> prefix keymap
        assert!(matches!(
            lookup_command(&mut i, &seq(24)),
            LookupResult::Prefix
        ));
        // C-x C-c -> save-buffers-kill-terminal (GNU's binding)
        let two = Value::Vec(std::rc::Rc::new(std::cell::RefCell::new(vec![
            Value::Int(24),
            Value::Int(3),
        ])));
        match lookup_command(&mut i, &two) {
            LookupResult::Command(v) => {
                assert_eq!(i.princ_to_string(&v), "save-buffers-kill-terminal")
            }
            _ => panic!("C-x C-c should resolve"),
        }
    }
}
