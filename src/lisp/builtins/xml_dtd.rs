//! DTD-bearing XML needs libxml2's entity nodes, not text substitution.
//!
//! Keep this optional backend separate from the quick-xml path. In particular,
//! GNU's `make_dom` exposes only an attribute's first value node and represents
//! general entity references in element content as nil. Expanding strings in
//! quick-xml would lose these boundaries and require another DTD parser.

use std::ffi::{CStr, c_char, c_int, c_void};
use std::sync::OnceLock;

use crate::lisp::{Interp, value::Value};

// Stable public libxml2 tree.h ABI. Only the common prefix is accessed for
// documents, DTDs and attributes; the extended fields below are accessed only
// for xmlNode instances. No borrowed native pointer escapes the document.
#[repr(C)]
struct Head {
    private: *mut c_void,
    kind: c_int,
    name: *const c_char,
    children: *const Head,
    last: *const Head,
    parent: *const Head,
    next: *const Head,
    prev: *const Head,
    doc: *const Head,
}

#[repr(C)]
struct Node {
    head: Head,
    ns: *mut c_void,
    content: *const c_char,
    properties: *const Head,
}

type ReadMemory =
    unsafe extern "C" fn(*const c_char, c_int, *const c_char, *const c_char, c_int) -> *mut Head;
type FreeDoc = unsafe extern "C" fn(*mut Head);
type RootElement = unsafe extern "C" fn(*const Head) -> *const Head;

struct Libxml {
    _lib: libloading::Library,
    read_memory: ReadMemory,
    free_doc: FreeDoc,
    root_element: RootElement,
}

fn library() -> Option<&'static Libxml> {
    static LIBXML: OnceLock<Option<Libxml>> = OnceLock::new();
    LIBXML
        .get_or_init(|| {
            let lib = crate::lisp::dynlib::open_library(
                "REMACS_LIBXML2_LIBRARY",
                &[
                    "libxml2.2.dylib",
                    "/usr/lib/libxml2.2.dylib",
                    "libxml2.dylib",
                    "libxml2.so.2",
                    "libxml2.so",
                ],
                "libxml2",
                if cfg!(target_os = "macos") {
                    "libxml2.dylib"
                } else {
                    "libxml2.so.2"
                },
            )?;
            // Signatures are the public libxml2 parser.h/tree.h declarations.
            Some(unsafe {
                // Initialize once before parallel parser calls (as GNU's
                // xmlCheckVersion does), and never call global cleanup.
                let init: unsafe extern "C" fn() =
                    crate::lisp::dynlib::sym(&lib, b"xmlInitParser\0")?;
                init();
                Libxml {
                    read_memory: crate::lisp::dynlib::sym(&lib, b"xmlReadMemory\0")?,
                    free_doc: crate::lisp::dynlib::sym(&lib, b"xmlFreeDoc\0")?,
                    root_element: crate::lisp::dynlib::sym(&lib, b"xmlDocGetRootElement\0")?,
                    _lib: lib,
                }
            })
        })
        .as_ref()
}

struct Document<'a> {
    ptr: *mut Head,
    lib: &'a Libxml,
}

impl Drop for Document<'_> {
    fn drop(&mut self) {
        // Owns exactly the successful xmlReadMemory result, including all its
        // nodes. RAII also frees it if building the Lisp DOM unwinds.
        unsafe { (self.lib.free_doc)(self.ptr) }
    }
}

unsafe fn string(ptr: *const c_char) -> String {
    // All callers first check a native content pointer for null, or use an
    // element/attribute name, which libxml2 always supplies on a valid tree.
    unsafe { CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned()
}

unsafe fn make_dom(i: &mut Interp, head: *const Head) -> Value {
    let h = unsafe { &*head };
    match h.kind {
        1 => {
            // XML_ELEMENT_NODE is an xmlNode, unlike an attribute or DTD.
            let n = unsafe { &*head.cast::<Node>() };
            let mut attrs = Vec::new();
            let mut property = n.properties;
            while !property.is_null() {
                let p = unsafe { &*property };
                if !p.children.is_null() {
                    let child = unsafe { &*p.children.cast::<Node>() };
                    if !child.content.is_null() {
                        attrs.push(super::attr_cons(i, &unsafe { string(p.name) }, &unsafe {
                            string(child.content)
                        }));
                    }
                }
                property = p.next;
            }
            let mut out = vec![
                Value::Sym(i.intern(&unsafe { string(h.name) })),
                Value::list(attrs),
            ];
            let mut child = h.children;
            while !child.is_null() {
                out.push(unsafe { make_dom(i, child) });
                child = unsafe { (*child).next };
            }
            Value::list(out)
        }
        3 | 4 | 8 => {
            // XML_TEXT_NODE, XML_CDATA_SECTION_NODE, XML_COMMENT_NODE.
            let n = unsafe { &*head.cast::<Node>() };
            if n.content.is_null() {
                Value::Nil
            } else {
                let text = Value::string(unsafe { string(n.content) });
                if h.kind == 8 {
                    Value::list(vec![Value::Sym(i.intern("comment")), Value::Nil, text])
                } else {
                    text
                }
            }
        }
        // Includes XML_ENTITY_REF_NODE and processing instructions. Do not
        // traverse entity children: they link to the declaration, not DOM kids.
        _ => Value::Nil,
    }
}

/// None means the optional system library is unavailable; Some(nil) means a
/// real parse failure. Call only at a DOCTYPE token/event boundary.
/// No external DTD/entity loading, validation, substitution or HUGE options:
/// preserve native entity nodes and libxml2's recursion/amplification limits.
pub(super) fn parse(i: &mut Interp, text: &str, discard_comments: bool) -> Option<Value> {
    let lib = library()?;
    let Ok(len) = c_int::try_from(text.len()) else {
        return Some(Value::Nil);
    };
    // Same options as GNU Emacs 31.1 xml.c, parse_region: NOERROR (32),
    // NOWARNING (64), NOBLANKS (256), NONET (2048). In particular, neither
    // NOENT nor DTDLOAD is enabled, so external resources are not loaded.
    let ptr = unsafe {
        (lib.read_memory)(
            text.as_ptr().cast(),
            len,
            c"".as_ptr(),
            c"utf-8".as_ptr(),
            32 | 64 | 256 | 2048,
        )
    };
    if ptr.is_null() {
        return Some(Value::Nil);
    }
    let doc = Document { ptr, lib };
    let mut out = Vec::new();
    let mut previous = Value::Nil;
    if !discard_comments {
        let mut node = unsafe { (*doc.ptr).children };
        while !node.is_null() {
            if previous.truthy() {
                out.push(previous);
            }
            previous = unsafe { make_dom(i, node) };
            node = unsafe { (*node).next };
        }
    }
    // Mirror GNU's top-level handling, including ignored DTD/PI nodes and
    // DISCARD-COMMENTS selecting just the root element.
    Some(if out.is_empty() {
        let root = unsafe { (lib.root_element)(doc.ptr) };
        if root.is_null() {
            Value::Nil
        } else {
            unsafe { make_dom(i, root) }
        }
    } else {
        let mut top = vec![Value::Sym(i.intern("top")), Value::Nil];
        top.extend(out);
        top.push(previous);
        Value::list(top)
    })
}
