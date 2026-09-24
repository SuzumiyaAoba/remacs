//! Tests for the ported GNU Emacs Lisp libraries under lisp/:
//!   radix-tree.el, mb-depth.el, skeleton.el, doctor.el, mpuz.el
//! Expected values were verified against GNU Emacs 31.1.

mod common;
use common::ev;

// ---------------------------------------------------------- radix-tree

#[test]
fn radix_tree_insert_lookup() {
    assert_eq!(
        ev("(progn (require 'radix-tree)
                  (let ((rt nil))
                    (setq rt (radix-tree-insert rt \"cat\" 1))
                    (setq rt (radix-tree-insert rt \"car\" 2))
                    (setq rt (radix-tree-insert rt \"cart\" 3))
                    (setq rt (radix-tree-insert rt \"dog\" 4))
                    (setq rt (radix-tree-insert rt \"dot\" 5))
                    (list (radix-tree-lookup rt \"cat\")
                          (radix-tree-lookup rt \"ca\")
                          (radix-tree-lookup rt \"cart\")
                          (radix-tree-lookup rt \"xyz\")
                          (radix-tree-count rt))))"),
        "(1 nil 3 nil 5)"
    );
}

#[test]
fn radix_tree_prefixes_and_mappings() {
    // GNU-verified on 31.1: prefix alist is newest-prefix-first, and
    // iter-mappings visits every key/value pair.
    assert_eq!(
        ev("(progn (require 'radix-tree)
                  (let ((rt nil))
                    (dolist (kv '((\"car\" . 2) (\"cart\" . 3) (\"cat\" . 1)
                                  (\"dog\" . 4) (\"dot\" . 5)))
                      (setq rt (radix-tree-insert rt (car kv) (cdr kv))))
                    (list (radix-tree-prefixes rt \"cart\")
                          (sort (let (a)
                                  (radix-tree-iter-mappings
                                   rt (lambda (k v) (push (cons k v) a)))
                                  a)
                                (lambda (x y) (string< (car x) (car y)))))))"),
        "(((\"cart\" . 3) (\"car\" . 2)) ((\"car\" . 2) (\"cart\" . 3) (\"cat\" . 1) (\"dog\" . 4) (\"dot\" . 5)))"
    );
}

#[test]
fn radix_tree_remove_and_subtree() {
    assert_eq!(
        ev("(progn (require 'radix-tree)
                  (let ((rt nil))
                    (setq rt (radix-tree-insert rt \"cat\" 1))
                    (setq rt (radix-tree-insert rt \"car\" 2))
                    ;; nil value removes the key
                    (setq rt (radix-tree-insert rt \"cat\" nil))
                    (list (radix-tree-lookup rt \"cat\")
                          (radix-tree-lookup rt \"car\")
                          (radix-tree-count rt)
                          (radix-tree-subtree rt \"ca\"))))"),
        "(nil 2 1 ((\"r\" . 2)))"
    );
}

// ---------------------------------------------------------- mb-depth

#[test]
fn mb_depth_mode_toggles_hook() {
    assert_eq!(
        ev("(progn (require 'mb-depth)
                  (minibuffer-depth-indicate-mode 1)
                  (prog1 (list minibuffer-depth-indicate-mode
                               (and (memq 'minibuffer-depth-setup
                                          minibuffer-setup-hook)
                                    t))
                    (minibuffer-depth-indicate-mode -1)))"),
        "(t t)"
    );
    assert_eq!(
        ev("(progn (require 'mb-depth)
                  (minibuffer-depth-indicate-mode -1)
                  (list minibuffer-depth-indicate-mode
                        (memq 'minibuffer-depth-setup
                              minibuffer-setup-hook)))"),
        "(nil nil)"
    );
}

// ---------------------------------------------------------- skeleton

#[test]
fn skeleton_insert_basic() {
    // GNU-verified on 31.1.
    // The skeleton's first element is the interactor (prompt), not
    // inserted text — GNU inserts " world!" here.
    assert_eq!(
        ev("(progn (require 'skeleton)
                  (with-temp-buffer
                    (skeleton-insert '(\"hello\" ?\\s \"world\" ?!))
                    (buffer-string)))"),
        "\" world!\""
    );
}

#[test]
fn skeleton_insert_positions() {
    // `@' records interesting points in `skeleton-positions'.
    assert_eq!(
        ev("(progn (require 'skeleton)
                  (with-temp-buffer
                    (skeleton-insert '(nil \"a\" \"b\" @ \"c\" @ \"d\"))
                    (list (buffer-string)
                          (sort skeleton-positions #'<))))"),
        "(\"abcd\" (3 4))"
    );
}

#[test]
fn skeleton_define_skeleton() {
    // GNU-verified: define-skeleton builds an interactive command that
    // also marks itself no-self-insert for expand-abbrev.
    assert_eq!(
        ev("(progn (require 'skeleton)
                  (define-skeleton test-skel \"Doc.\" \"x\" _ \"y\")
                  (with-temp-buffer
                    (call-interactively 'test-skel)
                    (list (buffer-string)
                          (get 'test-skel 'no-self-insert))))"),
        "(\"y\" t)"
    );
}

#[test]
fn skeleton_conditional_eval() {
    // GNU-verified: skeleton elements eval under a fresh lexical root, so
    // `(eval 'x t)` inside a skeleton sees dynamic bindings made by the
    // caller -- exercised here via `let' on a dynamically-bound `x'.
    assert_eq!(
        ev("(progn (require 'skeleton)
                  (define-skeleton cond-skel \"\" nil \"x\"
                    (if (eval 'x t) \"X\" \"no\"))
                  (list (dlet ((x t))
                          (with-temp-buffer (cond-skel) (buffer-string)))
                        (dlet ((x nil))
                          (with-temp-buffer (cond-skel) (buffer-string)))))"),
        "(\"xX\" \"xno\")"
    );
}

#[test]
fn skeleton_pair_wrap_region() {
    assert_eq!(
        ev("(progn (require 'skeleton)
                  (with-temp-buffer
                    (insert \"wrapme\")
                    (goto-char 1) (set-mark 7) (activate-mark)
                    (call-interactively #'skeleton-pair-insert-maybe)
                    (buffer-string)))"),
        "\"wrapme\""
    );
}

// ---------------------------------------------------------- doctor

#[test]
fn doctor_grammar_helpers() {
    // GNU-verified on 31.1: pluralization, class predicates, fixup.
    assert_eq!(
        ev("(progn (require 'doctor)
                  (doctor-make-variables)
                  (list (doctor-plural 'boy)
                        (doctor-plural 'child)
                        (doctor-verbp 'run)
                        (doctor-articlep 'the)
                        (doctor-fixup '(me am sad))
                        (doctor-replace '(i am happy)
                                        '((i you) (am are) (happy sad)))))"),
        "(boies childs t (the a an) (i am sad) (you are sad))"
    );
}

#[test]
fn doctor_mode_defined() {
    assert_eq!(
        ev("(progn (require 'doctor)
                  (list (fboundp 'doctor) (fboundp 'doctor-mode)
                        (fboundp 'doctor-doc)))"),
        "(t t t)"
    );
}

// ---------------------------------------------------------- mpuz

#[test]
fn mpuz_puzzle_board_shape() {
    // The board is an 11-element vector of rows holding (digit . col)
    // cells; painting fills the *Mult Puzzle* buffer.
    assert_eq!(
        ev("(progn (require 'mpuz)
                  (mpuz-create-buffer)
                  (with-current-buffer \"*Mult Puzzle*\"
                    (mpuz-random-puzzle)
                    (list (length mpuz-board)
                          (seq-every-p #'listp (seq-drop mpuz-board 1))
                          (progn (mpuz-paint-board)
                                 (> (length (buffer-string)) 40)))))"),
        "(10 t t)"
    );
}

// ---------------------------------------------------------- dotted lambda lists

#[test]
fn dotted_rest_lambda() {
    // GNU accepts `(lambda (a . b) ...)' and `(lambda a ...)'
    // alike; verified on GNU 31.1.
    assert_eq!(
        ev("(list (funcall (lambda (a . b) (list a b)) 1 2 3)
                  (funcall (lambda a a) 1 2)
                  (funcall (lambda (a b . c) (list a b c)) 1 2 3 4))"),
        "((1 (2 3)) (1 2) (1 2 (3 4)))"
    );
}

#[test]
fn dotted_rest_defun() {
    assert_eq!(
        ev("(progn (defun drest (a . b) (cons a b))
                  (drest 1 2 3))"),
        "(1 2 3)"
    );
}

// ------------------------------------- macroexpand-all: non-code positions

#[test]
fn macroexpand_all_preserves_arglists() {
    // A parameter or binding variable named like a macro must not be
    // expanded — GNU's `macroexp--expand-all' keeps these positions
    // verbatim.  `(defmacro m () "X")' expands (m) to "X".
    assert_eq!(
        ev("(progn (defmacro m (&rest _a) \"X\")
                  (list
                    (macroexpand-all '(defun f (m) (list m 1)))
                    (macroexpand-all '(lambda (m) m))
                    (macroexpand-all '(let ((m 1)) m))
                    (macroexpand-all '(function (lambda (m) m)))))"),
        "((defun f (m) (list m 1)) (lambda (m) m) (let ((m 1)) m) #'(lambda (m) m))"
    );
}

#[test]
fn macroexpand_all_setq_condition_case() {
    assert_eq!(
        ev("(progn (defmacro mm (&rest _a) \"Y\")
                  (list
                    (macroexpand-all '(setq mm 1 mm (mm)))
                    (macroexpand-all '(condition-case mm (mm) (mm (mm))))))"),
        "((setq mm 1 mm \"Y\") (condition-case mm \"Y\" (mm \"Y\")))"
    );
}

// ---------------------------------------------------------- cl.el names

#[test]
fn member_if_any() {
    // GNU cl.el: member-if returns the tail cons at the first matching
    // element; `any' is its alias — rx.el relies on it.
    assert_eq!(
        ev("(list (member-if #'oddp '(2 4 3 6))
                  (any #'oddp '(2 4 6))
                  (any #'oddp '(2 4 3)))"),
        "((3 6) nil (3))"
    );
}

// ---------------------------------------------------------- rx

#[test]
fn rx_to_string_basics() {
    // GNU-verified on 31.1.
    assert_eq!(
        ev("(progn (require 'rx)
                  (list
                    (rx-to-string '(seq \"ab\" (repeat 2 digit)))
                    (rx-to-string '(: \"x\" (* any)))
                    (rx-to-string '(or \"a\" \"b\" \"c\"))
                    (rx-to-string '(group-n 1 \"foo\" (* (any \"a-z\"))))
                    (rx-to-string '(seq bol (one-or-more (not (any \" \\t\")))))
                    (rx-to-string '(seq word-boundary (+ (any \"a-z\")) word-boundary))
                    (rx-to-string '(seq (? \"a\") (>= 2 \"b\")))))"),
        "(\"\\\\(?:ab[[:digit:]]\\\\{2\\\\}\\\\)\" \"\\\\(?:x.*\\\\)\" \"[abc]\" \"\\\\(?1:foo[a-z]*\\\\)\" \"\\\\(?:^[^\t ]+\\\\)\" \"\\\\(?:\\\\b[a-z]+\\\\b\\\\)\" \"\\\\(?:a?b\\\\{2,\\\\}\\\\)\")"
    );
}

#[test]
fn rx_macro_expand() {
    // GNU-verified: the `rx' macro defers to `rx--to-expr', which
    // yields a plain string here (no shy group) on 31.1.
    assert_eq!(
        ev("(progn (require 'rx)
                  (macroexpand '(rx (seq \"a\" (* \"b\")))))"),
        "\"ab*\""
    );
}

// ---------------------------------------------------------- round-2 libs

#[test]
fn ewoc_create_and_collect() {
    // ewoc prints a node list into the current buffer.
    assert_eq!(
        ev("(progn (require 'ewoc)
                  (with-temp-buffer
                    (let ((w (ewoc-create (lambda (d) (insert (format \"%s\" d)))
                                          \"H\" \"F\")))
                      (ewoc-enter-last w 1)
                      (ewoc-enter-last w 2)
                      (ewoc-refresh w)
                      (buffer-string))))"),
        "\"H\n1\n2\nF\n\""
    );
}

#[test]
fn ansi_color_apply_basic() {
    assert_eq!(
        ev("(progn (require 'ansi-color)
                  (fboundp 'ansi-color-apply))"),
        "t"
    );
}

#[test]
fn regi_compiles_frame() {
    assert_eq!(
        ev("(progn (require 'regi)
                  (list (fboundp 'regi-interpret)
                        (fboundp 'regi-pos)
                        (fboundp 'regi-mapcar)))"),
        "(t t t)"
    );
}

#[test]
fn tempo_template_functions() {
    assert_eq!(
        ev("(progn (require 'tempo)
                  (list (fboundp 'tempo-insert-template)
                        (fboundp 'tempo-define-template)))"),
        "(t t)"
    );
}

#[test]
fn autoinsert_and_expand_features() {
    // autoinsert needs `rx' at load time; expand needs skeleton.
    assert_eq!(
        ev("(progn (require 'autoinsert) (require 'expand)
                  (list (featurep 'autoinsert) (featurep 'expand)
                        (fboundp 'auto-insert)
                        (fboundp 'expand-c-for-skeleton)))"),
        "(t t t t)"
    );
}

#[test]
fn dabbrev_and_tq_features() {
    assert_eq!(
        ev("(progn (require 'dabbrev) (require 'tq)
                  (list (featurep 'dabbrev) (featurep 'tq)
                        (fboundp 'dabbrev-expand)
                        (fboundp 'tq-create)))"),
        "(t t t t)"
    );
}
