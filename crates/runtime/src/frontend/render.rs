//! Resolve faces and render editor state into a frontend-neutral grid.
use crate::{
    buffer::Buffer,
    lisp::{Interp, Value},
};
use std::collections::HashMap;

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
        m!(
            fg, bg, bold, italic, underline, overline, strike, inverse, extend
        );
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
            colors.insert(
                n.to_string(),
                ((r / 256) as u8, (g / 256) as u8, (b / 256) as u8),
            );
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
        self.colors.get(&canon).map(|&(r, g, b)| CellColor(r, g, b))
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
        "isearch"
        | "isearch-fail"
        | "highlight"
        | "secondary-selection"
        | "trailing-whitespace"
        | "match"
        | "query-replace"
        | "mode-line"
        | "mode-line-active"
        | "mode-line-inactive" => mix.inverse = Some(true),
        "region" => {
            // GNU's last-resort `(t :background "gray" :extend t)'.
            mix.bg = Some(CellColor(190, 190, 190));
            mix.extend = Some(true);
        }
        "lazy-highlight" => mix.underline = Some(true),
        "isearch-group-1" | "isearch-group-2" => mix.merge(&builtin_face_defaults(i, "isearch")),
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
    /// Overlay snapshots and face values. Precedence is computed for the
    /// overlays covering each position, including interval containment.
    overlays: Vec<(crate::buffer::Overlay, Value)>,
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
            (true, Some(mk)) if mk != bb.point() => Some((mk.min(bb.point()), mk.max(bb.point()))),
            _ => None,
        }
    };
    // Overlays covering this buffer, for this window (a `window'
    // property restricts an overlay to one window — lazy-highlight
    // uses it). Face candidates retain their slot order until point-specific
    // priority/containment ordering is computed below.
    let overlays: Vec<(crate::buffer::Overlay, Value)> = bb
        .overlays
        .iter()
        .enumerate()
        .filter(|(_, ov)| {
            if ov.buffer != Some(bb.id) || ov.start >= ov.end {
                return false;
            }
            ov.matches_window(i, win)
        })
        .filter_map(|(_, ov)| {
            let fv = ov.property(i, &Value::Sym(ctx.face));
            if fv.is_nil() {
                return None;
            }
            Some((ov.clone(), fv))
        })
        .collect();
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
fn style_at_pos(i: &Interp, ctx: &mut FaceCtx, wf: &WindowFaces, pos: usize) -> CellStyle {
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
    let mut overlays: Vec<_> = wf
        .overlays
        .iter()
        .enumerate()
        .filter(|(_, (overlay, _))| pos >= overlay.start && pos < overlay.end)
        .map(|(index, (overlay, _))| (overlay, index))
        .collect();
    crate::buffer::sort_overlays_by_priority(i, &mut overlays);
    for (_, index) in overlays {
        let face = wf.overlays[index].1.clone();
        mix.merge(&ctx.face_value(i, &face, 0));
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
                    (wb.suspend_hscroll, wb.old_point_position(), wb.min_hscroll)
                };
                let bb = b.borrow();
                let pt = (if std::rc::Rc::ptr_eq(w, &fb.selected) {
                    bb.point()
                } else {
                    w.borrow().point
                })
                .clamp(bb.begv, bb.zv);
                // GNU xdisp.c: point movement since the last redisplay
                // lifts an hscroll suspension; then record it as the
                // new old_point.
                let mut wb_mut = w.borrow_mut();
                if wb_mut.suspend_hscroll && pt != old_pt {
                    wb_mut.suspend_hscroll = false;
                }
                wb_mut.record_old_point(pt);
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
                let ahs_id = i.q_soft("auto-hscroll-mode").unwrap_or(u32::MAX);
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
        let mut visible_end = start;
        for row in 0..text_rows {
            let y = top + row;
            if y >= body_height {
                break;
            }
            // Line-aligned rendering: `start' is a line start because
            // the scroll code above always lands it on line_start().
            let (line, line_pos, row_end) = match i.buffers.get(buf_id) {
                Some(b) => {
                    let bb = b.borrow();
                    let ls = bb.text.line_start(bb.text.line_of_pos(start) + row);
                    let le = bb.text.line_end(ls);
                    let row_end = if le < bb.text_len() && bb.text.char_at(le) == '\n' {
                        le + 1
                    } else {
                        le
                    };
                    (bb.text.substring(ls, le), ls, row_end)
                }
                None => (String::new(), 0, 0),
            };
            visible_end = row_end;
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
        let tick = i
            .buffers
            .get(buf_id)
            .map(|b| b.borrow().text.mod_tick.get())
            .unwrap_or(0);
        w.borrow_mut().redisplay_end = Some((buf_id, tick, start, visible_end));
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
