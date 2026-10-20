use std::rc::Rc;

use rustc_hash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};

use super::graph::{Decoder, Encoder, Node, Ref, Result, decode_text, encode_text};
use super::runtime::Runtime;
use crate::buffer::{BufferSet, SharedText};
use crate::lisp::value::*;
use crate::lisp::{Interp, memory};

#[derive(Serialize, Deserialize)]
pub(super) struct Image {
    pub nodes: Vec<Node>,
    source_caches: Vec<Vec<u32>>,
    interned: Vec<Ref>,
    symbol_counters: (u64, u64),
    runtime: Runtime,
    buffers: Vec<Option<BufferData>>,
    texts: Vec<TextData>,
    buffer_order: Vec<usize>,
    buffer_counter: u64,
    undo_symbol: Ref,
    undo_inhibit: bool,
    current_buffer: usize,
    minibuffers: Vec<usize>,
    frames: Vec<Ref>,
    selected_frame: Option<Ref>,
    frame_data: Vec<FrameData>,
    window_data: Vec<WindowData>,
    main_thread: MainThreadData,
}

#[derive(Serialize, Deserialize)]
struct MainThreadData {
    name: Option<Vec<u32>>,
    result: Option<Ref>,
    last_error: Option<Ref>,
    pending_signal: Option<(Ref, Ref)>,
}

#[derive(Serialize, Deserialize)]
struct TextData {
    text: Vec<u32>,
    gap: (usize, usize, u64, bool),
    intervals: Vec<(usize, usize, Vec<Ref>)>,
    ticks: [u64; 3],
    had_intervals: bool,
}

#[derive(Serialize, Deserialize)]
struct BufferData {
    id: usize,
    name: Vec<u32>,
    text: usize,
    point: usize,
    last_window_start: usize,
    last_selected_window: Option<Ref>,
    mark: Option<Ref>,
    begv: usize,
    zv: usize,
    locals: Vec<(Ref, Ref)>,
    overlays: Vec<OverlayData>,
    file_name: Option<Vec<u32>>,
    undo_pt_before: Option<usize>,
    live: bool,
    base_buffer: Option<usize>,
    syntax_table: Option<Ref>,
    case_table: Option<Ref>,
    category_table: Option<Ref>,
    narrow_labels: Vec<(usize, usize, Ref)>,
    file_modtime_ns: String,
    file_modtime_size: String,
    create_lockfiles: bool,
    suspended_mode: Option<Ref>,
    last_name: Option<Vec<u32>>,
    inhibit_buffer_hooks: bool,
}

#[derive(Serialize, Deserialize)]
struct OverlayData {
    start: usize,
    end: usize,
    plist: Ref,
    front_advance: bool,
    rear_advance: bool,
    evaporate: bool,
    handle: Ref,
}

#[derive(Serialize, Deserialize)]
struct FrameData {
    id: usize,
    name: Ref,
    windows: Vec<Ref>,
    selected: Ref,
    minibuffer: Option<Ref>,
    root: Option<Ref>,
    menu_shift: bool,
    text_margin: usize,
    width: usize,
    height: usize,
    old_selected: Option<Ref>,
    buffer_list: Vec<usize>,
    buried_buffer_list: Vec<usize>,
    params: Ref,
    dead: bool,
}

#[derive(Serialize, Deserialize)]
struct WindowData {
    id: usize,
    buffer: usize,
    point: usize,
    start: usize,
    hscroll: usize,
    top: usize,
    height: usize,
    left: usize,
    width: usize,
    dedicated: Ref,
    minibuffer: bool,
    params: Ref,
    margins: (usize, usize),
    suspend_hscroll: bool,
    old_point: usize,
    min_hscroll: usize,
    use_time: u64,
    prev_buffers: Ref,
    next_buffers: Ref,
    horiz: Option<bool>,
    children: Vec<Ref>,
    parent: Option<Ref>,
    combination_limit: bool,
    dead: bool,
    new_total: String,
    new_pixel: String,
    new_normal: Ref,
    old_buffer: Ref,
    cursor_type: Ref,
}

impl Image {
    pub fn relocate_source_cache(&mut self, root: &str) {
        fn replace(text: &mut Vec<u32>, old: &[u32], new: &[u32]) {
            if text.starts_with(old)
                && (text.len() == old.len() || text.get(old.len()) == Some(&('/' as u32)))
            {
                text.splice(..old.len(), new.iter().copied());
            }
        }
        let new = encode_text(root);
        for old in &self.source_caches {
            for node in &mut self.nodes {
                if let Node::String { text, .. } = node {
                    replace(text, old, &new);
                }
            }
            for buffer in self.buffers.iter_mut().flatten() {
                if let Some(file) = &mut buffer.file_name {
                    replace(file, old, &new);
                }
            }
        }
    }
    fn external<'a>(
        &self,
        values: &'a FxHashMap<String, Value>,
        reference: &Ref,
    ) -> Result<&'a Value> {
        let Ref::Object(id) = reference else {
            return Err("Invalid session object reference".into());
        };
        let Some(Node::External(name)) = self.nodes.get(*id as usize) else {
            return Err("Invalid external dump node".into());
        };
        values
            .get(name)
            .ok_or_else(|| format!("Missing session object {name}"))
    }

    pub fn restore(&self, i: &mut Interp) -> Result<()> {
        if self
            .buffers
            .get(self.current_buffer)
            .and_then(Option::as_ref)
            .is_none_or(|b| !b.live)
        {
            return Err("Invalid current buffer in portable dump".into());
        }
        let names: Vec<_> = self
            .buffers
            .iter()
            .map(|b| b.as_ref().map(|b| decode_text(&b.name)))
            .collect();
        i.buffers = BufferSet::prepare_dump_slots(&names);
        i.current_buffer = self.current_buffer;
        let mut externals = FxHashMap::default();
        for (id, buffer) in self.buffers.iter().enumerate() {
            if buffer.is_some() {
                externals.insert(
                    format!("buffer:{id}"),
                    Value::Buffer(i.buffers.get(id).ok_or("Missing prepared buffer")?),
                );
            }
        }
        externals.insert("main-thread".into(), Value::Thread(i.threads[0].clone()));
        for window in &self.window_data {
            let object = crate::editor::Window::new(window.buffer);
            object.borrow_mut().id = window.id;
            if externals
                .insert(format!("window:{}", window.id), Value::Window(object))
                .is_some()
            {
                return Err("Duplicate window ids in portable dump".into());
            }
        }
        for frame in &self.frame_data {
            let Value::Window(selected) = self.external(&externals, &frame.selected)? else {
                return Err("Invalid selected window in dump".into());
            };
            let object = crate::editor::Frame {
                id: frame.id,
                name: Value::string_ref(""),
                windows: Vec::new(),
                selected: selected.clone(),
                minibuffer: None,
                root: None,
                menu_shift: frame.menu_shift,
                text_margin: frame.text_margin,
                width: frame.width,
                height: frame.height,
                old_selected: None,
                buffer_list: frame.buffer_list.clone(),
                buried_buffer_list: frame.buried_buffer_list.clone(),
                params: Value::Nil,
                dead: frame.dead,
            };
            if externals
                .insert(
                    format!("frame:{}", frame.id),
                    Value::Frame(memory::allocate(object, Value::Frame)),
                )
                .is_some()
            {
                return Err("Duplicate frame ids in portable dump".into());
            }
        }
        let mut decoder = Decoder::new(i, &self.nodes, &externals)?;
        let mut texts = Vec::new();
        for text in &self.texts {
            let intervals = text
                .intervals
                .iter()
                .map(|(a, b, p)| Ok((*a, *b, decoder.values(i, p)?)))
                .collect::<Result<_>>()?;
            texts.push(SharedText::from_dump(
                &decode_text(&text.text),
                intervals,
                text.ticks,
                text.had_intervals,
                text.gap,
            )?);
        }
        for (id, data) in self.buffers.iter().enumerate() {
            let Some(data) = data else {
                continue;
            };
            if data.id != id {
                return Err("Inconsistent buffer slot in portable dump".into());
            }
            let object = i.buffers.get(id).ok_or("Missing restored buffer")?;
            let text = texts
                .get(data.text)
                .ok_or("Invalid shared text id")?
                .clone();
            if data.live && (data.begv > data.point || data.point > data.zv || data.zv > text.len())
            {
                return Err("Invalid buffer bounds in portable dump".into());
            }
            let base_buffer = data
                .base_buffer
                .map(|id| {
                    i.buffers
                        .get(id)
                        .ok_or_else(|| "Invalid indirect base buffer".to_owned())
                })
                .transpose()?;
            let locals = data
                .locals
                .iter()
                .map(|(s, v)| Ok((decoder.symbol(s)?, decoder.value(i, v)?)))
                .collect::<Result<_>>()?;
            let overlays = data
                .overlays
                .iter()
                .map(|o| {
                    Ok(crate::buffer::Overlay {
                        start: o.start,
                        end: o.end,
                        buffer: None,
                        plist: decoder.value(i, &o.plist)?,
                        front_advance: o.front_advance,
                        rear_advance: o.rear_advance,
                        evaporate: o.evaporate,
                        handle: decoder.value(i, &o.handle)?,
                    })
                })
                .collect::<Result<_>>()?;
            let last_selected_window = data
                .last_selected_window
                .as_ref()
                .map(|r| match decoder.value(i, r)? {
                    Value::Window(w) => Ok(Rc::downgrade(&w)),
                    _ => Err("Invalid buffer display window".to_owned()),
                })
                .transpose()?;
            let mark = data
                .mark
                .as_ref()
                .map(|r| match decoder.value(i, r)? {
                    Value::Marker(m) => Ok(m),
                    _ => Err("Invalid dumped buffer mark".to_owned()),
                })
                .transpose()?;
            let syntax_table = data
                .syntax_table
                .as_ref()
                .map(|r| decoder.value(i, r))
                .transpose()?;
            let case_table = data
                .case_table
                .as_ref()
                .map(|r| decoder.value(i, r))
                .transpose()?;
            let category_table = data
                .category_table
                .as_ref()
                .map(|r| decoder.value(i, r))
                .transpose()?;
            let narrow_labels = data
                .narrow_labels
                .iter()
                .map(|(a, b, v)| Ok((*a, *b, decoder.value(i, v)?)))
                .collect::<Result<_>>()?;
            let suspended_mode = data
                .suspended_mode
                .as_ref()
                .map(|r| decoder.value(i, r))
                .transpose()?;
            {
                let mut b = object.borrow_mut();
                b.text = text;
                b.point = data.point;
                b.begv = data.begv;
                b.zv = data.zv;
                b.last_window_start = data.last_window_start;
                b.last_selected_window = last_selected_window;
                b.mark = mark;
                b.locals = locals;
                b.overlays = overlays;
                b.file_name = data.file_name.as_ref().map(|s| decode_text(s));
                b.undo_pt_before = data.undo_pt_before;
                b.live = data.live;
                b.base_buffer = base_buffer;
                b.syntax_table = syntax_table;
                b.case_table = case_table;
                b.category_table = category_table;
                b.narrow_labels = narrow_labels;
                b.file_modtime_ns = data
                    .file_modtime_ns
                    .parse()
                    .map_err(|_| "Invalid dumped file timestamp")?;
                b.file_modtime_size = data
                    .file_modtime_size
                    .parse()
                    .map_err(|_| "Invalid dumped file size")?;
                b.file_lock_name = None;
                b.create_lockfiles = data.create_lockfiles;
                b.suspended_mode = suspended_mode;
                b.last_name = data.last_name.as_ref().map(|s| decode_text(s));
                b.inhibit_buffer_hooks = data.inhibit_buffer_hooks;
            }
            object.borrow().text.add_viewer(&object);
        }
        let window = |i: &mut Interp, d: &mut Decoder<'_>, r: &Ref| -> Result<WindowRef> {
            match d.value(i, r)? {
                Value::Window(w) => Ok(w),
                _ => Err("Invalid window reference in dump".into()),
            }
        };
        for data in &self.window_data {
            let Value::Window(object) = externals
                .get(&format!("window:{}", data.id))
                .ok_or("Missing restored window")?
            else {
                return Err("Invalid window object".into());
            };
            let dedicated = decoder.value(i, &data.dedicated)?;
            let params = decoder.value(i, &data.params)?;
            let prev_buffers = decoder.value(i, &data.prev_buffers)?;
            let next_buffers = decoder.value(i, &data.next_buffers)?;
            let children = data
                .children
                .iter()
                .map(|r| window(i, &mut decoder, r))
                .collect::<Result<_>>()?;
            let parent = data
                .parent
                .as_ref()
                .map(|r| Ok::<_, String>(Rc::downgrade(&window(i, &mut decoder, r)?)))
                .transpose()?;
            let new_normal = decoder.value(i, &data.new_normal)?;
            let old_buffer = decoder.value(i, &data.old_buffer)?;
            let cursor_type = decoder.value(i, &data.cursor_type)?;
            let mut w = object.borrow_mut();
            w.buffer = data.buffer;
            w.point = data.point;
            w.start = data.start;
            w.hscroll = data.hscroll;
            w.top = data.top;
            w.height = data.height;
            w.left = data.left;
            w.width = data.width;
            w.dedicated = dedicated;
            w.minibuffer = data.minibuffer;
            w.params = params;
            w.margins = data.margins;
            w.suspend_hscroll = data.suspend_hscroll;
            w.old_point = data.old_point;
            w.min_hscroll = data.min_hscroll;
            w.use_time = data.use_time;
            w.prev_buffers = prev_buffers;
            w.next_buffers = next_buffers;
            w.horiz = data.horiz;
            w.children = children;
            w.parent = parent;
            w.combination_limit = data.combination_limit;
            w.dead = data.dead;
            w.new_total = data
                .new_total
                .parse()
                .map_err(|_| "Invalid dumped window size")?;
            w.new_pixel = data
                .new_pixel
                .parse()
                .map_err(|_| "Invalid dumped pixel size")?;
            w.new_normal = new_normal;
            w.old_buffer = old_buffer;
            w.cursor_type = cursor_type;
        }
        for data in &self.frame_data {
            let Value::Frame(object) = externals
                .get(&format!("frame:{}", data.id))
                .ok_or("Missing restored frame")?
            else {
                return Err("Invalid frame object".into());
            };
            let Value::Str(name) = decoder.value(i, &data.name)? else {
                return Err("Invalid dumped frame name".into());
            };
            let windows = data
                .windows
                .iter()
                .map(|r| window(i, &mut decoder, r))
                .collect::<Result<_>>()?;
            let selected = window(i, &mut decoder, &data.selected)?;
            let minibuffer = data
                .minibuffer
                .as_ref()
                .map(|r| window(i, &mut decoder, r))
                .transpose()?;
            let root = data
                .root
                .as_ref()
                .map(|r| window(i, &mut decoder, r))
                .transpose()?;
            let old_selected = data
                .old_selected
                .as_ref()
                .map(|r| window(i, &mut decoder, r))
                .transpose()?;
            let params = decoder.value(i, &data.params)?;
            let mut f = object.borrow_mut();
            f.name = name;
            f.windows = windows;
            f.selected = selected;
            f.minibuffer = minibuffer;
            f.root = root;
            f.old_selected = old_selected;
            f.params = params;
        }
        self.runtime.restore(i, &mut decoder)?;
        decoder.finish(i)?;
        let interned = self
            .interned
            .iter()
            .map(|r| decoder.symbol(r))
            .collect::<Result<Vec<_>>>()?;
        i.obarray.restore_dump_membership(
            &interned,
            &decoder.symbol_ids(),
            self.symbol_counters,
        )?;
        let undo = decoder.symbol(&self.undo_symbol)?;
        i.buffers.finish_dump_registry(
            self.buffer_order.clone(),
            self.buffer_counter,
            undo,
            self.undo_inhibit,
        )?;
        i.minibuf_list = self.minibuffers.clone();
        i.frames = self
            .frames
            .iter()
            .map(|r| match decoder.value(i, r)? {
                Value::Frame(f) => Ok(f),
                _ => Err("Invalid frame list in dump".to_owned()),
            })
            .collect::<Result<_>>()?;
        i.selected_frame = self
            .selected_frame
            .as_ref()
            .map(|r| match decoder.value(i, r)? {
                Value::Frame(f) => Ok(f),
                _ => Err("Invalid selected frame".to_owned()),
            })
            .transpose()?;
        let result = self
            .main_thread
            .result
            .as_ref()
            .map(|r| decoder.value(i, r))
            .transpose()?;
        let last_error = self
            .main_thread
            .last_error
            .as_ref()
            .map(|r| decoder.value(i, r))
            .transpose()?;
        let pending = self
            .main_thread
            .pending_signal
            .as_ref()
            .map(|(a, b)| Ok::<_, String>((decoder.value(i, a)?, decoder.value(i, b)?)))
            .transpose()?;
        {
            let mut t = i.threads[0].borrow_mut();
            t.name = self.main_thread.name.as_ref().map(|s| decode_text(s));
            t.result = result;
            t.last_error = last_error;
            t.pending_signal = pending;
        }
        // Re-establish weak marker chains after shared text has been installed.
        for (index, node) in self.nodes.iter().enumerate() {
            if let Node::Marker {
                buffer: Some(id), ..
            } = node
            {
                let buffer = i.buffers.get(*id).ok_or("Invalid marker buffer in dump")?;
                let Value::Marker(marker) = decoder.value(i, &Ref::Object(index as u32))? else {
                    return Err("Invalid restored marker".into());
                };
                buffer.borrow_mut().register_marker(&marker);
            }
        }
        crate::editor::reserve_dump_ids(
            self.window_data.iter().map(|w| w.id).max().unwrap_or(0),
            self.frame_data.iter().map(|f| f.id).max().unwrap_or(0),
            self.window_data
                .iter()
                .map(|w| w.use_time)
                .max()
                .unwrap_or(0),
        );
        Ok(())
    }

    pub fn capture(i: &Interp) -> Result<Self> {
        if i.treesit.has_dump_resources() {
            return Err("Cannot dump active tree-sitter objects".into());
        }
        if i.filenotify.has_dump_resources() {
            return Err("Cannot dump active file notification watches".into());
        }
        if !i.dbus.buses.is_empty() || !i.dbus.fds.is_empty() {
            return Err("Cannot dump active D-Bus connections".into());
        }
        if i.processes.iter().any(|p| !p.borrow().dead) {
            return Err("Cannot dump process objects".into());
        }
        if i.minibuf_level != 0 {
            return Err("Cannot dump an active minibuffer read".into());
        }
        let mut encoder = Encoder::new(i);
        let main = i.threads.first().ok_or("Missing main Lisp thread")?;
        encoder.external(&Value::Thread(main.clone()), "main-thread".into())?;
        let mut interned = Vec::new();
        for id in 0..i.obarray.len() as SymId {
            if i.obarray.is_core_symbol(id) || i.obarray.is_initial_symbol(id) {
                let reference = encoder.symbol(id)?;
                if i.obarray.is_initial_symbol(id) {
                    interned.push(reference);
                }
            }
        }
        let mut runtime = Runtime::capture(i, &mut encoder)?;
        let (buffers, buffer_order, buffer_counter, undo_symbol, undo_inhibit) =
            i.buffers.dump_registry();
        let mut texts = Vec::new();
        let mut text_ids = FxHashMap::default();
        let mut buffer_data = Vec::with_capacity(buffers.len());
        for buffer in buffers {
            buffer_data.push(
                buffer
                    .as_ref()
                    .map(|buffer| Self::buffer(buffer, &mut encoder, &mut texts, &mut text_ids))
                    .transpose()?,
            );
        }
        let frames = i
            .frames
            .iter()
            .map(|f| encoder.value(&Value::Frame(f.clone())))
            .collect::<Result<_>>()?;
        let selected_frame = i
            .selected_frame
            .as_ref()
            .map(|f| encoder.value(&Value::Frame(f.clone())))
            .transpose()?;
        let main = main.borrow();
        let main_thread = MainThreadData {
            name: main.name.as_deref().map(encode_text),
            result: main.result.as_ref().map(|v| encoder.value(v)).transpose()?,
            last_error: main
                .last_error
                .as_ref()
                .map(|v| encoder.value(v))
                .transpose()?,
            pending_signal: main
                .pending_signal
                .as_ref()
                .map(|(a, b)| Ok::<_, String>((encoder.value(a)?, encoder.value(b)?)))
                .transpose()?,
        };
        let undo_symbol = encoder.symbol(undo_symbol)?;
        let mut frame_data = Vec::new();
        let mut window_data = Vec::new();
        let mut seen = FxHashSet::default();
        loop {
            runtime.capture_weak(i, &mut encoder)?;
            encoder.drain()?;
            let pending: Vec<_> = encoder
                .external_objects()
                .into_iter()
                .filter(|(name, _)| !seen.contains(name))
                .collect();
            if pending.is_empty() {
                break;
            }
            for (name, value) in pending {
                seen.insert(name);
                match value {
                    Value::Frame(frame) => frame_data.push(Self::frame(&frame, &mut encoder)?),
                    Value::Window(window) => window_data.push(Self::window(&window, &mut encoder)?),
                    Value::Buffer(buffer) => {
                        let id = buffer.borrow().id;
                        if buffer_data.get(id).is_none_or(Option::is_none) {
                            return Err("Buffer outside its session registry".into());
                        }
                    }
                    Value::Thread(thread) if Rc::ptr_eq(&thread, &i.threads[0]) => {}
                    _ => return Err("Cannot dump this external runtime object".into()),
                }
            }
        }
        let nodes = encoder.finish()?;
        let mut source_caches = Vec::new();
        if let Some(cache) = crate::lisp::embedded_cache::directory() {
            source_caches.push(encode_text(&cache.to_string_lossy()));
            if let Ok(real) = std::fs::canonicalize(&cache) {
                let real = encode_text(&real.to_string_lossy());
                if !source_caches.contains(&real) {
                    source_caches.push(real);
                }
            }
        }
        Ok(Self {
            nodes,
            source_caches,
            interned,
            symbol_counters: i.obarray.dump_counters(),
            runtime,
            buffers: buffer_data,
            texts,
            buffer_order,
            buffer_counter,
            undo_symbol,
            undo_inhibit,
            current_buffer: i.current_buffer,
            minibuffers: i.minibuf_list.clone(),
            frames,
            selected_frame,
            frame_data,
            window_data,
            main_thread,
        })
    }

    fn buffer(
        buffer: &BufferRef,
        e: &mut Encoder<'_>,
        texts: &mut Vec<TextData>,
        ids: &mut FxHashMap<usize, usize>,
    ) -> Result<BufferData> {
        let b = buffer.borrow();
        if b.overlays.iter().any(|o| o.buffer.is_some()) {
            return Err("dumping overlays is not yet implemented".into());
        }
        let text = if let Some(id) = ids.get(&b.text.dump_identity()) {
            *id
        } else {
            let id = texts.len();
            ids.insert(b.text.dump_identity(), id);
            let intervals = b
                .text
                .props
                .borrow()
                .intervals()
                .iter()
                .map(|(a, b, p)| Ok((*a, *b, e.values(p)?)))
                .collect::<Result<_>>()?;
            texts.push(TextData {
                text: encode_text(&b.text.text()),
                gap: b.text.dump_gap(),
                intervals,
                ticks: [
                    b.text.mod_tick.get(),
                    b.text.chars_mod_tick.get(),
                    b.text.save_tick.get(),
                ],
                had_intervals: b.text.had_intervals.get(),
            });
            id
        };
        let optional =
            |e: &mut Encoder<'_>, v: &Option<Value>| v.as_ref().map(|v| e.value(v)).transpose();
        Ok(BufferData {
            id: b.id,
            name: encode_text(&b.name),
            text,
            point: b.point,
            last_window_start: b.last_window_start,
            last_selected_window: b
                .last_selected_window
                .as_ref()
                .and_then(|w| w.upgrade())
                .map(|w| e.value(&Value::Window(w)))
                .transpose()?,
            mark: b
                .mark
                .as_ref()
                .map(|m| e.value(&Value::Marker(m.clone())))
                .transpose()?,
            begv: b.begv,
            zv: b.zv,
            locals: b
                .locals
                .iter()
                .map(|(s, v)| Ok((e.symbol(*s)?, e.value(v)?)))
                .collect::<Result<_>>()?,
            overlays: b
                .overlays
                .iter()
                .map(|o| {
                    Ok(OverlayData {
                        start: o.start,
                        end: o.end,
                        plist: e.value(&o.plist)?,
                        front_advance: o.front_advance,
                        rear_advance: o.rear_advance,
                        evaporate: o.evaporate,
                        handle: e.value(&o.handle)?,
                    })
                })
                .collect::<Result<_>>()?,
            file_name: b.file_name.as_deref().map(encode_text),
            undo_pt_before: b.undo_pt_before,
            live: b.live,
            base_buffer: b.base_buffer.as_ref().map(|b| b.borrow().id),
            syntax_table: optional(e, &b.syntax_table)?,
            case_table: optional(e, &b.case_table)?,
            category_table: optional(e, &b.category_table)?,
            narrow_labels: b
                .narrow_labels
                .iter()
                .map(|(a, b, v)| Ok((*a, *b, e.value(v)?)))
                .collect::<Result<_>>()?,
            file_modtime_ns: b.file_modtime_ns.to_string(),
            file_modtime_size: b.file_modtime_size.to_string(),
            create_lockfiles: b.create_lockfiles,
            suspended_mode: optional(e, &b.suspended_mode)?,
            last_name: b.last_name.as_deref().map(encode_text),
            inhibit_buffer_hooks: b.inhibit_buffer_hooks,
        })
    }

    fn frame(frame: &FrameRef, e: &mut Encoder<'_>) -> Result<FrameData> {
        let f = frame.borrow();
        let window = |e: &mut Encoder<'_>, w: &WindowRef| e.value(&Value::Window(w.clone()));
        Ok(FrameData {
            id: f.id,
            name: e.value(&Value::Str(f.name.clone()))?,
            windows: f
                .windows
                .iter()
                .map(|w| window(e, w))
                .collect::<Result<_>>()?,
            selected: window(e, &f.selected)?,
            minibuffer: f.minibuffer.as_ref().map(|w| window(e, w)).transpose()?,
            root: f.root.as_ref().map(|w| window(e, w)).transpose()?,
            menu_shift: f.menu_shift,
            text_margin: f.text_margin,
            width: f.width,
            height: f.height,
            old_selected: f.old_selected.as_ref().map(|w| window(e, w)).transpose()?,
            buffer_list: f.buffer_list.clone(),
            buried_buffer_list: f.buried_buffer_list.clone(),
            params: e.value(&f.params)?,
            dead: f.dead,
        })
    }
    fn window(window: &WindowRef, e: &mut Encoder<'_>) -> Result<WindowData> {
        let w = window.borrow();
        Ok(WindowData {
            id: w.id,
            buffer: w.buffer,
            point: w.point,
            start: w.start,
            hscroll: w.hscroll,
            top: w.top,
            height: w.height,
            left: w.left,
            width: w.width,
            dedicated: e.value(&w.dedicated)?,
            minibuffer: w.minibuffer,
            params: e.value(&w.params)?,
            margins: w.margins,
            suspend_hscroll: w.suspend_hscroll,
            old_point: w.old_point,
            min_hscroll: w.min_hscroll,
            use_time: w.use_time,
            prev_buffers: e.value(&w.prev_buffers)?,
            next_buffers: e.value(&w.next_buffers)?,
            horiz: w.horiz,
            children: w
                .children
                .iter()
                .map(|w| e.value(&Value::Window(w.clone())))
                .collect::<Result<_>>()?,
            parent: w
                .parent
                .as_ref()
                .and_then(|w| w.upgrade())
                .map(|w| e.value(&Value::Window(w)))
                .transpose()?,
            combination_limit: w.combination_limit,
            dead: w.dead,
            new_total: w.new_total.to_string(),
            new_pixel: w.new_pixel.to_string(),
            new_normal: e.value(&w.new_normal)?,
            old_buffer: e.value(&w.old_buffer)?,
            cursor_type: e.value(&w.cursor_type)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complete_session_roundtrip_preserves_program_buffers_and_dispatch_metadata() {
        std::thread::Builder::new().stack_size(128 << 20).spawn(|| {
            let mut source=Interp::new();
            source.eval_str(r#"(progn
              (defvar dump-test-cycle (list 'payload))
              (setcdr dump-test-cycle dump-test-cycle)
              (defvar dump-test-alias dump-test-cycle)
              (defvar dump-test-shared (eval '(let ((count 6))
                (list (lambda () (setq count (1+ count))) (lambda () count))) t))
              (defun dump-test-function (arg) (+ arg 19))
              (defalias 'dump-test-function-alias 'dump-test-function)
              (defvar dump-test-buffer (get-buffer-create "dump-session-buffer"))
              (with-current-buffer dump-test-buffer
                (insert "日本語 abc")
                (put-text-property 1 3 'dump-property dump-test-cycle)
                (narrow-to-region 2 7)
                (goto-char 4)
                (set (make-local-variable 'dump-local) dump-test-cycle))
              (defvar dump-test-indirect (make-indirect-buffer dump-test-buffer "dump-session-indirect"))
              (defvar dump-test-marker (with-current-buffer dump-test-buffer (copy-marker 4)))
              nil)"#).unwrap();
            let image=Image::capture(&source).unwrap();
            let bytes=serde_json::to_vec(&image).unwrap();
            let image:Image=serde_json::from_slice(&bytes).unwrap();
            let mut restored=Interp::new();
            image.restore(&mut restored).unwrap();
            let result=restored.eval_str(r#"(list
              (eq dump-test-cycle dump-test-alias)
              (eq dump-test-cycle (cdr dump-test-cycle))
              (dump-test-function-alias 4)
              (funcall (car dump-test-shared))
              (funcall (cadr dump-test-shared))
              (with-current-buffer dump-test-buffer
                (list (point) (point-min) (point-max) (eq dump-local dump-test-cycle)
                      (eq (get-text-property 2 'dump-property) dump-test-cycle)))
              (eq (buffer-base-buffer dump-test-indirect) dump-test-buffer)
              (eq (marker-buffer dump-test-marker) dump-test-buffer)
              (marker-position dump-test-marker))"#).unwrap();
            assert_eq!(restored.prin1_to_string(&result),"(t t 23 7 7 (4 2 7 t t) t t 4)");
            restored.eval_str("(with-current-buffer dump-test-indirect (widen) (goto-char 1) (insert \"X\"))").unwrap();
            let result=restored.eval_str("(list (with-current-buffer dump-test-buffer (buffer-size)) (marker-position dump-test-marker))").unwrap();
            assert_eq!(restored.prin1_to_string(&result),"(8 5)");
        }).unwrap().join().unwrap();
    }
}
