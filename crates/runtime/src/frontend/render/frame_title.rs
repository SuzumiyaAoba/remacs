//! Native frame title formatting and redisplay preparation.
use super::*;
use crate::lisp::Flow;

const TITLE_CACHE: &str = "remacs--prepared-frame-title";

enum TitleInterpreter<'a> {
    ReadOnly(&'a Interp),
    Evaluating {
        interp: &'a mut Interp,
        remote_seen: &'a mut HashSet<usize>,
    },
}

struct FrameTitleSource<'a> {
    interpreter: TitleInterpreter<'a>,
    evaluated: bool,
    position_settled: bool,
    display_point: Option<(usize, usize)>,
    error: Option<Flow>,
}

impl<'a> FrameTitleSource<'a> {
    fn read(&self) -> &Interp {
        match &self.interpreter {
            TitleInterpreter::ReadOnly(i) => i,
            TitleInterpreter::Evaluating { interp, .. } => interp,
        }
    }

    fn is_evaluating(&self) -> bool {
        matches!(self.interpreter, TitleInterpreter::Evaluating { .. })
    }

    fn variable(&self, symbol: crate::lisp::SymId, buffer: usize) -> Value {
        if self.is_evaluating() {
            self.read().symbol_value(symbol)
        } else {
            self.read().symbol_value_in_buffer(symbol, buffer)
        }
    }

    fn evaluate(&mut self, context: &mut FrameTitleContext<'_>, form: &Value) -> Value {
        self.evaluated = true;
        let TitleInterpreter::Evaluating { interp, .. } = &mut self.interpreter else {
            return Value::Nil;
        };
        if interp
            .q_soft("inhibit-eval-during-redisplay")
            .is_some_and(|id| {
                let value = interp.symbol_value(id);
                value.truthy() && !matches!(value, Value::Sym(crate::lisp::obarray::sym::UNBOUND))
            })
        {
            return Value::Nil;
        }
        let result = super::super::mode_line::evaluate(interp, form);
        match result {
            Ok(value) => {
                if context.frame.borrow().dead {
                    self.error = Some(interp.error(":eval deleted the frame being displayed"));
                    return Value::Nil;
                }
                context.refresh(interp);
                self.position_settled = false;
                value
            }
            Err(flow) => {
                self.error = Some(flow);
                Value::Nil
            }
        }
    }

    fn before_specifier(&mut self, context: &mut FrameTitleContext<'_>, specifier: char) {
        let TitleInterpreter::Evaluating {
            interp,
            remote_seen,
        } = &mut self.interpreter
        else {
            return;
        };
        if matches!(specifier, 'p' | 'P' | 'o' | 'q') && !self.position_settled {
            let callback_point = interp.buffers.get(context.buffer_id).map(|buffer| {
                let point = buffer.borrow().point();
                if let Some((id, display_point)) = self.display_point
                    && id == context.buffer_id
                {
                    buffer.borrow_mut().set_point(display_point);
                }
                point
            });
            let (width, height) = {
                let frame = context.frame.borrow();
                (frame.width, frame.height)
            };
            let _ =
                render_frame_grid_with_input_cached(interp, &context.frame, width, height, None);
            if let Some(point) = callback_point
                && let Some(buffer) = interp.buffers.get(context.buffer_id)
            {
                buffer.borrow_mut().set_point(point);
            }
            context.refresh(interp);
            self.position_settled = true;
        }
        if specifier == '@' {
            let directory = interp
                .q_soft("default-directory")
                .map(|symbol| interp.symbol_value_in_buffer(symbol, context.buffer_id))
                .unwrap_or(Value::Nil);
            if !remote_seen.contains(&context.buffer_id)
                || buffer_directory_remote_cached(interp, context.buffer_id, &directory).is_none()
            {
                if let Err(flow) = prepare_mode_line_remote_buffer(interp, context.buffer_id) {
                    self.error = Some(flow);
                    return;
                }
                remote_seen.insert(context.buffer_id);
            }
            context.refresh(interp);
        }
    }
}

#[derive(Default)]
pub(super) struct FrameTitleDependencies {
    pub(super) remote: std::cell::Cell<bool>,
    pub(super) position: std::cell::Cell<bool>,
}

struct FrameTitleContext<'a> {
    frame: crate::editor::FrameRef,
    window: crate::editor::WindowRef,
    graphical: bool,
    risky: bool,
    buffer_id: usize,
    buffer_name: String,
    file_name: Option<String>,
    frame_name: String,
    mode_name: String,
    modified: bool,
    read_only: bool,
    narrowed: bool,
    memory_full: bool,
    multiple_frames: bool,
    recursion_depth: usize,
    process_status: &'static str,
    coding: String,
    coding_with_eol: String,
    remote: bool,
    dependencies: Option<&'a FrameTitleDependencies>,
    global_mode_string: Value,
    buffer_size: usize,
    numeric_field_limit: usize,
    window_start_offset: usize,
    window_end_offset: usize,
}

impl FrameTitleContext<'_> {
    fn refresh(&mut self, i: &Interp) {
        if let Some(mut next) = frame_title_context(
            i,
            &self.frame,
            &self.window,
            self.graphical,
            self.dependencies,
        ) {
            next.risky = self.risky;
            *self = next;
        }
    }
}

fn format_frame_title_string(
    source: &mut FrameTitleSource<'_>,
    format: &str,
    context: &mut FrameTitleContext<'_>,
    depth: usize,
) -> String {
    format_frame_title_string_field(source, format, context, FrameTitleField::default(), depth)
}

fn frame_title_specifier(context: &mut FrameTitleContext<'_>, specifier: char) -> Option<String> {
    let mut title = String::new();
    match specifier {
        '%' => title.push('%'),
        // GNU ignores line and column constructs in native frame titles.
        'c' | 'C' | 'l' => {}
        'b' => title.push_str(&context.buffer_name),
        'f' => title.push_str(context.file_name.as_deref().unwrap_or_default()),
        'F' => title.push_str(&context.frame_name),
        'm' => title.push_str(&context.mode_name),
        'n' => title.push_str(narrowing_indicator(context.narrowed)),
        's' => title.push_str(context.process_status),
        'z' => title.push_str(&context.coding),
        'Z' => title.push_str(&context.coding_with_eol),
        'i' => title.push_str(&context.buffer_size.to_string()),
        'I' => title.push_str(&readable_mode_line_size(context.buffer_size)),
        'p' | 'P' | 'o' | 'q' => {
            if let Some(dependencies) = context.dependencies {
                dependencies.position.set(true);
            }
            title.push_str(&buffer_position_indicator(
                context.buffer_size,
                context.window_start_offset,
                context.window_end_offset,
                specifier,
            ));
        }
        '@' => {
            if let Some(dependencies) = context.dependencies {
                dependencies.remote.set(true);
            }
            title.push(if context.remote { '@' } else { '-' });
        }
        'e' => {
            if context.memory_full {
                title.push_str(MEMORY_FULL_WARNING);
            }
        }
        '[' | ']' => {
            // GNU decode_mode_spec uses command_loop_level, excluding
            // minibuffer depth, and abbreviates levels above five.
            if context.recursion_depth > 5 {
                title.push_str(if specifier == '[' {
                    "[[[... "
                } else {
                    " ...]]]"
                });
            } else {
                title.extend(std::iter::repeat_n(specifier, context.recursion_depth));
            }
        }
        '*' | '+' | '&' => title.push(buffer_state_marker(
            context.read_only,
            context.modified,
            specifier,
        )),
        '-' => title.push('-'),
        _ => return None,
    }
    Some(title)
}

fn frame_title_percent(tail: &str) -> (Option<char>, usize, usize, bool) {
    let mut offset = 1;
    let mut width = 0i32;
    while let Some(digit) = tail
        .as_bytes()
        .get(offset)
        .filter(|digit| digit.is_ascii_digit())
    {
        // GNU stores the decimal modifier in a C int.
        width = width
            .wrapping_mul(10)
            .wrapping_add(i32::from(*digit - b'0'));
        offset += 1;
    }
    let has_width = offset > 1;
    let specifier = tail[offset..].chars().next().filter(|ch| *ch != '\0');
    let next = offset + specifier.map_or(0, char::len_utf8);
    (specifier, width.max(0) as usize, next, has_width)
}

#[derive(Clone, Copy, Default)]
struct FrameTitleField {
    minimum: usize,
    maximum: Option<usize>,
}

fn frame_title_prefix<'a>(
    i: &Interp,
    context: &FrameTitleContext<'_>,
    text: &'a str,
    maximum: Option<usize>,
) -> (&'a str, usize) {
    let mut columns = 0usize;
    for (byte, ch) in text.char_indices() {
        let width =
            crate::lisp::builtins::strfn::gnu_char_width_in_buffer(i, ch.into(), context.buffer_id);
        if ch == '\0' || maximum.is_some_and(|limit| width > limit.saturating_sub(columns)) {
            return (&text[..byte], columns);
        }
        columns = columns.saturating_add(width);
    }
    (text, columns)
}

fn format_frame_title_string_field(
    source: &mut FrameTitleSource<'_>,
    format: &str,
    context: &mut FrameTitleContext<'_>,
    field: FrameTitleField,
    depth: usize,
) -> String {
    let mut output = String::new();
    let mut columns = 0usize;
    let mut offset = 0;
    while source.error.is_none()
        && offset < format.len()
        && field.maximum.is_none_or(|limit| columns < limit)
    {
        let tail = &format[offset..];
        let literal_end = tail.find(['%', '\0']).unwrap_or(tail.len());
        let remaining = field.maximum.map(|limit| limit.saturating_sub(columns));
        let (piece, width, next) = if literal_end > 0 {
            let (_, width) =
                frame_title_prefix(source.read(), context, &tail[..literal_end], remaining);
            // A measured zero precision is unbounded in GNU's string sink.
            let (piece, width) =
                frame_title_prefix(source.read(), context, tail, (width > 0).then_some(width));
            (piece.to_owned(), width, offset + literal_end)
        } else if tail.starts_with('\0') {
            break;
        } else {
            let (Some(specifier), mut minimum, consumed, has_width) = frame_title_percent(tail)
            else {
                break;
            };
            source.before_specifier(context, specifier);
            let next = offset + consumed;
            let outer_padding = field.minimum.saturating_sub(columns);
            if outer_padding > 0 {
                minimum = minimum.min(outer_padding);
            }
            if specifier == 'M' {
                let piece = format_frame_title_field(
                    source,
                    &context.global_mode_string.clone(),
                    context,
                    depth + 1,
                    Some(FrameTitleField {
                        minimum,
                        maximum: remaining,
                    }),
                );
                let width = frame_title_prefix(source.read(), context, &piece, None).1;
                (piece, width, next)
            } else {
                let mut decoded = frame_title_specifier(context, specifier).unwrap_or_else(|| {
                    if has_width {
                        String::new()
                    } else {
                        format!("%{specifier}")
                    }
                });
                if matches!(specifier, 'i' | 'I') {
                    // Numeric decoders justify the number before the common
                    // sink pads the whole field. %I's unit is outside that width.
                    let digits = decoded.len().saturating_sub(usize::from(
                        specifier == 'I' && context.buffer_size >= 1000,
                    ));
                    let padding = minimum
                        .min(context.numeric_field_limit)
                        .saturating_sub(digits);
                    decoded = " ".repeat(padding) + &decoded;
                }
                let (prefix, width) =
                    frame_title_prefix(source.read(), context, &decoded, remaining);
                let padding = minimum.saturating_sub(width);
                let mut piece = prefix.to_owned();
                piece.extend(std::iter::repeat_n(' ', padding));
                // GNU pads even when the specified minimum exceeds precision.
                (piece, width.saturating_add(padding), next)
            }
        };
        output.push_str(&piece);
        columns = columns.saturating_add(width);
        offset = next;
    }
    output
}

fn format_frame_title_value(
    source: &mut FrameTitleSource<'_>,
    value: &Value,
    context: &mut FrameTitleContext<'_>,
    depth: usize,
) -> String {
    format_frame_title_field(source, value, context, depth, None)
}

fn format_frame_title_field(
    source: &mut FrameTitleSource<'_>,
    value: &Value,
    context: &mut FrameTitleContext<'_>,
    depth: usize,
    field: Option<FrameTitleField>,
) -> String {
    if source.error.is_some() {
        return String::new();
    }
    // GNU display_mode_element substitutes this element after 100 levels.
    let too_deep = (depth > 100).then(|| Value::string("*too-deep*"));
    let value = too_deep.as_ref().unwrap_or(value);
    let mut output = match value {
        Value::Nil => String::new(),
        Value::Str(string) => {
            let text = string.borrow().to_unicode_lossy();
            if let Some(field) = field {
                format_frame_title_string_field(source, &text, context, field, depth)
            } else {
                format_frame_title_string(source, &text, context, depth)
            }
        }
        Value::Sym(symbol) => {
            let symbol_value = source.variable(*symbol, context.buffer_id);
            let risky = context.risky;
            context.risky |= !source
                .read()
                .q_soft("risky-local-variable")
                .is_some_and(|property| source.read().get_prop(*symbol, property).truthy());
            let text = if matches!(&symbol_value, Value::Sym(other) if other == symbol
                || *other == crate::lisp::obarray::sym::UNBOUND)
            {
                String::new()
            } else if let Value::Str(text) = symbol_value {
                // A string-valued variable is literal, unlike a string
                // appearing directly in global-mode-string or a format list.
                let text = text.borrow().to_unicode_lossy();
                frame_title_prefix(
                    source.read(),
                    context,
                    &text,
                    field.and_then(|field| field.maximum),
                )
                .0
                .to_owned()
            } else {
                format_frame_title_field(source, &symbol_value, context, depth + 1, field)
            };
            context.risky = risky;
            text
        }
        Value::Cons(cell) => {
            let (head, rest) = {
                let cell = cell.borrow();
                (cell.car.clone(), cell.cdr.clone())
            };
            if let Value::Sym(symbol) = head {
                match source.read().symbol_name(symbol).as_str() {
                    ":eval" => {
                        if context.risky {
                            return String::new();
                        }
                        let Value::Cons(rest) = rest else {
                            return String::new();
                        };
                        let form = rest.borrow().car.clone();
                        let value = source.evaluate(context, &form);
                        return format_frame_title_field(source, &value, context, depth + 1, field);
                    }
                    ":propertize" => {
                        if context.risky {
                            return String::new();
                        }
                        let Value::Cons(rest) = rest else {
                            return String::new();
                        };
                        let value = rest.borrow().car.clone();
                        return format_frame_title_field(source, &value, context, depth + 1, field);
                    }
                    _ => {}
                }
            }
            if let Value::Int(width) = head {
                // GNU display_mode_element stores a fixnum width in a C int.
                let width = width as i32;
                let mut field = field.unwrap_or_default();
                if width < 0 {
                    let maximum = width.unsigned_abs() as usize;
                    field.maximum = Some(field.maximum.map_or(maximum, |old| old.min(maximum)));
                } else if width > 0 {
                    let minimum = width as usize;
                    field.minimum = field
                        .minimum
                        .max(field.maximum.map_or(minimum, |max| max.min(minimum)));
                }
                return format_frame_title_field(source, &rest, context, depth + 1, Some(field));
            }
            let Ok(elements) = value.list_to_vec() else {
                return String::new();
            };
            let condition = match &head {
                Value::Nil => Some(false),
                Value::Sym(symbol)
                    if !matches!(
                        source.read().symbol_name(*symbol).as_str(),
                        ":eval" | ":propertize"
                    ) =>
                {
                    let target = source.read().var_alias_target(*symbol);
                    Some(
                        if !source.is_evaluating()
                            && Some(target) == source.read().q_soft("multiple-frames")
                        {
                            context.multiple_frames
                        } else {
                            let value = source.variable(*symbol, context.buffer_id);
                            value.truthy()
                                && !matches!(value, Value::Sym(id) if id == crate::lisp::obarray::sym::UNBOUND)
                        },
                    )
                }
                _ => None,
            };
            if let Some(condition) = condition {
                let branch = elements
                    .get(if condition { 1 } else { 2 })
                    .cloned()
                    .unwrap_or(Value::Nil);
                return format_frame_title_field(source, &branch, context, depth + 1, field);
            }
            let mut text = String::new();
            let mut columns = 0usize;
            for (index, element) in elements.iter().enumerate() {
                if source.error.is_some()
                    || field
                        .and_then(|f| f.maximum)
                        .is_some_and(|maximum| columns >= maximum)
                {
                    break;
                }
                let child_field = field.map(|f| FrameTitleField {
                    minimum: if index + 1 == elements.len() {
                        f.minimum.saturating_sub(columns)
                    } else {
                        0
                    },
                    maximum: f.maximum.map(|max| max.saturating_sub(columns)),
                });
                let piece =
                    format_frame_title_field(source, element, context, depth + 1, child_field);
                if field.is_some() {
                    columns = columns
                        .saturating_add(frame_title_prefix(source.read(), context, &piece, None).1);
                }
                text.push_str(&piece);
            }
            text
        }
        _ => frame_title_prefix(
            source.read(),
            context,
            "*invalid*",
            field.and_then(|field| field.maximum),
        )
        .0
        .to_owned(),
    };
    if let Some(field) = field {
        let columns = frame_title_prefix(source.read(), context, &output, None).1;
        output.extend(std::iter::repeat_n(
            ' ',
            field.minimum.saturating_sub(columns),
        ));
    }
    output
}

fn frame_title_multiple_frames(i: &Interp, frame: &crate::editor::Frame) -> bool {
    // Frames currently share one runtime terminal. Match GNU's other-frame
    // test, including iconified frames and excluding child/tooltip/mini-only.
    i.frames.iter().any(|candidate| {
        let other = candidate.borrow();
        if other.dead || other.id == frame.id {
            return false;
        }
        let parameter = |name: &str| {
            i.q_soft(name)
                .map(|id| crate::lisp::eval::plist_get(&other.params, id))
                .unwrap_or(Value::Nil)
        };
        let mini_only =
            matches!(parameter("minibuffer"), Value::Sym(id) if Some(id) == i.q_soft("only"));
        !mini_only
            && parameter("parent-frame").is_nil()
            && parameter("tooltip").is_nil()
            && crate::editor::frame_visibility(i, candidate)
                != crate::frontend::FrameVisibility::Hidden
    })
}

fn frame_title_context<'a>(
    i: &Interp,
    frame: &crate::editor::FrameRef,
    window: &crate::editor::WindowRef,
    graphical: bool,
    dependencies: Option<&'a FrameTitleDependencies>,
) -> Option<FrameTitleContext<'a>> {
    let frame_state = frame.borrow();
    let buffer_id = window.borrow().buffer;
    let Some(buffer) = i.buffers.get(buffer_id) else {
        return None;
    };
    let (buffer_name, file_name, modified, narrowed, buffer_size, window_start, window_end) = {
        let buffer = buffer.borrow();
        let window = window.borrow();
        let end = window
            .redisplay_end
            .filter(|(id, tick, start, _)| {
                *id == buffer_id && *tick == buffer.text.mod_tick.get() && *start == window.start
            })
            .map(|(_, _, _, end)| end)
            .unwrap_or(buffer.zv);
        (
            buffer.name.to_unicode_lossy(),
            buffer
                .file_name
                .as_ref()
                .map(|name| name.to_unicode_lossy()),
            buffer.is_modified(),
            buffer.begv > 0 || buffer.zv < buffer.size(),
            buffer.zv.saturating_sub(buffer.begv),
            window.start.clamp(buffer.begv, buffer.zv) - buffer.begv,
            end.clamp(buffer.begv, buffer.zv) - buffer.begv,
        )
    };
    let frame_name = frame_state.name.borrow().to_unicode_lossy();
    let multibyte = i
        .q_soft("enable-multibyte-characters")
        .is_none_or(|symbol| i.symbol_value_in_buffer(symbol, buffer_id).truthy());
    let coding = i
        .q_soft("buffer-file-coding-system")
        .map(|symbol| i.symbol_value_in_buffer(symbol, buffer_id))
        .unwrap_or(Value::Nil);
    let decode_coding = |include_eol| {
        crate::lisp::builtins::misc::coding_mode_line(i, &coding, buffer_id, multibyte, include_eol)
    };
    let terminal_prefix = if graphical {
        String::new()
    } else {
        [&i.keyboard_coding, &i.terminal_coding]
            .into_iter()
            .map(|coding| {
                crate::lisp::builtins::misc::coding_mode_line(
                    i, coding, buffer_id, multibyte, false,
                )
            })
            .collect::<String>()
    };
    Some(FrameTitleContext {
        frame: frame.clone(),
        window: window.clone(),
        graphical,
        risky: false,
        buffer_id,
        buffer_name,
        file_name,
        frame_name,
        mode_name: i
            .q_soft("mode-name")
            .and_then(|symbol| match i.symbol_value_in_buffer(symbol, buffer_id) {
                Value::Str(name) => Some(name.borrow().to_unicode_lossy()),
                _ => None,
            })
            .unwrap_or_default(),
        modified,
        read_only: buffer_read_only(i, buffer_id),
        narrowed,
        memory_full: mode_line_variable(i, buffer_id, "memory-full")
            .is_some_and(|value| value.truthy()),
        multiple_frames: frame_title_multiple_frames(i, &frame_state),
        recursion_depth: i.recursion_depth,
        process_status: crate::lisp::process::buffer_process(i, buffer_id)
            .map_or("no process", |process| process.borrow().status),
        coding: terminal_prefix.clone() + &decode_coding(false),
        coding_with_eol: terminal_prefix + &decode_coding(true),
        remote: i.q_soft("default-directory").is_some_and(|symbol| {
            buffer_directory_remote(i, buffer_id, &i.symbol_value_in_buffer(symbol, buffer_id))
        }),
        dependencies,
        global_mode_string: i
            .q_soft("global-mode-string")
            .map(|symbol| i.symbol_value_in_buffer(symbol, buffer_id))
            .unwrap_or(Value::Nil),
        buffer_size,
        numeric_field_limit: frame_state.width.saturating_mul(4),
        window_start_offset: window_start,
        window_end_offset: window_end,
    })
}

struct RootTitle {
    format: Value,
    literal: Option<String>,
    explicit: bool,
    iconified: bool,
}

fn root_title(
    i: &Interp,
    frame: &crate::editor::FrameRef,
    buffer: usize,
    graphical: bool,
) -> RootTitle {
    let iconified = graphical
        && crate::editor::frame_visibility(i, frame) == crate::frontend::FrameVisibility::Iconified;
    if let Some(parameter) = i.q_soft("title")
        && let value @ Value::Str(_) =
            crate::lisp::eval::plist_get(&frame.borrow().params, parameter)
    {
        let Value::Str(text) = &value else {
            unreachable!()
        };
        let literal = Some(text.borrow().to_unicode_lossy());
        return RootTitle {
            format: value,
            literal,
            explicit: true,
            iconified,
        };
    }
    let variable = |name| {
        i.q_soft(name)
            .map(|symbol| i.symbol_value_in_buffer(symbol, buffer))
            .unwrap_or(Value::Nil)
    };
    let normal = variable("frame-title-format");
    let icon = variable("icon-title-format");
    let uses_icon = iconified && !matches!(icon, Value::Sym(crate::lisp::obarray::sym::T));
    let format = if uses_icon { icon } else { normal };
    let literal = (!uses_icon
        && matches!(
            format,
            Value::Nil | Value::Sym(crate::lisp::obarray::sym::T)
        ))
    .then(|| {
        i.buffers
            .get(buffer)
            .map(|b| b.borrow().name.to_unicode_lossy())
            .unwrap_or_default()
    });
    RootTitle {
        format,
        literal,
        explicit: false,
        iconified,
    }
}

fn cached_title(
    i: &Interp,
    frame: &crate::editor::FrameRef,
    context: &FrameTitleContext<'_>,
    root: &RootTitle,
) -> Option<String> {
    let cache = i.q_soft(TITLE_CACHE)?;
    let value = crate::lisp::eval::plist_get(&frame.borrow().params, cache);
    let values = value.list_to_vec().ok()?;
    if values.len() != 5
        || values[0].int() != Some(context.window.borrow().id as i128)
        || values[1].int() != Some(context.buffer_id as i128)
        || !crate::lisp::eq_values(&values[2], &root.format)
        || values[3].truthy() != root.iconified
    {
        return None;
    }
    let Value::Str(text) = &values[4] else {
        return None;
    };
    Some(text.borrow().to_unicode_lossy())
}

pub(super) fn frame_title(
    i: &Interp,
    frame: &crate::editor::FrameRef,
    graphical: bool,
    dependencies: Option<&FrameTitleDependencies>,
) -> String {
    let window = frame.borrow().selected.clone();
    let Some(mut context) = frame_title_context(i, frame, &window, graphical, dependencies) else {
        return frame.borrow().name.borrow().to_unicode_lossy();
    };
    let root = root_title(i, frame, context.buffer_id, graphical);
    if !root.explicit
        && let Some(title) = cached_title(i, frame, &context, &root)
    {
        return title;
    }
    if let Some(literal) = root.literal {
        return literal;
    }
    let mut source = FrameTitleSource {
        interpreter: TitleInterpreter::ReadOnly(i),
        evaluated: false,
        position_settled: false,
        display_point: None,
        error: None,
    };
    format_frame_title_value(&mut source, &root.format, &mut context, 0)
}

pub(super) fn prepare(
    i: &mut Interp,
    windows: &[crate::editor::WindowRef],
    remote_seen: &mut HashSet<usize>,
) -> Result<(), Flow> {
    let ids: HashSet<_> = windows
        .iter()
        .filter(|w| !w.borrow().dead)
        .map(|w| w.borrow().id)
        .collect();
    let frames = i.frames.clone();
    let cache = i.intern(TITLE_CACHE);
    for frame in frames {
        let (window, dead) = {
            let f = frame.borrow();
            (f.selected.clone(), f.dead)
        };
        if dead || !ids.contains(&window.borrow().id) || !i.buffer_live(window.borrow().buffer) {
            continue;
        }
        let params = frame.borrow().params.clone();
        frame.borrow_mut().params =
            crate::lisp::eval::plist_put_front(&params, &Value::Sym(cache), Value::Nil);
        let graphical = crate::editor::frame_is_graphical(i, &frame);
        let Some(mut context) = frame_title_context(i, &frame, &window, graphical, None) else {
            continue;
        };
        let root = root_title(i, &frame, context.buffer_id, graphical);
        if root.literal.is_some() {
            continue;
        }
        let mark = i.specbind_depth();
        let result = (|| {
            let multiple = i.q("multiple-frames");
            i.specbind(multiple, Value::from_bool(context.multiple_frames))?;
            super::super::mode_line::with_window(i, &window, |i| {
                let display_point = i
                    .buffers
                    .get(context.buffer_id)
                    .map(|buffer| buffer.borrow().point())
                    .unwrap_or(0);
                let mut source = FrameTitleSource {
                    interpreter: TitleInterpreter::Evaluating {
                        interp: i,
                        remote_seen,
                    },
                    evaluated: false,
                    position_settled: false,
                    display_point: Some((context.buffer_id, display_point)),
                    error: None,
                };
                let title = format_frame_title_value(&mut source, &root.format, &mut context, 0);
                if let Some(flow) = source.error {
                    return Err(flow);
                }
                Ok((source.evaluated, title))
            })
        })();
        let unwind = i.unbind_to(mark);
        let (evaluated, title) = result?;
        unwind?;
        if evaluated && !frame.borrow().dead && !window.borrow().dead {
            let final_root = root_title(i, &frame, context.buffer_id, graphical);
            let entry = Value::list(vec![
                Value::Int(window.borrow().id as i128),
                Value::Int(context.buffer_id as i128),
                final_root.format,
                Value::from_bool(final_root.iconified),
                Value::string(title),
            ]);
            let params = frame.borrow().params.clone();
            frame.borrow_mut().params =
                crate::lisp::eval::plist_put_front(&params, &Value::Sym(cache), entry);
        }
    }
    Ok(())
}
