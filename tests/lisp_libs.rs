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
        ev_out(
            "(progn (require 'dom)
                      (with-temp-buffer
                        (dom-pp '(div ((id . \"x\")) \"hi\"))
                        (princ (buffer-string))))"
        ),
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
        ev_out(
            "(progn (require 'subr-x)
                       (with-work-buffer
                         (insert \"abc\")
                         (princ (buffer-string))))"
        ),
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

// ------------------------------------------------------- map.el

#[test]
fn map_generic_lookup() {
    // alist, plist, hash-table, array lookup all GNU-verified.
    assert_eq!(
        ev("(progn (require 'map)
                  (list (map-elt '((a . 1) (b . 2)) 'b)
                        (map-elt '(x 1 y 2) 'y)
                        (map-elt [10 20] 1)))"),
        "(2 2 20)"
    );
    assert_eq!(
        ev("(progn (require 'map)
                  (let ((m (make-hash-table)))
                    (puthash 'k 9 m)
                    (list (map-elt m 'k) (map-elt m 'z 42))))"),
        "(9 42)"
    );
}

#[test]
fn map_put_and_delete() {
    // map-put! on an existing alist key updates in place;
    // map-delete removes the whole pair (alist-get REMOVE path).
    assert_eq!(
        ev("(progn (require 'map)
                  (let ((m (list '(a . 1) '(b . 2))))
                    (map-put! m 'a 10)
                    (map-delete m 'a)))"),
        "((b . 2))"
    );
    assert_eq!(
        ev("(progn (require 'map)
                  (let ((m '((a . 1) (b . 2))))
                    (map-put! m 'b 7)
                    (map-keys m)))"),
        "(a b)"
    );
}

#[test]
fn map_let_and_others() {
    assert_eq!(
        ev("(progn (require 'map)
                  (let ((m (make-hash-table :test #'equal)))
                    (puthash \"x\" 5 m)
                    (map-let ((\"x\" x)) m (or x 0))))"),
        "5"
    );
    assert_eq!(
        ev("(progn (require 'map)
                  (list (map-length '((a . 1) (b . 2)))
                        (map-contains-key '((a . 1)) 'a)))"),
        "(2 t)"
    );
}

// ------------------------------------------------------- thingatpt.el

#[test]
fn thingatpt_basics() {
    // GNU-verified goldens for the ported thingatpt + forward-symbol shim.
    assert_eq!(
        ev_out(
            "(progn (require 'thingatpt)
                      (with-temp-buffer
                        (insert \"foo_bar baz-qux 42\")
                        (goto-char 2)
                        (princ (list (thing-at-point 'symbol)
                                     (bounds-of-thing-at-point 'symbol))) (terpri)
                        (goto-char 10)
                        (princ (thing-at-point 'word)) (terpri)
                        (goto-char 1)
                        (princ (list (forward-thing 'word) (point))) (terpri)
                        (princ (symbol-at-point))))"
        ),
        "(foo_bar (1 . 8))\nbaz\n(t 4)\nfoo_bar"
    );
}

#[test]
fn thingatpt_uuid() {
    assert_eq!(
        ev("(progn (require 'thingatpt)
                  (with-temp-buffer
                    (insert \"uuid d3f7b8c2-1234-4abc-9def-0123456789ab x\")
                    (goto-char 8)
                    (bounds-of-thing-at-point 'uuid)))"),
        "(6 . 42)"
    );
    // forward-word now returns t on success / nil at the buffer edge (GNU).
    assert_eq!(
        ev("(with-temp-buffer (insert \"ab\") (goto-char 1)
                  (list (forward-word) (forward-word)))"),
        "(t nil)"
    );
}

// ------------------------------------------------------- avl-tree.el

#[test]
fn avl_tree_insert_order_delete() {
    // GNU-verified: in-order traversal yields sorted order, both
    // directions; delete keeps the tree balanced and correct.
    assert_eq!(
        ev("(progn (require 'avl-tree)
                  (let ((t1 (avl-tree-create #'<)))
                    (dolist (x '(5 3 8 1 4 7 9)) (avl-tree-enter t1 x))
                    (list (avl-tree-flatten t1)
                          (avl-tree-member t1 4)
                          (avl-tree-size t1)
                          (avl-tree-mapcar #'1+ t1))))"),
        "((1 3 4 5 7 8 9) 4 7 (2 4 5 6 8 9 10))"
    );
    assert_eq!(
        ev("(progn (require 'avl-tree)
                  (let ((t1 (avl-tree-create #'<)))
                    (dolist (x '(5 3 8 1 4 7 9)) (avl-tree-enter t1 x))
                    (avl-tree-delete t1 5)
                    (avl-tree-flatten t1)))"),
        "(1 3 4 7 8 9)"
    );
}

#[test]
fn avl_tree_iter_and_stack() {
    assert_eq!(
        ev_out(
            "(progn (require 'avl-tree)
                      (let ((t1 (avl-tree-create #'<)))
                        (avl-tree-enter t1 1) (avl-tree-enter t1 3)
                        (let ((it (avl-tree-iter t1)))
                          (princ (list (iter-next it) (iter-next it))) (terpri)
                          (princ (condition-case _e
                                     (progn (iter-next it) 'more)
                                   (iter-end-of-sequence 'done))) (terpri))
                        (let ((s (avl-tree-stack t1)))
                          (princ (list (avl-tree-stack-pop s)
                                       (avl-tree-stack-empty-p s))))))"
        ),
        "(1 3)\ndone\n(1 nil)"
    );
}

// ------------------------------------------------------- time-date.el

#[test]
fn time_date_decoded_add() {
    // GNU-verified: month overflow clamps day, year leap handling,
    // negative day delta, second carry.
    assert_eq!(
        ev("(progn (require 'time-date)
                  (list (decoded-time-add (list 30 30 12 31 1 2019 nil -1 nil)
                                          (make-decoded-time :month 1))
                        (decoded-time-add (list 0 0 0 29 2 2020 nil -1 nil)
                                          (make-decoded-time :year 1))
                        (decoded-time-add (list 0 0 0 1 1 2024 nil -1 nil)
                                          (make-decoded-time :day -1))))"),
        "((30 30 12 28 2 2019 nil -1 nil) (0 0 0 28 2 2021 nil -1 nil) (0 0 0 31 12 2023 nil -1 nil))"
    );
}

#[test]
fn time_date_helpers() {
    assert_eq!(
        ev("(progn (require 'time-date)
                  (list (date-days-in-month 2024 2)
                        (date-days-in-month 2023 2)
                        (decoded-time-period (make-decoded-time :hour 1 :minute 1 :second 1))
                        (decoded-time-set-defaults (list nil nil nil 15 nil nil nil nil nil))))"),
        "(29 28 3661 (0 0 0 15 1 1970 nil nil nil))"
    );
    // safe-date-to-time catches the \"Invalid date\" error.
    assert_eq!(
        ev("(progn (require 'time-date)
                  (list (safe-date-to-time \"not a date\")
                        (format-seconds \"%y %d %h %m %s\" 10000000)
                        (format-seconds \"%.3Y\" 10000000)))"),
        "(0 \"0 115 17 46 40\" \"000 years\")"
    );
}

#[test]
fn time_arith_forms() {
    // GNU timefns.c forms: (TICKS . HZ) preserved through convert;
    // hz==1 renders as integer; eq subtract yields the list zero.
    assert_eq!(
        ev("(list (time-convert 30 t) (time-convert 30 'integer)
                 (time-convert '(0 30 0 0) t) (time-convert '(1 2) t)
                 (time-add 30 0) (time-subtract '(30 . 1) '(0 . 1))
                 (time-subtract 30 30))"),
        "((30 . 1) 30 (30000000000000 . 1000000000000) (65538 . 1) 30 30 (0 0 0 0))"
    );
}

// ------------------------------------------------------- seq extras

#[test]
fn seq_indexed_and_positions() {
    assert_eq!(
        ev("(list (seq-map-indexed (lambda (e i) (list e i)) '(a b c))
                 (seq-positions '(a b a c a) 'a)
                 (seq-positions [1 2 3 2] 2 #'=))"),
        "(((a 0) (b 1) (c 2)) (0 2 4) (1 3))"
    );
    assert_eq!(
        ev_out(
            "(let ((acc nil))
                 (seq-do-indexed (lambda (e i) (push (list i e) acc)) '(x y))
                 (princ acc))"
        ),
        "((1 y) (0 x))"
    );
}

#[test]
fn seq_set_ops_and_split() {
    assert_eq!(
        ev("(list (seq-union '(1 2 3) '(2 3 4))
                 (seq-intersection '(1 2 3 4) '(2 4 6))
                 (seq-keep (lambda (x) (and (numberp x) (* x 10))) '(1 a 2 b 3))
                 (seq-split '(1 2 3 4 5 6 7) 3)
                 (seq-mapcat 'list '(1 2) 'vector))"),
        "((1 2 3 4) (2 4) (10 20 30) ((1 2 3) (4 5 6) (7)) [1 2])"
    );
    assert_eq!(
        ev("(list (seq-sort-by 'car '< '((3 . a) (1 . b) (2 . c)))
                 (condition-case e (seq-random-elt '()) (error (car e))))"),
        "(((1 . b) (2 . c) (3 . a)) error)"
    );
}

// --------------------------------------------------------- radix-tree

#[test]
fn radix_tree_ops() {
    // GNU-verified vs radix-tree.el on 31.1, including prefix-of-key
    // splits, subtree iteration, and the literal-prefix quirk of
    // `radix-tree-iter-mappings'.
    assert_eq!(
        ev("(progn (require 'radix-tree)
                  (let ((t1 radix-tree-empty))
                    (setq t1 (radix-tree-insert t1 \"al\" 'X))
                    (setq t1 (radix-tree-insert t1 \"alpha\" 'Y))
                    (setq t1 (radix-tree-insert t1 \"alpine\" 'Z))
                    (list (radix-tree-lookup t1 \"al\")
                          (radix-tree-lookup t1 \"alpha\")
                          (radix-tree-lookup t1 \"alpine\")
                          (radix-tree-lookup t1 \"nope\")
                          (radix-tree-prefixes t1 \"alp\")
                          (radix-tree-count t1))))"),
        "(X Y Z nil ((\"al\" . X)) 3)"
    );
    assert_eq!(
        ev("(progn (require 'radix-tree)
                  (let ((t1 radix-tree-empty) (m nil))
                    (setq t1 (radix-tree-insert t1 \"al\" 'X))
                    (setq t1 (radix-tree-insert t1 \"alpha\" 'Y))
                    (radix-tree-iter-mappings t1 (lambda (k v) (push (cons k v) m)) \"p\")
                    (sort m (lambda (a b) (string< (car a) (car b))))))"),
        "((\"pal\" . X) (\"palpha\" . Y))"
    );
}

// ---------------------------------------------------------- saveplace

#[test]
fn saveplace_roundtrip() {
    // GNU-verified: recording, file write, and point restore all match
    // saveplace.el on GNU 31.1.
    let dir = std::env::temp_dir().join(format!("remacs-sp-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("f.txt");
    let places = dir.join("places.eld");
    std::fs::write(&f, "line1\nline2\nline3\nline4\n").unwrap();
    let src = format!(
        "(progn (require 'saveplace)
                (let ((save-place-file \"{}\"))
                  (find-file \"{}\")
                  (save-place-local-mode 1)
                  (goto-char (point-min)) (forward-line 2) (move-to-column 2)
                  (save-place-to-alist)
                  (save-place-alist-to-file)
                  (setq save-place-alist nil)
                  (save-place-load-alist-from-file)
                  (kill-buffer)
                  (find-file \"{}\")
                  (save-place-find-file-hook)
                  (list (point) (line-number-at-pos) (current-column))))",
        places.display(),
        f.display(),
        f.display()
    );
    assert_eq!(ev(&src), "(15 3 2)");
    let written = std::fs::read_to_string(&places).unwrap();
    assert_eq!(
        written,
        format!(
            ";;; -*- coding: utf-8; mode: lisp-data -*-\n((\"{}\" . 15))",
            f.display()
        )
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn saveplace_mode_hooks() {
    // GNU-verified -Q state: global mode installs exactly these hooks
    // (kill-emacs-hook only when interactive).
    assert_eq!(
        ev("(progn (require 'saveplace)
                  (save-place-mode 1)
                  (list save-place-mode
                        (memq #'save-place-find-file-hook find-file-hook)
                        (memq #'save-place-dired-hook dired-initial-position-hook)
                        (memq #'save-place-to-alist kill-buffer-hook)))"),
        "(t (save-place-find-file-hook) (save-place-dired-hook) (save-place-to-alist uniquify-kill-buffer-function vc-kill-buffer-hook))"
    );
}

// ------------------------------------------------- round 19 runtime fixes

#[test]
fn macroexp_quote_is_function() {
    // GNU macroexp.el: `macroexp-quote' is a *function* — its argument is
    // evaluated before deciding whether to wrap in (quote …).  A bogus
    // defmacro redefinition left raw subexpressions unquoted.
    assert_eq!(ev("(list (macroexp-quote 5) (macroexp-quote 'x) (macroexp-quote nil))"),
        "(5 'x nil)");
}

#[test]
fn cl_typep_via_cl_macs() {
    // GNU: `cl-typep' is autoloaded from cl-macs; loading cl-macs must
    // restore the real (dumped) definition, and compound type specifiers
    // follow GNU's lattice.
    assert_eq!(
        ev("(progn (require 'cl-macs)
                  (list (cl-typep 'a 'symbol)
                        (cl-typep 5 'symbol)
                        (cl-typep 5 '(or vector (and symbol (not null))))
                        (cl-typep \"x\" '(or vector (and symbol (not null))))))"),
        "(t nil nil nil)"
    );
}

#[test]
fn cl_advised_dolist_wraps_in_nil_block() {
    // cl.el advises `dolist' with cl--wrap-in-nil-block.  Loading cl.el
    // used to loop forever: backquote-list* called dolist, the advice
    // wrapped it in cl-block, whose backquote body spawned another
    // backquote-list*.  GNU's while-based backquote-list* terminates.
    assert_eq!(
        ev("(progn (require 'cl)
                  (car (macroexpand-1 '(dolist (x '(1 2)) x))))"),
        "cl-block"
    );
}

#[test]
fn pcase_eieio_pattern_expands() {
    // GNU eieio.el provides a `pcase' pattern `(eieio FIELDS…)' used by
    // transient.el; without it expansion signals "Unknown eieio pattern".
    assert_eq!(
        ev("(progn (require 'eieio)
                  (car (macroexpand-1 '(pcase v ((eieio a b) (list a b))))))"),
        "if"
    );
}

#[test]
fn pcase_cl_type_pattern_expands() {
    // GNU cl-macs.el's `(cl-type TYPE)' pcase pattern (transient.el uses
    // `(cl-type symbol)' etc.); expansions dispatch on `cl-typep'.
    assert_eq!(
        ev("(progn (require 'cl-macs)
                  (car (macroexpand-1 '(pcase v ((cl-type symbol) v)))))"),
        "if"
    );
}

#[test]
fn quail_subdir_load_and_leim_list() {
    // `quail/NAME' references must resolve to the leim/quail copies even
    // where `language/NAME' shares the basename (burmese, czech, …); the
    // flat tree stores those as quail-NAME.el.
    assert_eq!(
        ev("(progn (require 'quail)
                  (load \"quail/arabic\")
                  (and (quail-package \"arabic\")
                       (list (car (assoc \"arabic\" input-method-alist)))))"),
        "(\"arabic\")"
    );
    // The colliding name resolves to the quail copy: language/czech.el
    // only calls `set-language-info-alist', while quail/czech.el
    // registers quail packages (GNU-verified: -Q already has 'czech in
    // features from the dumped language files).
    assert_eq!(
        ev("(progn (require 'quail)
                  (load \"quail/czech\")
                  (and (quail-package \"czech\") t))"),
        "t"
    );
}
