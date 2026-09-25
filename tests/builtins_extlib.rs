//! Tests for the external-library subrs:
//!   src/lisp/builtins/sqlite.rs  (sqlite-*)
//!   src/lisp/builtins/xml.rs     (libxml-parse-*-region)
//!   src/lisp/builtins/charset.rs (decode/encode-big5-char, -sjis-char)
//! Expected values were verified against GNU Emacs 31.1.

mod common;
use common::{ev, ev_err, ev_result};

// ---------------------------------------------------------------- sqlite

#[test]
fn sqlite_open_close_pred() {
    assert_eq!(ev("(sqlitep (sqlite-open))"), "t");
    assert_eq!(ev("(type-of (sqlite-open))"), "sqlite");
    assert_eq!(ev("(sqlitep 42)"), "nil");
    assert_eq!(ev("(sqlitep '(a))"), "nil");
    assert_eq!(ev("(sqlite-available-p)"), "t");
    assert_eq!(ev("(stringp (sqlite-version))"), "t");
    // sqlite-close returns t, including on an already-closed db.
    assert_eq!(
        ev("(let ((db (sqlite-open))) (list (sqlite-close db) (sqlite-close db) (sqlitep db)))"),
        "(t t t)"
    );
}

#[test]
fn sqlite_execute_rows_and_count() {
    // Affected-row count for DML; a row list for statements that
    // return data (GNU steps once and switches on SQLITE_ROW).
    assert_eq!(
        ev(
            "(let ((db (sqlite-open))) (sqlite-execute db \"create table t (a,b)\") (sqlite-execute db \"insert into t values (1,'x')\"))"
        ),
        "1"
    );
    assert_eq!(
        ev(
            "(let ((db (sqlite-open))) (sqlite-execute db \"create table t (a,b)\") (sqlite-execute db \"insert into t values (1,'x')\") (sqlite-execute db \"insert into t values (2,'y')\") (sqlite-execute db \"select * from t order by a\"))"
        ),
        "((1 \"x\") (2 \"y\"))"
    );
    // Parameter binding from a list and a vector.
    assert_eq!(
        ev(
            "(let ((db (sqlite-open))) (sqlite-execute db \"create table t (a,b)\") (sqlite-execute db \"insert into t values (?,?)\" '(7 \"q\")) (sqlite-execute db \"insert into t values (?,?)\" [8 \"r\"]) (sqlite-select db \"select a from t order by a\"))"
        ),
        "((7) (8))"
    );
    assert_eq!(
        ev(
            "(let ((db (sqlite-open))) (sqlite-execute db \"create table t (a)\") (sqlite-execute db \"insert into t values (1)\") (sqlite-execute db \"update t set a=5\") (sqlite-select db \"select a from t\"))"
        ),
        "((5))"
    );
}

#[test]
fn sqlite_select_return_types() {
    let pre = "(let ((db (sqlite-open))) (sqlite-execute db \"create table t (a,b)\") (sqlite-execute db \"insert into t values (1,'x')\")";
    assert_eq!(
        ev(&format!("{pre} (sqlite-select db \"select * from t\"))")),
        "((1 \"x\"))"
    );
    // `full' prepends the column-name list.
    assert_eq!(
        ev(&format!(
            "{pre} (sqlite-select db \"select * from t\" nil 'full))"
        )),
        "((\"a\" \"b\") (1 \"x\"))"
    );
}

#[test]
fn sqlite_set_lazy_more_p() {
    // `sqlite-more-p' stays t until `sqlite-next' steps past the end;
    // even an empty set reports t before the first `sqlite-next'.
    assert_eq!(
        ev(
            "(let* ((db (sqlite-open)) (set (sqlite-select db \"select 1 where 0\" nil 'set))) (list (sqlite-more-p set) (sqlite-next set) (sqlite-more-p set)))"
        ),
        "(t nil nil)"
    );
    assert_eq!(
        ev(
            "(let* ((db (sqlite-open)) (_ (sqlite-execute db \"create table t (a)\")) (_ (sqlite-execute db \"insert into t values (1)\")) (_ (sqlite-execute db \"insert into t values (2)\")) (set (sqlite-select db \"select * from t order by a\" nil 'set))) (list (sqlite-columns set) (sqlite-next set) (sqlite-next set) (sqlite-next set) (sqlite-more-p set) (sqlite-finalize set)))"
        ),
        "((\"a\") (1) (2) nil nil t)"
    );
    // A finalized set rejects further operations with sqlite-error.
    assert_eq!(
        ev(
            "(let* ((db (sqlite-open)) (set (sqlite-select db \"select 1\" nil 'set))) (sqlite-finalize set) (condition-case e (sqlite-next set) (sqlite-error (car e))))"
        ),
        "sqlite-error"
    );
}

#[test]
fn sqlite_transactions_and_pragma() {
    // Transaction helpers return t/nil, never signal on SQL failure.
    assert_eq!(
        ev(
            "(let ((db (sqlite-open))) (sqlite-execute db \"create table t (a)\") (list (sqlite-transaction db) (sqlite-execute db \"insert into t values (1)\") (sqlite-rollback db) (sqlite-select db \"select * from t\") (sqlite-rollback db) (sqlite-commit db)))"
        ),
        "(t 1 t nil nil nil)"
    );
    assert_eq!(
        ev(
            "(let ((db (sqlite-open))) (list (sqlite-pragma db \"foreign_keys = on\") (sqlite-pragma db \"bogus !!!\") (sqlite-execute-batch db \"create table u(x); insert into u values(9)\") (sqlite-select db \"select * from u\") (sqlite-execute-batch db \"not sql ;;;\")))"
        ),
        "(t nil t ((9)) nil)"
    );
}

#[test]
fn sqlite_errors() {
    // Closed db / prepare failure / non-allowlisted module all signal
    // sqlite-error, which `error' handlers also catch.
    assert_eq!(
        ev(
            "(let ((db (sqlite-open))) (sqlite-close db) (condition-case e (sqlite-execute db \"select 1\") (sqlite-error (car e))))"
        ),
        "sqlite-error"
    );
    assert_eq!(
        ev("(condition-case e (sqlite-select (sqlite-open) \"bogus ((\") (error (car e)))"),
        "sqlite-error"
    );
    assert_eq!(
        ev(
            "(condition-case e (sqlite-load-extension (sqlite-open) \"/tmp/evil.so\") (sqlite-error (car e)))"
        ),
        "sqlite-error"
    );
    // Non-sqlite arguments are a wrong-type-argument, not sqlite-error.
    assert_eq!(ev_err("(sqlite-close 42)"), "wrong-type-argument");
    assert_eq!(ev_err("(sqlite-next 42)"), "wrong-type-argument");
    assert!(ev_result("(sqlite-execute 42 \"x\")").is_err());
}

// ----------------------------------------------------------------- libxml

#[test]
fn libxml_xml_basic() {
    assert_eq!(
        ev(r#"(with-temp-buffer (insert "<a x=\"1\">t</a>") (libxml-parse-xml-region))"#),
        r#"(a ((x . "1")) "t")"#
    );
    // Nested elements and attributes.
    assert_eq!(
        ev(r#"(with-temp-buffer (insert "<r><c y=\"2\">x</c></r>") (libxml-parse-xml-region))"#),
        r#"(r nil (c ((y . "2")) "x"))"#
    );
    // CDATA stays a separate node from adjacent text.
    assert_eq!(
        ev(r#"(with-temp-buffer (insert "<a><![CDATA[cd]]>t</a>") (libxml-parse-xml-region))"#),
        r#"(a nil "cd" "t")"#
    );
    // Nested comments are kept; top-level comments are dropped.
    assert_eq!(
        ev(r#"(with-temp-buffer (insert "<a><!--c--><b/></a>") (libxml-parse-xml-region))"#),
        r#"(a nil (comment nil "c") (b nil))"#
    );
    // With top-level siblings GNU returns a `top' wrapper; the 4th arg
    // DISCARD-COMMENTS drops only top-level comments.
    assert_eq!(
        ev(r#"(with-temp-buffer (insert "<!--top--><a/>") (libxml-parse-xml-region))"#),
        r#"(top nil (comment nil "top") (a nil))"#
    );
    assert_eq!(
        ev(
            r#"(with-temp-buffer (insert "<!--top--><a/>") (libxml-parse-xml-region nil nil nil t))"#
        ),
        "(a nil)"
    );
}

#[test]
fn libxml_xml_failures() {
    // Malformed input and multiple root elements both give nil.
    assert_eq!(
        ev(r#"(with-temp-buffer (insert "<a/><b/>") (libxml-parse-xml-region))"#),
        "nil"
    );
    assert_eq!(
        ev(r#"(with-temp-buffer (insert "<a>") (libxml-parse-xml-region))"#),
        "nil"
    );
    assert_eq!(
        ev(r#"(with-temp-buffer (insert "no tags") (libxml-parse-xml-region))"#),
        "nil"
    );
}

#[test]
fn libxml_html() {
    // html5ever implied structure, minus the empty implied <head>.
    assert_eq!(
        ev(r#"(with-temp-buffer (insert "<p>hi<br>x</p>") (libxml-parse-html-region))"#),
        r#"(html nil (body nil (p nil "hi" (br nil) "x")))"#
    );
    // An implied <tbody> is unwrapped like GNU/libxml2; an explicit
    // one is retained.
    assert_eq!(
        ev(
            r#"(with-temp-buffer (insert "<table><tr><td>x</td></tr></table>") (libxml-parse-html-region))"#
        ),
        r#"(html nil (body nil (table nil (tr nil (td nil "x")))))"#
    );
    assert_eq!(
        ev(
            r#"(with-temp-buffer (insert "<table><tbody><tr><td>x</td></tr></tbody></table>") (libxml-parse-html-region))"#
        ),
        r#"(html nil (body nil (table nil (tbody nil (tr nil (td nil "x"))))))"#
    );
}

// ---------------------------------------------------------------- charset

#[test]
fn big5_decode_encode() {
    // Mapped codes decode through the Big5 table.
    assert_eq!(ev("(decode-big5-char #xA440)"), "19968"); // 一
    // Valid-but-unmapped codes return private Emacs characters:
    // 1245184 + (b1-0xA1)*191 + (b2-0x40).
    assert_eq!(ev("(decode-big5-char #xFEFE)"), "1263137");
    // GNU masks the low byte with 0x7F for the range check only, so
    // 0xC0-0xFE trails are valid and still map as their own codes.
    assert_eq!(ev("(decode-big5-char #xA4C0)"), "20998");
    // Invalid lead/trail bytes are an error.
    assert_eq!(ev_err("(decode-big5-char #xA13F)"), "error");
    assert_eq!(ev_err("(decode-big5-char #xA0A1)"), "error");
    // Private characters round-trip through encode.
    assert_eq!(ev("(encode-big5-char 1263137)"), "65278"); // #xFEFE
    assert_eq!(ev("(encode-big5-char 19968)"), "42048"); // #xA440
}

#[test]
fn sjis_decode_encode() {
    // decode-sjis-char uses the Shift-JIS-2004 (JISX0213) map, like GNU.
    assert_eq!(ev("(decode-sjis-char #x889F)"), "20124"); // 亜
    assert_eq!(ev("(decode-sjis-char #xED40)"), "30787");
    // Unmapped JISX0213 codes give private chars (base 1359872) that
    // round-trip through encode-sjis-char.
    assert_eq!(ev("(decode-sjis-char #x8795)"), "1361084");
    assert_eq!(ev("(encode-sjis-char 1361084)"), "34709"); // #x8795
    assert_eq!(ev("(encode-sjis-char 20124)"), "34975"); // #x889F
    // Halfwidth kana pass through JISX0201 (0xA1-0xDE; 0xDF errors).
    assert_eq!(ev("(decode-sjis-char #xA1)"), "65377");
    // Out-of-range bytes error.
    assert_eq!(ev_err("(decode-sjis-char #x87FE)"), "error");
    assert_eq!(ev_err("(decode-sjis-char #x807F)"), "error");
    assert_eq!(ev_err("(decode-sjis-char #xFD40)"), "error");
}
