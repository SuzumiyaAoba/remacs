//! Incremental search state, overlays, and nested input loop.
use super::*;

/// Incremental search loop (C-s / C-r). Reads keys directly until the
/// search exits via RET, C-g, or a non-search key (which is then
/// re-dispatched as a command, like Emacs).
///
/// `point` semantics follow Emacs: forward searches leave point after
/// the match, backward searches leave it at the match start. C-g
/// restores the entry position; RET exits and sets the mark there.
/// Live state of an incremental search.
pub(super) struct Isearch {
    pub(super) query: String,
    /// History of match positions; last is the current match.
    pub(super) positions: Vec<usize>,
    /// Match spans (beg, end) parallel to `positions`.
    pub(super) spans: Vec<(usize, usize)>,
    pub(super) failing: bool,
    /// Character index in QUERY where the currently failing suffix begins.
    pub(super) fail_pos: Option<usize>,
    /// Current value of `isearch-case-fold-search'.
    pub(super) case_fold: Value,
    /// Snapshots of query/match state, one per reversible key operation.
    history: Vec<IsearchHistory>,
    pub(super) start: usize,
    pub(super) backward: bool,
    pub(super) regexp: bool,
    quote_next: bool,
    /// The `isearch' overlay on the current match (handle or None).
    pub(super) match_ov: Option<Value>,
    /// `lazy-highlight' overlays on the other visible matches.
    pub(super) lazy_ovs: Vec<Value>,
}

struct IsearchHistory {
    query_len: usize,
    positions_len: usize,
    spans_len: usize,
    failing: bool,
    fail_pos: Option<usize>,
    case_fold: Value,
}

/// Run one search; returns (point, match-beg, match-end) — all
/// 0-based — or None when the query doesn't match from `from`.
fn isearch_search(
    i: &mut Interp,
    query: &str,
    backward: bool,
    regexp: bool,
    from: usize,
    case_fold_search: bool,
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
        "(let ((case-fold-search {})) \
           (progn (goto-char (min {} (point-max))) \
           (and ({} \"{}\" nil t) \
                (list (1- (point)) (1- (match-beginning 0)) (1- (match-end 0))))))",
        if case_fold_search { "t" } else { "nil" },
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

fn isearch_yank_word_or_char(i: &mut Interp, position: usize) -> Option<String> {
    let source = format!(
        "(save-excursion (goto-char (min {} (point-max))) \
           (buffer-substring-no-properties (point) \
             (progn (if (or (eq (char-syntax (or (char-after) 0)) ?w) \
                            (eq (char-syntax (or (char-after (1+ (point))) 0)) ?w)) \
                        (forward-word 1) (forward-char 1)) \
                    (point))))",
        position.saturating_add(1)
    );
    let Value::Str(text) = i.eval_str(&source).ok()? else {
        return None;
    };
    Some(text.borrow().to_unicode_lossy())
}

fn prepare_isearch_yanked_text(
    i: &mut Interp,
    text: &str,
    case_fold: &Value,
    regexp: bool,
) -> String {
    let mut value = Value::string(text);
    let not_yanks = i
        .intern_soft("search-upper-case")
        .map(|symbol| i.symbol_value(symbol))
        .is_some_and(|value| matches!(value, Value::Sym(symbol) if i.symbol_name(symbol) == "not-yanks"));
    if case_fold.truthy() && not_yanks
        && let Some(downcase) = i.intern_soft("downcase")
        && let Ok(downcased) = i.apply(&Value::Sym(downcase), vec![value.clone()])
    {
        value = downcased;
    }
    if regexp
        && let Some(regexp_quote) = i.intern_soft("regexp-quote")
        && let Ok(quoted) = i.apply(&Value::Sym(regexp_quote), vec![value.clone()])
    {
        value = quoted;
    }
    match value {
        Value::Str(text) => text.borrow().to_unicode_lossy(),
        other => i.princ_to_string(&other).to_unicode_lossy(),
    }
}

fn isearch_quoted_character(key: i128) -> Option<char> {
    let mut code = u32::try_from(key).ok()?;
    if key & CHAR_CTL != 0 {
        code &= !(CHAR_CTL as u32);
        code = match code {
            0x20 => 0,
            0x3f => 0x7f,
            0x40..=0x7e => code & 0x1f,
            _ => code,
        };
    }
    char::from_u32(code)
}

/// What the isearch driver should do after a key.
pub(super) enum IsearchAction {
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
    pub(super) fn new(i: &Interp, backward: bool, regexp: bool) -> Isearch {
        let start = i
            .current_buffer_ref()
            .map(|b| b.borrow().point())
            .unwrap_or(0);
        let case_fold = i
            .intern_soft("case-fold-search")
            .filter(|symbol| i.bound_p(*symbol))
            .map(|symbol| i.symbol_value(symbol))
            .unwrap_or(Value::Nil);
        Isearch {
            query: String::new(),
            positions: vec![start],
            spans: vec![(start, start)],
            failing: false,
            fail_pos: None,
            case_fold,
            history: Vec::new(),
            start,
            backward,
            regexp,
            quote_next: false,
            match_ov: None,
            lazy_ovs: Vec::new(),
        }
    }

    pub(super) fn cur(&self) -> usize {
        *self.positions.last().unwrap_or(&self.start)
    }

    pub(super) fn go_to(&self, i: &Interp, p: usize) {
        if let Some(b) = i.current_buffer_ref() {
            b.borrow_mut().set_point(p);
        }
    }

    fn save_history(&mut self) {
        self.history.push(IsearchHistory {
            query_len: self.query.len(),
            positions_len: self.positions.len(),
            spans_len: self.spans.len(),
            failing: self.failing,
            fail_pos: self.fail_pos,
            case_fold: self.case_fold.clone(),
        });
    }

    fn restore_history(&mut self, i: &Interp) {
        if let Some(previous) = self.history.pop() {
            self.query.truncate(previous.query_len);
            self.positions.truncate(previous.positions_len);
            self.spans.truncate(previous.spans_len);
            self.failing = previous.failing;
            self.fail_pos = previous.fail_pos;
            self.case_fold = previous.case_fold;
            self.go_to(i, self.cur());
        }
    }

    fn case_fold_for_search(&mut self, i: &mut Interp) -> bool {
        if matches!(&self.case_fold, Value::Sym(symbol) if *symbol == crate::lisp::obarray::sym::T)
            && i
                .intern_soft("search-upper-case")
                .is_some_and(|symbol| i.bound_p(symbol) && i.symbol_value(symbol).truthy())
            && let Some(no_upper_case) = i.intern_soft("isearch-no-upper-case-p")
            && let Ok(result) = i.apply(
                &Value::Sym(no_upper_case),
                vec![
                    Value::string(self.query.clone()),
                    if self.regexp { Value::t() } else { Value::Nil },
                ],
            )
        {
            self.case_fold = if result.truthy() {
                Value::t()
            } else {
                Value::Nil
            };
        }
        self.case_fold.truthy()
    }

    fn search_updated_query(&mut self, i: &mut Interp, failure_start: usize) {
        let cur = self.cur();
        let case_fold_search = self.case_fold_for_search(i);
        let hit = if self.positions.len() > 1 {
            let (match_start, match_end) = *self.spans.last().unwrap_or(&(cur, cur));
            if self.backward {
                match isearch_search(
                    i,
                    &self.query,
                    false,
                    self.regexp,
                    match_start,
                    case_fold_search,
                ) {
                    Some((_, beg, end)) if beg == match_start && end <= self.start => {
                        Some((beg, beg, end))
                    }
                    _ => isearch_search(
                        i,
                        &self.query,
                        true,
                        self.regexp,
                        self.start.min(match_end + 1),
                        case_fold_search,
                    ),
                }
            } else {
                isearch_search(
                    i,
                    &self.query,
                    false,
                    self.regexp,
                    match_start,
                    case_fold_search,
                )
            }
        } else {
            isearch_search(
                i,
                &self.query,
                self.backward,
                self.regexp,
                self.start,
                case_fold_search,
            )
        };
        match hit {
            Some((position, beg, end)) => {
                self.positions.push(position);
                self.spans.push((beg, end));
                self.failing = false;
                self.fail_pos = None;
                self.go_to(i, position);
            }
            None => {
                self.failing = true;
                self.fail_pos = Some(failure_start);
            }
        }
    }

    /// Handle one key event; updates query/point/failing state.
    pub(super) fn step(&mut self, i: &mut Interp, key: i128) -> IsearchAction {
        let cur = self.cur();
        if self.quote_next {
            self.quote_next = false;
            if let Some(ch) = isearch_quoted_character(key) {
                let failure_start = self
                    .fail_pos
                    .unwrap_or_else(|| self.query.chars().count());
                self.save_history();
                self.query.push(ch);
                self.search_updated_query(i, failure_start);
            }
            return IsearchAction::Continue;
        }
        match key {
            // C-g: undo a failing search back to the last successful state;
            // abort only when the search is currently successful.
            7 => {
                if self.failing {
                    while self.failing && !self.history.is_empty() {
                        self.restore_history(i);
                    }
                    // Every operation that can make the search fail records
                    // a snapshot first. Keep this fallback defensive in case
                    // a future state transition violates that invariant.
                    self.failing = false;
                    self.fail_pos = None;
                    self.go_to(i, self.cur());
                    IsearchAction::Continue
                } else {
                    self.go_to(i, self.start);
                    IsearchAction::Abort
                }
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
                self.save_history();
                let back = key == 18;
                let wrap_enabled = i
                    .intern_soft("isearch-wrap-pause")
                    .map(|symbol| {
                        if i.bound_p(symbol) {
                            i.symbol_value(symbol).truthy()
                        } else {
                            true
                        }
                    })
                    .unwrap_or(true);
                let from = if self.failing && back == self.backward && wrap_enabled {
                    i.current_buffer_ref()
                        .map(|buffer| {
                            let buffer = buffer.borrow();
                            if back {
                                buffer.text_len()
                            } else {
                                buffer.begv
                            }
                        })
                        .unwrap_or(cur)
                } else if back {
                    cur.saturating_sub(1)
                } else {
                    cur + 1
                };
                let failure_start = self.fail_pos.unwrap_or(0);
                let case_fold_search = self.case_fold_for_search(i);
                match isearch_search(i, &self.query, back, self.regexp, from, case_fold_search) {
                    Some((p, b, e)) => {
                        self.positions.push(p);
                        self.spans.push((b, e));
                        self.failing = false;
                        self.fail_pos = None;
                        self.go_to(i, p);
                    }
                    None => {
                        self.failing = true;
                        self.fail_pos = Some(failure_start);
                    }
                }
                IsearchAction::Continue
            }
            // DEL: unwind one step of the search.
            127 => {
                self.restore_history(i);
                IsearchAction::Continue
            }
            c if c == 17 || c == (CHAR_CTL | 'q' as i128) => {
                self.quote_next = true;
                IsearchAction::Continue
            }
            c if c == 9
                || c == 10
                || c == (CHAR_CTL | 'i' as i128)
                || c == (CHAR_CTL | 'j' as i128) =>
            {
                if let Some(ch) = isearch_quoted_character(c) {
                    let failure_start = self
                        .fail_pos
                        .unwrap_or_else(|| self.query.chars().count());
                    self.save_history();
                    self.query.push(ch);
                    self.search_updated_query(i, failure_start);
                }
                IsearchAction::Continue
            }
            c if c == 25 || c == (CHAR_CTL | 'y' as i128) => {
                let current_kill = Value::Sym(i.intern("current-kill"));
                match i.apply(&current_kill, vec![Value::Int(0)]) {
                    Ok(Value::Str(text)) => {
                        let text = text.borrow().to_unicode_lossy();
                        if !text.is_empty() {
                            let failure_start = self
                                .fail_pos
                                .unwrap_or_else(|| self.query.chars().count());
                            self.save_history();
                            let text = prepare_isearch_yanked_text(
                                i,
                                &text,
                                &self.case_fold,
                                self.regexp,
                            );
                            self.query.push_str(&text);
                            self.search_updated_query(i, failure_start);
                        }
                    }
                    Ok(other) => {
                        let flow = i.wrong_type_mut("stringp", &other);
                        report_flow(i, flow);
                    }
                    Err(flow) => report_flow(i, flow),
                }
                IsearchAction::Continue
            }
            c if c == 23 || c == (CHAR_CTL | 'w' as i128) => {
                let anchor = if self.backward {
                    self.spans.last().map(|span| span.1).unwrap_or(cur)
                } else {
                    cur
                };
                if let Some(text) = isearch_yank_word_or_char(i, anchor)
                    && !text.is_empty()
                {
                    let failure_start = self
                        .fail_pos
                        .unwrap_or_else(|| self.query.chars().count());
                    self.save_history();
                    let text = prepare_isearch_yanked_text(
                        i,
                        &text,
                        &self.case_fold,
                        self.regexp,
                    );
                    self.query.push_str(&text);
                    self.search_updated_query(i, failure_start);
                }
                IsearchAction::Continue
            }
            c if c == (CHAR_META | 'c' as i128) => {
                self.save_history();
                self.case_fold = if self.case_fold.truthy() {
                    Value::Nil
                } else {
                    Value::Sym(i.intern("yes"))
                };
                self.failing = false;
                self.fail_pos = None;
                IsearchAction::Continue
            }
            c if c >= 32
                && c < 0x110000
                && c & (CHAR_META | CHAR_CTL | CHAR_SHIFT | CHAR_SUPER | CHAR_HYPER) == 0 =>
            {
                let failure_start = self
                    .fail_pos
                    .unwrap_or_else(|| self.query.chars().count());
                self.save_history();
                if let Some(ch) = char::from_u32(c as u32) {
                    self.query.push(ch);
                }
                self.search_updated_query(i, failure_start);
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
    pub(super) fn label(&self) -> (String, Option<(usize, usize)>) {
        let dir = if self.backward { " backward" } else { "" };
        let re = if self.regexp { " regexp" } else { "" };
        let status = if self.failing { "Failing " } else { "" };
        let prefix = format!("{}I-search{}{}: ", status, dir, re);
        let prefix_length = prefix.chars().count();
        let query_length = self.query.chars().count();
        let failure = self.failing.then(|| {
            (
                prefix_length + self.fail_pos.unwrap_or(0).min(query_length),
                prefix_length + query_length,
            )
        });
        (format!("{prefix}{}", self.query), failure)
    }

    /// Call a user-supplied `isearch-message-function' with the Lisp-visible
    /// search state bound for the duration of the callback.
    pub(super) fn call_message_function(&self, i: &mut Interp) -> bool {
        let Some(symbol) = i.intern_soft("isearch-message-function") else {
            return false;
        };
        if !i.bound_p(symbol) {
            return false;
        }
        let function = i.symbol_value(symbol);
        if !function.truthy() {
            return false;
        }

        let result = self.with_lisp_state(i, |i| i.apply(&function, Vec::new()));
        if let Err(flow) = result {
            report_flow(i, flow);
        }
        true
    }

    pub(super) fn run_lisp_hook(&self, i: &mut Interp, name: &str) {
        let result = self.with_lisp_state(i, |i| {
            crate::lisp::builtins::evalfn::safe_call_hook(i, name)
        });
        if let Err(flow) = result {
            report_flow(i, flow);
        }
    }

    pub(super) fn run_end_hook(&self, i: &mut Interp, quit: bool) {
        let mark = i.specbind_depth();
        if let Some(symbol) = i.intern_soft("isearch-mode-end-hook-quit")
            && i.bound_p(symbol)
            && let Err(flow) = i.specbind(symbol, if quit { Value::t() } else { Value::Nil })
        {
            let _ = i.unbind_to(mark);
            report_flow(i, flow);
            return;
        }
        self.run_lisp_hook(i, "isearch-mode-end-hook");
        if let Err(flow) = i.unbind_to(mark) {
            report_flow(i, flow);
        }
    }

    fn with_lisp_state<T>(
        &self,
        i: &mut Interp,
        action: impl FnOnce(&mut Interp) -> Result<T, crate::lisp::error::Flow>,
    ) -> Result<T, crate::lisp::error::Flow> {
        let mark = i.specbind_depth();
        let (match_start, match_end) = self.spans.last().copied().unwrap_or((self.start, self.start));
        let bindings = [
            ("isearch-string", Value::string(self.query.clone())),
            ("isearch-message", Value::string(self.query.clone())),
            (
                "isearch-forward",
                if self.backward { Value::Nil } else { Value::t() },
            ),
            (
                "isearch-regexp",
                if self.regexp { Value::t() } else { Value::Nil },
            ),
            ("isearch-case-fold-search", self.case_fold.clone()),
            (
                "isearch-success",
                if self.failing { Value::Nil } else { Value::t() },
            ),
            ("isearch-error", Value::Nil),
            (
                "isearch-other-end",
                Value::Int((if self.backward { match_end } else { match_start }) as i128 + 1),
            ),
        ];
        for (name, value) in bindings {
            let Some(symbol) = i.intern_soft(name) else {
                continue;
            };
            if !i.bound_p(symbol) {
                continue;
            }
            if let Err(flow) = i.specbind(symbol, value) {
                let _ = i.unbind_to(mark);
                return Err(flow);
            }
        }
        let result = action(i);
        match (result, i.unbind_to(mark)) {
            (Ok(value), Ok(())) => Ok(value),
            (Err(flow), _) | (Ok(_), Err(flow)) => Err(flow),
        }
    }

    /// One `apply' of a Lisp helper; errors are ignored (highlighting
    /// must never break the search itself).
    pub(super) fn call(i: &mut Interp, name: &str, args: Vec<Value>) {
        let f = match i.intern_soft(name) {
            Some(s) => Value::Sym(s),
            None => return,
        };
        let _ = i.apply(&f, args);
    }

    /// Delete the highlight overlays (`isearch-dehighlight').
    pub(super) fn clear(&mut self, i: &mut Interp) {
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
    pub(super) fn refresh_overlays(&mut self, i: &mut Interp) {
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
                let f = Value::Sym(i.q("make-overlay"));
                if let Ok(ov) = i.apply(
                    &f,
                    vec![
                        Value::Int(span.0 as i128 + 1),
                        Value::Int(span.1 as i128 + 1),
                    ],
                ) {
                    let pri = Value::Sym(i.q("priority"));
                    let face = Value::Sym(i.q("face"));
                    let isearch = Value::Sym(i.q("isearch"));
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
        let sw = Value::Sym(i.q("selected-window"));
        let wv = match i.apply(&sw, vec![]) {
            Ok(v) => v,
            Err(_) => return,
        };
        let ws_sym = i.q("window-start");
        let we_sym = i.q("window-end");
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
        let case_fold_search = self.case_fold.truthy();
        let pri = Value::Sym(i.q("priority"));
        let face = Value::Sym(i.q("face"));
        let lazy = Value::Sym(i.q("lazy-highlight"));
        let winprop = Value::Sym(i.q("window"));
        while count < 200 {
            let Some((_, b, e)) =
                isearch_search(i, &self.query, false, self.regexp, pos, case_fold_search)
            else {
                break;
            };
            if b >= wend || e <= b {
                break;
            }
            pos = e;
            if (b, e) == span {
                continue;
            }
            let f = Value::Sym(i.q("make-overlay"));
            let Ok(ov) = i.apply(
                &f,
                vec![Value::Int(b as i128 + 1), Value::Int(e as i128 + 1)],
            ) else {
                break;
            };
            Self::call(
                i,
                "overlay-put",
                vec![ov.clone(), pri.clone(), Value::Int(1000)],
            );
            Self::call(
                i,
                "overlay-put",
                vec![ov.clone(), face.clone(), lazy.clone()],
            );
            Self::call(
                i,
                "overlay-put",
                vec![ov.clone(), winprop.clone(), wv.clone()],
            );
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
pub(super) fn isearch_loop<T: KeyIo>(
    term: &std::rc::Rc<std::cell::RefCell<T>>,
    i: &mut Interp,
    backward: bool,
    regexp: bool,
) -> io::Result<()> {
    let mut st = Isearch::new(i, backward, regexp);
    let mode_symbol = i.intern("isearch-mode");
    let _ = i.set_symbol(mode_symbol, Value::string(" Isearch"));
    st.run_lisp_hook(i, "isearch-mode-hook");
    let mut render_needed = true;
    let mut message_needed = true;
    let mut update_hook_needed = true;
    let mut end_hook_quit = false;
    loop {
        let event_activity = match crate::lisp::eval::threads::yield_now(i).and_then(|ran_thread| {
            crate::lisp::builtins::evalfn::thread_wait_events(i)
                .map(|ran_events| ran_thread || ran_events)
        }) {
            Ok(activity) => activity,
            Err(flow) => {
                report_flow(i, flow);
                st.clear(i);
                break;
            }
        };
        let window_activity = match crate::editor::changes::run(i) {
            Ok(activity) => activity,
            Err(flow) => {
                report_flow(i, flow);
                true
            }
        };
        let render_on_idle = term.borrow().render_on_idle();
        let render_requested = term.borrow_mut().take_render_request();
        render_needed |= event_activity || window_activity || render_on_idle || render_requested;
        if render_needed {
            if message_needed {
                if !st.call_message_function(i) {
                    let (label, failure_range) = st.label();
                    let properties = failure_range
                        .map(|(start, end)| {
                            vec![(
                                start,
                                end,
                                vec![
                                    Value::Sym(i.intern("face")),
                                    Value::Sym(i.intern("isearch-fail")),
                                ],
                            )]
                        })
                        .unwrap_or_default();
                    i.set_echo_message_with_properties(EmacsText::from(label.as_str()), properties);
                }
                message_needed = false;
            }
            if update_hook_needed {
                st.run_lisp_hook(i, "isearch-update-post-hook");
                update_hook_needed = false;
            }
            let label = i.echo_message.to_unicode_lossy();
            let mut t = term.borrow_mut();
            let _ = t.render(i);
            let _ = t.draw_echo_message_with_state(i, &label);
            render_needed = false;
        }
        let polled_key = term.borrow_mut().poll_key(Duration::from_millis(50))?;
        apply_frame_selection(term, i);
        super::sync_mouse_absolute_pixel_position(term, i);
        let Some(key) = polled_key else {
            continue;
        };
        let action = st.step(i, key);
        message_needed = true;
        update_hook_needed = true;
        // Rebuild the isearch/lazy-highlight overlays so the next
        // render paints the matches (GNU: isearch-update →
        // isearch-highlight + lazy-highlight).
        st.refresh_overlays(i);
        render_needed = true;
        match action {
            IsearchAction::Continue => {}
            IsearchAction::Abort => {
                end_hook_quit = true;
                break;
            }
            IsearchAction::Done => break,
            IsearchAction::ReDispatch(other) => {
                st.clear(i);
                let mode_symbol = i.intern("isearch-mode");
                let _ = i.set_symbol(mode_symbol, Value::Nil);
                st.run_end_hook(i, false);
                let seq = Value::vector(vec![Value::Int(other)]);
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
    let mode_symbol = i.intern("isearch-mode");
    let _ = i.set_symbol(mode_symbol, Value::Nil);
    st.run_end_hook(i, end_hook_quit);
    Ok(())
}
