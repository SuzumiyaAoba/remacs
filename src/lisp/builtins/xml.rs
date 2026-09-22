//! `libxml-parse-xml-region' / `libxml-parse-html-region'.
//!
//! GNU parses the region with libxml2 and returns a Lisp DOM:
//!   (TAG ((ATTR-SYM . "value") ...) CHILD ...)  for elements
//!   (comment nil "text")                        for comments
//!   nil                                         for PIs (a GNU quirk)
//! Top level: a single node is returned bare; multiple nodes get a
//! synthetic `top' wrapper.  XML parse errors yield nil; the HTML parser
//! (html5ever) is lenient like libxml2's HTML mode and implies
//! html/body structure.

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

// ---------- XML via roxmltree ----------

fn xml_node(i: &mut Interp, n: roxmltree::Node<'_, '_>, out: &mut Vec<Value>) {
    match n.node_type() {
        roxmltree::NodeType::Element => {
            let attrs: Vec<Value> = n
                .attributes()
                .map(|a| attr_cons(i, a.name(), a.value()))
                .collect();
            let mut node = vec![
                Value::Sym(i.intern(n.tag_name().name())),
                Value::list(attrs),
            ];
            let mut kids = Vec::new();
            for c in n.children() {
                xml_node(i, c, &mut kids);
            }
            node.extend(kids);
            out.push(Value::list(node));
        }
        roxmltree::NodeType::Text => {
            if let Some(t) = n.text() {
                out.push(Value::string(t));
            }
        }
        roxmltree::NodeType::Comment => {
            out.push(Value::list(vec![
                Value::Sym(i.intern("comment")),
                Value::Nil,
                Value::string(n.text().unwrap_or("")),
            ]));
        }
        // GNU emits a bare nil for processing instructions.
        roxmltree::NodeType::PI => out.push(Value::Nil),
        _ => {}
    }
}

fn parse_xml(i: &mut Interp, text: &str) -> Value {
    let doc = match roxmltree::Document::parse_with_options(
        text,
        roxmltree::ParsingOptions {
            allow_dtd: true,
            ..Default::default()
        },
    ) {
        Ok(d) => d,
        Err(_) => return Value::Nil,
    };
    let mut out = Vec::new();
    for n in doc.root().children() {
        xml_node(i, n, &mut out);
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

// ---------- HTML via html5ever + rcdom ----------

fn html_node(
    i: &mut Interp,
    h: &markup5ever_rcdom::Handle,
    out: &mut Vec<Value>,
) {
    use markup5ever_rcdom::NodeData;
    match &h.data {
        NodeData::Element { name, attrs, .. } => {
            let mut kids = Vec::new();
            for c in h.children.borrow().iter() {
                html_node(i, c, &mut kids);
            }
            // libxml2 doesn't materialize an empty <head>; drop it.
            if &*name.local == "head" && kids.is_empty() {
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
    let mut out = Vec::new();
    for c in dom.document.children.borrow().iter() {
        html_node(i, c, &mut out);
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
    Ok(parse_xml(i, &text))
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
