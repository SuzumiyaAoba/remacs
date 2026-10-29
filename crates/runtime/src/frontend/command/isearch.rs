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
    /// Last successful regexp match-data, including group 0, 0-based.
    match_groups: Vec<Option<(usize, usize)>>,
    pub(super) failing: bool,
    wrapped: bool,
    /// Character index in QUERY where the currently failing suffix begins.
    pub(super) fail_pos: Option<usize>,
    /// Current value of `isearch-case-fold-search'.
    pub(super) case_fold: Value,
    /// Current value of `isearch-invisible'.
    invisible: Value,
    /// Snapshots of query/match state, one per reversible key operation.
    history: Vec<IsearchHistory>,
    pub(super) start: usize,
    /// Search boundary, reset to the current match when direction changes.
    search_origin: usize,
    pub(super) backward: bool,
    pub(super) regexp: bool,
    regexp_function: Value,
    word_search: bool,
    char_fold: bool,
    symbol_search: bool,
    lax_whitespace: bool,
    regexp_lax_whitespace: bool,
    quote_next: bool,
    yank_until_char: bool,
    help_prefix: bool,
    cx_prefix: bool,
    cx8_prefix: bool,
    cx8_emoji_prefix: bool,
    /// Whether the previous isearch command yanked from the kill ring.
    kill_yank_active: bool,
    prefix_argument: Option<i128>,
    prefix_digits: bool,
    meta_s_prefix: bool,
    meta_s_h_prefix: bool,
    /// The `isearch' overlay on the current match (handle or None).
    pub(super) match_ov: Option<Value>,
    /// `lazy-highlight' overlays on the other visible matches.
    pub(super) lazy_ovs: Vec<Value>,
    /// Pending visible-match scan, advanced by the isearch loop so the
    /// configured initial delay and interval do not block input handling.
    lazy_scan: Option<LazyHighlightScan>,
    /// Per-capture overlays on the current regexp match.
    submatch_ovs: Vec<Value>,
    lazy_count_current: Option<usize>,
    lazy_count_total: Option<usize>,
    lazy_count_invisible: Option<usize>,
    lazy_count_prefix: String,
    lazy_count_suffix: String,
}

struct LazyHighlightScan {
    position: usize,
    window_end: usize,
    current_span: (usize, usize),
    window: Value,
    next_at: std::time::Instant,
    interval: std::time::Duration,
    max_at_a_time: usize,
}

fn isearch_delay_seconds(i: &mut Interp, name: &str, default: f64) -> f64 {
    let Some(symbol) = i.intern_soft(name).filter(|symbol| i.bound_p(*symbol)) else {
        return default;
    };
    match i.symbol_value(symbol) {
        Value::Int(seconds) => (seconds as f64).max(0.0),
        Value::Float(seconds) if (*seconds).is_finite() => (*seconds).max(0.0),
        _ => default,
    }
}

fn isearch_delay_duration(seconds: f64) -> std::time::Duration {
    std::time::Duration::try_from_secs_f64(seconds)
        .unwrap_or_else(|_| std::time::Duration::from_secs(365 * 24 * 60 * 60))
}

struct IsearchHistory {
    query_len: usize,
    positions_len: usize,
    spans_len: usize,
    match_groups: Vec<Option<(usize, usize)>>,
    failing: bool,
    wrapped: bool,
    fail_pos: Option<usize>,
    case_fold: Value,
    regexp: bool,
    regexp_function: Value,
    word_search: bool,
    char_fold: bool,
    symbol_search: bool,
    lax_whitespace: bool,
    regexp_lax_whitespace: bool,
    invisible: Value,
    backward: bool,
    search_origin: usize,
    kill_yank_active: bool,
}

/// Run one search; returns the point, match span, and 0-based spans for
/// the full match and each capture group, or None when it doesn't match.
fn isearch_search_pattern(
    i: &mut Interp,
    query: &str,
    regexp: bool,
    word_search: bool,
    char_fold: bool,
    symbol_search: bool,
    regexp_function: &Value,
    regexp_function_lax: bool,
    lax_whitespace: bool,
) -> Option<(String, bool)> {
    let (mut pattern, mut regexp) = if char_fold {
        let function = Value::Sym(i.intern("char-fold-to-regexp"));
        match i.apply(&function, vec![Value::string(query)]) {
            Ok(Value::Str(pattern)) => (pattern.borrow().to_unicode_lossy(), true),
            _ => return None,
        }
    } else if symbol_search {
        (
            crate::buffer::primitives::symbol_search_regexp(i, query, false).to_unicode_lossy(),
            true,
        )
    } else if word_search {
        (
            crate::buffer::primitives::word_search_regexp(i, query, false).to_unicode_lossy(),
            true,
        )
    } else if !regexp_function.is_nil() {
        let function = regexp_function.clone();
        match i.apply(
            &function,
            vec![Value::string(query), Value::from_bool(regexp_function_lax)],
        ) {
            Ok(Value::Str(pattern)) => (pattern.borrow().to_unicode_lossy(), true),
            _ => return None,
        }
    } else {
        (query.to_owned(), regexp)
    };
    if lax_whitespace
        && let Some(lax_pattern) = isearch_lax_whitespace_pattern(i, &pattern, regexp)
    {
        pattern = lax_pattern;
        regexp = true;
    }
    Some((pattern, regexp))
}

fn isearch_search(
    i: &mut Interp,
    query: &str,
    backward: bool,
    regexp: bool,
    word_search: bool,
    char_fold: bool,
    symbol_search: bool,
    regexp_function: &Value,
    regexp_function_lax: bool,
    lax_whitespace: bool,
    from: usize,
    case_fold_search: bool,
    allow_invisible: bool,
) -> Option<(usize, usize, usize, Vec<Option<(usize, usize)>>)> {
    if query.is_empty() {
        return Some((from, from, from, vec![Some((from, from))]));
    }
    let (pattern, regexp) = isearch_search_pattern(
        i,
        query,
        regexp,
        word_search,
        char_fold,
        symbol_search,
        regexp_function,
        regexp_function_lax,
        lax_whitespace,
    )?;
    let mut esc = String::new();
    for c in pattern.chars() {
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
    let mut search_from = from;
    let mut skipped_invisible = false;
    loop {
        let src = format!(
            "(let ((case-fold-search {})) \
               (progn (goto-char (min {} (point-max))) \
               (and ({} \"{}\" nil t) \
                    (list (1- (point)) (1- (match-beginning 0)) (1- (match-end 0)) \
                          (match-data t)))))",
            if case_fold_search { "t" } else { "nil" },
            search_from + 1,
            fn_name,
            esc
        );
        let hit = match i.eval_str(&src) {
            Ok(v) => v.list_to_vec().ok().and_then(|items| {
                let position = items.first()?.int()? as usize;
                let beg = items.get(1)?.int()? as usize;
                let end = items.get(2)?.int()? as usize;
                let data = items.get(3)?.list_to_vec().ok()?;
                let groups = data
                    .chunks_exact(2)
                    .map(|pair| match (pair[0].int(), pair[1].int()) {
                        (Some(beg), Some(end)) => Some((
                            (beg as usize).saturating_sub(1),
                            (end as usize).saturating_sub(1),
                        )),
                        _ => None,
                    })
                    .collect();
                Some((position, beg, end, groups))
            }),
            _ => None,
        };
        let Some(hit @ (_, beg, end, _)) = hit else {
            if skipped_invisible
                && let Some(buffer) = i.current_buffer_ref()
            {
                buffer.borrow_mut().set_point(from);
            }
            return None;
        };
        if allow_invisible || !isearch_span_invisible(i, beg, end) {
            return Some(hit);
        }
        skipped_invisible = true;
        let next = if backward {
            if beg == end { beg.saturating_sub(1) } else { beg }
        } else if beg == end {
            end.saturating_add(1)
        } else {
            end
        };
        if next == search_from {
            if let Some(buffer) = i.current_buffer_ref() {
                buffer.borrow_mut().set_point(from);
            }
            return None;
        }
        search_from = next;
    }
}

fn isearch_span_invisible(i: &mut Interp, beg: usize, end: usize) -> bool {
    let invisible_p = Value::Sym(i.intern("invisible-p"));
    (beg..end).any(|position| {
        i.apply(&invisible_p, vec![Value::Int(position as i128 + 1)])
            .is_ok_and(|value| value.truthy())
    })
}

fn isearch_lax_whitespace_pattern(i: &mut Interp, query: &str, regexp: bool) -> Option<String> {
    let whitespace = i
        .intern_soft("search-whitespace-regexp")
        .filter(|symbol| i.bound_p(*symbol))
        .map(|symbol| i.symbol_value(symbol))?;
    let Value::Str(whitespace) = whitespace else {
        return None;
    };
    let whitespace = whitespace.borrow().to_unicode_lossy();
    if whitespace.is_empty() || !query.contains(' ') {
        return None;
    }

    if regexp {
        let mut pattern = String::new();
        let mut in_class = false;
        let mut escaped = false;
        let mut replaced = false;
        for ch in query.chars() {
            if escaped {
                pattern.push(ch);
                escaped = false;
            } else if ch == '\\' {
                pattern.push(ch);
                escaped = true;
            } else if ch == '[' {
                in_class = true;
                pattern.push(ch);
            } else if ch == ']' {
                in_class = false;
                pattern.push(ch);
            } else if ch == ' ' && !in_class {
                pattern.push_str(&whitespace);
                replaced = true;
            } else {
                pattern.push(ch);
            }
        }
        return replaced.then_some(pattern);
    }

    let regexp_quote = Value::Sym(i.intern("regexp-quote"));
    let mut pattern = String::new();
    let mut literal = String::new();
    for ch in query.chars() {
        if ch == ' ' {
            if !literal.is_empty() {
                let quoted = i.apply(&regexp_quote, vec![Value::string(literal.clone())]).ok()?;
                let Value::Str(quoted) = quoted else { return None; };
                pattern.push_str(&quoted.borrow().to_unicode_lossy());
                literal.clear();
            }
            pattern.push_str(&whitespace);
        } else {
            literal.push(ch);
        }
    }
    if !literal.is_empty() {
        let quoted = i.apply(&regexp_quote, vec![Value::string(literal)]).ok()?;
        let Value::Str(quoted) = quoted else { return None; };
        pattern.push_str(&quoted.borrow().to_unicode_lossy());
    }
    Some(pattern)
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

fn isearch_yank_symbol_or_char(i: &mut Interp, position: usize) -> Option<String> {
    let source = format!(
        "(save-excursion (goto-char (min {} (point-max))) \
           (buffer-substring-no-properties (point) \
             (progn (if (or (eq (char-syntax (or (char-after) 0)) ?w) \
                            (eq (char-syntax (or (char-after) 0)) ?_) \
                            (eq (char-syntax (or (char-after (1+ (point))) 0)) ?w) \
                            (eq (char-syntax (or (char-after (1+ (point))) 0)) ?_)) \
                        (forward-symbol 1) (forward-char 1)) \
                    (point))))",
        position.saturating_add(1)
    );
    let Value::Str(text) = i.eval_str(&source).ok()? else {
        return None;
    };
    Some(text.borrow().to_unicode_lossy())
}

fn isearch_yank_char(i: &mut Interp, position: usize) -> Option<String> {
    let source = format!(
        "(save-excursion (goto-char (min {} (point-max))) \
           (buffer-substring-no-properties (point) \
             (progn (forward-char 1) (point))))",
        position.saturating_add(1)
    );
    let Value::Str(text) = i.eval_str(&source).ok()? else {
        return None;
    };
    Some(text.borrow().to_unicode_lossy())
}

fn isearch_yank_line(i: &mut Interp, position: usize) -> Option<String> {
    let source = format!(
        "(save-excursion (goto-char (min {} (point-max))) \
           (let ((inhibit-field-text-motion t)) \
             (buffer-substring-no-properties (point) \
               (line-end-position (if (eolp) 2 1)))))",
        position.saturating_add(1)
    );
    let Value::Str(text) = i.eval_str(&source).ok()? else {
        return None;
    };
    Some(text.borrow().to_unicode_lossy())
}

fn isearch_yank_until_char(i: &mut Interp, position: usize, delimiter: char) -> Option<String> {
    let source = format!(
        "(save-excursion (goto-char (min {} (point-max))) \
           (let ((start (point)) \
                 (end (search-forward (char-to-string {}) nil t))) \
             (when end (buffer-substring-no-properties start (1- end)))))",
        position.saturating_add(1),
        delimiter as u32
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

fn isearch_current_kill(i: &mut Interp, rotation: i128) -> Option<String> {
    let current_kill = Value::Sym(i.intern("current-kill"));
    match i.apply(&current_kill, vec![Value::Int(rotation)]) {
        Ok(Value::Str(text)) => Some(text.borrow().to_unicode_lossy()),
        Ok(other) => {
            let flow = i.wrong_type_mut("stringp", &other);
            report_flow(i, flow);
            None
        }
        Err(flow) => {
            report_flow(i, flow);
            None
        }
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

fn isearch_input_method_text(
    i: &mut Interp,
    key: i128,
) -> Result<Option<String>, crate::lisp::error::Flow> {
    let method = i
        .intern_soft("current-input-method")
        .filter(|symbol| i.bound_p(*symbol))
        .map(|symbol| i.symbol_value(symbol))
        .unwrap_or(Value::Nil);
    if !method.truthy() {
        return Ok(None);
    }

    // GNU's isearch-x reads through the active input method in a minibuffer.
    // Use a temporary buffer here so Quail's composition overlays and region
    // edits cannot touch the buffer being searched.
    let method = i.prin1_to_string(&method).to_unicode_lossy();
    let source = format!(
        "(with-temp-buffer (activate-input-method {}) \
           (funcall input-method-function {}))",
        method, key
    );
    let result = i.eval_str(&source)?;
    let events = match result {
        Value::Nil => Vec::new(),
        Value::Int(code) => vec![Value::Int(code)],
        Value::Str(text) => return Ok(Some(text.borrow().to_unicode_lossy())),
        other => other
            .list_to_vec()
            .map_err(|_| i.wrong_type_mut("listp", &other))?,
    };
    let text = events
        .into_iter()
        .filter_map(|event| event.int())
        .filter_map(|code| u32::try_from(code).ok().and_then(char::from_u32))
        .collect();
    Ok(Some(text))
}

fn finish_transient_input_method(i: &mut Interp) {
    let active = i
        .intern_soft("current-transient-input-method")
        .is_some_and(|symbol| i.bound_p(symbol) && i.symbol_value(symbol).truthy());
    if active {
        let command = Value::Sym(i.intern("deactivate-transient-input-method"));
        if let Err(flow) = i.apply(&command, Vec::new()) {
            report_flow(i, flow);
        }
    }
}

fn edit_isearch_query(
    i: &mut Interp,
    query: &str,
    regexp: bool,
) -> Result<String, crate::lisp::error::Flow> {
    let initial = i
        .prin1_to_string(&Value::string(query))
        .to_unicode_lossy();
    let history = if regexp {
        "regexp-search-ring"
    } else {
        "search-ring"
    };
    let source = format!(
        "(let ((map (copy-keymap minibuffer-local-map))) \
           (define-key map \"\\C-s\" 'exit-minibuffer) \
           (define-key map \"\\C-r\" 'exit-minibuffer) \
           (read-from-minibuffer \"I-search edit: \" {} map nil '{}' nil t))",
        initial, history
    );
    match i.eval_str(&source)? {
        Value::Str(text) => Ok(text.borrow().to_unicode_lossy()),
        other => Err(i.wrong_type_mut("stringp", &other)),
    }
}

fn read_nonincremental_search(
    i: &mut Interp,
    regexp: bool,
) -> Result<String, crate::lisp::error::Flow> {
    let prompt = if regexp { "RE search: " } else { "Search: " };
    let prompt = i
        .prin1_to_string(&Value::string(prompt))
        .to_unicode_lossy();
    let history = if regexp {
        "regexp-search-ring"
    } else {
        "search-ring"
    };
    let source = format!(
        "(read-from-minibuffer {} \"\" nil nil '{}' nil t)",
        prompt, history
    );
    match i.eval_str(&source)? {
        Value::Str(text) => Ok(text.borrow().to_unicode_lossy()),
        other => Err(i.wrong_type_mut("stringp", &other)),
    }
}

fn read_isearch_character_by_name(
    i: &mut Interp,
) -> Result<String, crate::lisp::error::Flow> {
    let read_char = Value::Sym(i.intern("read-char-by-name"));
    let value = i.apply(
        &read_char,
        vec![Value::string("Add character to search (Unicode name or hex): ")],
    )?;
    let Some(character) = value
        .int()
        .and_then(|code| u32::try_from(code).ok())
        .and_then(char::from_u32)
    else {
        return Err(i.wrong_type_mut("characterp", &value));
    };
    Ok(character.to_string())
}

fn read_isearch_emoji(i: &mut Interp) -> Result<String, crate::lisp::error::Flow> {
    let require = Value::Sym(i.intern("require"));
    let emoji_feature = Value::Sym(i.intern("emoji"));
    i.apply(&require, vec![emoji_feature])?;
    let initialize = Value::Sym(i.intern("emoji--init"));
    i.apply(&initialize, Vec::new())?;
    let read_emoji = Value::Sym(i.intern("emoji--read-emoji"));
    let result = i.apply(&read_emoji, Vec::new())?;
    let Value::Cons(result) = result else {
        return Err(i.wrong_type_mut("consp", &result));
    };
    let (glyph, derived) = {
        let result = result.borrow();
        (result.car.clone(), result.cdr.clone())
    };
    let emoji = if derived.is_nil() {
        glyph
    } else {
        let mut choices = vec![glyph];
        if let Ok(derived) = derived.list_to_vec() {
            choices.extend(derived);
        }
        let completing_read = Value::Sym(i.intern("completing-read"));
        i.apply(
            &completing_read,
            vec![
                Value::string("Select derivation: "),
                Value::list(choices),
                Value::Nil,
                Value::t(),
            ],
        )?
    };
    match emoji {
        Value::Str(text) => Ok(text.borrow().to_unicode_lossy()),
        other => Err(i.wrong_type_mut("stringp", &other)),
    }
}

/// What the isearch driver should do after a key.
pub(super) enum IsearchAction {
    /// Keep reading keys.
    Continue,
    /// Exit, restoring point to the entry position (C-g).
    Abort,
    /// Exit, keeping point and pushing the mark at entry (RET).
    Done,
    /// Run `occur' using the current I-search string, then exit the loop.
    Occur(String),
    /// Start query replacement using the current I-search string.
    QueryReplace(bool),
    /// Highlight all matches for the current I-search string and exit.
    Highlight(String, bool),
    /// Run a command without leaving I-search.
    RunCommand(i128),
    /// Exit and re-dispatch this key as a command.
    ReDispatch(i128),
}

impl Isearch {
    pub(super) fn new(i: &mut Interp, backward: bool, regexp: bool) -> Isearch {
        let start = i
            .current_buffer_ref()
            .map(|b| b.borrow().point())
            .unwrap_or(0);
        let search_default_mode = i
            .intern_soft("search-default-mode")
            .filter(|symbol| i.bound_p(*symbol))
            .map(|symbol| i.symbol_value(symbol))
            .unwrap_or(Value::Nil);
        let default_regexp = matches!(
            &search_default_mode,
            Value::Sym(symbol) if *symbol == crate::lisp::obarray::sym::T
        );
        let regexp_function = if !regexp
            && !default_regexp
            && crate::lisp::builtins::data::functionp_val(i, &search_default_mode)
        {
            search_default_mode
        } else {
            Value::Nil
        };
        let regexp = regexp || default_regexp;
        let case_fold = i
            .intern_soft("case-fold-search")
            .filter(|symbol| i.bound_p(*symbol))
            .map(|symbol| i.symbol_value(symbol))
            .unwrap_or(Value::Nil);
        let invisible = i
            .intern_soft("search-invisible")
            .filter(|symbol| i.bound_p(*symbol))
            .map(|symbol| i.symbol_value(symbol))
            .unwrap_or(Value::Nil);
        let lax_whitespace = i
            .intern_soft("isearch-lax-whitespace")
            .filter(|symbol| i.bound_p(*symbol))
            .map(|symbol| i.symbol_value(symbol).truthy())
            .unwrap_or(true);
        let regexp_lax_whitespace = i
            .intern_soft("isearch-regexp-lax-whitespace")
            .filter(|symbol| i.bound_p(*symbol))
            .map(|symbol| i.symbol_value(symbol).truthy())
            .unwrap_or(false);
        Isearch {
            query: String::new(),
            positions: vec![start],
            spans: vec![(start, start)],
            match_groups: Vec::new(),
            failing: false,
            wrapped: false,
            fail_pos: None,
            case_fold,
            invisible,
            history: Vec::new(),
            start,
            search_origin: start,
            backward,
            regexp,
            regexp_function,
            word_search: false,
            char_fold: false,
            symbol_search: false,
            lax_whitespace,
            regexp_lax_whitespace,
            quote_next: false,
            yank_until_char: false,
            help_prefix: false,
            cx_prefix: false,
            cx8_prefix: false,
            cx8_emoji_prefix: false,
            kill_yank_active: false,
            prefix_argument: None,
            prefix_digits: false,
            meta_s_prefix: false,
            meta_s_h_prefix: false,
            match_ov: None,
            lazy_ovs: Vec::new(),
            lazy_scan: None,
            submatch_ovs: Vec::new(),
            lazy_count_current: None,
            lazy_count_total: None,
            lazy_count_invisible: None,
            lazy_count_prefix: String::new(),
            lazy_count_suffix: String::new(),
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
            match_groups: self.match_groups.clone(),
            failing: self.failing,
            wrapped: self.wrapped,
            fail_pos: self.fail_pos,
            case_fold: self.case_fold.clone(),
            regexp: self.regexp,
            regexp_function: self.regexp_function.clone(),
            word_search: self.word_search,
            char_fold: self.char_fold,
            symbol_search: self.symbol_search,
            lax_whitespace: self.lax_whitespace,
            regexp_lax_whitespace: self.regexp_lax_whitespace,
            invisible: self.invisible.clone(),
            backward: self.backward,
            search_origin: self.search_origin,
            kill_yank_active: self.kill_yank_active,
        });
    }

    fn restore_history(&mut self, i: &Interp) {
        if let Some(previous) = self.history.pop() {
            self.query.truncate(previous.query_len);
            self.positions.truncate(previous.positions_len);
            self.spans.truncate(previous.spans_len);
            self.match_groups = previous.match_groups;
            self.failing = previous.failing;
            self.wrapped = previous.wrapped;
            self.fail_pos = previous.fail_pos;
            self.case_fold = previous.case_fold;
            self.regexp = previous.regexp;
            self.regexp_function = previous.regexp_function;
            self.word_search = previous.word_search;
            self.char_fold = previous.char_fold;
            self.symbol_search = previous.symbol_search;
            self.lax_whitespace = previous.lax_whitespace;
            self.regexp_lax_whitespace = previous.regexp_lax_whitespace;
            self.invisible = previous.invisible;
            self.backward = previous.backward;
            self.search_origin = previous.search_origin;
            self.kill_yank_active = previous.kill_yank_active;
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

    fn use_previous_search(&mut self, i: &mut Interp) -> bool {
        let ring_symbol = i.intern(if self.regexp {
            "regexp-search-ring"
        } else {
            "search-ring"
        });
        let Value::Cons(ring) = i.symbol_value(ring_symbol) else {
            return false;
        };
        let first = ring.borrow().car.clone();
        let Value::Str(text) = first else {
            return false;
        };
        self.query = text.borrow().to_unicode_lossy();
        if let Some(symbol) = i.intern_soft("isearch-last-case-fold-search")
            .filter(|symbol| i.bound_p(*symbol))
        {
            self.case_fold = i.symbol_value(symbol);
        }
        let pointer = i.intern(if self.regexp {
            "regexp-search-ring-yank-pointer"
        } else {
            "search-ring-yank-pointer"
        });
        let _ = i.set_symbol(pointer, Value::Int(0));
        true
    }

    fn complete_search_ring(&self, i: &mut Interp) -> Result<Option<String>, crate::lisp::error::Flow> {
        let ring_symbol = i.intern(if self.regexp {
            "regexp-search-ring"
        } else {
            "search-ring"
        });
        let ring = i.symbol_value(ring_symbol);
        let completion_ignore_case = i.intern("completion-ignore-case");
        let try_completion = Value::Sym(i.intern("try-completion"));
        let depth = i.specbind_depth();
        let result = i.specbind(completion_ignore_case, self.case_fold.clone()).and_then(|()| {
            i.apply(
                &try_completion,
                vec![Value::string(self.query.clone()), ring],
            )
        });
        let result = match (result, i.unbind_to(depth)) {
            (Ok(result), Ok(())) => result,
            (Err(flow), _) | (Ok(_), Err(flow)) => return Err(flow),
        };
        match result {
            Value::Nil => Ok(None),
            Value::Sym(symbol) if i.symbol_name(symbol) == "t" => {
                Ok(Some(self.query.clone()))
            }
            Value::Str(text) => Ok(Some(text.borrow().to_unicode_lossy())),
            other => Err(i.wrong_type_mut("stringp", &other)),
        }
    }

    fn update_search_ring(&self, i: &mut Interp) {
        if self.query.is_empty() {
            return;
        }
        let (ring_name, max_name) = if self.regexp {
            ("regexp-search-ring", "regexp-search-ring-max")
        } else {
            ("search-ring", "search-ring-max")
        };
        let ring = i.intern(ring_name);
        if !i.bound_p(ring)
            && let Err(flow) = i.set_symbol(ring, Value::Nil)
        {
            report_flow(i, flow);
            return;
        }
        let max_symbol = i.intern(max_name);
        let max = if i.bound_p(max_symbol) {
            i.symbol_value(max_symbol)
        } else {
            Value::Int(16)
        };
        let value = Value::string(self.query.clone());
        let add_to_history = Value::Sym(i.intern("add-to-history"));
        let delete_duplicates = i.intern("history-delete-duplicates");
        let depth = i.specbind_depth();
        let result = i.specbind(delete_duplicates, Value::t()).and_then(|()| {
            i.apply(
                &add_to_history,
                vec![Value::Sym(ring), value, max, Value::t()],
            )
        });
        match (result, i.unbind_to(depth)) {
            (Ok(_), Ok(())) => {}
            (Err(flow), _) | (Ok(_), Err(flow)) => report_flow(i, flow),
        }
        let last_case_fold = i.intern("isearch-last-case-fold-search");
        if let Err(flow) = i.set_symbol(last_case_fold, self.case_fold.clone()) {
            report_flow(i, flow);
        }
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
                    self.word_search,
                    self.char_fold,
                    self.symbol_search,
                    &self.regexp_function,
                    true,
                    self.lax_whitespace_enabled(),
                    match_start,
                    case_fold_search,
                    self.invisible.truthy(),
                ) {
                    Some((_, beg, end, groups))
                        if beg == match_start && end <= self.search_origin =>
                    {
                        Some((beg, beg, end, groups))
                    }
                    _ => isearch_search(
                        i,
                        &self.query,
                        true,
                        self.regexp,
                        self.word_search,
                        self.char_fold,
                        self.symbol_search,
                        &self.regexp_function,
                        true,
                        self.lax_whitespace_enabled(),
                        self.search_origin.min(match_end + 1),
                        case_fold_search,
                        self.invisible.truthy(),
                    ),
                }
            } else {
                isearch_search(
                    i,
                    &self.query,
                    false,
                    self.regexp,
                    self.word_search,
                    self.char_fold,
                    self.symbol_search,
                    &self.regexp_function,
                    true,
                    self.lax_whitespace_enabled(),
                    match_start,
                    case_fold_search,
                    self.invisible.truthy(),
                )
            }
        } else {
            isearch_search(
                i,
                &self.query,
                self.backward,
                self.regexp,
                self.word_search,
                self.char_fold,
                self.symbol_search,
                &self.regexp_function,
                true,
                self.lax_whitespace_enabled(),
                self.search_origin,
                case_fold_search,
                self.invisible.truthy(),
            )
        };
        match hit {
            Some((position, beg, end, groups)) => {
                self.positions.push(position);
                self.spans.push((beg, end));
                self.match_groups = groups;
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

    fn search_replaced_query(&mut self, i: &mut Interp, regexp_function_lax: bool) {
        let case_fold_search = self.case_fold_for_search(i);
        match isearch_search(
            i,
            &self.query,
            self.backward,
            self.regexp,
            self.word_search,
            self.char_fold,
            self.symbol_search,
            &self.regexp_function,
            regexp_function_lax,
            self.lax_whitespace_enabled(),
            self.cur(),
            case_fold_search,
            self.invisible.truthy(),
        ) {
            Some((position, beg, end, groups)) => {
                self.positions.push(position);
                self.spans.push((beg, end));
                self.match_groups = groups;
                self.failing = false;
                self.fail_pos = None;
                self.go_to(i, position);
            }
            None => {
                self.failing = true;
                self.fail_pos = Some(0);
            }
        }
    }

    fn append_query_text(&mut self, i: &mut Interp, text: &str) {
        if text.is_empty() {
            return;
        }
        let failure_start = self
            .fail_pos
            .unwrap_or_else(|| self.query.chars().count());
        self.save_history();
        self.query.push_str(text);
        self.search_updated_query(i, failure_start);
    }

    fn call_lisp_command(&self, i: &mut Interp, name: &str) {
        let command = Value::Sym(i.intern(name));
        if let Err(flow) = self.with_lisp_state(i, |i| i.apply(&command, Vec::new())) {
            report_flow(i, flow);
        }
    }

    fn delete_query_char(&mut self, i: &mut Interp) {
        if self.query.is_empty() {
            return;
        }
        self.save_history();
        self.query.pop();
        let anchor = self
            .spans
            .last()
            .map(|span| if self.backward { span.1 } else { span.0 })
            .unwrap_or(self.start);
        self.positions.push(anchor);
        self.spans.push((anchor, anchor));
        self.failing = false;
        self.fail_pos = None;
        self.go_to(i, anchor);
        self.search_replaced_query(i, true);
    }

    fn search_buffer_boundary(&mut self, i: &mut Interp, last: bool) {
        if self.query.is_empty() {
            return;
        }
        self.save_history();
        let (begv, end) = i
            .current_buffer_ref()
            .map(|buffer| {
                let buffer = buffer.borrow();
                (buffer.begv, buffer.text_len())
            })
            .unwrap_or((self.start, self.start));
        let case_fold_search = self.case_fold_for_search(i);
        let hit = isearch_search(
            i,
            &self.query,
            last,
            self.regexp,
            self.word_search,
            self.char_fold,
            self.symbol_search,
            &self.regexp_function,
            false,
            self.lax_whitespace_enabled(),
            if last { end } else { begv },
            case_fold_search,
            self.invisible.truthy(),
        );
        match hit {
            Some((position, beg, end, groups)) => {
                self.positions.push(position);
                self.spans.push((beg, end));
                self.match_groups = groups;
                self.failing = false;
                self.fail_pos = None;
                self.go_to(i, position);
            }
            None => {
                self.failing = true;
                self.fail_pos = Some(0);
            }
        }
    }

    fn repeat_searches(
        &mut self,
        i: &mut Interp,
        backward: bool,
        mut from: usize,
        mut count: usize,
        failure_start: usize,
        case_fold_search: bool,
    ) {
        if self.query.is_empty() {
            return;
        }
        while count > 0 {
            match isearch_search(
                i,
                &self.query,
                backward,
                self.regexp,
                self.word_search,
                self.char_fold,
                self.symbol_search,
                &self.regexp_function,
                false,
                self.lax_whitespace_enabled(),
                from,
                case_fold_search,
                self.invisible.truthy(),
            ) {
                Some((position, beg, end, groups)) => {
                    self.positions.push(position);
                    self.spans.push((beg, end));
                    self.match_groups = groups;
                    self.failing = false;
                    self.fail_pos = None;
                    self.go_to(i, position);
                    count -= 1;
                    from = if backward {
                        position.saturating_sub(1)
                    } else {
                        position.saturating_add(1)
                    };
                }
                None => {
                    self.failing = true;
                    self.fail_pos = Some(failure_start);
                    break;
                }
            }
        }
    }

    fn regexp_for_query(&self, i: &mut Interp) -> Option<String> {
        let (pattern, regexp) = isearch_search_pattern(
            i,
            &self.query,
            self.regexp,
            self.word_search,
            self.char_fold,
            self.symbol_search,
            &self.regexp_function,
            false,
            self.lax_whitespace_enabled(),
        )?;
        if regexp {
            return Some(pattern);
        }
        let quote = Value::Sym(i.intern("regexp-quote"));
        match i.apply(&quote, vec![Value::string(pattern)]).ok()? {
            Value::Str(pattern) => Some(pattern.borrow().to_unicode_lossy()),
            _ => None,
        }
    }

    fn lax_whitespace_enabled(&self) -> bool {
        if self.regexp {
            self.regexp_lax_whitespace
        } else {
            self.lax_whitespace
        }
    }

    fn toggle_regexp(&mut self, i: &mut Interp) {
        self.save_history();
        self.regexp = !self.regexp;
        self.regexp_function = Value::Nil;
        self.word_search = false;
        self.char_fold = false;
        self.symbol_search = false;
        self.failing = false;
        self.fail_pos = None;
        self.search_replaced_query(i, false);
    }

    fn toggle_word_search(&mut self, i: &mut Interp) {
        self.save_history();
        self.word_search = !self.word_search;
        self.regexp_function = Value::Nil;
        if self.word_search {
            self.regexp = false;
        }
        self.char_fold = false;
        self.symbol_search = false;
        self.failing = false;
        self.fail_pos = None;
        self.search_replaced_query(i, false);
    }

    fn toggle_char_fold(&mut self, i: &mut Interp) {
        self.save_history();
        self.char_fold = !self.char_fold;
        self.regexp_function = Value::Nil;
        if self.char_fold {
            self.regexp = false;
            self.word_search = false;
            self.symbol_search = false;
        }
        self.failing = false;
        self.fail_pos = None;
        self.search_replaced_query(i, false);
    }

    fn toggle_symbol_search(&mut self, i: &mut Interp) {
        self.save_history();
        self.symbol_search = !self.symbol_search;
        self.regexp_function = Value::Nil;
        if self.symbol_search {
            self.regexp = false;
            self.word_search = false;
            self.char_fold = false;
        }
        self.failing = false;
        self.fail_pos = None;
        self.search_replaced_query(i, false);
    }

    fn toggle_lax_whitespace(&mut self, i: &mut Interp) {
        self.save_history();
        if self.regexp {
            self.regexp_lax_whitespace = !self.regexp_lax_whitespace;
        } else {
            self.lax_whitespace = !self.lax_whitespace;
        }
        self.failing = false;
        self.fail_pos = None;
        self.search_replaced_query(i, false);
    }

    fn toggle_invisible(&mut self, i: &mut Interp) {
        self.save_history();
        self.invisible = if self.invisible.truthy() {
            Value::Nil
        } else {
            i.intern_soft("search-invisible")
                .filter(|symbol| i.bound_p(*symbol))
                .map(|symbol| i.symbol_value(symbol))
                .filter(Value::truthy)
                .unwrap_or_else(|| Value::Sym(i.intern("open")))
        };
        self.failing = false;
        self.fail_pos = None;
        self.search_replaced_query(i, false);
    }

    /// Handle one key event; updates query/point/failing state.
    pub(super) fn step(&mut self, i: &mut Interp, key: i128) -> IsearchAction {
        let is_yank_pop = key == (CHAR_META | 'y' as i128);
        let is_yank_kill = key == 25 || key == (CHAR_CTL | 'y' as i128);
        if !is_yank_pop && !is_yank_kill {
            self.kill_yank_active = false;
        }
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
        if self.yank_until_char {
            self.yank_until_char = false;
            let Some(delimiter) = isearch_quoted_character(key) else {
                return IsearchAction::ReDispatch(key);
            };
            let anchor = if self.backward {
                self.spans.last().map(|span| span.1).unwrap_or(cur)
            } else {
                cur
            };
            match isearch_yank_until_char(i, anchor, delimiter) {
                Some(text) if !text.is_empty() => {
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
                None => {
                    let message = format!("`{delimiter}' not found");
                    i.set_echo_message_with_properties(EmacsText::from(message.as_str()), Vec::new());
                }
                _ => {}
            }
            return IsearchAction::Continue;
        }
        if self.cx8_emoji_prefix {
            self.cx8_emoji_prefix = false;
            if key == 13 {
                match read_isearch_emoji(i) {
                    Ok(text) => self.append_query_text(i, &text),
                    Err(flow) => report_flow(i, flow),
                }
                return IsearchAction::Continue;
            }
            return IsearchAction::ReDispatch(key);
        }
        if self.cx8_prefix {
            self.cx8_prefix = false;
            if key == 13 {
                match read_isearch_character_by_name(i) {
                    Ok(text) => self.append_query_text(i, &text),
                    Err(flow) => report_flow(i, flow),
                }
                return IsearchAction::Continue;
            }
            if key == 'e' as i128 {
                self.cx8_emoji_prefix = true;
                return IsearchAction::Continue;
            }
            return IsearchAction::ReDispatch(key);
        }
        if self.cx_prefix {
            self.cx_prefix = false;
            if key == '\\' as i128 {
                self.call_lisp_command(i, "isearch-transient-input-method");
                return IsearchAction::Continue;
            }
            if key == '8' as i128 {
                self.cx8_prefix = true;
                return IsearchAction::Continue;
            }
            return IsearchAction::ReDispatch(key);
        }
        if self.help_prefix {
            self.help_prefix = false;
            if key == 8 || key == (CHAR_CTL | 'h' as i128) {
                let help = Value::Sym(i.intern("isearch-help-for-help"));
                if let Err(flow) = self.with_lisp_state(i, |i| i.apply(&help, Vec::new())) {
                    report_flow(i, flow);
                }
                return IsearchAction::Continue;
            }
            return IsearchAction::ReDispatch(key);
        }
        if self.meta_s_prefix {
            self.meta_s_prefix = false;
            match key {
                c if c == 'h' as i128 => {
                    self.meta_s_h_prefix = true;
                    return IsearchAction::Continue;
                }
                c if c == 'i' as i128 => {
                    self.toggle_invisible(i);
                    return IsearchAction::Continue;
                }
                c if c == 'r' as i128 => {
                    self.toggle_regexp(i);
                    return IsearchAction::Continue;
                }
                c if c == 'w' as i128 => {
                    self.toggle_word_search(i);
                    return IsearchAction::Continue;
                }
                c if c == '_' as i128 => {
                    self.toggle_symbol_search(i);
                    return IsearchAction::Continue;
                }
                c if c == '\'' as i128 => {
                    self.toggle_char_fold(i);
                    return IsearchAction::Continue;
                }
                c if c == ' ' as i128 => {
                    self.toggle_lax_whitespace(i);
                    return IsearchAction::Continue;
                }
                c if c == 'o' as i128 => {
                    return self
                        .regexp_for_query(i)
                        .map_or(IsearchAction::Continue, IsearchAction::Occur);
                }
                c if c == 5 || c == (CHAR_CTL | 'e' as i128) => {
                    let anchor = if self.backward {
                        self.spans.last().map(|span| span.1).unwrap_or(cur)
                    } else {
                        cur
                    };
                    if let Some(text) = isearch_yank_line(i, anchor)
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
                    return IsearchAction::Continue;
                }
                c if c == (CHAR_META | '<' as i128) => {
                    self.search_buffer_boundary(i, false);
                    return IsearchAction::Continue;
                }
                c if c == (CHAR_META | '>' as i128) => {
                    self.search_buffer_boundary(i, true);
                    return IsearchAction::Continue;
                }
                7 => return self.step(i, key),
                other => return IsearchAction::ReDispatch(other),
            }
        }
        if self.meta_s_h_prefix {
            self.meta_s_h_prefix = false;
            if key == 'r' as i128 {
                return self
                    .regexp_for_query(i)
                    .map_or(IsearchAction::Continue, |pattern| {
                        IsearchAction::Highlight(pattern, false)
                    });
            }
            if key == 'l' as i128 {
                return self
                    .regexp_for_query(i)
                    .map_or(IsearchAction::Continue, |pattern| {
                        IsearchAction::Highlight(pattern, true)
                    });
            }
            return IsearchAction::ReDispatch(key);
        }
        let isearch_allow_prefix = i
            .intern_soft("isearch-allow-prefix")
            .map(|symbol| !i.bound_p(symbol) || i.symbol_value(symbol).truthy())
            .unwrap_or(true);
        if isearch_allow_prefix && (key == 21 || key == (CHAR_CTL | 'u' as i128)) {
            self.prefix_argument = Some(
                self.prefix_argument
                    .map(|argument| argument.saturating_mul(4))
                    .unwrap_or(4),
            );
            self.prefix_digits = false;
            return IsearchAction::Continue;
        }
        if isearch_allow_prefix
            && let Some(argument) = self.prefix_argument
            && (48..=57).contains(&key)
        {
            let digit = key - 48;
            self.prefix_argument = Some(if self.prefix_digits {
                argument.saturating_mul(10).saturating_add(digit)
            } else {
                digit
            });
            self.prefix_digits = true;
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
                if self.query.is_empty()
                    && i
                        .intern_soft("search-nonincremental-instead")
                        .map(|symbol| i.symbol_value(symbol).truthy())
                        .unwrap_or(true)
                {
                    match read_nonincremental_search(i, self.regexp) {
                        Ok(query) => {
                            self.query = query;
                            self.failing = false;
                            self.fail_pos = None;
                            self.search_replaced_query(i, false);
                        }
                        Err(flow) => {
                            report_flow(i, flow);
                            self.go_to(i, self.start);
                            return IsearchAction::Abort;
                        }
                    }
                }
                if let Some(b) = i.current_buffer_ref() {
                    b.borrow_mut().set_mark(self.start);
                }
                IsearchAction::Done
            }
            // C-s / C-r: repeat search in that direction from just
            // past (before) the current match.
            c if c == 19
                || c == 18
                || c == (CHAR_META | 19)
                || c == (CHAR_META | 18)
                || c == (CHAR_META | CHAR_CTL | 's' as i128)
                || c == (CHAR_META | CHAR_CTL | 'r' as i128) =>
            {
                let key_back = c == 18
                    || c == (CHAR_META | 18)
                    || c == (CHAR_META | CHAR_CTL | 'r' as i128);
                let prefix_argument = self.prefix_argument.take().unwrap_or(1);
                self.prefix_digits = false;
                if prefix_argument == 0 {
                    return IsearchAction::Continue;
                }
                let back = if prefix_argument < 0 {
                    !key_back
                } else {
                    key_back
                };
                let repeat_count = usize::try_from(
                    prefix_argument.checked_abs().unwrap_or(i128::MAX),
                )
                .unwrap_or(usize::MAX);
                let same_direction = back == self.backward;
                let repeat_on_direction_change = i
                    .intern_soft("isearch-repeat-on-direction-change")
                    .is_some_and(|symbol| i.bound_p(symbol) && i.symbol_value(symbol).truthy());
                if !same_direction && !repeat_on_direction_change {
                    self.save_history();
                    self.backward = back;
                    let span = self.spans.last().copied().unwrap_or((cur, cur));
                    let point = if back { span.1 } else { span.0 };
                    // Later query extensions in the new direction must
                    // continue from the current match, not the entry point.
                    self.search_origin = point;
                    self.positions.push(point);
                    self.spans.push(span);
                    self.failing = false;
                    self.fail_pos = None;
                    self.go_to(i, point);
                    if repeat_count > 1 {
                        let case_fold_search = self.case_fold_for_search(i);
                        let from = if back {
                            point.saturating_sub(1)
                        } else {
                            point.saturating_add(1)
                        };
                        self.repeat_searches(
                            i,
                            back,
                            from,
                            repeat_count - 1,
                            0,
                            case_fold_search,
                        );
                    }
                    return IsearchAction::Continue;
                }
                let reused_previous = self.query.is_empty()
                    && same_direction
                    && self.use_previous_search(i);
                if self.query.is_empty() {
                    return IsearchAction::Continue;
                }
                self.save_history();
                if !same_direction {
                    let span = self.spans.last().copied().unwrap_or((cur, cur));
                    // Keep the new direction's boundary at the old match.
                    self.search_origin = if back { span.1 } else { span.0 };
                }
                self.backward = back;
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
                let wrapped = self.failing && same_direction && wrap_enabled;
                if wrapped {
                    self.wrapped = true;
                }
                let from = if wrapped {
                    i.current_buffer_ref()
                        .map(|buffer| {
                            let buffer = buffer.borrow();
                            if back { buffer.zv } else { buffer.begv }
                        })
                        .unwrap_or(cur)
                } else if reused_previous {
                    cur
                } else if back {
                    cur.saturating_sub(1)
                } else {
                    cur + 1
                };
                let failure_start = self.fail_pos.unwrap_or(0);
                let case_fold_search = self.case_fold_for_search(i);
                self.repeat_searches(i, back, from, repeat_count, failure_start, case_fold_search);
                IsearchAction::Continue
            }
            // DEL: unwind one step of the search.
            127 => {
                self.restore_history(i);
                IsearchAction::Continue
            }
            c if c == 8 || c == (CHAR_CTL | 'h' as i128) => {
                self.help_prefix = true;
                IsearchAction::Continue
            }
            c if c == 24 || c == (CHAR_CTL | 'x' as i128) => {
                self.cx_prefix = true;
                IsearchAction::Continue
            }
            c if c == 17 || c == (CHAR_CTL | 'q' as i128) => {
                self.quote_next = true;
                IsearchAction::Continue
            }
            c if c == 30 || c == (CHAR_CTL | '^' as i128) => {
                self.call_lisp_command(i, "isearch-toggle-specified-input-method");
                IsearchAction::Continue
            }
            c if c == 28 || c == (CHAR_CTL | '\\' as i128) => {
                self.call_lisp_command(i, "isearch-toggle-input-method");
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
            c if c == (CHAR_META | CHAR_CTL | 'w' as i128)
                || c == (CHAR_META | 23) =>
            {
                let anchor = if self.backward {
                    self.spans.last().map(|span| span.1).unwrap_or(cur)
                } else {
                    cur
                };
                if let Some(text) = isearch_yank_symbol_or_char(i, anchor)
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
            c if c == (CHAR_META | CHAR_CTL | 'z' as i128)
                || c == (CHAR_META | 26) =>
            {
                self.yank_until_char = true;
                IsearchAction::Continue
            }
            c if c == (CHAR_META | CHAR_CTL | 'y' as i128)
                || c == (CHAR_META | 25) =>
            {
                let anchor = if self.backward {
                    self.spans.last().map(|span| span.1).unwrap_or(cur)
                } else {
                    cur
                };
                if let Some(text) = isearch_yank_char(i, anchor)
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
            c if c == 25 || c == (CHAR_CTL | 'y' as i128) => {
                if let Some(text) = isearch_current_kill(i, 0)
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
                    self.kill_yank_active = true;
                    self.search_updated_query(i, failure_start);
                }
                IsearchAction::Continue
            }
            c if c == (CHAR_META | 'y' as i128) => {
                let replace_yank = self.kill_yank_active;
                if replace_yank {
                    self.restore_history(i);
                }
                if let Some(text) = isearch_current_kill(i, if replace_yank { 1 } else { 0 })
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
                    self.kill_yank_active = true;
                    self.search_updated_query(i, failure_start);
                }
                IsearchAction::Continue
            }
            c if c == (CHAR_META | CHAR_CTL | 'd' as i128)
                || c == (CHAR_META | 4) =>
            {
                self.delete_query_char(i);
                IsearchAction::Continue
            }
            c if c == (CHAR_META | 9)
                || c == (CHAR_META | CHAR_CTL | 'i' as i128) =>
            {
                self.save_history();
                match self.complete_search_ring(i) {
                    Ok(Some(query)) => {
                        self.query = query;
                        match edit_isearch_query(i, &self.query, self.regexp) {
                            Ok(query) => {
                                self.query = query;
                                self.failing = false;
                                self.fail_pos = None;
                                self.search_replaced_query(i, false);
                            }
                            Err(flow) => report_flow(i, flow),
                        }
                    }
                    Ok(None) => {}
                    Err(flow) => report_flow(i, flow),
                }
                IsearchAction::Continue
            }
            c if c == (CHAR_META | '%' as i128) => IsearchAction::QueryReplace(false),
            c if c == (CHAR_META | CHAR_CTL | '%' as i128) => {
                IsearchAction::QueryReplace(true)
            }
            c if c == (CHAR_META | 'p' as i128) || c == (CHAR_META | 'n' as i128) => {
                let advance = c == (CHAR_META | 'n' as i128);
                let ring_symbol = i.intern(if self.regexp {
                    "regexp-search-ring"
                } else {
                    "search-ring"
                });
                let ring = i
                    .symbol_value(ring_symbol)
                    .list_to_vec()
                    .unwrap_or_default();
                if !ring.is_empty() {
                    let pointer_symbol = i.intern(if self.regexp {
                        "regexp-search-ring-yank-pointer"
                    } else {
                        "search-ring-yank-pointer"
                    });
                    let length = ring.len() as i128;
                    let pointer = i
                        .symbol_value(pointer_symbol)
                        .int()
                        .unwrap_or(if advance { 0 } else { -1 });
                    let index = (pointer + if advance { -1 } else { 1 }).rem_euclid(length);
                    if let Value::Str(text) = ring[index as usize].clone() {
                        self.save_history();
                        let _ = i.set_symbol(pointer_symbol, Value::Int(index));
                        self.query = text.borrow().to_unicode_lossy();
                        if let Some(symbol) = i
                            .intern_soft("isearch-last-case-fold-search")
                            .filter(|symbol| i.bound_p(*symbol))
                        {
                            self.case_fold = i.symbol_value(symbol);
                        }
                        self.failing = false;
                        self.fail_pos = None;
                        self.search_replaced_query(i, false);
                    }
                }
                IsearchAction::Continue
            }
            c if c == (CHAR_META | 'e' as i128) => {
                self.save_history();
                match edit_isearch_query(i, &self.query, self.regexp) {
                    Ok(query) => {
                        self.query = query;
                        self.failing = false;
                        self.fail_pos = None;
                        self.search_replaced_query(i, false);
                    }
                    Err(flow) => report_flow(i, flow),
                }
                IsearchAction::Continue
            }
            c if c == (CHAR_META | 's' as i128) => {
                self.meta_s_prefix = true;
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
            c if c == (CHAR_META | 'r' as i128) => {
                self.toggle_regexp(i);
                IsearchAction::Continue
            }
            c if c >= 32
                && c < 0x110000
                && c & (CHAR_META | CHAR_CTL | CHAR_SHIFT | CHAR_SUPER | CHAR_HYPER) == 0 =>
            {
                let failure_start = self
                    .fail_pos
                    .unwrap_or_else(|| self.query.chars().count());
                let text = match isearch_input_method_text(i, c) {
                    Ok(Some(text)) => text,
                    Ok(None) => char::from_u32(c as u32)
                        .map(|ch| ch.to_string())
                        .unwrap_or_default(),
                    Err(flow) => {
                        report_flow(i, flow);
                        String::new()
                    }
                };
                if !text.is_empty() {
                    finish_transient_input_method(i);
                    self.save_history();
                    self.query.push_str(&text);
                    self.search_updated_query(i, failure_start);
                }
                IsearchAction::Continue
            }
            other => {
                let option = i
                    .intern_soft("search-exit-option")
                    .map(|symbol| i.symbol_value(symbol))
                    .unwrap_or(Value::t());
                if matches!(&option, Value::Sym(symbol) if i.symbol_name(*symbol) == "append") {
                    if let Some(ch) = isearch_quoted_character(other) {
                        let failure_start = self
                            .fail_pos
                            .unwrap_or_else(|| self.query.chars().count());
                        self.save_history();
                        self.query.push(ch);
                        self.search_updated_query(i, failure_start);
                    }
                    IsearchAction::Continue
                } else if matches!(&option, Value::Sym(symbol) if i.symbol_name(*symbol) == "edit") {
                    self.save_history();
                    match edit_isearch_query(i, &self.query, self.regexp) {
                        Ok(query) => {
                            self.query = query;
                            self.failing = false;
                            self.fail_pos = None;
                            self.search_replaced_query(i, false);
                        }
                        Err(flow) => report_flow(i, flow),
                    }
                    IsearchAction::Continue
                } else if option.is_nil() {
                    IsearchAction::RunCommand(other)
                } else {
                    if let Some(b) = i.current_buffer_ref() {
                        b.borrow_mut().set_mark(self.start);
                    }
                    IsearchAction::ReDispatch(other)
                }
            }
        }
    }

    /// Echo-area label, e.g. `Failing I-search backward: foo`.
    pub(super) fn label(&self) -> (String, Option<(usize, usize)>) {
        if self.help_prefix {
            return ("I-search help: ".to_owned(), None);
        }
        if self.yank_until_char {
            return ("Yank until character: ".to_owned(), None);
        }
        let dir = if self.backward { " backward" } else { "" };
        let re = if self.regexp { " regexp" } else { "" };
        let status = if self.failing { "Failing " } else { "" };
        let wrapped = if self.wrapped {
            if self.failing { "wrapped " } else { "Wrapped " }
        } else {
            ""
        };
        let prefix = format!(
            "{}{}{}I-search{}{}: ",
            self.lazy_count_prefix, status, wrapped, dir, re
        );
        let prefix_length = prefix.chars().count();
        let query_length = self.query.chars().count();
        let failure = self.failing.then(|| {
            (
                prefix_length + self.fail_pos.unwrap_or(0).min(query_length),
                prefix_length + query_length,
            )
        });
        (
            format!("{prefix}{}{}", self.query, self.lazy_count_suffix),
            failure,
        )
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
        let (match_start, match_end) = self
            .spans
            .last()
            .copied()
            .unwrap_or((self.start, self.start));
        let regexp_function = if self.word_search {
            Value::Sym(i.intern("word-search-regexp"))
        } else if self.char_fold {
            Value::Sym(i.intern("char-fold-to-regexp"))
        } else if self.symbol_search {
            Value::Sym(i.intern("isearch-symbol-regexp"))
        } else {
            self.regexp_function.clone()
        };
        let input_method_function = i
            .intern_soft("input-method-function")
            .filter(|symbol| i.bound_p(*symbol))
            .map(|symbol| i.symbol_value(symbol))
            .unwrap_or(Value::Nil);
        let bindings = [
            ("isearch-string", Value::string(self.query.clone())),
            ("isearch-message", Value::string(self.query.clone())),
            (
                "isearch-forward",
                if self.backward {
                    Value::Nil
                } else {
                    Value::t()
                },
            ),
            (
                "isearch-regexp",
                if self.regexp { Value::t() } else { Value::Nil },
            ),
            ("isearch-regexp-function", regexp_function),
            ("isearch-case-fold-search", self.case_fold.clone()),
            ("isearch-invisible", self.invisible.clone()),
            (
                "isearch-lax-whitespace",
                if self.lax_whitespace {
                    Value::t()
                } else {
                    Value::Nil
                },
            ),
            (
                "isearch-regexp-lax-whitespace",
                if self.regexp_lax_whitespace {
                    Value::t()
                } else {
                    Value::Nil
                },
            ),
            ("isearch-input-method-function", input_method_function),
            (
                "isearch-success",
                if self.failing { Value::Nil } else { Value::t() },
            ),
            (
                "isearch-wrapped",
                if self.wrapped { Value::t() } else { Value::Nil },
            ),
            (
                "isearch-lazy-count-current",
                self.lazy_count_current
                    .map(|count| Value::Int(count as i128))
                    .unwrap_or(Value::Nil),
            ),
            (
                "isearch-lazy-count-total",
                self.lazy_count_total
                    .map(|count| Value::Int(count as i128))
                    .unwrap_or(Value::Nil),
            ),
            (
                "isearch-lazy-count-invisible",
                self.lazy_count_invisible
                    .map(|count| Value::Int(count as i128))
                    .unwrap_or(Value::Nil),
            ),
            ("isearch-error", Value::Nil),
            (
                "isearch-other-end",
                Value::Int(
                    (if self.backward {
                        match_end
                    } else {
                        match_start
                    }) as i128
                        + 1,
                ),
            ),
        ];
        for (name, value) in bindings {
            let forced_variable = matches!(
                name,
                "isearch-wrapped"
                    | "isearch-lazy-count-current"
                    | "isearch-lazy-count-total"
                    | "isearch-lazy-count-invisible"
            );
            let symbol = if forced_variable {
                i.intern(name)
            } else {
                let Some(symbol) = i.intern_soft(name) else {
                    continue;
                };
                symbol
            };
            if !forced_variable && !i.bound_p(symbol) {
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
        self.lazy_scan = None;
        if let Some(ov) = self.match_ov.take() {
            Self::call(i, "delete-overlay", vec![ov]);
        }
        for ov in self.lazy_ovs.drain(..) {
            Self::call(i, "delete-overlay", vec![ov]);
        }
        for ov in self.submatch_ovs.drain(..) {
            Self::call(i, "delete-overlay", vec![ov]);
        }
    }

    fn clear_lazy_count(&mut self) {
        self.lazy_count_current = None;
        self.lazy_count_total = None;
        self.lazy_count_invisible = None;
        self.lazy_count_prefix.clear();
        self.lazy_count_suffix.clear();
    }

    fn format_lazy_count(
        i: &mut Interp,
        format_name: &str,
        default_format: Option<&str>,
        current: usize,
        total: usize,
        invisible: Option<usize>,
        backward: bool,
    ) -> String {
        let format_value = i
            .intern_soft(format_name)
            .filter(|symbol| i.bound_p(*symbol))
            .map(|symbol| i.symbol_value(symbol))
            .unwrap_or_else(|| default_format.map(Value::string).unwrap_or(Value::Nil));
        if format_value.is_nil() {
            return String::new();
        }

        let format_function = Value::Sym(i.intern("format"));
        let current = if backward && current > 0 {
            total.saturating_sub(current).saturating_add(1)
        } else {
            current
        };
        let mut total_value = Value::Int(total as i128);
        if let Some(invisible) = invisible {
            let invisible_format = i
                .intern_soft("lazy-count-invisible-format")
                .filter(|symbol| i.bound_p(*symbol))
                .map(|symbol| i.symbol_value(symbol))
                .unwrap_or_else(|| Value::string(" (invisible %s)"));
            if !invisible_format.is_nil()
                && let Ok(Value::Str(suffix)) = i.apply(
                    &format_function,
                    vec![invisible_format, Value::Int(invisible as i128)],
                )
            {
                total_value =
                    Value::string(format!("{total}{}", suffix.borrow().to_unicode_lossy()));
            }
        }

        match i.apply(
            &format_function,
            vec![format_value, Value::Int(current as i128), total_value],
        ) {
            Ok(Value::Str(value)) => value.borrow().to_unicode_lossy(),
            _ => String::new(),
        }
    }

    fn update_lazy_count(&mut self, i: &mut Interp, span: (usize, usize), restore: usize) {
        self.clear_lazy_count();
        let count_enabled = i
            .intern_soft("isearch-lazy-count")
            .filter(|symbol| i.bound_p(*symbol))
            .map(|symbol| i.symbol_value(symbol).truthy())
            .unwrap_or(false);
        if !count_enabled || self.query.is_empty() || self.failing {
            return;
        }

        let (begv, zv) = i
            .current_buffer_ref()
            .map(|buffer| {
                let buffer = buffer.borrow();
                (buffer.begv, buffer.zv)
            })
            .unwrap_or((self.start, self.start));
        let case_fold_search = self.case_fold_for_search(i);
        let mut pos = begv;
        let mut total = 0usize;
        let mut current_forward = 0usize;
        let mut invisible = 0usize;
        while pos <= zv {
            let Some((_, beg, end, _)) = isearch_search(
                i,
                &self.query,
                false,
                self.regexp,
                self.word_search,
                self.char_fold,
                self.symbol_search,
                &self.regexp_function,
                false,
                self.lax_whitespace_enabled(),
                pos,
                case_fold_search,
                true,
            ) else {
                break;
            };
            if beg > zv || end > zv {
                break;
            }
            total = total.saturating_add(1);
            if (beg, end) == span {
                current_forward = total;
            }
            if isearch_span_invisible(i, beg, end) {
                invisible = invisible.saturating_add(1);
            }
            let next = if beg == end {
                end.saturating_add(1)
            } else {
                end
            };
            if next <= pos {
                break;
            }
            pos = next;
        }
        self.go_to(i, restore);
        let current = current_forward;
        let invisible = (invisible > 0).then_some(invisible);
        self.lazy_count_current = Some(current);
        self.lazy_count_total = Some(total);
        self.lazy_count_invisible = invisible;
        self.lazy_count_prefix = Self::format_lazy_count(
            i,
            "lazy-count-prefix-format",
            Some("%s/%s "),
            current,
            total,
            invisible,
            self.backward,
        );
        self.lazy_count_suffix = Self::format_lazy_count(
            i,
            "lazy-count-suffix-format",
            None,
            current,
            total,
            invisible,
            self.backward,
        );
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
        self.lazy_scan = None;
        for ov in self.submatch_ovs.drain(..) {
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
        if !self.query.is_empty() && span.0 < span.1 && !self.failing && lazy_on {
            self.update_lazy_count(i, span, restore);
        } else {
            self.clear_lazy_count();
        }
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

        // GNU's `isearch-highlight' overlays each captured subexpression
        // above the whole-match overlay.  Match positions are kept in the
        // search state because the lazy-highlight scan below performs more
        // regexp searches and overwrites Lisp's live match-data.
        let submatches_on = i
            .intern_soft("search-highlight-submatches")
            .map(|s| i.symbol_value(s).truthy())
            .unwrap_or(true);
        let groups_match_span = self
            .match_groups
            .first()
            .copied()
            .flatten()
            .is_some_and(|group_zero| group_zero == span);
        if self.regexp && submatches_on && groups_match_span {
            let facep = Value::Sym(i.intern("facep"));
            let mut group = 0usize;
            for (beg, end) in self.match_groups.iter().skip(1).flatten() {
                group += 1;
                let face_name = format!("isearch-group-{group}");
                let face = i.intern_soft(&face_name).filter(|face| {
                    i.apply(&facep, vec![Value::Sym(*face)])
                        .is_ok_and(|value| value.truthy())
                });
                let face = match face {
                    Some(face) => face,
                    None => {
                        // Like GNU, recycle the known group faces when a
                        // higher-numbered face has not been defined.
                        group = 1;
                        i.intern("isearch-group-1")
                    }
                };
                let make_overlay = Value::Sym(i.q("make-overlay"));
                let Ok(ov) = i.apply(
                    &make_overlay,
                    vec![Value::Int(*beg as i128 + 1), Value::Int(*end as i128 + 1)],
                ) else {
                    continue;
                };
                let face_property = Value::Sym(i.q("face"));
                let priority = Value::Sym(i.q("priority"));
                Self::call(
                    i,
                    "overlay-put",
                    vec![ov.clone(), face_property, Value::Sym(face)],
                );
                Self::call(
                    i,
                    "overlay-put",
                    vec![ov.clone(), priority, Value::Int(1002)],
                );
                self.submatch_ovs.push(ov);
            }
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
        let max_at_a_time = i
            .intern_soft("lazy-highlight-max-at-a-time")
            .map(|symbol| i.symbol_value(symbol))
            .and_then(|value| value.int())
            .map(|limit| usize::try_from(limit.max(1)).unwrap_or(usize::MAX))
            .unwrap_or(usize::MAX);
        let no_delay_length = i
            .intern_soft("lazy-highlight-no-delay-length")
            .filter(|symbol| i.bound_p(*symbol))
            .and_then(|symbol| i.symbol_value(symbol).int())
            .unwrap_or(3)
            .max(0) as usize;
        let initial_delay = if self.query.chars().count() < no_delay_length {
            isearch_delay_seconds(i, "lazy-highlight-initial-delay", 0.25)
        } else {
            0.0
        };
        let interval = isearch_delay_duration(isearch_delay_seconds(
            i,
            "lazy-highlight-interval",
            0.0,
        ));
        let delay = isearch_delay_duration(initial_delay);
        let now = std::time::Instant::now();
        self.lazy_scan = Some(LazyHighlightScan {
            position: wstart,
            window_end: wend,
            current_span: span,
            window: wv,
            next_at: now.checked_add(delay).unwrap_or(now),
            interval,
            max_at_a_time,
        });
        if delay.is_zero() {
            self.update_lazy_highlights(i);
        }
    }

    /// Add the next visible lazy-highlight batch when its timer is due.
    /// Positive intervals yield after each match so the command loop can
    /// process input between updates; an interval of zero preserves the
    /// existing immediate batch behavior.
    pub(super) fn update_lazy_highlights(&mut self, i: &mut Interp) -> bool {
        let Some(mut scan) = self.lazy_scan.take() else {
            return false;
        };
        let now = std::time::Instant::now();
        if now < scan.next_at {
            self.lazy_scan = Some(scan);
            return false;
        }

        let restore = i
            .current_buffer_ref()
            .map(|buffer| buffer.borrow().point())
            .unwrap_or(self.cur());
        let mut count = 0usize;
        let mut highlighted = false;
        let batch_limit = if scan.interval.is_zero() {
            scan.max_at_a_time
        } else {
            1
        };
        let case_fold_search = self.case_fold.truthy();
        let pri = Value::Sym(i.q("priority"));
        let face = Value::Sym(i.q("face"));
        let lazy = Value::Sym(i.q("lazy-highlight"));
        let winprop = Value::Sym(i.q("window"));
        while count < batch_limit {
            let Some((_, beg, end, _)) = isearch_search(
                i,
                &self.query,
                false,
                self.regexp,
                self.word_search,
                self.char_fold,
                self.symbol_search,
                &self.regexp_function,
                false,
                self.lax_whitespace_enabled(),
                scan.position,
                case_fold_search,
                self.invisible.truthy(),
            ) else {
                scan.position = scan.window_end;
                break;
            };
            if beg >= scan.window_end || end <= beg {
                scan.position = scan.window_end;
                break;
            }
            scan.position = end;
            if (beg, end) == scan.current_span {
                continue;
            }
            let make_overlay = Value::Sym(i.q("make-overlay"));
            let Ok(overlay) = i.apply(
                &make_overlay,
                vec![Value::Int(beg as i128 + 1), Value::Int(end as i128 + 1)],
            ) else {
                scan.position = scan.window_end;
                break;
            };
            Self::call(
                i,
                "overlay-put",
                vec![overlay.clone(), pri.clone(), Value::Int(1000)],
            );
            Self::call(
                i,
                "overlay-put",
                vec![overlay.clone(), face.clone(), lazy.clone()],
            );
            Self::call(
                i,
                "overlay-put",
                vec![overlay.clone(), winprop.clone(), scan.window.clone()],
            );
            self.lazy_ovs.push(overlay);
            highlighted = true;
            count += 1;
        }
        self.go_to(i, restore);

        if scan.position < scan.window_end {
            let now = std::time::Instant::now();
            scan.next_at = now.checked_add(scan.interval).unwrap_or(now);
            self.lazy_scan = Some(scan);
        }
        highlighted
    }
}

/// Incremental search loop (C-s / C-r). Reads keys directly until the
/// search exits via RET, C-g, or a non-search key (which is then
/// re-dispatched as a command, like Emacs).
fn run_isearch_highlight(i: &mut Interp, pattern: String, case_fold: Value, lines: bool) {
    let depth = i.specbind_depth();
    let result = (|| {
        let case_fold_search = i.intern("case-fold-search");
        i.specbind(case_fold_search, case_fold)?;
        let search_upper_case = i.intern("search-upper-case");
        i.specbind(search_upper_case, Value::Nil)?;
        let require = Value::Sym(i.intern("require"));
        let hi_lock = Value::Sym(i.intern("hi-lock"));
        i.apply(&require, vec![hi_lock])?;
        let read_face = Value::Sym(i.intern("hi-lock-read-face-name"));
        let face = i.apply(&read_face, Vec::new())?;
        let highlight = Value::Sym(i.intern(if lines {
            "highlight-lines-matching-regexp"
        } else {
            "highlight-regexp"
        }));
        i.apply(&highlight, vec![Value::string(pattern), face])?;
        Ok(())
    })();
    match (result, i.unbind_to(depth)) {
        (Ok(()), Ok(())) => {}
        (Err(flow), _) | (Ok(()), Err(flow)) => report_flow(i, flow),
    }
}

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
        render_needed |= st.update_lazy_highlights(i);
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
            IsearchAction::Done => {
                if !st.failing {
                    st.update_search_ring(i);
                }
                break;
            }
            IsearchAction::RunCommand(other) => {
                let seq = Value::vector(vec![Value::Int(other)]);
                if let LookupResult::Command(cmd) = lookup_command(i, &seq) {
                    match i.command_execute(&cmd) {
                        Ok(_) => post_command_undo_boundary(i),
                        Err(flow) => report_flow(i, flow),
                    }
                }
                if let Some(buffer) = i.current_buffer_ref() {
                    let point = buffer.borrow().point();
                    st.search_origin = point;
                    st.positions = vec![point];
                    st.spans = vec![(point, point)];
                    st.match_groups.clear();
                }
                st.failing = false;
                st.fail_pos = None;
                st.refresh_overlays(i);
            }
            IsearchAction::ReDispatch(other) => {
                if !st.failing {
                    st.update_search_ring(i);
                }
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
            IsearchAction::QueryReplace(regexp) => {
                st.update_search_ring(i);
                st.clear(i);
                let command = Value::Sym(i.intern(if regexp {
                    "isearch-query-replace-regexp"
                } else {
                    "isearch-query-replace"
                }));
                if let Err(flow) = st.with_lisp_state(i, |i| i.apply(&command, Vec::new())) {
                    report_flow(i, flow);
                }
                let mode_symbol = i.intern("isearch-mode");
                if !i.symbol_value(mode_symbol).is_nil() {
                    let _ = i.set_symbol(mode_symbol, Value::Nil);
                    st.run_end_hook(i, false);
                }
                return Ok(());
            }
            IsearchAction::Occur(pattern) => {
                if !st.failing {
                    st.update_search_ring(i);
                }
                st.clear(i);
                let mode_symbol = i.intern("isearch-mode");
                let _ = i.set_symbol(mode_symbol, Value::Nil);
                st.run_end_hook(i, false);

                let depth = i.specbind_depth();
                let case_fold_search = i.intern("case-fold-search");
                let search_upper_case = i.intern("search-upper-case");
                let occur = Value::Sym(i.intern("occur"));
                let result = i
                    .specbind(case_fold_search, st.case_fold.clone())
                    .and_then(|()| i.specbind(search_upper_case, Value::Nil))
                    .and_then(|()| {
                        i.apply(&occur, vec![Value::string(pattern), Value::Nil])
                            .map(|_| ())
                    });
                if let Err(flow) = match (result, i.unbind_to(depth)) {
                    (Ok(()), Ok(())) => Ok(()),
                    (Err(flow), _) | (Ok(()), Err(flow)) => Err(flow),
                } {
                    report_flow(i, flow);
                }
                return Ok(());
            }
            IsearchAction::Highlight(pattern, lines) => {
                if !st.failing {
                    st.update_search_ring(i);
                }
                st.clear(i);
                let mode_symbol = i.intern("isearch-mode");
                let _ = i.set_symbol(mode_symbol, Value::Nil);
                st.run_end_hook(i, false);
                run_isearch_highlight(i, pattern, st.case_fold.clone(), lines);
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
