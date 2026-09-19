//! Emacs 31.1 compatibility tests — reader, printer, sequences,
//! plists, format, kbd — verified by differential probing.

mod common;
use common::{ev, ev_err};

// ---------- reader ----------

#[test]
fn reader_hash_syntax() {
    // #s(...) records read as a distinct type.
    assert_eq!(
        ev("(type-of (car (read-from-string \"#s(a 1 2)\")))"),
        "record"
    );
    assert_eq!(ev("(car (read-from-string \"#s(a 1 2)\"))"), "#s(a 1 2)");
    // #N= / #N# labels: circular ref is eq to the object itself.
    assert_eq!(
        ev("(let ((x (car (read-from-string \"#1=(a . #1#)\")))) (eq x (cdr x)))"),
        "t"
    );
    // Shared structure: both refs point to the same object.
    assert_eq!(
        ev("(let ((x (car (read-from-string \"(#1=(a b) . #1#)\")))) (eq (car x) (cdr x)))"),
        "t"
    );
    // #_ reads the next token as an interned symbol.
    assert_eq!(ev("(car (read-from-string \"#_5 7\"))"), "\\5");
    assert_eq!(
        ev("(eq 'b (cadr (car (read-from-string \"(a #_b c)\"))))"),
        "t"
    );
    // Bare dot is invalid.
    assert_eq!(
        ev_err("(car (read-from-string \".\"))"),
        "invalid-read-syntax"
    );
    // #(...) is not valid vector syntax in Emacs.
    assert_eq!(
        ev_err("(car (read-from-string \"#(1 2)\"))"),
        "invalid-read-syntax"
    );
}

#[test]
fn reader_char_mods() {
    // Control chars fold: ?\C-a is the char 1, not a modifier bit.
    assert_eq!(ev("?\\C-a"), "1");
    assert_eq!(ev("?\\C-@"), "0");
    assert_eq!(ev("?\\C-?"), "127");
    assert_eq!(ev("?\\M-a"), "134217825");
}

// ---------- printer ----------

#[test]
fn printer_abbrevs() {
    assert_eq!(ev("(prin1-to-string '(quote a))"), "\"'a\"");
    assert_eq!(ev("(prin1-to-string '(function f))"), "\"#'f\"");
    assert_eq!(ev("(prin1-to-string '`(a ,b))"), "\"`(a ,b)\"");
    assert_eq!(ev("(prin1-to-string 'x)"), "\"x\"");
    // Uninterned symbols: no #: (print-gensym nil).
    assert_eq!(ev("(prin1-to-string (make-symbol \"z\"))"), "\"z\"");
    // Symbol whose name is numeric gets an escape ("50" -> \50).
    assert_eq!(ev("(prin1-to-string (gensym 5))"), "\"\\\\50\"");
    // Strings keep literal newlines.
    assert_eq!(ev("(prin1-to-string \"a\nb\")"), "\"\\\"a\nb\\\"\"");
}

// ---------- sequences ----------

#[test]
fn sequences() {
    assert_eq!(ev_err("(concat \"a\" 5)"), "wrong-type-argument");
    assert_eq!(ev("(substring [1 2 3] 1)"), "[2 3]");
    assert_eq!(ev("(substring [1 2 3] -1)"), "[3]");
    assert_eq!(ev("(reverse \"abc\")"), "\"cba\"");
    assert_eq!(ev("(reverse [1 2 3])"), "[3 2 1]");
    assert_eq!(ev("(nreverse [1 2])"), "[2 1]");
    assert_eq!(ev("(elt '(1 2) 5)"), "nil");
    assert_eq!(ev("(elt '(1 2) -1)"), "1");
    assert_eq!(ev("(nth -1 '(a b))"), "a");
    assert_eq!(ev("(remove 'b '(a b c))"), "(a c)");
    assert_eq!(ev("(remove 2 [1 2 3])"), "[1 3]");
    assert_eq!(ev("(remq 'a '(a b a))"), "(b)");
    assert_eq!(ev_err("(cdaar '((1 . 2)))"), "wrong-type-argument");
}

#[test]
fn string_funcs() {
    assert_eq!(ev("(string-blank-p \"   \")"), "0");
    assert_eq!(ev("(string-blank-p \"x\")"), "nil");
    assert_eq!(ev("(string-trim \"xxhixx\" \"x+\")"), "\"hixx\"");
    assert_eq!(ev("(string-trim \"  hi  \")"), "\"hi\"");
    assert_eq!(ev("(string-trim-left \"xxhixx\" \"x+\")"), "\"hixx\"");
    assert_eq!(ev("(string-trim-right \"xxhixx\" \"x+\")"), "\"xxhi\"");
    assert_eq!(ev("(split-string \"a b  c\")"), "(\"a\" \"b\" \"c\")");
    assert_eq!(ev("(split-string \" a \")"), "(\"\" \"a\" \"\")");
    assert_eq!(ev("(string-to-number \"10abc\")"), "10");
    assert_eq!(ev("(string-to-number \"-5x\")"), "-5");
    assert_eq!(ev("(string-to-number \".5x\")"), "0.5");
    assert_eq!(ev("(string-to-number \"1.5e2z\")"), "150.0");
    assert_eq!(ev("(string-to-number \"abc\")"), "0");
    assert_eq!(ev("(format \"%1$s %2$s %1$s\" \"a\" \"b\")"), "\"a b a\"");
}

// ---------- plists / symbols ----------

#[test]
fn plist_order() {
    assert_eq!(
        ev("(let ((s 'qq1)) (put s 'a 1) (put s 'b 2) (symbol-plist s))"),
        "(a 1 b 2)"
    );
    assert_eq!(ev("(boundp 'nil)"), "t");
    assert_eq!(ev("(boundp 't)"), "t");
    assert_eq!(ev("(fboundp 'if)"), "t");
    assert_eq!(ev("(fboundp 'when)"), "t");
    assert_eq!(ev("(subrp 'car)"), "nil");
    assert_eq!(ev("(subrp #'car)"), "nil");
    assert_eq!(ev_err("(unintern \"nope\")"), "wrong-number-of-arguments");
    assert_eq!(ev("(commandp 'next-line)"), "t");
}

// ---------- kbd / events ----------

#[test]
fn kbd_events() {
    assert_eq!(ev("(kbd \"C-x C-f\")"), "\"\u{18}\u{6}\"");
    assert_eq!(ev("(kbd \"S-TAB\")"), "[33554441]");
    assert_eq!(ev("(kbd \"M-<return>\")"), "[M-return]");
    assert_eq!(ev("(kbd \"abc\")"), "\"abc\"");
    assert_eq!(ev("(text-char-description 27)"), "\"^[\"");
    assert_eq!(ev("(text-char-description 1)"), "\"^A\"");
    assert_eq!(ev("(text-char-description 127)"), "\"^?\"");
    assert_eq!(
        ev_err("(text-char-description 134217791)"),
        "wrong-type-argument"
    );
}

// ---------- gensym ----------

#[test]
fn gensym_counter() {
    // gensym uses its own counter (prefix + N).
    assert_eq!(
        ev("(progn (gensym) (gensym \"zz\") (symbol-name (gensym)))"),
        "\"g2\""
    );
}
