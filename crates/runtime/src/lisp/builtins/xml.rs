//! `libxml-parse-xml-region' / `libxml-parse-html-region'.
//!
//! GNU parses the region with libxml2 and returns a Lisp DOM:
//!   (TAG ((ATTR-SYM . "value") ...) CHILD ...)  for elements
//!   (comment nil "text")                        for comments
//!   nil                                         for PIs (a GNU quirk)
//! Text and CDATA are separate string children.  Top level: a single
//! node is returned bare; multiple nodes get a synthetic `top' wrapper.
//! XML parse errors yield nil; the HTML parser (html5ever) is lenient
//! like libxml2's HTML mode and implies html/body structure.

#[path = "xml_dtd.rs"]
mod dtd;

use html5ever::tokenizer::{Token, TokenSink, TokenSinkResult};
use html5ever::tree_builder::{ElementFlags, NodeOrText, QuirksMode, Tracer, TreeSink};
use html5ever::{Attribute, ExpandedName, QualName, tendril::StrTendril};
use markup5ever_rcdom::{Handle, NodeData, RcDom};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use super::{S, arg};

use crate::lisp::error::Flow;
use crate::lisp::value::{Subr, Value};
use crate::lisp::{EvalResult, Interp};

/// GNU `parse_region': START/END are coerced via integer-or-marker-p
/// (in argument order, so arg0's error wins), nil defaults to
/// BEGV_pos/ZV_pos.  The coerced *positions* are checked against the
/// accessible range — with the low/high pair swapped, so a reversed
/// but in-range region is legal — and violations signal
/// `args-out-of-range' with (BUFFER START END) where each slot is the
/// arg object, or the coerced integer default when the arg was nil.
/// Returns START..END as a string.
fn region_text(i: &mut Interp, a: &[Value]) -> Result<String, Flow> {
    let b = i
        .current_buffer_ref()
        .ok_or_else(|| i.error("No current buffer"))?;
    let bb = b.borrow();
    // 1-based positions: begv/zv are 0-based char indices.
    let lo_pos = bb.begv as i128 + 1;
    let hi_pos = bb.zv as i128 + 1;
    let coerce = |i: &mut Interp, v: &Value, default: i128| -> Result<i128, Flow> {
        match v {
            Value::Nil => Ok(default),
            Value::Int(n) => Ok(*n),
            Value::Marker(m) => {
                // GNU only reads the marker's position; its buffer is
                // irrelevant (a foreign-buffer marker still indexes
                // into the current buffer).
                let mm = m.borrow();
                if mm.buffer.is_some() {
                    Ok(mm.position as i128 + 1)
                } else {
                    Err(i.error("Marker does not point anywhere"))
                }
            }
            _ => Err(i.wrong_type_mut("integer-or-marker-p", v)),
        }
    };
    let s = coerce(i, &arg(a, 0), lo_pos)?;
    let e = coerce(i, &arg(a, 1), hi_pos)?;
    if s.min(e) < lo_pos || s.max(e) > hi_pos {
        let sym = i.q("args-out-of-range");
        let or_int = |v: Value, n: i128| {
            if matches!(v, Value::Nil) {
                Value::Int(n)
            } else {
                v
            }
        };
        let data = vec![
            Value::Buffer(b.clone()),
            or_int(arg(a, 0), s),
            or_int(arg(a, 1), e),
        ];
        return Err(i.signal_data(sym, data));
    }
    Ok(bb
        .text
        .substring(s.min(e) as usize - 1, s.max(e) as usize - 1))
}

/// GNU `parse_region' also CHECK_STRINGs BASE-URL when non-nil (after
/// the region checks); the value itself is unused.
fn check_base_url(i: &mut Interp, a: &[Value]) -> Result<(), Flow> {
    match arg(a, 2) {
        Value::Nil | Value::Str(_) => Ok(()),
        v => Err(i.wrong_type_mut("stringp", &v)),
    }
}

// ---------- HTML via html5ever + rcdom ----------

/// Track each tbody's own provenance, rather than the presence of a tag
/// anywhere in the source. TreeBuilder creates implicit tbody elements while
/// processing tr/td/th, but explicit ones while processing a tbody start tag.
/// Keep the handles alive so their addresses cannot be reused during recovery.
#[derive(Default)]
struct HtmlDom {
    dom: RcDom,
    tbody_start: Cell<bool>,
    explicit_tbodies: RefCell<HashMap<*const markup5ever_rcdom::Node, Handle>>,
    p_end: Cell<bool>,
    implied_p_end: RefCell<Option<Handle>>,
    table_context: RefCell<HtmlTableContext>,
    saw_html_table: Cell<bool>,
}

/// These HTML elements can only occur in the open-elements portion of
/// TreeBuilder's traced handles, never in its active formatting/head/form
/// handles. Their stack order identifies the current table and row/group.
/// Snapshot before processing a token: pending table text is flushed before
/// that token changes the stack. An implied tbody is unwrapped later as usual.
#[derive(Default)]
struct HtmlTableContext {
    table: RefCell<Option<Handle>>,
    parent: RefCell<Option<Handle>>,
}

impl Tracer for HtmlTableContext {
    type Handle = Handle;

    fn trace_handle(&self, node: &Handle) {
        if let NodeData::Element { name, .. } = &node.data {
            if name.ns.as_ref() == "http://www.w3.org/1999/xhtml" {
                match name.local.as_ref() {
                    "table" => {
                        *self.table.borrow_mut() = Some(node.clone());
                        *self.parent.borrow_mut() = Some(node.clone());
                    }
                    "tbody" | "thead" | "tfoot" | "tr" => {
                        *self.parent.borrow_mut() = Some(node.clone());
                    }
                    _ => {}
                }
            }
        }
    }
}

impl HtmlDom {
    fn suppress_implied_p(&self, child: &NodeOrText<Handle>) -> bool {
        matches!(child, NodeOrText::AppendNode(node)
            if self.implied_p_end.borrow().as_ref()
                .is_some_and(|implied| Rc::ptr_eq(implied, node)))
    }
}

impl TreeSink for HtmlDom {
    type Handle = Handle;
    type Output = Self;
    type ElemName<'a> = ExpandedName<'a>;

    fn finish(self) -> Self {
        self
    }

    fn create_element(&self, name: QualName, attrs: Vec<Attribute>, flags: ElementFlags) -> Handle {
        if name.local.as_ref() == "table" && name.ns.as_ref() == "http://www.w3.org/1999/xhtml" {
            self.saw_html_table.set(true);
        }
        let explicit_tbody = self.tbody_start.get()
            && name.local.as_ref() == "tbody"
            && name.ns.as_ref() == "http://www.w3.org/1999/xhtml";
        let implied_p_end = self.p_end.get()
            && name.local.as_ref() == "p"
            && name.ns.as_ref() == "http://www.w3.org/1999/xhtml";
        let node = self.dom.create_element(name, attrs, flags);
        if explicit_tbody {
            self.explicit_tbodies
                .borrow_mut()
                .insert(Rc::as_ptr(&node), node.clone());
        }
        if implied_p_end {
            *self.implied_p_end.borrow_mut() = Some(node.clone());
        }
        node
    }

    // Delegate tree construction, apart from libxml-compatible table content
    // and ignored paragraph end tags.
    // Template contents, namespace integration and selectedcontent cloning
    // retain their normal html5ever behavior.
    fn parse_error(&self, msg: std::borrow::Cow<'static, str>) {
        self.dom.parse_error(msg);
    }
    fn get_document(&self) -> Handle {
        self.dom.get_document()
    }
    fn elem_name<'a>(&'a self, target: &'a Handle) -> ExpandedName<'a> {
        self.dom.elem_name(target)
    }
    fn create_comment(&self, text: StrTendril) -> Handle {
        self.dom.create_comment(text)
    }
    fn create_pi(&self, target: StrTendril, data: StrTendril) -> Handle {
        self.dom.create_pi(target, data)
    }
    fn append(&self, parent: &Handle, child: NodeOrText<Handle>) {
        // The temporary p is immediately closed by TreeBuilder. Do not attach
        // it: an ignored </p> must not split the surrounding character data.
        if !self.suppress_implied_p(&child) {
            self.dom.append(parent, child);
        }
    }
    fn append_based_on_parent_node(
        &self,
        element: &Handle,
        prev: &Handle,
        child: NodeOrText<Handle>,
    ) {
        if self.suppress_implied_p(&child) {
            return;
        }
        let context = self.table_context.borrow();
        if context
            .table
            .borrow()
            .as_ref()
            .is_some_and(|table| Rc::ptr_eq(table, element))
        {
            if let Some(parent) = context.parent.borrow().as_ref() {
                // libxml2 retains content in its table/row/group. Keep both
                // text and elements here: moving only text would reorder it
                // relative to a foster-parented element such as <b> or <div>.
                self.dom.append(parent, child);
                return;
            }
        }
        self.dom.append_based_on_parent_node(element, prev, child);
    }
    fn append_doctype_to_document(&self, name: StrTendril, public: StrTendril, system: StrTendril) {
        self.dom.append_doctype_to_document(name, public, system);
    }
    fn mark_script_already_started(&self, node: &Handle) {
        self.dom.mark_script_already_started(node);
    }
    fn pop(&self, node: &Handle) {
        self.dom.pop(node);
    }
    fn get_template_contents(&self, target: &Handle) -> Handle {
        self.dom.get_template_contents(target)
    }
    fn same_node(&self, x: &Handle, y: &Handle) -> bool {
        self.dom.same_node(x, y)
    }
    fn set_quirks_mode(&self, mode: QuirksMode) {
        self.dom.set_quirks_mode(mode);
    }
    fn append_before_sibling(&self, sibling: &Handle, node: NodeOrText<Handle>) {
        if !self.suppress_implied_p(&node) {
            self.dom.append_before_sibling(sibling, node);
        }
    }
    fn add_attrs_if_missing(&self, target: &Handle, attrs: Vec<Attribute>) {
        self.dom.add_attrs_if_missing(target, attrs);
    }
    fn associate_with_form(
        &self,
        target: &Handle,
        form: &Handle,
        nodes: (&Handle, Option<&Handle>),
    ) {
        self.dom.associate_with_form(target, form, nodes);
    }
    fn remove_from_parent(&self, target: &Handle) {
        self.dom.remove_from_parent(target);
    }
    fn reparent_children(&self, node: &Handle, new_parent: &Handle) {
        self.dom.reparent_children(node, new_parent);
    }
    fn is_mathml_annotation_xml_integration_point(&self, node: &Handle) -> bool {
        self.dom.is_mathml_annotation_xml_integration_point(node)
    }
    fn set_current_line(&self, line: u64) {
        self.dom.set_current_line(line);
    }
    fn allow_declarative_shadow_roots(&self, parent: &Handle) -> bool {
        self.dom.allow_declarative_shadow_roots(parent)
    }
    fn attach_declarative_shadow(
        &self,
        location: &Handle,
        template: &Handle,
        attrs: &[Attribute],
    ) -> bool {
        self.dom
            .attach_declarative_shadow(location, template, attrs)
    }
    fn maybe_clone_an_option_into_selectedcontent(&self, option: &Handle) {
        self.dom.maybe_clone_an_option_into_selectedcontent(option);
    }
}

/// Supply token provenance to the DOM sink and record an explicit head.
/// html5ever also inserts
/// an empty head for body-only input, which libxml2 does not expose.
/// Observing tokens *and* tree creation avoids mistaking comments, quoted
/// attributes, raw text, or a late/ignored <head> for an explicit head.
struct HtmlTokenTracker {
    tree: html5ever::tree_builder::TreeBuilder<Handle, HtmlDom>,
    explicit_head: RefCell<Option<Handle>>,
    saw_head_start: Cell<bool>,
}

impl HtmlTokenTracker {
    fn head(&self) -> Option<Handle> {
        let document = self.tree.sink.dom.document.children.borrow();
        let html = document.iter().find(|node| {
            matches!(&node.data, NodeData::Element { name, .. }
                if name.local.as_ref() == "html"
                    && name.ns.as_ref() == "http://www.w3.org/1999/xhtml")
        })?;
        html.children
            .borrow()
            .iter()
            .find(|node| {
                matches!(&node.data, NodeData::Element { name, .. }
                if name.local.as_ref() == "head"
                    && name.ns.as_ref() == "http://www.w3.org/1999/xhtml")
            })
            .cloned()
    }
}

impl TokenSink for HtmlTokenTracker {
    type Handle = Handle;

    fn process_token(&self, token: Token, line: u64) -> TokenSinkResult<Handle> {
        // Avoid scanning the open-element stack in documents without tables.
        // Only actual HTML table creation enables tracking, starting with the
        // following token; comments, attributes and foreign names cannot do so.
        if self.tree.sink.saw_html_table.get() {
            let table_context = HtmlTableContext::default();
            self.tree.trace_handles(&table_context);
            *self.tree.sink.table_context.borrow_mut() = table_context;
        }
        let may_create_head = matches!(&token, Token::TagToken(tag)
            if tag.kind == html5ever::tokenizer::StartTag && tag.name.as_ref() == "head")
            // The first head start either creates the head or finds an
            // implied one. Later head tags cannot change its provenance.
            && !self.saw_head_start.replace(true)
            && self.head().is_none();
        self.tree
            .sink
            .tbody_start
            .set(matches!(&token, Token::TagToken(tag)
            if tag.kind == html5ever::tokenizer::StartTag && tag.name.as_ref() == "tbody"));
        self.tree
            .sink
            .p_end
            .set(matches!(&token, Token::TagToken(tag)
            if tag.kind == html5ever::tokenizer::EndTag && tag.name.as_ref() == "p"));
        let result = self.tree.process_token(token, line);
        self.tree.sink.tbody_start.set(false);
        self.tree.sink.p_end.set(false);
        self.tree.sink.implied_p_end.borrow_mut().take();
        if may_create_head {
            *self.explicit_head.borrow_mut() = self.head();
        }
        result
    }

    fn end(&self) {
        self.tree.end();
    }

    fn adjusted_current_node_present_but_not_in_html_namespace(&self) -> bool {
        self.tree
            .adjusted_current_node_present_but_not_in_html_namespace()
    }
}

fn html_node(
    i: &mut Interp,
    h: &markup5ever_rcdom::Handle,
    explicit_tbodies: &HashMap<*const markup5ever_rcdom::Node, Handle>,
    explicit_head: Option<&Handle>,
    out: &mut Vec<Value>,
) {
    use markup5ever_rcdom::NodeData;
    match &h.data {
        NodeData::Element { name, attrs, .. } => {
            let mut kids = Vec::new();
            for c in h.children.borrow().iter() {
                html_node(i, c, explicit_tbodies, explicit_head, &mut kids);
            }
            // Only the implicit empty head is absent from libxml2's DOM.
            if &*name.local == "head"
                && kids.is_empty()
                && !explicit_head.is_some_and(|head| std::rc::Rc::ptr_eq(head, h))
            {
                return;
            }
            // html5ever implies <tbody> like HTML5; libxml2 doesn't.
            // Unwrap only this node's synthesized group. Another explicit
            // tbody, even in this same table, must not affect its provenance.
            if &*name.local == "tbody"
                && name.ns.as_ref() == "http://www.w3.org/1999/xhtml"
                && !explicit_tbodies.contains_key(&Rc::as_ptr(h))
            {
                out.extend(kids);
                return;
            }
            let attr_list: Vec<Value> = attrs
                .borrow()
                .iter()
                // HTML names are ASCII-case-insensitive, including foreign
                // attributes that HTML5 adjusts back to e.g. SVG viewBox.
                .map(|a| attr_cons(i, &a.name.local.as_ref().to_ascii_lowercase(), &a.value))
                .collect();
            let mut node = vec![
                Value::Sym(i.intern(name.local.as_ref())),
                Value::list(attr_list),
            ];
            node.extend(kids);
            out.push(Value::list(node));
        }
        NodeData::Text { contents } => {
            out.push(Value::string(contents.borrow().to_string()));
        }
        NodeData::Comment { contents } => {
            out.push(Value::list(vec![
                Value::Sym(i.q("comment")),
                Value::Nil,
                Value::string(contents.to_string()),
            ]));
        }
        NodeData::ProcessingInstruction { .. } => out.push(Value::Nil),
        _ => {}
    }
}

fn parse_html(i: &mut Interp, text: &str, drop_comments: bool) -> Value {
    let tracker = HtmlTokenTracker {
        tree: html5ever::tree_builder::TreeBuilder::new(HtmlDom::default(), Default::default()),
        explicit_head: RefCell::new(None),
        saw_head_start: Cell::new(false),
    };
    let tokenizer = html5ever::tokenizer::Tokenizer::new(tracker, Default::default());
    let input = html5ever::buffer_queue::BufferQueue::default();
    input.push_back(text.into());
    // As in html5ever's Parser, resume after script/encoding indicators.
    while !matches!(tokenizer.feed(&input), html5ever::TokenizerResult::Done) {}
    tokenizer.end();
    let explicit_head = tokenizer.sink.explicit_head.into_inner();
    let tracked_dom = tokenizer.sink.tree.sink;
    let explicit_tbodies = tracked_dom.explicit_tbodies.into_inner();
    let dom = tracked_dom.dom;
    let mut out = Vec::new();
    for c in dom.document.children.borrow().iter() {
        // GNU's DISCARD-COMMENTS selects the document's root element;
        // make_dom still preserves comments anywhere inside that element.
        if drop_comments && matches!(&c.data, markup5ever_rcdom::NodeData::Comment { .. }) {
            continue;
        }
        html_node(i, c, &explicit_tbodies, explicit_head.as_ref(), &mut out);
    }
    match out.as_slice() {
        [single] => single.clone(),
        _ => {
            let mut top = vec![Value::Sym(i.q("top")), Value::Nil];
            top.extend(out);
            Value::list(top)
        }
    }
}

fn f_libxml_parse_xml_region(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let text = region_text(i, &a)?;
    check_base_url(i, &a)?;
    let drop_comments = arg(&a, 3).truthy();
    Ok(parse_xml(i, &text, drop_comments))
}

fn f_libxml_parse_html_region(i: &mut Interp, a: crate::lisp::Argv) -> EvalResult {
    let text = region_text(i, &a)?;
    check_base_url(i, &a)?;
    // GNU libxml2: an empty (whitespace-only) document yields nil.
    if text.trim().is_empty() {
        return Ok(Value::Nil);
    }
    let drop_comments = arg(&a, 3).truthy();
    Ok(parse_html(i, &text, drop_comments))
}

fn attr_cons(i: &mut Interp, name: &str, val: &str) -> Value {
    Value::Cons(crate::lisp::value::Cons::new(
        Value::Sym(i.intern(name)),
        Value::string(val),
    ))
}

// ---------- XML via quick-xml (event-driven, keeps CDATA separate) ----------

/// Namespace prefixes don't appear in GNU's DOM: `x:r` → `r',
/// `x:a' → `a', and xmlns declarations are dropped.
fn local_name(qname: &[u8]) -> &str {
    let s = std::str::from_utf8(qname).unwrap_or("");
    s.rsplit(':').next().unwrap_or(s)
}

struct XmlFrame {
    name: Vec<u8>,
    attrs: Vec<Value>,
    kids: Vec<Value>,
    /// Last kid came from CDATA: libxml2 keeps CDATA as its own string
    /// node, so plain text must not merge into it.
    tail_cdata: bool,
}

/// Append character data to the current frame.  libxml2 merges adjacent
/// text/entity content into one string node; CDATA stands alone.
fn push_text(f: &mut XmlFrame, s: &str, cdata: bool) {
    if s.is_empty() {
        return;
    }
    if !cdata && !f.tail_cdata {
        if let Some(Value::Str(prev)) = f.kids.last_mut() {
            prev.borrow_mut().push_str(s);
            return;
        }
    }
    f.kids.push(Value::string(s));
    f.tail_cdata = cdata;
}

fn push_node(f: &mut XmlFrame, v: Value) {
    f.kids.push(v);
    f.tail_cdata = false;
}

/// XML 1.0 (Fifth Edition), productions [2], [4] and [4a]. Rust chars
/// already exclude surrogates; XML also excludes most C0 controls and
/// U+FFFE/U+FFFF, but permits other Unicode noncharacters and C1 controls.
fn xml_char(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\r' | '\u{20}'..='\u{d7ff}'
        | '\u{e000}'..='\u{fffd}' | '\u{10000}'..='\u{10ffff}')
}

fn xml_name_start(c: char) -> bool {
    matches!(c, ':' | 'A'..='Z' | '_' | 'a'..='z'
        | '\u{c0}'..='\u{d6}' | '\u{d8}'..='\u{f6}' | '\u{f8}'..='\u{2ff}'
        | '\u{370}'..='\u{37d}' | '\u{37f}'..='\u{1fff}'
        | '\u{200c}'..='\u{200d}' | '\u{2070}'..='\u{218f}'
        | '\u{2c00}'..='\u{2fef}' | '\u{3001}'..='\u{d7ff}'
        | '\u{f900}'..='\u{fdcf}' | '\u{fdf0}'..='\u{fffd}'
        | '\u{10000}'..='\u{effff}')
}

fn xml_name(name: &[u8]) -> bool {
    let Ok(name) = std::str::from_utf8(name) else {
        return false;
    };
    let mut chars = name.chars();
    chars.next().is_some_and(xml_name_start)
        && chars.all(|c| {
            xml_name_start(c)
                || matches!(c, '-' | '.' | '0'..='9' | '\u{b7}'
                    | '\u{300}'..='\u{36f}' | '\u{203f}'..='\u{2040}')
        })
}

/// Resolve `&name;' / `&#NNN;' references.  None = undeclared entity,
/// malformed reference, or forbidden XML character.
fn resolve_ref(name: &str) -> Option<String> {
    match name {
        "amp" => Some("&".into()),
        "lt" => Some("<".into()),
        "gt" => Some(">".into()),
        "quot" => Some("\"".into()),
        "apos" => Some("'".into()),
        _ => {
            let rest = name.strip_prefix('#')?;
            let n = if let Some(h) = rest.strip_prefix('x') {
                if h.is_empty() || !h.bytes().all(|c| c.is_ascii_hexdigit()) {
                    return None;
                }
                u32::from_str_radix(h, 16).ok()?
            } else {
                if rest.is_empty() || !rest.bytes().all(|c| c.is_ascii_digit()) {
                    return None;
                }
                rest.parse().ok()?
            };
            let c = char::from_u32(n)?;
            xml_char(c).then(|| c.to_string())
        }
    }
}

/// Unescape attributes with the same strict reference rules as text.
/// Do this only once: `&amp;#0;' is the literal string "&#0;".
fn xml_unescape(mut text: &str) -> Option<String> {
    let mut out = String::new();
    while let Some(pos) = text.find('&') {
        out.push_str(&text[..pos]);
        text = &text[pos + 1..];
        let end = text.find(';')?;
        out.push_str(&resolve_ref(&text[..end])?);
        text = &text[end + 1..];
    }
    out.push_str(text);
    Some(out)
}

/// quick-xml checks attribute syntax but permits adjacent quoted values
/// without the XML-required separating whitespace (`a='1'b='2'`).
fn xml_attribute_spacing(raw: &[u8]) -> bool {
    let mut quote = None;
    for (pos, &c) in raw.iter().enumerate() {
        if let Some(q) = quote {
            if c == q {
                quote = None;
                if raw
                    .get(pos + 1)
                    .is_some_and(|c| !matches!(c, b' ' | b'\t' | b'\n' | b'\r'))
                {
                    return false;
                }
            }
        } else if matches!(c, b'\'' | b'"') {
            quote = Some(c);
        }
    }
    true
}

fn parse_xml(i: &mut Interp, text: &str, drop_comments: bool) -> Value {
    use quick_xml::events::Event;
    // Validate the entire region, including CDATA and ignored markup.
    // Reference characters are checked separately after decoding.
    if !text.chars().all(xml_char) {
        return Value::Nil;
    }
    let mut r = quick_xml::Reader::from_str(text);
    r.config_mut().check_end_names = true;
    r.config_mut().check_comments = true;
    // Bottom frame is a virtual root collecting top-level nodes.
    let mut stack = vec![XmlFrame {
        name: Vec::new(),
        attrs: Vec::new(),
        kids: Vec::new(),
        tail_cdata: false,
    }];
    let xml_attrs = |i: &mut Interp,
                     e: &quick_xml::events::BytesStart<'_>,
                     r: &quick_xml::Reader<&[u8]>|
     -> Option<Vec<Value>> {
        if !xml_attribute_spacing(e.attributes_raw()) {
            return None;
        }
        let mut attrs = Vec::new();
        // Keep quick-xml's duplicate-name and attribute syntax checks,
        // and propagate errors instead of silently discarding attributes.
        for a in e.attributes() {
            let a = a.ok()?;
            let name = a.key.as_ref();
            if !xml_name(name) || a.value.contains(&b'<') {
                return None;
            }
            // XML 1.0 sections 2.11 and 3.3.3: normalize literal
            // CRLF/CR first, then attribute TAB/LF to spaces, BEFORE
            // unescaping. Character references such as &#13; must keep
            // their referenced characters, and ordinary spaces must not
            // be trimmed or collapsed for these CDATA attributes.
            let raw = r.decoder().decode(&a.value).ok()?;
            let text = quick_xml::events::BytesText::from_escaped(raw);
            let normalized = text.xml10_content().ok()?;
            let normalized = normalized.replace(['\t', '\n'], " ");
            let v = xml_unescape(&normalized)?;
            // Only xmlns and xmlns:prefix declare namespaces. Ordinary
            // names such as xmlnsfoo must remain in the attribute list.
            // Declarations are omitted from the DOM, but their names,
            // values, and uniqueness still have to be well-formed.
            if name == b"xmlns" || name.starts_with(b"xmlns:") {
                continue;
            }
            attrs.push(attr_cons(i, local_name(a.key.as_ref()), &v));
        }
        Some(attrs)
    };
    // libxml2 requires exactly one root element; comments/PIs may
    // surround it at top level.
    let mut n_roots = 0u32;
    loop {
        // quick-xml's DOCTYPE scanner counts '<' and '>' even inside
        // quoted entity values. Dispatch at an event boundary, before that
        // scanner can reject a legal declaration such as <!ENTITY x '<'>.
        // Comments, CDATA and PIs have already been consumed as whole events,
        // so lookalike strings inside them never trigger the native backend.
        let next = &text[r.buffer_position() as usize..];
        let next = if r.buffer_position() == 0 {
            next.strip_prefix('\u{feff}').unwrap_or(next)
        } else {
            next
        };
        if next.starts_with("<!DOCTYPE") {
            // Without libxml2, fail closed rather than silently ignore the
            // declarations. Ordinary XML still uses the parser below.
            return dtd::parse(i, text, drop_comments).unwrap_or(Value::Nil);
        }
        match r.read_event() {
            Ok(Event::Start(e)) => {
                if !xml_name(e.name().as_ref()) {
                    return Value::Nil;
                }
                let Some(attrs) = xml_attrs(i, &e, &r) else {
                    return Value::Nil;
                };
                if stack.len() == 1 {
                    n_roots += 1;
                }
                stack.push(XmlFrame {
                    name: e.name().as_ref().to_vec(),
                    attrs,
                    kids: Vec::new(),
                    tail_cdata: false,
                });
            }
            Ok(Event::Empty(e)) => {
                if !xml_name(e.name().as_ref()) {
                    return Value::Nil;
                }
                let Some(attrs) = xml_attrs(i, &e, &r) else {
                    return Value::Nil;
                };
                if stack.len() == 1 {
                    n_roots += 1;
                }
                let node = Value::list(vec![
                    Value::Sym(i.intern(local_name(e.name().as_ref()))),
                    Value::list(attrs),
                ]);
                push_node(stack.last_mut().unwrap(), node);
            }
            Ok(Event::End(e)) => {
                if stack.len() <= 1 {
                    return Value::Nil; // stray close tag
                }
                let f = stack.pop().unwrap();
                if f.name != e.name().as_ref() {
                    return Value::Nil;
                }
                let mut node = vec![
                    Value::Sym(i.intern(local_name(&f.name))),
                    Value::list(f.attrs),
                ];
                node.extend(f.kids);
                push_node(stack.last_mut().unwrap(), Value::list(node));
            }
            Ok(Event::Text(e)) => {
                if e.windows(3).any(|s| s == b"]]>") {
                    return Value::Nil;
                }
                // Normalize literal line endings before GeneralRef events
                // are resolved, so numeric CR/LF references stay intact.
                if let Ok(t) = e.xml10_content() {
                    push_text(stack.last_mut().unwrap(), &t, false);
                }
            }
            Ok(Event::CData(e)) => {
                if let Ok(t) = e.xml10_content() {
                    push_text(stack.last_mut().unwrap(), &t, true);
                }
            }
            Ok(Event::Comment(e)) => {
                // GNU's DISCARD-COMMENTS drops only top-level comments.
                if !(drop_comments && stack.len() == 1) {
                    let t = e
                        .xml10_content()
                        .map(|t| t.into_owned())
                        .unwrap_or_default();
                    push_node(
                        stack.last_mut().unwrap(),
                        Value::list(vec![
                            Value::Sym(i.q("comment")),
                            Value::Nil,
                            Value::string(t),
                        ]),
                    );
                }
            }
            // GNU emits a bare nil for processing instructions.
            Ok(Event::PI(_)) => push_node(stack.last_mut().unwrap(), Value::Nil),
            Ok(Event::Decl(_)) => {}
            Ok(Event::DocType(_)) => {
                // Preserve declared entity nodes and GNU's attribute value
                // boundaries. A DOCTYPE-like string in a comment or CDATA
                // does not reach this branch. A missing runtime library is
                // an unsupported DTD parse, never an ignored declaration.
                return dtd::parse(i, text, drop_comments).unwrap_or(Value::Nil);
            }
            Ok(Event::GeneralRef(e)) => {
                let name = e.decode().map(|n| n.into_owned()).unwrap_or_default();
                match resolve_ref(&name) {
                    Some(s) => push_text(stack.last_mut().unwrap(), &s, false),
                    None => return Value::Nil,
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => return Value::Nil,
        }
    }
    if stack.len() != 1 || n_roots != 1 {
        return Value::Nil; // unclosed elements / wrong number of roots
    }
    let mut out = stack.pop().unwrap().kids;
    // Non-whitespace text outside the root is a parse error for libxml2.
    out.retain(|v| !matches!(v, Value::Str(s) if s.borrow().trim().is_empty()));
    if out.iter().any(|v| matches!(v, Value::Str(_))) {
        return Value::Nil;
    }
    match out.as_slice() {
        [single] => single.clone(),
        _ => {
            let mut top = vec![Value::Sym(i.q("top")), Value::Nil];
            top.extend(out);
            Value::list(top)
        }
    }
}

pub(crate) static SUBRS: &[Subr] = &[
    S!(
        "libxml-parse-xml-region",
        0,
        4,
        f_libxml_parse_xml_region,
        "Parse the region as an XML document.\nSTART and END default to the accessible portion of the buffer.\nBASE-URL is ignored; DISCARD-COMMENTS is accepted like GNU.\nReturns a DOM list, or nil on parse failure."
    ),
    S!(
        "libxml-parse-html-region",
        0,
        4,
        f_libxml_parse_html_region,
        "Parse the region as an HTML document.\nSTART and END default to the accessible portion of the buffer.\nBASE-URL is ignored; DISCARD-COMMENTS drops comment nodes like GNU.\nReturns a DOM list with implied html/body structure."
    ),
];
