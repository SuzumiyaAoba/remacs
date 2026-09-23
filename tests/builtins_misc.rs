//! Tests for the misc subrs (src/lisp/builtins/misc.rs).

mod common;
use common::ev;

#[test]
fn make_string() {
    assert_eq!(ev("(make-string 3 ?a)"), "\"aaa\"");
    assert_eq!(ev("(make-string 0 ?a)"), "\"\"");
    assert_eq!(ev("(make-string -1 ?a)"), "\"\"");
}

#[test]
fn plist_ops() {
    assert_eq!(ev("(plist-get '(a 1 b 2) 'b)"), "2");
    assert_eq!(ev("(plist-get '(a 1 b 2) 'c)"), "nil");
    assert_eq!(ev("(plist-get '(a 1 b 2) 'c 'eq)"), "nil");
    // plist-put returns the whole new plist.
    assert_eq!(ev("(let ((p '(a 1))) (plist-put p 'b 2) p)"), "(a 1 b 2)");
    assert_eq!(ev("(let ((p '(a 1))) (plist-put p 'a 9) p)"), "(a 9)");
    assert_eq!(ev("(plist-member '(a 1 b 2) 'b)"), "(b 2)");
    assert_eq!(ev("(plist-member '(a 1 b 2) 'z)"), "nil");
}

#[test]
fn take_ntake() {
    assert_eq!(ev("(take 2 '(1 2 3 4))"), "(1 2)");
    assert_eq!(ev("(take 0 '(1 2))"), "nil");
    assert_eq!(ev("(take 9 '(1 2))"), "(1 2)");
    assert_eq!(ev("(take 2 [1 2 3])"), "[1 2]");
    assert_eq!(ev("(take 2 \"abcd\")"), "\"ab\"");
    assert_eq!(ev("(let ((l (list 1 2 3 4))) (ntake 2 l))"), "(1 2)");
    // ntake mutates the original list.
    assert_eq!(ev("(let ((l (list 1 2 3 4))) (ntake 2 l) l)"), "(1 2)");
}

#[test]
fn base64() {
    assert_eq!(ev("(base64-encode-string \"hi\")"), "\"aGk=\"");
    assert_eq!(ev("(base64-decode-string \"aGk=\")"), "\"hi\"");
    assert_eq!(
        ev("(base64-encode-string \"hello world\")"),
        "\"aGVsbG8gd29ybGQ=\""
    );
    assert_eq!(
        ev("(base64-decode-string \"aGVsbG8gd29ybGQ=\")"),
        "\"hello world\""
    );
    assert_eq!(ev("(base64-encode-string \"\")"), "\"\"");
}

#[test]
fn crypto_hash() {
    // Reference digests.
    assert_eq!(ev("(md5 \"abc\")"), "\"900150983cd24fb0d6963f7d28e17f72\"");
    assert_eq!(
        ev("(secure-hash 'sha1 \"abc\")"),
        "\"a9993e364706816aba3e25717850c26c9cd0d89d\""
    );
    assert_eq!(
        ev("(secure-hash 'sha256 \"abc\")"),
        "\"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad\""
    );
    assert_eq!(ev("(md5 \"\")"), "\"d41d8cd98f00b204e9800998ecf8427e\"");
}

#[test]
fn format_spec() {
    assert_eq!(
        ev("(format-spec \"%a-%b\" '((?a . \"x\") (?b . \"y\")))"),
        "\"x-y\""
    );
    assert_eq!(ev("(format-spec \"%a\" '((?a . \"1\")))"), "\"1\"");
    assert_eq!(ev("(format-spec \"%%\" '((?a . \"x\")))"), "\"%\"");
    // %s/%S style specifiers.
    assert_eq!(ev("(format-spec \"%s\" '((?s . \"z\")))"), "\"z\"");
}

#[test]
fn apply_partially() {
    assert_eq!(ev("(funcall (apply-partially #'+ 1) 2 3)"), "6");
    assert_eq!(
        ev("(funcall (apply-partially #'concat \"a\" \"b\") \"c\")"),
        "\"abc\""
    );
}

#[test]
fn string_seq_conv() {
    assert_eq!(ev("(string-to-list \"ab\")"), "(97 98)");
    assert_eq!(ev("(string-to-list \"\")"), "nil");
    assert_eq!(ev("(string-to-vector \"ab\")"), "[97 98]");
    assert_eq!(ev("(string-to-vector nil)"), "[]");
}

#[test]
fn char_width() {
    assert_eq!(ev("(char-width ?a)"), "1");
    assert_eq!(ev("(char-width ?\\t)"), "8");
    assert_eq!(ev("(char-width ?\\n)"), "0");
    assert_eq!(ev("(char-width ?\\C-a)"), "2");
    // CJK wide char.
    assert_eq!(ev("(char-width ?あ)"), "2");
}

#[test]
fn assoc_string() {
    assert_eq!(
        ev("(assoc-string \"a\" '((\"a\" . 1) (\"b\" . 2)))"),
        "(\"a\" . 1)"
    );
    assert_eq!(ev("(assoc-string \"A\" '((\"a\" . 1)) t)"), "(\"a\" . 1)");
    assert_eq!(ev("(assoc-string \"z\" '((\"a\" . 1)))"), "nil");
}

#[test]
fn error_message_string() {
    // Matches GNU Emacs print_error_message semantics.
    assert_eq!(
        ev("(error-message-string '(arith-error))"),
        "\"Arithmetic error\""
    );
    assert_eq!(
        ev("(error-message-string '(wrong-type-argument integerp \"x\"))"),
        "\"Wrong type argument: integerp, \\\"x\\\"\""
    );
    assert_eq!(ev("(error-message-string '(error \"oops\"))"), "\"oops\"");
    assert_eq!(
        ev("(error-message-string '(end-of-buffer))"),
        "\"End of buffer\""
    );
    assert_eq!(
        ev("(error-message-string '(void-function foo))"),
        "\"Symbol’s function definition is void: foo\""
    );
    assert_eq!(
        ev("(error-message-string '(file-error \"msg\" \"extra\"))"),
        "\"msg: extra\""
    );
    assert_eq!(
        ev("(error-message-string '(unknown-sym 1 2))"),
        "\"peculiar error: 1, 2\""
    );
    assert_eq!(
        ev("(error-message-string '(user-error \"oops\"))"),
        "\"oops\""
    );
    assert_eq!(ev("(error-message-string '(error))"), "\"peculiar error\"");
}

#[test]
fn gensym() {
    let r = ev("(let ((g (gensym))) (symbol-name g))");
    assert!(r.starts_with("\"g"), "gensym name: {r}");
    // gensym symbols are uninterned.
    assert_eq!(
        ev("(let ((g (gensym))) (eq g (intern (symbol-name g))))"),
        "nil"
    );
}

#[test]
fn symbol_misc() {
    assert_eq!(ev("(documentation-stringp \"x\")"), "t");
    assert_eq!(ev("(documentation-stringp 1)"), "nil");
    // GNU requires the subr object; a symbol signals (subrp SYM).
    assert_eq!(ev("(subr-arity (symbol-function 'car))"), "(1 . 1)");
    assert_eq!(ev("(subr-arity (symbol-function '+))"), "(0 . many)");
    assert_eq!(
        ev("(condition-case e (subr-arity 'car) (error e))"),
        "(wrong-type-argument subrp car)"
    );
}

#[test]
fn time_fns() {
    // decode-time of a known time spec: (sec min hour day mon year dow dst zone)
    // (100 . 1) = 100 ticks at hz 1 = 100 seconds after the epoch.
    assert_eq!(ev("(nth 0 (decode-time '(100 . 1)))"), "40");
    assert_eq!(ev("(length (decode-time '(100 . 1)))"), "9");
    assert_eq!(ev("(type-of (current-time))"), "cons");
    assert_eq!(ev("(stringp (emacs-uptime))"), "t");
    assert_eq!(ev("(type-of (emacs-uptime))"), "string");
}

#[test]
fn misc_predicates() {
    assert_eq!(ev("(directory-name-p \"/tmp/\")"), "t");
    assert_eq!(ev("(directory-name-p \"/tmp\")"), "nil");
}

#[test]
fn indirect_function() {
    assert_eq!(ev("(indirect-function 'car)"), "#<subr car>");
    assert_eq!(
        ev("(progn (defalias 'my-car 'car) (indirect-function 'my-car))"),
        "#<subr car>"
    );
}
