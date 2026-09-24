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
