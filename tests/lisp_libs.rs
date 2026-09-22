//! Tests for the ported GNU Emacs Lisp libraries under lisp/:
//!   ring.el, let-alist.el, thunk.el, dom.el, pcase.el, subr-x.el
//! Expected values were verified against GNU Emacs 31.1.
//!
//! thunk/named-let assert `lexical-binding' at macro-expansion time, so
//! their tests bind it via a leading (setq lexical-binding t) form.

mod common;
use common::{ev, ev_out};

// ---------------------------------------------------------------- ring

#[test]
fn ring_basics() {
    assert_eq!(
        ev("(progn (require 'ring)
                  (let ((r (make-ring 3)))
                    (dotimes (i 5) (ring-insert r i))
                    (list (ring-length r) (ring-ref r 0) (ring-member r 3))))"),
        "(3 4 1)"
    );
}

#[test]
fn ring_remove_and_empty() {
    assert_eq!(
        ev("(progn (require 'ring)
                  (let ((r (make-ring 4)))
                    (ring-insert r 'a) (ring-insert r 'b)
                    (ring-remove r 0)
                    (list (ring-length r) (ring-ref r 0) (ring-empty-p r))))"),
        "(1 a nil)"
    );
    assert_eq!(
        ev("(progn (require 'ring) (ring-empty-p (make-ring 2)))"),
        "t"
    );
}

// ---------------------------------------------------------------- let-alist

#[test]
fn let_alist_binds_dot_names() {
    assert_eq!(
        ev("(progn (require 'let-alist)
                  (let-alist '((a . 1) (b . 2)) (list .a .b)))"),
        "(1 2)"
    );
    assert_eq!(
        ev("(progn (require 'let-alist)
                  (let-alist '((s . ((x . 10)))) .s.x))"),
        "10"
    );
}

// ---------------------------------------------------------------- thunk

#[test]
fn thunk_delay_force_memoizes() {
    assert_eq!(
        ev("(progn (setq lexical-binding t) (require 'thunk)
                  (let ((n 0))
                    (let ((th (thunk-delay (setq n (1+ n)) n)))
                      (thunk-force th)
                      (list n (thunk-evaluated-p th)))))"),
        "(1 t)"
    );
}

#[test]
fn thunk_let_star() {
    assert_eq!(
        ev("(progn (setq lexical-binding t) (require 'thunk)
                  (thunk-let* ((x (+ 1 2)) (y (* x 10))) (list x y)))"),
        "(3 30)"
    );
}

// ---------------------------------------------------------------- dom

#[test]
fn dom_children_by_tag_and_pp() {
    assert_eq!(
        ev("(progn (require 'dom)
                  (with-temp-buffer
                    (insert \"<html><body><p>a</p><p>b</p></body></html>\")
                    (let ((d (libxml-parse-html-region (point-min) (point-max))))
                      (list (length (dom-by-tag d 'p))
                            (dom-tag (car (dom-by-tag d 'p)))))))"),
        "(2 p)"
    );
    // dom-pp inserts a printed DOM representation into the current buffer.
    assert_eq!(
        ev_out("(progn (require 'dom)
                      (with-temp-buffer
                        (dom-pp '(div ((id . \"x\")) \"hi\"))
                        (princ (buffer-string))))"),
        "(div ((id . \"x\"))\n \"hi\")"
    );
}

// ---------------------------------------------------------------- pcase

#[test]
fn pcase_basic_patterns() {
    assert_eq!(
        ev("(progn (require 'pcase)
                  (list (pcase 3 (1 'one) (3 'three) (_ 'other))
                        (pcase '(1 2) (`(,a ,b) (list b a)))
                        (pcase 'x ('x 'it))))"),
        "(three (2 1) it)"
    );
}

#[test]
fn pcase_let_and_pred() {
    assert_eq!(
        ev("(progn (require 'pcase)
                  (pcase-let ((`(,a ,b) '(1 2)) (c 3))
                    (+ a b c)))"),
        "6"
    );
    assert_eq!(
        ev("(progn (require 'pcase)
                  (pcase 5 ((pred oddp) 'odd) (_ 'even)))"),
        "odd"
    );
}

#[test]
fn pcase_lambda() {
    assert_eq!(
        ev("(progn (require 'pcase)
                  (mapcar (pcase-lambda (`(,a ,b)) (+ a b)) '((1 2) (3 4))))"),
        "(3 7)"
    );
}

// ---------------------------------------------------------------- subr-x

#[test]
fn subr_x_named_let() {
    assert_eq!(
        ev("(progn (setq lexical-binding t) (require 'subr-x)
                  (named-let lp ((i 0) (acc nil))
                    (if (= i 4) (nreverse acc)
                      (lp (1+ i) (cons i acc)))))"),
        "(0 1 2 3)"
    );
}

#[test]
fn subr_x_string_and_list_utils() {
    assert_eq!(
        ev("(progn (require 'subr-x)
                  (list (string-join '(\"a\" \"b\") \"-\")
                        (string-trim \"  x  \")
                        (if-let* ((v '(1 2))) (car v))))"),
        "(\"a-b\" \"x\" 1)"
    );
    // thread-first inserts the accumulated value as the FIRST argument:
    // (- (* (+ 1 2) 10) 100) => -70.
    assert_eq!(
        ev("(progn (require 'subr-x)
                  (thread-first (+ 1 2) (* 10) (- 100)))"),
        "-70"
    );
}

#[test]
fn subr_x_work_buffer() {
    assert_eq!(
        ev_out("(progn (require 'subr-x)
                       (with-work-buffer
                         (insert \"abc\")
                         (princ (buffer-string))))"),
        "abc"
    );
}

// ------------------------------------------------------- lexical capture

#[test]
fn top_level_lambda_captures_when_lexical() {
    // Under `lexical-binding' the top-level lambda captures a fresh
    // lexical frame; (eq x x) on floats is object identity (GNU: t).
    assert_eq!(
        ev("(progn (setq lexical-binding t)
                  (let ((x 3.5)) (eq x x)))"),
        "t"
    );
    assert_eq!(
        ev("(progn (setq lexical-binding t)
                  (let ((n 5)) (funcall (lambda () n))))"),
        "5"
    );
    // GNU: once a lexical env exists (internal-interpreter-environment
    // non-nil), a dynamic rebind of `lexical-binding' to nil does not
    // switch evaluation back to dynamic scoping.
    assert_eq!(
        ev("(progn (setq lexical-binding t)
                  (let ((lexical-binding nil))
                    (let ((n 5)) (funcall (lambda () n)))))"),
        "5"
    );
}
