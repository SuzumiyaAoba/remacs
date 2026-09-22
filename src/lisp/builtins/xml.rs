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

use html5ever::tendril::TendrilSink;

use super::{S, arg};
use crate::buffer::primitives::pos_idx;
use crate::lisp::error::Flow;
use crate::lisp::value::{Subr, Value};
use crate::lisp::{EvalResult, Interp};

/// Extract START..END (1-based, either may be nil) as a string.
fn region_text(i: &mut Interp, a: &[Value]) -> Result<String, Flow> {
    let b = i
        .current_buffer_ref()
        .ok_or_else(|| i.error("No current buffer"))?;
    let bb = b.borrow();
    let len = bb.text.len();
    let s = match &arg(a, 0) {
        Value::Nil => bb.begv,
        Value::Int(n) => pos_idx(len, *n),
        v => return Err(i.wrong_type_mut("integerp", v)),
    };
    let e = match &arg(a, 1) {
        Value::Nil => bb.zv,
        Value::Int(n) => pos_idx(len, *n),
        v => return Err(i.wrong_type_mut("integerp", v)),
    };
    Ok(bb.text.substring(s.min(e), s.max(e)))
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

/// Resolve `&name;' / `&#NNN;' references.  None = undeclared entity,
/// which makes libxml2 fail the document.
fn resolve_ref(name: &str) -> Option<String> {
    match name {
        "amp" => Some("&".into()),
        "lt" => Some("<".into()),
        "gt" => Some(">".into()),
        "quot" => Some("\"".into()),
        "apos" => Some("'".into()),
        _ => {
            let rest = name.strip_prefix('#')?;
            let n = if let Some(h) = rest.strip_prefix(['x', 'X']) {
                u32::from_str_radix(h, 16).ok()?
            } else {
                rest.parse().ok()?
            };
            Some(char::from_u32(n)?.to_string())
        }
    }
}

fn parse_xml(i: &mut Interp, text: &str, drop_comments: bool) -> Value {
    use quick_xml::events::Event;
    let mut r = quick_xml::Reader::from_str(text);
    r.config_mut().check_end_names = true;
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
     -> Vec<Value> {
        let mut attrs = Vec::new();
        for a in e.attributes().with_checks(false).flatten() {
            if a.key.as_ref().starts_with(b"xmlns") {
                continue;
            }
            let v = a
                .decode_and_unescape_value(r.decoder())
                .map(|v| v.into_owned())
                .unwrap_or_default();
            attrs.push(attr_cons(i, local_name(a.key.as_ref()), &v));
        }
        attrs
    };
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => {
                stack.push(XmlFrame {
                    name: e.name().as_ref().to_vec(),
                    attrs: xml_attrs(i, &e, &r),
                    kids: Vec::new(),
                    tail_cdata: false,
                });
            }
            Ok(Event::Empty(e)) => {
                let node = Value::list(vec![
                    Value::Sym(i.intern(local_name(e.name().as_ref()))),
                    Value::list(xml_attrs(i, &e, &r)),
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
                if let Ok(t) = e.decode() {
                    push_text(stack.last_mut().unwrap(), &t, false);
                }
            }
            Ok(Event::CData(e)) => {
                if let Ok(t) = e.decode() {
                    push_text(stack.last_mut().unwrap(), &t, true);
                }
            }
            Ok(Event::Comment(e)) => {
                // GNU's DISCARD-COMMENTS drops only top-level comments.
                if !(drop_comments && stack.len() == 1) {
                    let t = e.decode().map(|t| t.into_owned()).unwrap_or_default();
                    push_node(
                        stack.last_mut().unwrap(),
                        Value::list(vec![
                            Value::Sym(i.intern("comment")),
                            Value::Nil,
                            Value::string(t),
                        ]),
                    );
                }
            }
            // GNU emits a bare nil for processing instructions.
            Ok(Event::PI(_)) => push_node(stack.last_mut().unwrap(), Value::Nil),
            Ok(Event::Decl(_)) | Ok(Event::DocType(_)) => {}
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
    if stack.len() != 1 {
        return Value::Nil; // unclosed elements
    }
    let mut out = stack.pop().unwrap().kids;
    // Non-whitespace text outside the root is a parse error for libxml2.
    out.retain(|v| !matches!(v, Value::Str(s) if s.borrow().trim().is_empty()));
    if out.iter().any(|v| matches!(v, Value::Str(_))) {
        return Value::Nil;
    }
    match out.as_slice() {
        [] => Value::Nil,
        [single] => single.clone(),
        _ => {
            let mut top = vec![Value::Sym(i.intern("top")), Value::Nil];
            top.extend(out);
            Value::list(top)
        }
    }
}

// ---------- HTML via html5ever + rcdom ----------

fn html_node(
    i: &mut Interp,
    h: &markup5ever_rcdom::Handle,
    src_has_tbody: bool,
    out: &mut Vec<Value>,
) {
    use markup5ever_rcdom::NodeData;
    match &h.data {
        NodeData::Element { name, attrs, .. } => {
            let mut kids = Vec::new();
            for c in h.children.borrow().iter() {
                html_node(i, c, src_has_tbody, &mut kids);
            }
            // libxml2 doesn't materialize an empty <head>; drop it.
            if &*name.local == "head" && kids.is_empty() {
                return;
            }
            // html5ever implies <tbody> like HTML5; libxml2 doesn't.
            // When the source has no tbody tag, unwrap it into the parent.
            if &*name.local == "tbody" && !src_has_tbody {
                out.extend(kids);
                return;
            }
            let attr_list: Vec<Value> = attrs
                .borrow()
                .iter()
                .map(|a| attr_cons(i, a.name.local.as_ref(), &a.value))
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
                Value::Sym(i.intern("comment")),
                Value::Nil,
                Value::string(contents.to_string()),
            ]));
        }
        NodeData::ProcessingInstruction { .. } => out.push(Value::Nil),
        _ => {}
    }
}

fn parse_html(i: &mut Interp, text: &str) -> Value {
    let dom = html5ever::parse_document(
        markup5ever_rcdom::RcDom::default(),
        Default::default(),
    )
    .one(text.to_string());
    let lower = text.to_lowercase();
    let src_has_tbody = lower.contains("<tbody");
    let mut out = Vec::new();
    for c in dom.document.children.borrow().iter() {
        html_node(i, c, src_has_tbody, &mut out);
    }
    match out.as_slice() {
        [single] => single.clone(),
        _ => {
            let mut top = vec![Value::Sym(i.intern("top")), Value::Nil];
            top.extend(out);
            Value::list(top)
        }
    }
}

fn f_libxml_parse_xml_region(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let text = region_text(i, &a)?;
    let drop_comments = arg(&a, 3).truthy();
    Ok(parse_xml(i, &text, drop_comments))
}

fn f_libxml_parse_html_region(i: &mut Interp, a: Vec<Value>) -> EvalResult {
    let text = region_text(i, &a)?;
    Ok(parse_html(i, &text))
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
        "Parse the region as an HTML document.\nSTART and END default to the accessible portion of the buffer.\nReturns a DOM list with implied html/body structure."
    ),
];
