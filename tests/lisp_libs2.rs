//! Tests for the ported GNU Emacs Lisp libraries under lisp/:
//!   radix-tree.el, mb-depth.el, skeleton.el, doctor.el, mpuz.el
//! Expected values were verified against GNU Emacs 31.1.

mod common;
use common::{ev, ev_out};

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
    // GNU 31.1 rejects a dotted arglist tail when the closure is called
    // (`invalid-function'), and a bare-symbol arglist when the closure
    // is built: cconv's `(cl-assert (listp args))' under lexical
    // binding, Fmake_interpreted_closure's CHECK_LIST otherwise.
    assert_eq!(
        ev(
            "(list (condition-case e (eval '(funcall (lambda (a . b) (list a b)) 1 2 3) t)
                    (error (car e)))
                  (condition-case e (eval '(funcall (lambda a a) 1 2) t) (error e))
                  (condition-case e (eval '(funcall (lambda a a) 1 2) nil) (error e))
                  (condition-case e (eval '(funcall (lambda (a b . c) (list a b c)) 1 2 3 4) t)
                    (error (car e))))"
        ),
        "(invalid-function (cl-assertion-failed (listp args)) \
(wrong-type-argument listp a) invalid-function)"
    );
}

#[test]
fn dotted_rest_defun() {
    // GNU 31.1: `defun' itself trips over the dotted tail.
    assert_eq!(
        ev("(condition-case e
               (eval '(progn (defun drest (a . b) (cons a b)) (drest 1 2 3)) t)
             (error e))"),
        "(wrong-type-argument listp b)"
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
        // GNU expands (defun f ARGS . BODY) → (defalias 'f #'(lambda ...)).
        "((defalias 'f #'(lambda (m) (list m 1))) #'(lambda (m) m) (let ((m 1)) m) #'(lambda (m) m))"
    );
}

#[test]
fn macroexpand_all_setq_condition_case() {
    // GNU expands each (setq VAR VAL) pair pair-wise into a progn.
    assert_eq!(
        ev("(progn (defmacro mm (&rest _a) \"Y\")
                  (list
                    (macroexpand-all '(setq mm 1 mm (mm)))
                    (macroexpand-all '(condition-case mm (mm) (mm (mm))))))"),
        "((progn (setq mm 1) (setq mm \"Y\")) (condition-case mm \"Y\" (mm \"Y\")))"
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

// ---------------------------------------------------------- timezone

#[test]
fn timezone_parse_and_format() {
    // GNU-verified on 31.1.
    assert_eq!(
        ev("(progn (require 'timezone)
                  (list (timezone-parse-date \"Mon, 15 Jan 2024 10:30:00 -0500\")
                        (timezone-parse-time \"10:30:45\")
                        (timezone-zone-to-minute \"PST\")
                        (timezone-zone-to-minute \"-0500\")
                        (timezone-make-arpa-date 2024 1 15 \"10:30:00\" \"PST\")
                        (timezone-make-time-string 10 30 45)))"),
        "([\"2024\" \"1\" \"15\" \"10:30:00\" \"-0500\"] [\"10\" \"30\" \"45\"] -480 -300 \"15 Jan 2024 10:30:00 PST\" \"10:30:45\")"
    );
}

// ---------------------------------------------------------- delim-col

#[test]
fn delimit_columns_region_and_format() {
    // GNU-verified on 31.1.
    assert_eq!(
        ev("(progn (require 'delim-col)
                  (list (with-temp-buffer
                          (insert \"a,b,c\\n1,22,3\\n\")
                          (delimit-columns-region (point-min) (point-max))
                          (buffer-string))
                        (with-temp-buffer
                          (delimit-columns-format \"  \")
                          (buffer-string))))"),
        "(\"a,b,c \n1,22,3\n\" \",   \")"
    );
}

// ---------------------------------------------------------- cookie1

#[test]
fn cookie_snarf_parses_phrase_file() {
    // GNU-verified: snarfed vector is reversed (last phrase first).
    assert_eq!(
        ev("(progn (require 'cookie1)
                  (write-region \"hdr.\\n%%\\nFirst cookie.\\n%%\\nSecond cookie.\\n%%\\nThird one.\\n%%\\n\"
                                nil \"/tmp/remacs-ck-test.txt\" nil 'quiet)
                  (let ((v (cookie-snarf \"/tmp/remacs-ck-test.txt\")))
                    (list (length v) (aref v 0) (aref v 2))))"),
        "(3 \"Third one.\" \"First cookie.\")"
    );
}

// ---------------------------------------------------------- env

#[test]
fn env_substitute_and_getenv_internal() {
    // GNU-verified on 31.1: unset vars substitute as empty.
    assert_eq!(
        ev("(progn (require 'env)
                  (let ((process-environment '(\"A=1\" \"B=2\")))
                    (list (getenv-internal \"A\" process-environment)
                          (getenv-internal \"ZZZ\" process-environment)
                          (substitute-env-vars \"pre $A mid ${B} end $ZZZ\"))))"),
        "(\"1\" nil \"pre 1 mid 2 end \")"
    );
}

// ---------------------------------------------------------- novice

#[test]
fn disabled_command_routes_to_handler() {
    // GNU-verified on 31.1 batch: `command-execute' on a `disabled'
    // command runs `disabled-command-function' (autoloaded from
    // novice.el), which errors in batch with args-out-of-range.
    assert_eq!(
        ev("(condition-case e
                  (command-execute 'narrow-to-region)
                (error (car e)))"),
        "args-out-of-range"
    );
    // A non-disabled command dispatches normally.
    assert_eq!(ev("(command-execute 'self-insert-command)"), "nil");
    // GNU's dumped `disabled' marks.
    assert_eq!(
        ev("(list (get 'narrow-to-region 'disabled)
                  (get 'upcase-region 'disabled)
                  (get 'scroll-left 'disabled)
                  (get 'scroll-right 'disabled))"),
        "(t t t nil)"
    );
}

// ---------------------------------------------------------- filecache

#[test]
fn file_cache_add_and_delete() {
    // GNU-verified on 31.1.
    assert_eq!(
        ev("(progn (require 'filecache)
                  (write-region \"x\" nil \"/tmp/remacs-fc-test.txt\" nil 'quiet)
                  (file-cache-add-file \"/tmp/remacs-fc-test.txt\")
                  (list (assoc \"remacs-fc-test.txt\" file-cache-alist)
                        (assoc \"no-such\" file-cache-alist)
                        (file-cache-delete-file \"/tmp/remacs-fc-test.txt\")
                        (assoc \"remacs-fc-test.txt\" file-cache-alist)))"),
        "((\"remacs-fc-test.txt\" \"/private/tmp/\") nil nil (\"remacs-fc-test.txt\" \"/private/tmp/\"))"
    );
}

// ---------------------------------------------------------- align

#[test]
fn align_regexp_basic() {
    // GNU-verified on 31.1.
    assert_eq!(
        ev("(progn (require 'align)
                  (with-temp-buffer
                    (insert \"a = 1;\\nbb = 22;\\nccc = 333;\\n\")
                    (align-regexp (point-min) (point-max) \"\\\\(\\\\s-*\\\\)=\" 1 1 nil)
                    (buffer-string)))"),
        "\"a\t= 1;\nbb\t= 22;\nccc\t= 333;\n\""
    );
}

// ---------------------------------------------------------- tabulated-list

#[test]
fn tabulated_list_print_and_sort() {
    // GNU-verified on 31.1: printed columns, column sort, entry id.
    assert_eq!(
        ev("(progn (require 'tabulated-list)
                  (with-temp-buffer
                    (tabulated-list-mode)
                    (setq tabulated-list-format [(\"Name\" 12 t) (\"Size\" 8 t)])
                    (setq tabulated-list-entries
                          '((\"a\" [\"alpha\" \"100\"])
                            (\"b\" [\"beta\" \"2500\"])
                            (\"c\" [\"gamma\" \"30\"])))
                    (tabulated-list-init-header)
                    (tabulated-list-print)
                    (list (buffer-substring-no-properties (point-min) (point-max))
                          (progn (tabulated-list--sort-by-column-name \"Size\")
                                 (tabulated-list-print)
                                 (buffer-substring-no-properties (point-min) (point-max)))
                          (tabulated-list-get-id))))"),
        "(\"alpha        100\nbeta         2500\ngamma        30\n\" \
         \"alpha        100\nbeta         2500\ngamma        30\n\" \"a\")"
    );
}

#[test]
fn bidi_string_mark_left_to_right_appends_lrm() {
    // GNU-verified on 31.1: U+200E appended iff STR has a strong R/AL
    // char, propertized `invisible' like GNU's (prin1 shows props).
    assert_eq!(
        ev("(list (bidi-string-mark-left-to-right \"abc\")
                 (bidi-string-mark-left-to-right \"אבג\")
                 (bidi-string-mark-left-to-right \"aאב\"))"),
        "(\"abc\" #(\"אבג‎\" 3 4 (invisible t)) #(\"aאב‎\" 3 4 (invisible t)))"
    );
}

// ---------------------------------------------------------- tildify

#[test]
fn tildify_region_inserts_hard_space() {
    // GNU-verified on 31.1: the default pattern hardens spaces after
    // single-letter words (I, A, O, ...); "a" is not one of them.
    assert_eq!(
        ev("(progn (require 'tildify)
                  (list (with-temp-buffer
                          (insert \"I think so, ok\")
                          (tildify-region (point-min) (point-max) t)
                          (buffer-string))
                        (with-temp-buffer
                          (insert \"a b  c\")
                          (tildify-region (point-min) (point-max) t)
                          (buffer-string))))"),
        "(\"I\u{a0}think so, ok\" \"a b  c\")"
    );
}

// ---------------------------------------------------------- rot13

#[test]
fn rot13_region_and_string() {
    // GNU-verified on 31.1.
    assert_eq!(
        ev("(progn (require 'rot13)
                  (list (with-temp-buffer
                          (insert \"Hello, World! ABC-xyz\")
                          (rot13-region (point-min) (point-max))
                          (buffer-string))
                        (rot13-string \"Abc Xyz\")
                        (rot13-string (rot13-string \"Roundtrip\"))))"),
        "(\"Uryyb, Jbeyq! NOP-klm\" \"Nop Klm\" \"Roundtrip\")"
    );
}

// ---------------------------------------------------------- soundex

#[test]
fn soundex_known_codes() {
    // GNU-verified on 31.1 (Knuth examples).
    assert_eq!(
        ev("(progn (require 'soundex)
                  (list (soundex \"Robert\") (soundex \"Rupert\")
                        (soundex \"Ashcraft\") (soundex \"Tymczak\")
                        (soundex \"Pfister\") (soundex \"Euler\")
                        (soundex \"A\") (soundex \"a2b3\")))"),
        "(\"R163\" \"R163\" \"A226\" \"T522\" \"P236\" \"E460\" \"A000\" \"A100\")"
    );
}

// ---------------------------------------------------------- hex-util

#[test]
fn hex_util_roundtrip() {
    // GNU-verified on 31.1.
    assert_eq!(
        ev("(progn (require 'hex-util)
                  (list (encode-hex-string \"abc\")
                        (decode-hex-string \"616263\")
                        (decode-hex-string (encode-hex-string \"roundtrip!\"))))"),
        "(\"616263\" \"abc\" \"roundtrip!\")"
    );
}

// ---------------------------------------------------------- password-cache

#[test]
fn password_cache_add_read_remove() {
    // GNU-verified on 31.1. `password-cache-remove' destructively clears the
    // stored string, so compare by content before removal.
    assert_eq!(
        ev("(progn (require 'password-cache)
                  (password-cache-add \"key1\" \"secret\")
                  (list (equal (password-read-from-cache \"key1\") \"secret\")
                        (password-read-from-cache \"missing\")
                        (password-in-cache-p \"key1\")
                        (progn (password-cache-remove \"key1\")
                               (password-in-cache-p \"key1\"))))"),
        "(t nil t nil)"
    );
}

// ---------------------------------------------------------- underline

#[test]
fn underline_region_roundtrip() {
    // GNU-verified on 31.1: hardcopy-style _X sequences.
    assert_eq!(
        ev("(progn (require 'underline)
                  (with-temp-buffer
                    (insert \"abc\")
                    (underline-region (point-min) (point-max))
                    (list (buffer-string)
                          (progn (ununderline-region (point-min) (point-max))
                                 (buffer-string)))))"),
        "(\"_a_b_c\" \"abc\")"
    );
}

// ---------------------------------------------------------- studly

#[test]
fn studlify_region_basic() {
    // GNU-verified on 31.1.
    assert_eq!(
        ev("(progn (require 'studly)
                  (list (with-temp-buffer
                          (insert \"hello world\")
                          (studlify-region (point-min) (point-max))
                          (buffer-string))
                        (with-temp-buffer
                          (insert \"a-b c.d\")
                          (studlify-region (point-min) (point-max))
                          (buffer-string))))"),
        "(\"hello woRld\" \"a-b c.d\")"
    );
}

// ---------------------------------------------------------- gomoku

#[test]
fn gomoku_board_and_score() {
    // GNU-verified on 31.1: index math, qtuples, initial score table.
    assert_eq!(
        ev("(progn (require 'gomoku)
                  (let ((gomoku-board-width 7) (gomoku-board-height 7)
                        (gomoku-vector-length 73))
                    (gomoku-init-board)
                    (gomoku-init-score-table)
                    (list (gomoku-xy-to-index 3 2)
                          (gomoku-index-to-x 19) (gomoku-index-to-y 19)
                          (aref gomoku-board 0)
                          (aref gomoku-score-table (gomoku-xy-to-index 3 3))
                          (gomoku-nb-qtuples 3 3)
                          (gomoku-nb-qtuples 1 1)
                          (gomoku-strongest-square))))"),
        "(19 3 2 -1 70 10 3 36)"
    );
}

// ---------------------------------------------------------- eval-when-compile

#[test]
fn eval_when_compile_evals_when_not_compiling() {
    // GNU: outside the byte compiler, `eval-when-compile' behaves like
    // `progn' (hex-util.el relies on this for its defmacros).
    assert_eq!(
        ev("(progn (require 'hex-util)
                  (list (eval-when-compile (+ 1 2))
                        (fboundp 'num-to-hex-char)))"),
        "(3 t)"
    );
}

// ---------------------------------------------------------- cl-remove keywords

#[test]
fn cl_remove_keyword_semantics() {
    // GNU-verified on 31.1, including the :from-end/:count quirk:
    // GNU takes the from-end path only when `count < len/2' (integer
    // division), so :count 2 on a 5-list removes from the FRONT.
    assert_eq!(
        ev("(progn (require 'cl-lib)
                  (list (cl-remove 1 '(1 2 1 3 1) :if-not (lambda (x) (= x 1)))
                        (cl-remove 1 '(1 2 1 3 1) :from-end t :count 1)
                        (cl-remove 1 '(1 2 1 3 1) :from-end t :count 2)
                        (cl-remove 1 '(1 2 1 3 1) :from-end t :count 3)
                        (cl-remove 1 '(1 2 1 3 1) :start 1 :end 4)
                        (cl-remove \"a\" '(\"a\" \"b\" \"A\") :test #'equal)
                        (cl-remove 1 '(1 2 3) :test-not #'eql)
                        (cl-remove 'x '((x . 1) (y . 2) (x . 3)) :key #'car)
                        (cl-remove-if #'evenp '(1 2 3 4 5 6))
                        (cl-remove-if-not #'evenp '(1 2 3 4 5 6))
                        (cl-delete-if #'evenp '(1 2 3 4 5 6))))"),
        "((1 1 1) (1 2 1 3) (2 3 1) (2 3) (1 2 3 1) (\"b\" \"A\") (1) ((y . 2)) (1 3 5) (2 4 6) (1 3 5))"
    );
}

// ---------------------------------------------------------- cl-flet / cl-labels

#[test]
fn cl_flet_lexical_escape() {
    // GNU cl-flet rewrites local calls to funcall on generated lexical
    // vars; a lambda escaping the body still sees the binding.
    // (eval FORM t) gives a fresh lexical env; under dynamic binding
    // GNU likewise fails to capture, so lexical eval is required.
    assert_eq!(
        ev("(require 'cl-lib)
            (eval '(let ((f (cl-flet ((h (x) (* x 2)))
                             (lambda (y) (h y)))))
                   (funcall f 5)) t)"),
        "10"
    );
}

#[test]
fn cl_labels_recursion() {
    // GNU-verified on 31.1: direct recursion, mutual recursion, and an
    // escaping closure over a recursive local function.
    assert_eq!(
        ev(
            "(require 'cl-lib)
            (eval '(list (cl-labels ((f (n) (if (<= n 1) 1 (* n (f (1- n))))))
                          (f 5))
                        (cl-labels ((ev (n) (if (= n 0) t (od (1- n))))
                                    (od (n) (if (= n 0) nil (ev (1- n)))))
                          (ev 10))
                        (let ((f (cl-labels ((g (n) (if (<= n 0) 0 (+ n (g (1- n))))))
                                   (lambda (k) (g k)))))
                          (funcall f 4))) t)"
        ),
        "(120 t 10)"
    );
}

// ---------------------------------------------------------- macroexpand env

#[test]
fn macroexpand_environment_argument() {
    // GNU `macroexpand-1'/`macroexpand' consult the optional
    // ENVIRONMENT alist before global definitions; a nil definition
    // shadows (stops expansion), an expander is applied to the
    // argument list, and an identical result stops the loop.
    assert_eq!(
        ev(
            "(list (macroexpand-1 '(f a b) '((f . (lambda (a b) `(g ,a ,b)))))
                 (macroexpand-1 '(f a b) '((f . nil)))
                 (macroexpand-1 '(f a b)))"
        ),
        "((g a b) (f a b) (f a b))"
    );
}

// ---------------------------------------------------------- rx-let

#[test]
fn rx_let_local_definitions() {
    // GNU-verified on 31.1: rx-let binds local rx symbols during
    // macro expansion via `macroexpand-all-environment'.
    assert_eq!(
        ev("(progn (require 'rx)
                  (list (let ((r (rx-let ((delim (+ (any \"xy\"))))
                                   (rx (seq bol delim \"z\")))))
                          r)
                        (macroexpand '(rx-let ((delim (+ (any \"xy\"))))
                                        (rx (seq bol delim \"z\"))))))"),
        "(\"^[xy]+z\" (progn \"^[xy]+z\"))"
    );
}

// ---------------------------------------------------------- md4

#[test]
fn md4_known_digests() {
    // GNU-verified on 31.1 (RFC 1320 test vectors).
    assert_eq!(
        ev("(progn (require 'md4) (require 'hex-util)
                  (list (encode-hex-string (md4 \"abc\" 3))
                        (encode-hex-string (md4 \"\" 0))
                        (encode-hex-string (md4 \"abcdefghijklmnopqrstuvwxyz\" 26))))"),
        "(\"a448017aaf21d8525fc10ae87aa6729d\" \"31d6cfe0d16ae931b73c59d7e0c089c0\" \"d79e1c308aa5bbcdeea8ed63df412da9\")"
    );
}

// ---------------------------------------------------------- external-completion

#[test]
fn external_completion_table_cl_flet() {
    // GNU-verified on 31.1: `external-completion-table' returns a
    // closure over a `cl-flet'-bound `lookup-internal' that keeps
    // working after the cl-flet scope has exited.
    assert_eq!(
        ev("(progn (require 'external-completion)
                  (eval '(let ((tbl (external-completion-table
                                     'foo
                                     (lambda (string point)
                                       (cl-remove-if-not
                                         (lambda (s) (string-prefix-p string s))
                                         '(\"alpha\" \"alpine\" \"beta\"))))))
                           (list (funcall tbl \"al\" nil 'lambda)
                                 (funcall tbl \"al\" nil '(external-completion--allc . 2))
                                 (funcall tbl \"al\" nil '(external-completion--tryc . 0))))
                        t))"),
        "(nil (external-completion--allc \"alpha\" \"alpine\") (external-completion--tryc \"al\" . 0))"
    );
}

// ---------------------------------------------------------- case-table / chistory / midnight

#[test]
fn case_table_chistory_midnight_load() {
    // GNU-verified on 31.1: embedded libs load and define their APIs.
    // `describe-buffer-case-table' prints the case table; a fresh
    // temp buffer's is empty.
    assert_eq!(
        ev(
            "(progn (require 'case-table) (require 'chistory) (require 'midnight)
                  (list (with-temp-buffer
                          (describe-buffer-case-table)
                          (buffer-string))
                        (fboundp 'list-command-history)
                        (fboundp 'clean-buffer-list)))"
        ),
        "(\"\" t t)"
    );
}

// ---------------------------------------------------------- elide-head

#[test]
fn elide_head_loads() {
    // GNU-verified on 31.1: elide-head loads (via rx-let) and binds
    // the headers-to-hide alist; `elide-head' leaves a non-matching
    // buffer unchanged.
    assert_eq!(
        ev("(progn (require 'elide-head)
                  (list (consp elide-head-headers-to-hide)
                        (with-temp-buffer
                          (insert \"int main() {}\\n\")
                          (elide-head)
                          (buffer-string))))"),
        "(t \"int main() {}\n\")"
    );
}

// ---------------------------------------------------------- regexp-opt

#[test]
fn regexp_opt_gnu_factoring() {
    // GNU-verified on 31.1: the Lisp regexp-opt (shadowing the Rust
    // subr) factors common prefixes/suffixes, merges single-char
    // branches into charsets, and honors the PAREN variants.
    assert_eq!(
        ev("(list (regexp-opt '(\"abc\" \"abd\"))
                 (regexp-opt '(\"authorization from the X Consortium.\"
                               \"THE USE OR OTHER DEALINGS IN THE SOFTWARE.\"))
                 (regexp-opt '(\"car\" \"cat\" \"cow\" \"dog\"))
                 (regexp-opt '(\"a\" \"ab\" \"abc\"))
                 (regexp-opt '(\"\" \"abc\"))
                 (regexp-opt '(\"a\" \"b\" \"c\" \"d\"))
                 (regexp-opt '(\"abc\" \"xbc\" \"ybc\"))
                 (regexp-opt '(\"foo\" \"bar\") t)
                 (regexp-opt '(\"foo\" \"bar\") 'words)
                 (regexp-opt '(\"foo\" \"bar\") 'symbols)
                 (regexp-opt '(\"foo\" \"bar\") \"\\\\(?1:\")
                 (regexp-opt nil)
                 (regexp-opt '(\"only\")))"),
        "(\"\\\\(?:ab[cd]\\\\)\" \
          \"\\\\(?:\\\\(?:THE USE OR OTHER DEALINGS IN THE SOFTWARE\\\\|authorization from the X Consortium\\\\)\\\\.\\\\)\" \
          \"\\\\(?:c\\\\(?:a[rt]\\\\|ow\\\\)\\\\|dog\\\\)\" \
          \"\\\\(?:a\\\\(?:bc?\\\\)?\\\\)\" \
          \"\\\\(?:abc\\\\)?\" \
          \"[a-d]\" \
          \"\\\\(?:[axy]bc\\\\)\" \
          \"\\\\(bar\\\\|foo\\\\)\" \
          \"\\\\<\\\\(bar\\\\|foo\\\\)\\\\>\" \
          \"\\\\_<\\\\(bar\\\\|foo\\\\)\\\\_>\" \
          \"\\\\(?1:bar\\\\|foo\\\\)\" \
          \"\\\\(?:\\\\`a\\\\`\\\\)\" \
          \"\\\\(?:only\\\\)\")"
    );
}

#[test]
fn regexp_opt_charset_ranges() {
    // GNU-verified on 31.1: ranges condensed, metachars repositioned.
    assert_eq!(
        ev("(list (regexp-opt-charset '(?a ?b ?c))
                 (regexp-opt-charset '(?a ?z ?- ?^ ?\\]))
                 (regexp-opt-charset '(?0 ?1 ?2 ?5 ?6 ?7))
                 (regexp-opt-charset '(?a ?b ?c ?d))
                 (regexp-opt-charset nil))"),
        "(\"[abc]\" \"[]az^-]\" \"[012567]\" \"[a-d]\" \"\\\\`a\\\\`\")"
    );
}

// ---------------------------------------------------------- format-spec / tabify / obarray

#[test]
fn format_spec_flags_and_errors() {
    // GNU-verified on 31.1: flag combos, missing/invalid specs.
    assert_eq!(
        ev("(progn (require 'format-spec)
                  (list (format-spec \"%a %b %s\" '((?a . \"alpha\") (?b . \"beta\") (?s . \"str\")))
                        (format-spec \"%05d %x\" '((?d . 42) (?x . 255)))
                        (condition-case e (format-spec \"%z\" '((?a . 1)))
                          (error e))
                        (condition-case e (format-spec \"%a %u\" '((?a . \"x\")))
                          (error (car e)))
                        (format-spec \"%a %u\" '((?a . \"x\")) t)
                        (format-spec \"%a%%b\" '((?a . \"x\")))
                        (format-spec \"%-8s|\" '((?s . \"mid\")))
                        (format-spec \"%^8s\" '((?s . \"mid\")))
                        (format-spec-make \"foo\" 42 'sym \"str\")))"),
        "(\"alpha beta str\" \"00042 255\" (error \"Invalid format character: ‘%z’\") error \"x %u\" \"x%b\" \"mid     |\" \"     MID\" ((\"foo\" . 42) (sym . \"str\")))"
    );
}

#[test]
fn tabify_untabify_obarray() {
    // GNU-verified on 31.1.
    assert_eq!(
        ev(
            "(progn (require 'tabify) (require 'obarray) (require 'scroll-lock)
                  (list (with-temp-buffer
                          (insert \"a        b\\n\")
                          (untabify (point-min) (point-max))
                          (buffer-string))
                        (with-temp-buffer
                          (insert \"a        b\\n\")
                          (tabify (point-min) (point-max))
                          (buffer-string))
                        (with-temp-buffer
                          (insert \"x   y\\n\")
                          (tabify (point-min) (point-max))
                          (buffer-string))
                        (length (mapatoms (lambda (s) s) (obarray-make 10)))
                        (fboundp 'scroll-lock-mode)))"
        ),
        "(\"a        b\n\" \"a\t b\n\" \"x   y\n\" 0 t)"
    );
}

// --------------------------------------- require feature validation + misc ports

#[test]
fn require_signals_when_feature_not_provided() {
    // GNU-verified on 31.1: userlock.el never calls (provide 'userlock),
    // so `require' must signal after loading the file.
    assert_eq!(
        ev("(condition-case e (require 'userlock)
             (error (list (car e) (featurep 'userlock))))"),
        "(error nil)"
    );
    // And a successful require still returns the feature.
    assert_eq!(ev("(progn (require 'ansi-osc) 'done)"), "done");
}

#[test]
fn keymap_read_only_bind_menu_item() {
    // GNU-verified on 31.1 (keymap.el).
    assert_eq!(
        ev("(list (keymap-read-only-bind #'ignore)
                 (let ((buffer-read-only t)) (keymap--read-only-filter 'cmd))
                 (let ((buffer-read-only nil)) (keymap--read-only-filter 'cmd)))"),
        "((menu-item \"\" ignore :filter keymap--read-only-filter) cmd nil)"
    );
}

#[test]
fn ansi_osc_help_macro_master_ports() {
    // GNU-verified on 31.1.
    assert_eq!(
        ev(
            "(progn (require 'ansi-osc) (require 'help-macro) (require 'master)
                  (list (fboundp 'ansi-osc-apply-on-region)
                        (keymapp ansi-osc-hyperlink-map)
                        (fboundp 'make-help-screen)
                        (fboundp 'master-mode)
                        (fboundp 'master-says)))"
        ),
        "(t t t t t)"
    );
}

#[test]
fn rfn_eshadow_scroll_all_send_to_flow_ctrl() {
    // GNU-verified on 31.1: autoloaded entry points exist; flow-ctrl's
    // functions stay unfbound after load on both sides (parity).
    assert_eq!(
        ev("(progn (require 'rfn-eshadow) (require 'scroll-all)
                  (require 'send-to) (require 'flow-ctrl)
                  (list (fboundp 'rfn-eshadow-setup-minibuffer)
                        (fboundp 'scroll-all-mode)
                        scroll-all-mode
                        (fboundp 'flow-control)
                        (boundp 'flow-control-c-u-s)))"),
        "(t t nil nil nil)"
    );
}

// ------------------------------------------------- rtree / time-stamp / misc ports

#[test]
fn rtree_make_and_memq() {
    // GNU-verified on 31.1: nested (range . (left right)) node shape.
    assert_eq!(
        ev("(progn (require 'rtree)
                  (let ((t1 (rtree-make '(10 . 20))))
                    (list (rtree-memq t1 15)
                          (rtree-memq t1 5)
                          (rtree-memq t1 20)
                          (rtree-memq t1 21))))"),
        "(((10 . 20) nil) nil ((10 . 20) nil) nil)"
    );
}

#[test]
fn time_stamp_format_and_zone_type() {
    // GNU-verified on 31.1.
    assert_eq!(
        ev("(progn (require 'time-stamp)
                  (list (time-stamp-zone-type-p 't)
                        (time-stamp-zone-type-p 'wall)
                        (time-stamp-zone-type-p 5)
                        (time-stamp-zone-type-p \"x\")
                        (time-stamp-string \"%Y-%02m-%02d\" '(26000 0 0))
                        (time-stamp-string \"%H:%02M\" '(0 0 0))
                        (time-stamp-string \"%05d\" '(0 0 0))))"),
        "((t wall) (wall) t t \"2023-12-30\" \"09:00\" \"00001\")"
    );
}

#[test]
fn emacs_lock_bind_key_mouse_copy_t_mouse() {
    // GNU-verified on 31.1: entry points exist; mouse-copy's
    // `mouse-copy-secondary-pasting' is absent on both sides.
    assert_eq!(
        ev("(progn (require 'emacs-lock) (require 'bind-key)
                  (require 'mouse-copy) (require 't-mouse)
                  (list (fboundp 'emacs-lock-mode)
                        (fboundp 'describe-personal-keybindings)
                        (fboundp 'mouse-drag-secondary-pasting)
                        (fboundp 'mouse-copy-secondary-pasting)
                        (fboundp 't-mouse-mode)))"),
        "(t t t nil nil)"
    );
}

// ------------------------------------------------ jka-compr / file-name handlers

#[test]
fn jka_cmpr_hook_is_dumped_and_installs() {
    // GNU-verified on 31.1: jka-cmpr-hook is a dumped feature — its
    // definitions exist at startup and auto-compression-mode installs
    // the file-name handler entry.
    assert_eq!(
        ev("(list (fboundp 'jka-compr-installed-p)
                  auto-compression-mode
                  (consp (jka-compr-installed-p)))"),
        "(t t t)"
    );
}

#[test]
fn find_file_name_handler_filters_by_operations() {
    // GNU-verified on 31.1 (fileio.c): a handler symbol carrying an
    // `operations' property only serves those operations —
    // `jka-compr-handler' answers `insert-file-contents'/`load' but
    // not `file-name-sans-versions'.  `inhibit-file-name-handlers' is
    // not consulted by the subr itself (GNU-verified: result unchanged).
    assert_eq!(
        ev(
            "(list (find-file-name-handler \"foo.gz\" 'insert-file-contents)
                  (find-file-name-handler \"foo.gz\" 'load)
                  (find-file-name-handler \"foo.gz\" 'file-name-sans-versions)
                  (let ((inhibit-file-name-handlers '(jka-compr-handler)))
                    (find-file-name-handler \"foo.gz\" 'insert-file-contents))
                  (file-name-sans-versions \"foo.gz\"))"
        ),
        "(jka-compr-handler jka-compr-handler nil jka-compr-handler \"foo.gz\")"
    );
}

#[test]
fn jka_compr_compression_info() {
    // GNU-verified on 31.1: .gz gets gzip info, .txt gets nil.
    assert_eq!(
        ev("(progn (require 'jka-compr)
                  (list (aref (jka-compr-get-compression-info \"foo.gz\") 2)
                        (jka-compr-get-compression-info \"foo.txt\")))"),
        "(\"gzip\" nil)"
    );
}

#[test]
fn misc_lpr_hl_line_ecomplete_loadhist_entry_points() {
    // GNU-verified on 31.1.
    assert_eq!(
        ev("(progn (require 'misc) (require 'lpr) (require 'hl-line)
                  (require 'loadhist) (require 'help-at-pt)
                  (require 'avoid) (require 'mouse-drag) (require 'visual-wrap)
                  (require 'ecomplete)
                  (let ((ecomplete-database nil))
                    (ecomplete-add-item 'mail \"a@b.com\" \"Alice\")
                    (list (fboundp 'zap-up-to-char)
                          (fboundp 'copy-from-above-command)
                          (fboundp 'lpr-buffer)
                          (fboundp 'hl-line-mode)
                          (fboundp 'global-hl-line-mode)
                          (fboundp 'unload-feature)
                          (fboundp 'feature-symbols)
                          (fboundp 'help-at-pt-string)
                          (fboundp 'mouse-avoidance-mode)
                          (fboundp 'mouse-drag-drag)
                          (fboundp 'visual-wrap-prefix-mode)
                          (nth 3 (ecomplete-get-item 'mail \"a@b.com\")))))"),
        "(t t t t t t t t t t t \"Alice\")"
    );
}

// --------------------------------------------- disp-table / window-x / xt-mouse / tmm

#[test]
fn disp_table_slots_and_makers() {
    // GNU-verified on 31.1.
    assert_eq!(
        ev("(progn (require 'disp-table)
                  (list (fboundp 'standard-display-table)
                        (fboundp 'make-display-table)
                        (fboundp 'display-table-slot)
                        (let ((dt (make-display-table)))
                          (list (display-table-slot dt 'truncation)
                                (display-table-slot dt 'wrap)
                                (display-table-slot dt 'escape)))))"),
        "(nil t t (nil nil nil))"
    );
}

#[test]
fn window_x_xt_mouse_yank_media_tmm() {
    // GNU-verified on 31.1 entry points.
    assert_eq!(
        ev("(progn (require 'window-x) (require 'xt-mouse)
                  (require 'yank-media) (require 'tmm)
                  (list (fboundp 'rotate-window)
                        (fboundp 'window-tree-normal-sizes)
                        (fboundp 'xterm-mouse-mode)
                        (boundp 'xterm-mouse-debug-buffer)
                        (fboundp 'yank-media)
                        (boundp 'yank-media-types)
                        (fboundp 'tmm-menubar)
                        (boundp 'tmm-short-cut-style)))"),
        "(nil t t t t nil t nil)"
    );
}

#[test]
fn text_property_search_prop_match() {
    // GNU-verified on 31.1: forward search returns a prop-match
    // object with beginning/end/value.
    assert_eq!(
        ev("(progn (require 'text-property-search)
                  (with-temp-buffer
                    (insert \"hello\")
                    (put-text-property 1 3 'face 'bold)
                    (goto-char (point-min))
                    (let ((m (text-property-search-forward 'face 'bold t)))
                      (when m
                        (list (prop-match-beginning m)
                              (prop-match-end m)
                              (prop-match-value m))))))"),
        "(1 3 bold)"
    );
}

#[test]
fn bs_entry_points_and_config() {
    // GNU-verified on 31.1.
    assert_eq!(
        ev("(progn (require 'bs)
                  (list (fboundp 'bs-show)
                        (fboundp 'bs--get-name)
                        (boundp 'bs-configurations)
                        (boundp 'bs-default-configuration)
                        (length bs-configurations)
                        bs-default-sort-name
                        (assq 'files bs-configurations)))"),
        "(t t t t 4 \"by nothing\" nil)"
    );
}

#[test]
fn cl_destructuring_bind_dotted_tail() {
    // GNU-verified on 31.1: a dotted pattern tail binds the rest
    // of the list, like `&rest'.
    assert_eq!(
        ev("(require 'cl-lib)
            (list (cl-destructuring-bind (a . b) '(1 2 3) (list a b))
                 (cl-destructuring-bind (a b . c) '(1 2 3 4) (list a b c)))"),
        "((1 (2 3)) (1 2 (3 4)))"
    );
}

#[test]
fn cl_defstruct_accepts_docstring() {
    // GNU-verified on 31.1: `cl-defstruct' accepts a docstring
    // between the name and the slot list.
    assert_eq!(
        ev(
            "(require 'cl-lib)
            (progn (cl-defstruct ec-probe-struct \"A struct doc.\" one (two 2))
                  (let ((s (make-ec-probe-struct :one 1)))
                    (list (ec-probe-struct-one s)
                          (ec-probe-struct-two s)
                          (ec-probe-struct-p s))))"
        ),
        "(1 2 t)"
    );
}

#[test]
fn editorconfig_load_and_fnmatch() {
    // GNU-verified on 31.1: fnmatch returns a match position (0
    // based) or nil.
    assert_eq!(
        ev("(progn (require 'editorconfig)
                  (list (fboundp 'editorconfig-fnmatch-p)
                        (editorconfig-fnmatch-p \"foo.c\" \"*.c\")
                        (editorconfig-fnmatch-p \"foo.h\" \"*.{c,h}\")
                        (editorconfig-fnmatch-p \"a/b/c\" \"a/**\")
                        (editorconfig-fnmatch-p \"deep/x/y.z\" \"**/*.z\")
                        (fboundp 'editorconfig-mode)
                        (boundp 'editorconfig-mode)))"),
        "(t 0 0 0 0 t t)"
    );
}

#[test]
fn editorconfig_tools_and_conf_mode_autoloads() {
    // GNU-verified on 31.1: editorconfig-apply is an autoload at startup
    // (editorconfig-mode-apply is not fbound); conf-*-mode entries are
    // autoloads from conf-mode.el; editorconfig-conf-mode derives from
    // conf-unix-mode.
    assert_eq!(
        ev("(list (fboundp 'editorconfig-apply)
                  (fboundp 'editorconfig-mode-apply)
                  (fboundp 'editorconfig-find-current-editorconfig)
                  (fboundp 'editorconfig-display-current-properties)
                  (autoloadp (symbol-function 'conf-unix-mode)))"),
        "(t nil t t t)"
    );
    assert_eq!(
        ev("(progn (editorconfig-conf-mode)
                  (list major-mode (featurep 'conf-mode)
                        (fboundp 'conf-mode-initialize)))"),
        "(editorconfig-conf-mode t t)"
    );
}

#[test]
fn project_vc_git_detection() {
    // GNU-verified on 31.1: in a Git worktree (the test crate lives in
    // one), project-current returns (vc Git ROOT/) after vc-git loads
    // through vc-hooks' vc-handled-backends chain.
    assert_eq!(
        ev("(progn (require 'project)
                  (let ((p (project-current t)))
                    (list (car p) (nth 1 p)
                          (file-directory-p (nth 2 p))
                          (featurep 'vc-git)
                          (boundp 'vc-use-incoming-outgoing-prefixes))))"),
        "(vc Git t t t)"
    );
    // vc-hooks is preloaded (GNU dumps it): backend list and prefix map.
    assert_eq!(
        ev("(list vc-handled-backends (keymapp 'vc-prefix-map))"),
        "((RCS CVS SVN SCCS SRC Bzr Git Hg) t)"
    );
}

#[test]
fn epg_config_entry_points() {
    // GNU-verified on 31.1.
    assert_eq!(
        ev("(progn (require 'epg-config)
                  (list (fboundp 'epg-find-configuration)
                        (fboundp 'epg-configuration)
                        (fboundp 'epg-check-configuration)
                        (fboundp 'epg-required-version-p)
                        (fboundp 'epg-expand-group)
                        (condition-case e
                            (epg-required-version-p 'OpenPGP \"1.0\")
                          (error (car e)))))"),
        "(t t t t t t)"
    );
}

#[test]
fn array_dos_vars_fringe_font_core_dynamic_setting() {
    // GNU-verified on 31.1.
    assert_eq!(
        ev(
            "(progn (require 'array) (require 'dos-vars) (require 'fringe)
                  (require 'font-core) (require 'dynamic-setting)
                  (list (fboundp 'array-mode)
                        (boundp 'array-mode-map)
                        (array--limit-index 3 5)
                        (array--limit-index -1 5)
                        (array--limit-index 9 5)
                        (get 'array-mode 'mode-class)
                        (fboundp 'fringe-mode)
                        (boundp 'fringes-outside-margins)
                        (boundp 'font-lock-mode)
                        (fboundp 'dynamic-setting-handle-config-changed-event)
                        (boundp 'special-event-map)))"
        ),
        "(t t 3 1 5 special t t t t t)"
    );
}

#[test]
fn double_isearch_map_entry_points() {
    // GNU-verified on 31.1: double.el binds [ignore] on both
    // `double-map' and the dumped `isearch-mode-map'.
    assert_eq!(
        ev("(progn (require 'double)
                  (list (fboundp 'double-mode)
                        (fboundp 'double-translate-key)
                        (fboundp 'double-read-event)
                        (boundp 'double-map)
                        (boundp 'isearch-mode-map)
                        (lookup-key double-map [ignore])
                        (functionp (lookup-key isearch-mode-map [ignore]))))"),
        "(t t t t t nil t)"
    );
}

#[test]
fn generator_iter_defun_yields() {
    // GNU-verified on 31.1: iter-defun generators yield in order and
    // signal `iter-end-of-sequence' when exhausted.
    assert_eq!(
        ev("(progn (require 'generator)
                  (eval '(progn
                           (iter-defun cnt3 (n) (dotimes (i n) (iter-yield i)))
                           (let ((it (cnt3 3)))
                             (list (iter-next it) (iter-next it) (iter-next it)
                                   (condition-case e (iter-next it)
                                     (iter-end-of-sequence 'done)))))
                        t))"),
        "(0 1 2 done)"
    );
}

#[test]
fn fileloop_entry_points() {
    // GNU-verified on 31.1.
    assert_eq!(
        ev("(progn (require 'fileloop)
                  (list (fboundp 'fileloop-initialize)
                        (fboundp 'fileloop-continue)
                        (fboundp 'fileloop-initialize-replace)
                        (fboundp 'fileloop-next-file)))"),
        "(t t t t)"
    );
}

#[test]
fn display_fill_column_indicator_entry_points() {
    // GNU-verified on 31.1: the library provides the minor modes; the
    // buffer-local variable itself is a C-side binding.
    assert_eq!(
        ev("(progn (require 'display-fill-column-indicator)
                  (list (fboundp 'display-fill-column-indicator-mode)
                        (fboundp 'global-display-fill-column-indicator-mode)
                        (boundp 'display-fill-column-indicator)
                        display-fill-column-indicator
                        (local-variable-if-set-p 'display-fill-column-indicator)))"),
        "(t t t nil t)"
    );
}

#[test]
fn widget_w32_vars_entry_points() {
    // GNU-verified on 31.1: widget.el is dumped (define-widget); w32-vars
    // defines its shell list on all non-cygwin systems.
    assert_eq!(
        ev("(progn (require 'w32-vars)
                  (list (fboundp 'define-widget)
                        (boundp 'w32-system-shells)
                        (car w32-system-shells)
                        (progn (define-widget 'my-w 'editable-field \"d\")
                               (get 'my-w 'widget-documentation))))"),
        "(t t \"cmd\" \"d\")"
    );
}

#[test]
fn tool_bar_and_version_loaded() {
    // GNU-verified on 31.1: tool-bar/widget/version are loaded by loadup;
    // `version' has no provide, so `require' errors like GNU.
    assert_eq!(
        ev("(list (boundp 'tool-bar-map)
                  (fboundp 'tool-bar-mode)
                  (fboundp 'tool-bar-local-item-from-menu)
                  (fboundp 'emacs-repository-get-branch)
                  (boundp 'emacs-repository-version)
                  (condition-case e (progn (require 'version) 'loaded)
                    (error (car e))))"),
        "(t t t t t error)"
    );
}

#[test]
fn reposition_reveal_dos_fns_sqlite_entry_points() {
    // GNU-verified on 31.1: require loads each feature and binds its
    // entry points (autoload-style in GNU's dump).
    assert_eq!(
        ev("(progn (require 'reposition)
                  (list (fboundp 'reposition-window)
                        (fboundp 'repos-count-screen-lines)))"),
        "(t t)"
    );
    assert_eq!(
        ev("(progn (require 'reveal)
                  (list (fboundp 'reveal-mode)
                        (boundp 'reveal-open-map)))"),
        "(t nil)"
    );
    assert_eq!(
        ev("(progn (require 'dos-fns)
                  (list (fboundp 'dos-mode25)
                        (fboundp 'dos-8+3-filename)))"),
        "(t t)"
    );
    assert_eq!(
        ev("(progn (require 'sqlite)
                  (list (featurep 'sqlite)
                        (fboundp 'sqlite-open)
                        (boundp 'sqlite--allowed-words)))"),
        "(t t nil)"
    );
}

#[test]
fn tty_tip_entry_points() {
    // GNU-verified on 31.1: require 'tty-tip provides the feature and
    // binds the minor mode.
    assert_eq!(
        ev("(progn (require 'tty-tip)
                  (list (featurep 'tty-tip)
                        (fboundp 'tty-tip-mode)
                        (boundp 'tty-tip-mode)))"),
        "(t t t)"
    );
}

#[test]
fn xdg_entry_points() {
    // GNU-verified on 31.1: `rx' is bound at -Q (loadup-loaded but
    // unprovided), so `rx'-using libraries expand at load; xdg defines
    // the base-dir helpers while the user-dir helpers stay unbound.
    assert_eq!(ev("(list (fboundp 'rx) (featurep 'rx))"), "(t nil)");
    assert_eq!(
        ev("(progn (require 'xdg)
                  (list (featurep 'xdg)
                        (fboundp 'xdg-data-home)
                        (fboundp 'xdg-config-home)
                        (fboundp 'xdg-cache-home)
                        (fboundp 'xdg-state-home)
                        (fboundp 'xdg-runtime-dir)
                        (fboundp 'xdg-user-dir)
                        (fboundp 'xdg-user-dirs)))"),
        "(t t t t t t t nil)"
    );
    // XDG_DATA_HOME/CONFIG_HOME unset: falls back to ~/.local/share etc.
    assert_eq!(
        ev("(progn (require 'xdg)
                  (list (string-suffix-p \"/.local/share\" (xdg-data-home))
                        (string-suffix-p \"/.cache\" (xdg-cache-home))))"),
        "(t t)"
    );
}

#[test]
fn sqlite_mode_entry_points() {
    // GNU-verified on 31.1: sqlite-mode.el is not dumped; require 'sqlite-mode
    // defines the mode and its keymap but not the --db/--tables internals.
    assert_eq!(
        ev("(progn (require 'sqlite-mode)
                  (list (featurep 'sqlite-mode)
                        (fboundp 'sqlite-mode)
                        (fboundp 'sqlite-mode-list-tables)
                        (fboundp 'sqlite-mode-delete)
                        (boundp 'sqlite-mode-tables)
                        (boundp 'sqlite-mode--db)
                        (boundp 'sqlite-mode-map)
                        (keymapp sqlite-mode-map)))"),
        "(t t t t nil nil t t)"
    );
}

#[test]
fn fillarray_covers_all_array_kinds() {
    // GNU-verified on 31.1: fillarray handles vectors, char-tables
    // (top slots + defalt + extras; ASCII slot & parent preserved),
    // bool-vectors and strings.
    assert_eq!(ev("(fillarray (vector 1 2) 9)"), "[9 9]");
    // char-table: ASCII chars keep their entries, the rest take ITEM.
    assert_eq!(
        ev("(let ((ct (make-char-table 'test nil)))
             (set-char-table-range ct '(65 . 100) 'a)
             (set-char-table-parent ct (make-char-table 'test nil))
             (fillarray ct 'X)
             (list (aref ct 97) (aref ct 2000)
                   (char-table-range ct nil)
                   (char-table-p (char-table-parent ct))))"),
        "(a X X t)"
    );
    assert_eq!(
        ev("(let ((b (make-bool-vector 3 nil)))
             (fillarray b 'x) (list (aref b 0) (aref b 2)))"),
        "(t t)"
    );
    assert_eq!(ev("(fillarray \"abc\" ?z)"), "\"zzz\"");
    assert_eq!(
        ev("(condition-case e (fillarray 5 1) (error (car e)))"),
        "wrong-type-argument"
    );
}

#[test]
fn isearch_x_ibuf_macs_entry_points() {
    // GNU-verified on 31.1: neither file provides a feature, so
    // `require' signals (matching GNU) while `load' binds the defs.
    assert_eq!(
        ev("(condition-case e (progn (require 'isearch-x) 'ok)
             (error (car e)))"),
        "error"
    );
    assert_eq!(
        ev("(progn (load \"isearch-x\" nil t)
                  (fboundp 'isearch-define-mode-toggle))"),
        "t"
    );
    assert_eq!(
        ev("(progn (require 'ibuf-macs)
                  (list (featurep 'ibuf-macs)
                        (fboundp 'ibuffer-aif)
                        (fboundp 'define-ibuffer-column)
                        (fboundp 'define-ibuffer-sorter)
                        (fboundp 'define-ibuffer-op)
                        (fboundp 'define-ibuffer-filter)))"),
        "(t t t t t t)"
    );
}

#[test]
fn w32_fns_and_ebuff_menu_entry_points() {
    // GNU-verified on 31.1: w32-fns.el loads but defines nothing on
    // non-w32 hosts (and provides no feature); ebuff-menu provides
    // Electric-buffer-menu.
    assert_eq!(
        ev("(progn (load \"w32-fns\" nil t)
                  (list (fboundp 'w32-get-clipboard-data)
                        (fboundp 'w32-shell-execute)))"),
        "(nil nil)"
    );
    assert_eq!(
        ev("(progn (require 'ebuff-menu)
                  (list (featurep 'ebuff-menu)
                        (fboundp 'electric-buffer-list)
                        (fboundp 'electric-buffer-menu-mode)
                        (keymapp electric-buffer-menu-mode-map)))"),
        "(t t t t)"
    );
}

#[test]
fn games_and_misc_tooling_entry_points() {
    // GNU-verified on 31.1: all twelve libraries load byte-identically
    // and expose the same entry points.
    assert_eq!(
        ev("(progn (require 'zone)
                  (list (featurep 'zone) (fboundp 'zone)
                        (fboundp 'zone-when-idle)))"),
        "(t t t)"
    );
    assert_eq!(
        ev("(progn (require 'tooltip)
                  (list (featurep 'tooltip) (fboundp 'tooltip-mode)
                        (boundp 'tooltip-delay)))"),
        "(t t t)"
    );
    assert_eq!(
        ev("(progn (require 'ogonek)
                  (list (featurep 'ogonek) (fboundp 'ogonek-jak)
                        (fboundp 'ogonek-how)
                        (fboundp 'ogonek-lookup-encoding)))"),
        "(t t t t)"
    );
    assert_eq!(
        ev("(progn (require 'kinsoku)
                  (list (featurep 'kinsoku) (boundp 'kinsoku-limit)))"),
        "(t t)"
    );
    assert_eq!(
        ev("(progn (require 'utf-7)
                  (list (featurep 'utf-7) (fboundp 'utf-7-encode)
                        (fboundp 'utf-7-decode)))"),
        "(t t t)"
    );
    assert_eq!(
        ev("(progn (require 'mail-utils)
                  (list (featurep 'mail-utils)
                        (fboundp 'mail-strip-quoted-names)
                        (fboundp 'mail-file-babyl-p)))"),
        "(t t t)"
    );
    assert_eq!(
        ev("(progn (require 'dissociate)
                  (list (featurep 'dissociate)
                        (fboundp 'dissociated-press)))"),
        "(t t)"
    );
    assert_eq!(
        ev("(progn (require 'fortune)
                  (list (featurep 'fortune) (fboundp 'fortune)
                        (fboundp 'fortune-message)))"),
        "(t t t)"
    );
    assert_eq!(
        ev("(progn (require 'gamegrid)
                  (list (featurep 'gamegrid) (fboundp 'gamegrid-init)
                        (fboundp 'gamegrid-set-cell)))"),
        "(t t t)"
    );
    assert_eq!(
        ev("(progn (require 'morse)
                  (list (featurep 'morse) (fboundp 'morse-region)
                        (fboundp 'unmorse-region)))"),
        "(t t t)"
    );
    assert_eq!(
        ev("(progn (require 'spook)
                  (list (featurep 'spook) (fboundp 'spook)
                        (fboundp 'snarf-spooks)
                        (boundp 'spook-phrases-file)))"),
        "(t t t t)"
    );
    // ls-lisp provides its own insert-directory, same as GNU.
    assert_eq!(
        ev("(progn (require 'ls-lisp)
                  (list (featurep 'ls-lisp)
                        (boundp 'ls-lisp-use-insert-directory-program)
                        (fboundp 'insert-directory)))"),
        "(t t t)"
    );
}

#[test]
fn mail_quote_printable_emits_rfc2047_safe_words() {
    assert_eq!(
        ev(r#"(progn
              (require 'mail-utils)
              (let* ((prefix "=?ISO-8859-1?Q?")
                     (specials (concat "x" (string 0 10 13 127) "y"))
                     (wrapped (mail-quote-printable specials t))
                     (long-words
                      (split-string
                       (mail-quote-printable (make-string 200 ?a) t)
                       (string 10 32) t)))
                (and
                 (equal (mail-quote-printable "a\nb" t)
                        "=?ISO-8859-1?Q?a=0Ab?=")
                 (equal wrapped
                        "=?ISO-8859-1?Q?x=00=0A=0D=7Fy?=")
                 (equal (mail-quote-printable (concat (string #xe9) "=") t)
                        "=?ISO-8859-1?Q?=E9=3D?=")
                 (equal (mail-quote-printable specials)
                        "x=00=0A=0D=7Fy")
                 (equal (mail-quote-printable (concat "word" (string 32 9 10)))
                        "word=20=09=0A")
                 (equal (mail-quote-printable "" t) "")
                 (= (length long-words) 4)
                 (not (memq nil
                            (mapcar (lambda (word)
                                      (and (<= (length word) 75)
                                           (string-prefix-p prefix word)
                                           (string-suffix-p "?=" word)))
                                    long-words)))
                 (with-temp-buffer
                   (insert "a\nb")
                   (mail-quote-printable-region
                    (point-min) (point-max) t)
                   (equal (buffer-string) "=?ISO-8859-1?Q?a=0Ab?=")))))"#),
        "t"
    );
}

#[test]
fn url_parse_query_string_discards_empty_fields() {
    assert_eq!(
        ev(r#"(progn
              (require 'url-util)
              (list (url-parse-query-string "")
                    (url-build-query-string (url-parse-query-string ""))
                    (url-parse-query-string "&")
                    (url-parse-query-string "a=1&&b=2")
                    (url-parse-query-string "=x")))"#),
        "(nil \"\" nil ((\"a\" \"1\") (\"b\" \"2\")) ((\"\" \"x\")))"
    );
}

#[test]
fn binhex_external_decoder_confines_header_filename() {
    assert_eq!(
        ev(r#"(progn
              (require 'cl-lib)
              (require 'binhex)
              (let* ((root (make-temp-file "qa-binhex-path-" t))
                     (temporary-file-directory
                      (file-name-as-directory (expand-file-name "tmp" root)))
                     (outside (expand-file-name "outside.data" root)))
                (make-directory temporary-file-directory)
                (unwind-protect
                    (progn
                      (with-temp-file outside (insert "protected"))
                      (with-temp-buffer
                        (insert (concat ":" (make-string 63 ?a) "\n"))
                        (cl-letf (((symbol-function 'binhex-decode-region-internal)
                                   (lambda (&rest _) "../outside"))
                                  ((symbol-function 'call-process-region)
                                   (lambda (&rest _)
                                     (with-temp-file "outside.data"
                                       (insert "decoded"))
                                     0)))
                          (binhex-decode-region-external (point-min) (point-max))
                          (and (equal (buffer-string) "decoded")
                               (equal (with-temp-buffer
                                        (insert-file-contents-literally outside)
                                        (buffer-string))
                                      "protected")
                               (not (file-exists-p
                                     (expand-file-name "outside.data"
                                                       temporary-file-directory)))))))
                  (delete-directory root t))))"#),
        "t"
    );
}

#[test]
fn register_configuration_points_survive_buffer_kill() {
    assert_eq!(
        ev(r#"(progn
              (require 'cl-lib)
              (require 'register)
              (let* ((file (make-temp-file "qa-register-marker-"))
                     (register-alist nil))
                (unwind-protect
                    (progn
                      (with-temp-file file (insert "0123456789"))
                      (let ((buffer (find-file-noselect file)))
                        (with-current-buffer buffer
                          (goto-char 4)
                          (point-to-register ?p t)
                          (window-configuration-to-register ?w)
                          (frame-configuration-to-register ?f))
                        (kill-buffer buffer))
                      (let ((expected (list 'file-query file 4))
                            (restored t))
                        (and
                         (equal (cadr (get-register ?p)) expected)
                         (equal (cadr (get-register ?w)) expected)
                         (equal (cadr (get-register ?f)) expected)
                         (cl-letf (((symbol-function 'set-frame-configuration)
                                    (lambda (&rest _) nil))
                                   ((symbol-function 'set-window-configuration)
                                    (lambda (&rest _) nil))
                                   ((symbol-function 'y-or-n-p)
                                    (lambda (&rest _) t)))
                           (dolist (register '(?p ?w ?f))
                             (jump-to-register register)
                             (unless (and (equal (buffer-file-name) file)
                                          (= (point) 4))
                               (setq restored nil)))
                           restored))))
                  (when (get-file-buffer file)
                    (kill-buffer (get-file-buffer file)))
                  (delete-file file))))"#),
        "t"
    );
}

#[test]
fn bindat_unpack_rejects_truncated_fixed_strings() {
    assert_eq!(
        ev(r#"(progn
              (require 'bindat)
              (let ((str-error
                     (condition-case err
                         (progn
                           (bindat-unpack '((payload str 4))
                                          (unibyte-string ?A))
                           'no-error)
                       (error (car err))))
                    (strz-error
                     (condition-case err
                         (progn
                           (bindat-unpack '((name strz 4))
                                          (unibyte-string ?A 0))
                           'no-error)
                       (error (car err)))))
                (list str-error strz-error
                      (bindat-unpack '((payload str 4))
                                     (unibyte-string ?A ?B ?C ?D))
                      (bindat-unpack '((name strz 4))
                                     (unibyte-string ?A 0 ?B ?C)))))"#),
        "(error error ((payload . \"ABCD\")) ((name . \"A\")))"
    );
}

#[test]
fn bindat_pack_preserves_field_sizes() {
    assert_eq!(
        ev(r#"(progn
              (require 'bindat)
              (list
               (string-to-list
                (bindat-pack '((a u16) (b u8)) '((a . nil) (b . 1))))
               (string-to-list
                (bindat-pack '((a vec 2 u8) (b u8)) '((a . [1]) (b . 2))))
               (string-to-list
                (bindat-pack '((a vec 2 u16) (b u8)) '((a . nil) (b . 1))))
               (string-to-list
                (bindat-pack '((name strz)) '((name . "abc"))))
               (string-to-list
                (bindat-pack '((name strz)) '((name . nil))))))"#),
        "((0 0 1) (1 0 2) (0 0 0 0 1) (97 98 99 0) (0))"
    );
}

#[test]
fn bindat_pack_rejects_embedded_nul_in_fixed_strz() {
    assert_eq!(
        ev(r#"(progn
              (require 'bindat)
              (let ((raw (make-string 5 ?x)))
                (list
                 (condition-case err
                     (progn
                       (bindat-pack '((name strz 5)) '((name . "a\0b")) raw)
                       'no-error)
                   (error (car err)))
                 (string-to-list raw))))"#),
        "(error (120 120 120 120 120))"
    );
}

#[test]
fn bindat_uint_rejects_non_byte_aligned_bit_lengths() {
    assert_eq!(
        ev(r#"(progn
              (require 'bindat)
              (let* ((spec (bindat-type (struct (value uint 12))))
                     (data '((value . 2748)))
                     (valid-spec (bindat-type uint 16))
                     (valid-data 2748))
                (list
                 (condition-case err
                     (bindat-length spec data)
                   (error (car err)))
                 (condition-case err
                     (string-to-list (bindat-pack spec data))
                   (error (car err)))
                 (condition-case err
                     (bindat-unpack spec (unibyte-string #x0a #xbc))
                   (error (car err)))
                 (list (bindat-length valid-spec valid-data)
                       (string-to-list (bindat-pack valid-spec valid-data))
                       (bindat-unpack valid-spec (unibyte-string #x0a #xbc))))))"#),
        "(error error error (2 (10 188) 2748))"
    );
}

#[test]
fn nslookup_helpers_pass_untrusted_names_as_argv() {
    assert_eq!(
        ev(r#"(progn
              (require 'net-utils)
              (let ((calls nil)
                    (host "example.invalid; echo injected")
                    (name-server "ns; echo injected")
                    (nslookup-program "nslookup-test"))
                (cl-letf (((symbol-function 'process-file)
                           (lambda (program _infile _destination _display &rest args)
                             (setq calls (cons (cons program args) calls))
                             (insert
                              (if (equal (car args) "-type=A")
                                  "Name: example.invalid\nAddress: 192.0.2.1\n"
                                "example.invalid has AAAA address 2001:db8::1\n"))
                             0))
                          ((symbol-function 'shell-command-to-string)
                           (lambda (&rest _) (error "unexpected shell invocation"))))
                  (list (nslookup-host-ipv4 host name-server)
                        (nslookup-host-ipv6 host name-server)
                        (nreverse calls)))))"#),
        "(\"192.0.2.1\" \"2001:db8::1\" ((\"nslookup-test\" \"-type=A\" \"example.invalid; echo injected\" \"ns; echo injected\") (\"nslookup-test\" \"-type=AAAA\" \"example.invalid; echo injected\" \"ns; echo injected\")))"
    );
}

#[test]
fn tramp_kubernetes_completion_passes_context_as_argv() {
    assert_eq!(
        ev(r#"(progn
              (require 'tramp-container)
              (let ((calls nil)
                    (tramp-kubernetes-context "prod; echo injected")
                    (tramp-kubernetes-namespace "ns; echo injected"))
                (cl-letf (((symbol-function 'tramp-expand-args)
                           (lambda (&rest _) "kubectl-test"))
                          ((symbol-function 'process-file)
                           (lambda (program _infile _destination _display &rest args)
                             (setq calls (cons (cons program args) calls))
                             0))
                          ((symbol-function 'shell-command-to-string)
                           (lambda (&rest _) (error "unexpected shell invocation"))))
                  (tramp-kubernetes--completion-function "kubernetes"))
                (let* ((call (car (nreverse calls)))
                       (args (cdr call)))
                  (list (equal (car call) "kubectl-test")
                        (equal (nth 0 args) "--context=prod; echo injected")
                        (equal (nth 1 args) "--namespace=ns; echo injected")
                        (equal (nth 2 args) "get")
                        (equal (nth 3 args) "pods")
                        (equal (nth 4 args) "--no-headers")
                        (equal (nth 5 args) "-o")
                        (and (= (length args) 7)
                             (string-prefix-p "jsonpath=" (nth 6 args)))))))"#),
        "(t t t t t t t t)"
    );
}

#[test]
fn tramp_kubernetes_context_data_quotes_context_for_jsonpath() {
    assert_eq!(
        ev(r#"(progn
              (require 'tramp-container)
              (let* ((context "my\"\\context\\")
                     (expected-quoted "\"my\\\"\\u005ccontext\\u005c\"")
                     (expected-jsonpath
                      (concat "jsonpath={.contexts[?(@.name == "
                              expected-quoted ")]}"))
                     (calls nil)
                     (tramp-kubernetes-program "kubectl-test"))
                (cl-letf (((symbol-function 'tramp-kubernetes--current-context)
                           (lambda (_vec) context))
                          ((symbol-function 'process-file)
                           (lambda (program _infile _destination _display &rest args)
                             (setq calls (cons (cons program args) calls))
                             (insert "{}")
                             0))
                          ((symbol-function 'shell-command-to-string)
                           (lambda (&rest _) (error "unexpected shell invocation"))))
                  (tramp-kubernetes--current-context-data nil))
                (let ((call (car calls)))
                  (list (equal (tramp-kubernetes--jsonpath-quote-string context)
                               expected-quoted)
                        (equal (car call) "kubectl-test")
                        (equal (cdr call)
                               (list "config" "view" "-o" expected-jsonpath))))))"#),
        "(t t t)"
    );
}

#[test]
fn smtpmail_requires_354_before_sending_message_data() {
    assert_eq!(
        ev(r#"(progn
              (require 'smtpmail)
              (let ((trace-buffer nil)
                    (responses '((250 "250 sender accepted")
                                 (250 "250 recipient accepted")
                                 (250 "250 invalid DATA response")
                                 (221 "221 goodbye")))
                    (commands nil)
                    (data-sent nil)
                    (smtpmail-smtp-server "smtp.example")
                    (smtpmail-smtp-service 25)
                    (smtpmail-mail-address "sender@example.com")
                    (smtpmail-debug-info t))
                (unwind-protect
                    (with-temp-buffer
                      (insert "From: sender@example.com\nTo: recipient@example.com\n\nbody\n")
                      (let ((result
                             (cl-letf (((symbol-function 'open-network-stream)
                                        (lambda (_name buffer &rest _)
                                          (setq trace-buffer buffer)
                                          (cons 'smtp-test-process
                                                '(:greeting "220 smtp.example ready\r\n"
                                                  :capabilities "250 HELP\r\n"))))
                                       ((symbol-function 'set-process-filter)
                                        (lambda (&rest _) nil))
                                       ((symbol-function 'set-process-coding-system)
                                        (lambda (&rest _) nil))
                                       ((symbol-function 'process-buffer)
                                        (lambda (&rest _) trace-buffer))
                                       ((symbol-function 'delete-process)
                                        (lambda (&rest _) nil))
                                       ((symbol-function 'message-narrow-to-headers)
                                        (lambda (&rest _) nil))
                                       ((symbol-function 'smtpmail-fqdn)
                                        (lambda () "client.example"))
                                       ((symbol-function 'smtpmail-try-auth-methods)
                                        (lambda (&rest _) nil))
                                       ((symbol-function 'smtpmail-send-command)
                                        (lambda (_process command)
                                          (push command commands)))
                                       ((symbol-function 'smtpmail-read-response)
                                        (lambda (_process)
                                          (prog1 (car responses)
                                            (setq responses (cdr responses)))))
                                       ((symbol-function 'smtpmail-send-data)
                                        (lambda (&rest _) (setq data-sent t))))
                               (smtpmail-via-smtp
                                '("recipient@example.com") (current-buffer)))))
                        (list result data-sent (nreverse commands))))
                  (when (buffer-live-p trace-buffer)
                    (kill-buffer trace-buffer)))))"#),
        "(\"250 invalid DATA response in response to DATA\" nil (\"MAIL FROM:<sender@example.com>\" \"RCPT TO:<recipient@example.com>\" \"DATA\" \"QUIT\"))"
    );
}

#[test]
fn smtpmail_rejects_overlong_lines_before_data() {
    assert_eq!(
        ev(r#"(progn
              (require 'smtpmail)
              (let* ((smtpmail-code-conv-from 'utf-8-unix)
                     (direct-sent nil)
                     (direct-result
                      (with-temp-buffer
                        (condition-case err
                            (cl-letf (((symbol-function 'process-send-string)
                                       (lambda (_process data)
                                         (push data direct-sent)))
                                      ((symbol-function 'message-narrow-to-headers)
                                       (lambda (&rest _) nil)))
                              (smtpmail-send-data-1 nil (make-string 500 ?é))
                              nil)
                          (error err))))
                     (valid-sent nil)
                     (valid-result
                      (with-temp-buffer
                        (cl-letf (((symbol-function 'process-send-string)
                                   (lambda (_process data)
                                     (push data valid-sent)))
                                  ((symbol-function 'message-narrow-to-headers)
                                   (lambda (&rest _) nil)))
                          (smtpmail-send-data-1 nil (make-string 998 ?a))
                          t)))
                     (responses '((250 "250 sender accepted")
                                  (250 "250 recipient accepted")
                                  (354 "354 Start mail input")
                                  (221 "221 goodbye")))
                     (commands nil)
                     (network-opened nil)
                     (data-sent nil)
                     (trace-buffer nil)
                     (smtpmail-smtp-server "smtp.example")
                     (smtpmail-smtp-service 25)
                     (smtpmail-mail-address "sender@example.com")
                     (smtpmail-debug-info t)
                     (smtp-result
                      (unwind-protect
                          (with-temp-buffer
                            (insert "From: sender@example.com\nTo: recipient@example.com\n\n"
                                    (make-string 999 ?a))
                            (condition-case err
                                (cl-letf (((symbol-function 'open-network-stream)
                                           (lambda (_name buffer &rest _)
                                             (setq network-opened t
                                                   trace-buffer buffer)
                                             (cons 'smtp-test-process
                                                   '(:greeting "220 smtp.example ready\r\n"
                                                     :capabilities "250 HELP\r\n"))))
                                          ((symbol-function 'set-process-filter)
                                           (lambda (&rest _) nil))
                                          ((symbol-function 'set-process-coding-system)
                                           (lambda (&rest _) nil))
                                          ((symbol-function 'process-buffer)
                                           (lambda (&rest _) trace-buffer))
                                          ((symbol-function 'delete-process)
                                           (lambda (&rest _) nil))
                                          ((symbol-function 'message-narrow-to-headers)
                                           (lambda (&rest _) nil))
                                          ((symbol-function 'smtpmail-fqdn)
                                           (lambda () "client.example"))
                                          ((symbol-function 'smtpmail-try-auth-methods)
                                           (lambda (&rest _) nil))
                                          ((symbol-function 'smtpmail-send-command)
                                           (lambda (_process command)
                                             (push command commands)))
                                          ((symbol-function 'smtpmail-read-response)
                                           (lambda (_process)
                                             (prog1 (car responses)
                                               (setq responses (cdr responses)))))
                                          ((symbol-function 'smtpmail-send-data)
                                           (lambda (&rest _) (setq data-sent t))))
                                  (smtpmail-via-smtp
                                   '("recipient@example.com") (current-buffer)))
                              (error err)))
                        (when (buffer-live-p trace-buffer)
                          (kill-buffer trace-buffer)))))
                (list (eq (car direct-result) 'user-error)
                      (null direct-sent)
                      (eq valid-result t)
                      (equal (mapcar #'string-bytes (nreverse valid-sent)) '(998 2))
                      (eq (car smtp-result) 'user-error)
                      (not network-opened)
                      (not (member "DATA" commands))
                      (not data-sent))))"#),
        "(t t t t t t t t)"
    );
}

#[test]
fn smtpmail_requires_three_digit_reply_codes() {
    assert_eq!(
        ev(r#"(progn
              (require 'smtpmail)
              (let ((lines '("25 OK\r\n"
                             "2500 OK\r\n"
                             "25-First\r\n25 Done\r\n"
                             "250-First\r\n2500 Done\r\n"
                             "250 OK\r\n"
                             "250-First\r\n250 Done\r\n")))
                (list
                 (mapcar
                  (lambda (line)
                    (with-temp-buffer
                      (insert line)
                      (let* ((smtpmail-read-point (point-min))
                             (response (smtpmail-read-response nil)))
                        (list (car response) (smtpmail-ok-p response)))))
                  lines)
                 (mapcar #'smtpmail-response-code
                         '("25 OK\r\n" "2500 OK\r\n" "250 OK\r\n")))))"#),
        "(((nil nil) (nil nil) (nil nil) (nil nil) (250 t) (250 t)) (nil nil 250))"
    );
}

#[test]
fn timeclock_splits_overnight_intervals_by_date() {
    assert_eq!(
        ev(r#"(progn
              (require 'timeclock)
              (let ((file (make-temp-file "timeclock-overnight"))
                    (timeclock-workday (* 9 3600.0)))
                (unwind-protect
                    (progn
                      (with-temp-file file
                        (insert "h 2025/01/01 00:00:00 8\n"
                                "i 2025/01/01 23:00:00 project\n"
                                "o 2025/01/02 01:00:00\n"))
                      (let* ((data (timeclock-log-data nil file))
                             (days (nreverse (timeclock-day-alist data))))
                        (list
                         (mapcar (lambda (day)
                                   (list (car day)
                                         (timeclock-day-length (cdr day))
                                         (timeclock-day-required (cdr day))))
                                 days)
                         (timeclock-entry-list-length
                          (cdr (assoc "project"
                                      (timeclock-project-alist data)))))))
                  (delete-file file))))"#),
        "(((\"2025/01/01\" 3600.0 28800.0) (\"2025/01/02\" 3600.0 32400.0)) 7200.0)"
    );
}

#[test]
fn bookmark_rename_rejects_empty_name_atomically() {
    assert_eq!(
        ev(r#"(progn
              (require 'bookmark)
              (let ((bookmark-alist
                     '(("target" (filename . "/tmp/target"))))
                    (bookmark-alist-modification-count 0)
                    (bookmark-current-bookmark "target"))
                (cl-letf (((symbol-function 'bookmark-maybe-load-default-file)
                           (lambda () nil)))
                  (let ((error-symbol
                         (condition-case err
                             (progn (bookmark-rename "target" "") 'no-error)
                           (error (car err)))))
                    (list error-symbol
                          (bookmark-all-names)
                          bookmark-current-bookmark
                          bookmark-alist-modification-count)))))"#),
        "(user-error (\"target\") \"target\" 0)"
    );
}

#[test]
fn abbrev_suggest_matches_punctuation_inside_expansion() {
    assert_eq!(
        ev(r#"(progn
              (require 'abbrev)
              (let ((global-abbrev-table (make-abbrev-table))
                    (local-abbrev-table nil)
                    (abbrev-suggest-hint-threshold 0)
                    captured)
                (define-abbrev global-abbrev-table "fb" "foo-bar")
                (fset 'abbrev--suggest-inform-user
                      (lambda (expansion)
                        (push (cons (nth 0 expansion) (nth 1 expansion))
                              captured)))
                (with-temp-buffer
                  (insert "foo-bar")
                  (abbrev--suggest-maybe-suggest))
                (list (length captured)
                      (caar captured)
                      (cdar captured))))"#),
        "(1 \"foo-bar\" \"fb\")"
    );
}

#[test]
fn ecomplete_returns_a_multiline_candidate_as_one_match() {
    assert_eq!(
        ev(r#"(progn
              (require 'ecomplete)
              (let ((ecomplete-auto-select t)
                    (ecomplete-database
                     '((mail ("key" 1 0 "alpha\nbeta")))))
                (let ((single (ecomplete-display-matches 'mail "alpha" t)))
                  (let ((ecomplete-auto-select nil)
                        (ecomplete-sort-predicate #'ecomplete-newness)
                        (ecomplete-database
                         '((mail ("multi" 1 2 "alpha\nbeta")
                                 ("single" 1 1 "alpha one"))))
                        (keys (list (kbd "<down>") (kbd "RET"))))
                    (let ((selected
                           (cl-letf (((symbol-function 'read-key-sequence)
                                      (lambda (&rest _)
                                        (prog1 (car keys)
                                          (setq keys (cdr keys))))))
                             (ecomplete-display-matches 'mail "alpha" t))))
                      (list (equal single "alpha\nbeta")
                            (equal selected "alpha one")))))))"#),
        "(t t)"
    );
}

#[test]
fn image_dired_uses_at_least_one_column_in_narrow_windows() {
    assert_eq!(
        ev(r#"(progn
              (require 'image-dired)
              (with-temp-buffer
                (insert "abc")
                (let ((image-dired-thumbs-per-row 3))
                  (cl-letf (((symbol-function 'image-dired-thumbnail-window)
                             (lambda (&rest _) nil))
                            ((symbol-function 'window-body-width)
                             (lambda (&rest _) 10))
                            ((symbol-function 'frame-char-width)
                             (lambda (&rest _) 8))
                            ((symbol-function 'image-dired--thumb-size)
                             (lambda (&rest _) 100))
                            ((symbol-function 'image-dired-image-at-point-p)
                             (lambda (&rest _) t)))
                    (image-dired-line-up-dynamic)
                    (equal (buffer-string) (concat "a\nb\nc\n"))))))"#),
        "t"
    );
}

#[test]
fn file_mode_prompts_reject_malformed_octal_prefixes() {
    assert_eq!(
        ev(r#"(progn
              (require 'files)
              (require 'dired-aux)
              (let* ((read-error
                      (condition-case err
                          (cl-letf (((symbol-function 'read-string)
                                     (lambda (&rest _) "755note")))
                            (read-file-modes))
                        (error (car err))))
                     (read-valid
                      (cl-letf (((symbol-function 'read-string)
                                 (lambda (&rest _) "755")))
                        (read-file-modes)))
                     (applied nil)
                     (dired-error
                      (condition-case err
                          (cl-letf (((symbol-function 'dired-get-marked-files)
                                     (lambda (&rest _) '("dummy")))
                                    ((symbol-function 'dired-get-filename)
                                     (lambda (&rest _) nil))
                                    ((symbol-function 'dired-mark-read-string)
                                     (lambda (&rest _) "755note"))
                                    ((symbol-function 'set-file-modes)
                                     (lambda (_file mode &rest _)
                                       (setq applied mode)))
                                    ((symbol-function 'dired-do-redisplay)
                                     (lambda (&rest _) nil))
                                    ((symbol-function 'dired-post-do-command)
                                     (lambda (&rest _) nil)))
                            (dired-do-chmod)
                            'no-error)
                        (error (car err)))))
                (list read-error read-valid dired-error applied)))"#),
        "(user-error 493 user-error nil)"
    );
}

#[test]
fn vc_annotate_rejects_invalid_and_zero_spans() {
    assert_eq!(
        ev(r#"(progn
              (require 'vc-annotate)
              (let* ((vc-annotate-display-mode 'fullscale)
                     (nonnumeric
                      (condition-case err
                          (cl-letf (((symbol-function 'read-string)
                                     (lambda (&rest _) "not-a-number")))
                            (vc-annotate--read-span "Span? "))
                        (error (car err))))
                     (partial
                      (condition-case err
                          (cl-letf (((symbol-function 'read-string)
                                     (lambda (&rest _) "20days")))
                            (vc-annotate--read-span "Span? "))
                        (error (car err))))
                     (valid
                      (cl-letf (((symbol-function 'read-string)
                                 (lambda (&rest _) "1.5")))
                        (vc-annotate--read-span "Span? ")))
                     (zero
                      (condition-case err
                          (vc-annotate-display-select nil 0.0)
                        (error (car err)))))
                (list nonnumeric partial valid zero
                      vc-annotate-display-mode)))"#),
        "(user-error user-error 1.5 user-error fullscale)"
    );
}

#[test]
fn change_log_font_lock_recognizes_slash_date_formats() {
    assert_eq!(
        ev(r#"(progn
              (require 'add-log)
              (let ((add-log-time-format (lambda (_time _zone) "2026/10/28")))
                (with-temp-buffer
                  (insert "2026/10/28  Alice <alice@example.com>\n\n\t* file: change\n")
                  (change-log-mode)
                  (font-lock-ensure)
                  (get-text-property (point-min) 'face))))"#),
        "change-log-date"
    );
}

#[test]
fn erc_identd_rejects_invalid_port_strings_before_opening_socket() {
    assert_eq!(
        ev(r#"(progn
              (require 'erc-identd)
              (let* ((invalid-results
                      (mapcar
                       (lambda (port)
                         (let ((service nil)
                               (erc-identd-process nil))
                           (condition-case err
                               (cl-letf (((symbol-function 'make-network-process)
                                          (lambda (&rest args)
                                            (setq service (plist-get args :service))
                                            'fake-process))
                                         ((symbol-function 'set-process-query-on-exit-flag)
                                          (lambda (&rest _) nil)))
                                 (erc-identd-start port)
                                 (list 'accepted service))
                             (error (list (car err) service)))))
                       '("garbage" "8113junk")))
                     (service nil)
                     (erc-identd-process nil))
                (cl-letf (((symbol-function 'make-network-process)
                           (lambda (&rest args)
                             (setq service (plist-get args :service))
                             'fake-process))
                          ((symbol-function 'set-process-query-on-exit-flag)
                           (lambda (&rest _) nil)))
                  (erc-identd-start "8113"))
                (list invalid-results service)))"#),
        "(((user-error nil) (user-error nil)) 8113)"
    );
}

#[test]
fn archive_ar_extracts_bsd_extended_member_names() {
    assert_eq!(
        ev(r##"(progn
              (require 'arc-mode)
              (let* ((file (make-temp-file "bsd-ar"))
                     (name "long_member.txt")
                     (archive
                      (concat "!<arch>\n"
                              "#1/15" (make-string 11 32)
                              "0" (make-string 11 32)
                              "0" (make-string 5 32)
                              "0" (make-string 5 32)
                              "100644" (make-string 2 32)
                              "16" (make-string 8 32)
                              (string 96) "\n"
                              name "x"))
                     descs)
                (unwind-protect
                    (progn
                      (with-temp-buffer
                        (set-buffer-multibyte nil)
                        (insert archive)
                        (let ((coding-system-for-write 'no-conversion))
                          (write-region (point-min) (point-max) file nil 'silent)))
                      (cl-letf (((symbol-function 'archive--summarize-descs)
                                 (lambda (files) (setq descs files))))
                        (with-temp-buffer
                          (set-buffer-multibyte nil)
                          (insert-file-contents-literally file)
                          (archive-ar-summarize)))
                      (let ((contents
                             (with-temp-buffer
                               (set-buffer-multibyte nil)
                               (archive-ar-extract file name)
                               (buffer-string))))
                        (list (archive--file-desc-ext-file-name (car descs))
                              (archive--file-desc-size (car descs))
                              contents)))
                  (delete-file file))))"##),
        "(\"long_member.txt\" 1 \"x\")"
    );
}

#[test]
fn archive_ar_recognizes_gnu_and_bsd_symbol_tables() {
    assert_eq!(
        ev(r#"(progn
              (require 'arc-mode)
              (let ((names nil)
                    (record
                     (lambda (name)
                       (concat
                        (format "%-16s%-12d%-6d%-6d%-8o%-10d`\n"
                                name 0 0 0 #o100644 0)))))
                (cl-letf (((symbol-function 'archive--summarize-descs)
                           (lambda (descs)
                             (setq names
                                   (mapcar
                                    (lambda (desc)
                                      (substring-no-properties
                                       (archive--file-desc-ext-file-name desc)))
                                    descs)))))
                  (with-temp-buffer
                    (set-buffer-multibyte nil)
                    (insert "!<arch>\n"
                            (funcall record "/SYM64/")
                            (funcall record "__.SYMDEF")
                            (funcall record "/")
                            (funcall record "foo.o"))
                    (archive-ar-summarize)))
                names))"#),
        "(\".<LookupTable>.\" \".<LookupTable>.\" \".<LookupTable>.\" \"foo.o\")"
    );
}

#[test]
fn mouse_avoidance_ignores_unreported_pointer_movement() {
    assert_eq!(
        ev(r#"(progn
              (require 'cl-lib)
              (require 'avoid)
              (let ((cursor-type t)
                    (executing-kbd-macro nil)
                    (track-mouse nil)
                    (last-input-event ?a)
                    (position '(test-frame 10 . 20))
                    banished)
                (cl-letf (((symbol-function 'frame-pointer-visible-p)
                           (lambda (&rest _) t))
                          ((symbol-function 'mouse-position)
                           (lambda (&rest _) position))
                          ((symbol-function 'selected-frame)
                           (lambda (&rest _) 'test-frame))
                          ((symbol-function 'mouse-avoidance-banish-mouse)
                           (lambda (&rest _) (setq banished t))))
                  (mouse-avoidance-ignore-p)
                  (setq position '(test-frame 100 . 100))
                  (mouse-avoidance-banish)
                  (let ((moved-banished banished))
                    (setq banished nil)
                    (mouse-avoidance-banish)
                    (let ((still-ignored banished))
                      (mouse-avoidance-pre-command-hook)
                      (mouse-avoidance-banish)
                      (list moved-banished still-ignored banished))))))"#),
        "(nil nil t)"
    );
}

#[test]
fn idlwave_after_count_breakpoint_rejects_invalid_counts() {
    assert_eq!(
        ev(r#"(progn
              (require 'cl-lib)
              (require 'idlw-shell)
              (let* ((breakpoints
                      (cl-find-if
                       (lambda (entry)
                         (and (consp entry) (equal (car entry) "Breakpoints")))
                       (cdr idlwave-shell-menu-def)))
                     (special
                      (cl-find-if
                       (lambda (entry)
                         (and (consp entry)
                              (equal (car entry) "Set Special Breakpoint")))
                       (cdr breakpoints)))
                     (item
                      (cl-find-if
                       (lambda (entry)
                         (and (vectorp entry)
                              (equal (aref entry 0) "Set After Count Breakpoint")))
                       (cdr special)))
                     (action (aref item 1))
                     (answer "oops")
                     passed)
                (cl-letf (((symbol-function 'read-string)
                           (lambda (&rest _) answer))
                          ((symbol-function 'idlwave-shell-break-here)
                           (lambda (count) (setq passed count))))
                  (let ((bad-text
                         (condition-case err
                             (eval action)
                           (user-error (car err)))))
                    (setq answer "0")
                    (let ((zero
                           (condition-case err
                               (eval action)
                             (user-error (car err)))))
                      (setq answer "3junk")
                      (let ((trailing
                             (condition-case err
                                 (eval action)
                               (user-error (car err)))))
                        (setq answer "2")
                        (eval action)
                        (list bad-text zero trailing passed)))))))"#),
        "(user-error user-error user-error 2)"
    );
}

#[test]
fn erc_identd_rejects_zero_and_out_of_range_query_ports() {
    assert_eq!(
        ev(r#"(progn
              (require 'cl-lib)
              (require 'erc-identd)
              (let (reply replies)
                (cl-letf (((symbol-function 'send-string)
                           (lambda (_proc string) (setq reply string)))
                          ((symbol-function 'stop-process)
                           (lambda (&rest _) nil))
                          ((symbol-function 'delete-process)
                           (lambda (&rest _) nil)))
                  (dolist (request '("0, 23\r\n" "70000, 23\r\n"
                                     "23, 65536\r\n" "1, 65535\r\n"))
                    (setq reply nil)
                    (erc-identd-filter 'fake request)
                    (push reply replies)))
                (setq replies (nreverse replies))
                (list (equal (nth 0 replies)
                             "0, 23 : ERROR : INVALID-PORT\n")
                      (equal (nth 1 replies)
                             "70000, 23 : ERROR : INVALID-PORT\n")
                      (equal (nth 2 replies)
                             "23, 65536 : ERROR : INVALID-PORT\n")
                      (and (string-match-p
                            "\\`1, 65535 : USERID : " (nth 3 replies))
                           t))))"#),
        "(t t t t)"
    );
}

#[test]
fn ange_ftp_preserves_literal_at_suffix_unless_ls_marks_symlinks() {
    assert_eq!(
        ev(r#"(progn
              (require 'dired)
              (require 'ange-ftp)
              (let ((listing
                     "lrwxrwxrwx 1 owner group 6 Jan 1 00:00 link@ -> target\n")
                    (regexp
                     "^lrwxrwxrwx 1 owner group 6 Jan 1 00:00 "))
                (list
                 (with-temp-buffer
                   (let ((directory-listing-before-filename-regexp regexp)
                         (dired-ls-F-marks-symlinks nil))
                     (insert listing)
                     (goto-char (point-min))
                     (let ((files (ange-ftp-ls-parser "-F")))
                       (list (gethash "link@" files)
                             (gethash "link" files)))))
                 (with-temp-buffer
                   (let ((directory-listing-before-filename-regexp regexp)
                         (dired-ls-F-marks-symlinks t))
                     (insert listing)
                     (goto-char (point-min))
                     (let ((files (ange-ftp-ls-parser "-F")))
                       (list (gethash "link@" files)
                             (gethash "link" files))))))))"#),
        "((\"target\" nil) (nil \"target\"))"
    );
}

#[test]
fn gnus_server_compact_server_updates_group_lines_for_its_server() {
    assert_eq!(
        ev(r#"(progn
              (require 'cl-lib)
              (require 'gnus-srvr)
              (let* ((gnus-group-buffer " *gnus-compact-test*")
                     (group-buffer (get-buffer-create gnus-group-buffer))
                     (gnus-original-article-buffer nil)
                     updates)
                (unwind-protect
                    (progn
                      (with-current-buffer group-buffer
                        (let ((inhibit-read-only t)
                              (start (point-min)))
                          (erase-buffer)
                          (setq start (point))
                          (insert "first\n")
                          (put-text-property start (1- (point))
                                             'gnus-group "mail.first")
                          (setq start (point))
                          (insert "unrelated\n")
                          (put-text-property start (1- (point))
                                             'gnus-group "other.group")
                          (setq start (point))
                          (insert "second\n")
                          (put-text-property start (1- (point))
                                             'gnus-group "mail.second")))
                      (cl-letf (((symbol-function 'gnus-server-server-name)
                                 (lambda (&rest _) "nnml:test"))
                                ((symbol-function 'gnus-server-to-method)
                                 (lambda (&rest _) '(nnml "test")))
                                ((symbol-function 'gnus-get-function)
                                 (lambda (&rest _) t))
                                ((symbol-function 'gnus-message)
                                 (lambda (&rest _) nil))
                                ((symbol-function 'gnus-open-server)
                                 (lambda (&rest _) t))
                                ((symbol-function 'gnus-request-compact)
                                 (lambda (&rest _) t))
                                ((symbol-function 'gnus-kill-buffer)
                                 (lambda (&rest _) nil))
                                ((symbol-function 'gnus-find-method-for-group)
                                 (lambda (group)
                                   (if (member group '("mail.first" "mail.second"))
                                       '(nnml "test")
                                     '(nnml "other"))))
                                ((symbol-function 'gnus-group-update-group-line)
                                 (lambda (&rest _)
                                   (push (gnus-group-group-name) updates))))
                        (gnus-server-compact-server))
                      (nreverse updates))
                  (when (buffer-live-p group-buffer)
                    (kill-buffer group-buffer)))))"#),
        "(\"mail.first\" \"mail.second\")"
    );
}

#[test]
fn ff_find_the_other_file_extracts_case_folded_stub() {
    assert_eq!(
        ev(r#"(progn
              (require 'cl-lib)
              (require 'find-file)
              (let ((ff-case-fold-search t)
                    (ff-search-directories '("."))
                    (ff-other-file-alist '(("\\.c$" (".h"))))
                    (ff-always-try-to-create nil)
                    (case-fold-search nil)
                    captured)
                (with-temp-buffer
                  (setq buffer-file-name "FOO.C")
                  (setq-local case-fold-search nil)
                  (cl-letf (((symbol-function 'ff-get-file)
                             (lambda (_dirs stub suffixes &optional _window)
                               (setq captured (list stub suffixes))
                               nil)))
                    (ff-find-the-other-file))
                  captured)))"#),
        "(\"FOO\" (\".h\"))"
    );
}

#[test]
fn eshell_define_literal_alias_is_readable() {
    assert_eq!(
        ev(r#"(progn
              (require 'esh-var)
              (let ((eshell-variable-aliases-list nil))
                (eshell/define "MYVAR" '("hello"))
                (eshell-get-variable "MYVAR")))"#),
        "\"hello\""
    );
}

#[test]
fn calc_user_define_edit_accepts_string_keyboard_macros() {
    assert_eq!(
        ev(r#"(progn
              (require 'cl-lib)
              (require 'calc-prog)
              (let* ((key-entry (cons ?a "abc"))
                     (edit-buffer (get-buffer-create " *calc-edit-test*"))
                     finish)
                (unwind-protect
                    (progn
                      (cl-letf (((symbol-function 'read-char)
                                 (lambda (&rest _) ?a))
                                ((symbol-function 'calc-user-key-map)
                                 (lambda (&rest _) (list key-entry)))
                                ((symbol-function 'message)
                                 (lambda (&rest _) nil))
                                ((symbol-function 'calc--edit-mode)
                                 (lambda (handler &optional _allow-ret _title)
                                   (setq finish handler)
                                   (set-buffer edit-buffer)
                                   (let ((inhibit-read-only t))
                                     (erase-buffer))
                                   (setq-local calc-edit-top (point-min))))
                                ((symbol-function 'calc-edit-format-macro-buffer)
                                 (lambda (&rest _) nil))
                                ((symbol-function 'calc-show-edit-buffer)
                                 (lambda (&rest _) nil)))
                        (calc-user-define-edit))
                      (funcall finish)
                      (list (stringp (cdr key-entry))
                            (functionp (cdr key-entry))))
                  (when (buffer-live-p edit-buffer)
                    (kill-buffer edit-buffer)))))"#),
        "(nil t)"
    );
}

#[test]
fn elint_update_env_keeps_preloaded_function_definitions() {
    assert_eq!(
        ev(r#"(progn
              (require 'elint)
              (with-temp-buffer
                (let ((elint-preloaded-env
                       (elint-env-add-func
                        (elint-make-env) 'preloaded-probe '(arg))))
                  (insert "(preloaded-probe 1)\n")
                  (elint-update-env)
                  (and (elint-env-find-func
                        elint-buffer-env 'preloaded-probe)
                       t))))"#),
        "t"
    );
}

#[test]
fn filename_completion_preserves_literal_tilde_directories_and_remote_prefixes() {
    assert_eq!(
        ev(r#"(progn
              (require 'minibuffer)
              (require 'tramp)
              (let* ((root (make-temp-file "remacs-tilde-" t))
                     (dirname (concat (file-name-as-directory root)
                                      "~" (user-login-name)))
                     (target (expand-file-name "target" dirname))
                     (input (concat (file-name-as-directory dirname) "tar"))
                     (default-directory root))
                (unwind-protect
                    (progn
                      (make-directory dirname)
                      (with-temp-file target (insert "x"))
                      (list (file-name-all-completions "tar" dirname)
                            (completion--file-name-table input nil t)
                            (and (string-match-p
                                  "\\`/ssh:host:"
                                  (completion--sifn-regardless-of-system-users
                                   "/ssh:host:/foo/~bar/baz"))
                                 t)))
                  (delete-directory root t))))"#),
        "((\"target\") (\"target\") t)"
    );
}

#[test]
fn cd_completion_excludes_file_when_later_cdpath_entry_has_directory() {
    assert_eq!(
        ev(r#"(progn
              (require 'files)
              (let* ((root (make-temp-file "cd-completion-" t))
                     (dir-a (file-name-as-directory
                             (expand-file-name "A" root)))
                     (dir-b (file-name-as-directory
                             (expand-file-name "B" root)))
                     (cd-path (list dir-a dir-b))
                     (table (apply-partially
                             #'locate-file-completion-table cd-path nil))
                     (cd-table (apply-partially
                                #'files--cd-completion-table cd-path nil))
                     (predicate
                      (lambda (name)
                        (locate-file name cd-path nil
                                     (lambda (path)
                                       (and (file-directory-p path) 'dir-ok)))))
                     original-result cd-result)
                (unwind-protect
                    (progn
                      (make-directory dir-a)
                      (make-directory dir-b)
                      (with-temp-file (expand-file-name "a" dir-a)
                        (insert "file"))
                      (make-directory (expand-file-name "a" dir-b))
                      (setq original-result
                            (sort (all-completions "a" table predicate)
                                  #'string<)
                            cd-result (all-completions "a" cd-table predicate))
                      (list original-result cd-result))
                  (delete-directory root t))))"#),
        "((\"a\" \"a/\") (\"a/\"))"
    );
}

#[test]
fn gv_setter_preserves_uninterned_symbol_identity() {
    assert_eq!(
        ev(r#"(progn
              (require 'gv)
              (let* ((one (make-symbol "same-name"))
                     (two (make-symbol "same-name"))
                     (obarray-a (make-vector 17 0))
                     (obarray-b (make-vector 17 0))
                     (three (let ((obarray obarray-a)) (intern "same-name")))
                     (four (let ((obarray obarray-b)) (intern "same-name"))))
                (list (eq one two)
                      (eq (gv-setter one) (gv-setter two))
                      (eq (gv-setter one) (gv-setter one))
                      (eq three four)
                      (eq (gv-setter three) (gv-setter four))
                      (eq (gv-setter three) (gv-setter three)))))"#),
        "(nil nil t nil nil t)"
    );
}

#[test]
fn oclosure_mutable_slot_setter_has_setter_documentation() {
    assert_eq!(
        ev(r#"(progn
              (require 'oclosure)
              (require 'gv)
              (require 'simple)
              (oclosure-define (probe-oc-1673) "Probe type" (slot :mutable t))
              (let* ((getter 'probe-oc-1673--slot)
                     (setter (gv-setter getter))
                     (getter-doc (documentation getter))
                     (setter-doc (documentation setter)))
                (list (and (string-match-p "\\`Access slot" getter-doc) t)
                      (and (string-match-p "\\`Set slot" setter-doc) t)
                      (and (string-match-p "(fn VAL OBJ)\\'" setter-doc) t)
                      (not (equal getter-doc setter-doc)))))"#),
        "(t t t t)"
    );
}

#[test]
fn info_fontifies_nested_curly_quotes() {
    assert_eq!(
        ev(r#"(progn
              (require 'info)
              (with-temp-buffer
                (insert "‘outer ‘inner’ outer’")
                (Info-mode)
                (font-lock-ensure)
                (list (eq (get-text-property 3 'face) 'Info-quoted)
                      (eq (get-text-property 10 'face) 'Info-quoted)
                      (eq (get-text-property 18 'face) 'Info-quoted))))"#),
        "(t t t)"
    );
}

#[test]
fn nnfeed_serializes_group_names_as_lisp_strings() {
    assert_eq!(
        ev(r#"(progn
              (require 'cl-lib)
              (require 'nnfeed)
              (let* ((quoted-group "news\"with\\slash")
                     (description
                      (with-temp-buffer
                        (nnfeed--group-description
                         "news with spaces"
                         [nil nil nil nil nil "A description"])
                        (goto-char (point-min))
                        (let ((group (read (current-buffer))))
                          (skip-chars-forward " \t")
                          (list group
                                (buffer-substring-no-properties
                                 (point) (line-end-position))))))
                     (group-response
                      (with-temp-buffer
                        (let ((nntp-server-buffer (current-buffer)))
                          (cl-letf (((symbol-function 'nnfeed--current-server-no-prefix)
                                     (lambda (&rest _) "server"))
                                    ((symbol-function 'nnfeed--group-data)
                                     (lambda (group _server)
                                       (vector group
                                               (make-hash-table :test 'equal)
                                               (make-hash-table :test 'eql)
                                               0 1 "")))
                                    ((symbol-function 'nnfeed-message)
                                     (lambda (&rest _) nil)))
                            (nnfeed-request-group quoted-group nil t)
                            (goto-char (point-min))
                            (skip-chars-forward "0-9 ")
                            (read (current-buffer))))))
                     (list-response
                      (with-temp-buffer
                        (let ((nntp-server-buffer (current-buffer))
                              (table (make-hash-table :test 'equal)))
                          (puthash quoted-group
                                   (vector quoted-group nil nil 0 1 nil) table)
                          (cl-letf (((symbol-function 'nnfeed--parse-feed)
                                     (lambda (&rest _) table)))
                            (nnfeed-request-list "server")
                            (goto-char (point-min))
                            (read (current-buffer)))))) )
                (list (equal (car description) "news with spaces")
                      (equal (cadr description) "A description")
                      (equal group-response quoted-group)
                      (equal list-response quoted-group))))"#),
        "(t t t t)"
    );
}

#[test]
fn cc_forward_label_skips_ternary_colon_in_case_label() {
    assert_eq!(
        ev(r#"(progn
              (require 'cc-mode)
              (with-temp-buffer
                (insert "switch (x) {\ncase 1 ? 2 : 3:\nreturn 1;\n}\n")
                (c-mode)
                (goto-char (point-min))
                (search-forward "case")
                (backward-char 4)
                (c-forward-label)
                (buffer-substring (point) (line-end-position))))"#),
        "\"\""
    );
}

#[test]
fn cc_fill_paragraph_wraps_at_word_boundary_when_point_on_first_word() {
    // Regression for #1678: the comment opener and the more-indented next
    // line must not make filling split the first word in the comment.
    assert_eq!(
        ev(r#"(progn
              (require 'cc-mode)
              (with-temp-buffer
                (insert "/* Bla bla bla bla bla\n        bla bla\n*/\n")
                (c-mode)
                (setq fill-column 12)
                (goto-char (point-min))
                (search-forward "Bla")
                (backward-char 3)
                (c-fill-paragraph nil)
                (equal (substring-no-properties (buffer-string))
                       "/* Bla bla\n        bla\n        bla\n        bla\n        bla\n        bla\n        */\n")))"#),
        "t"
    );
}

#[test]
fn f90_change_keywords_changes_every_word_of_multiword_keywords() {
    // Regression for #1679: F90 has keywords such as "sync all".
    // Evaluate just these definitions; the mode's menu setup is unrelated.
    assert_eq!(
        ev(r#"(progn
              (with-temp-buffer
                (setq lexical-binding t)
                (insert-file-contents "lisp/f90.el")
                (goto-char (point-min))
                (let ((wanted '(f90-keywords-re f90-procedures-re
                                f90-operators-re f90-hpf-keywords-re
                                f90-upcase-keywords f90-capitalize-keywords
                                f90-downcase-keywords f90-change-keywords))
                      form)
                  (condition-case nil
                      (while t
                        (setq form (read (current-buffer)))
                        (when (and (memq (car-safe form) '(defconst defun))
                                   (memq (cadr form) wanted))
                          (eval form)))
                    (end-of-file nil)))
                (let ((upper-text
                       (with-temp-buffer
                         (insert "sync all\n")
                         (f90-upcase-keywords)
                         (buffer-string)))
                      (capitalized-text
                       (with-temp-buffer
                         (insert "sync all\n")
                         (f90-capitalize-keywords)
                         (buffer-string)))
                      (lower-text
                       (with-temp-buffer
                         (insert "SYNC ALL\n")
                         (f90-downcase-keywords)
                         (buffer-string))))
                  (list (equal upper-text "SYNC ALL\n")
                        (equal capitalized-text "Sync All\n")
                        (equal lower-text "sync all\n")))))"#),
        "(t t t)"
    );
}

#[test]
fn refill_backward_kill_word_keeps_point_before_space() {
    assert_eq!(
        ev(r#"(progn
              (require 'refill)
              (with-temp-buffer
                (refill-mode 1)
                (setq-local fill-column 25)
                (insert "Foo bar blablabla asdgf\nword asdfas dfasdfasd\nasd asdfa sdfasd sdf\n")
                (goto-char (point-min))
                (forward-line 1)
                (search-forward "word")
                (setq this-command 'backward-kill-word)
                (backward-kill-word 1)
                (refill-post-command-function)
                (list (current-column)
                      (buffer-substring (line-beginning-position)
                                        (line-end-position)))))"#),
        "(0 \" asdfas dfasdfasd asd\")"
    );
}

#[test]
fn dired_noselect_opens_directory_with_newline_name() {
    assert_eq!(
        ev(r#"(progn
              (require 'dired)
              (let* ((dired-listing-switches "-al")
                     (base (make-temp-file
                            (expand-file-name "dired-newline-"
                                              temporary-file-directory)
                            t))
                     (dir (concat base "\n"))
                     buffer)
                (unwind-protect
                    (progn
                      (rename-file base dir)
                      (setq buffer (dired-noselect dir))
                      (with-current-buffer buffer
                        (and dired-subdir-alist
                             (equal default-directory
                                    (file-name-as-directory dir))
                             t)))
                  (when (buffer-live-p buffer) (kill-buffer buffer))
                  (when (file-directory-p base) (delete-directory base t))
                  (when (file-directory-p dir) (delete-directory dir t)))))"#),
        "t"
    );
}

#[test]
fn prolog_find_term_counts_top_level_arguments() {
    assert_eq!(
        ev(r#"(progn
              (require 'prolog)
              (let ((one-argument
                     (with-temp-buffer
                       (insert "foo(a,b).\nfoo(a).\n")
                       (goto-char (point-min))
                       (prolog-find-term "foo" 1)
                       (equal (buffer-substring
                               (line-beginning-position) (line-end-position))
                              "foo(a).")))
                    (two-arguments
                     (with-temp-buffer
                       (insert "foo(bar(a,b), 'c,d').\nfoo(a,b).\n")
                       (goto-char (point-min))
                       (prolog-find-term "foo" 2)
                       (equal (buffer-substring
                               (line-beginning-position) (line-end-position))
                              "foo(bar(a,b), 'c,d')."))))
                (list one-argument two-arguments)))"#),
        "(t t)"
    );
}

#[test]
fn semantic_symref_translates_grep_extended_grouping() {
    assert_eq!(
        ev(r#"(progn
              (require 'semantic/symref)
              (require 'cl-lib)
              (and
               (with-temp-buffer
                 (insert "foobar\n")
                 (cl-letf (((symbol-function 'find-buffer-visiting)
                            (lambda (&rest _) (current-buffer)))
                           ((symbol-function 'semantic-current-tag)
                            (lambda ()
                              (and (looking-at "foobar") 'foo-tag)))
                           ((symbol-function 'semantic-tag-copy)
                            (lambda (tag &rest _) tag))
                           ((symbol-function 'semantic--tag-put-property)
                            (lambda (&rest _) nil)))
                   (eq (semantic-symref-hit-to-tag-via-buffer
                        '(1 . "test") "foo(bar)" 'regexp)
                       'foo-tag)))
               (let ((repetition
                      (semantic-symref--grep-regexp-to-emacs-regexp "foo+"))
                     (literal-plus
                      (semantic-symref--grep-regexp-to-emacs-regexp "foo\\+")))
                 (and (string-match-p (concat "\\`" repetition "\\'") "foooo")
                      (not (string-match-p (concat "\\`" repetition "\\'")
                                           "foo+"))
                      (string-match-p (concat "\\`" literal-plus "\\'") "foo+")
                      (not (string-match-p (concat "\\`" literal-plus "\\'")
                                           "fooo"))))))"#),
        "t"
    );
}

#[test]
fn dired_guesses_local_man_command_with_selected_path() {
    assert_eq!(
        ev(r#"(progn
              (require 'dired-aux)
              (require 'cl-lib)
              (cl-letf (((symbol-function 'Man-support-local-filenames)
                         (lambda () 'man)))
                (let* ((file "/tmp/foo.1")
                       (command (dired-guess-default (list file))))
                  (list command
                        (dired-shell-stuff-it command (list file) t)
                        (car (dired-guess-default '("/tmp/foo.1.gz")))))))"#),
        "(\"man\" \"man /tmp/foo.1\" \"man\")"
    );
}

#[test]
fn todo_mode_converts_legacy_date_time_with_calendar_form() {
    assert_eq!(
        ev(r#"(progn
              (require 'todo-mode)
              (let* ((calendar-date-display-form '(monthname "-" day "-" year))
                     (expected (concat (calendar-month-name 1 t) "-01-2001")))
                (with-temp-buffer
                  (insert "2001-01-01 10:00")
                  (goto-char (point-min))
                  (re-search-forward todo-legacy-date-time-regexp)
                  (todo-convert-legacy-date-time)
                  (equal (buffer-string) (concat expected " 10:00")))))"#),
        "t"
    );
}

#[test]
fn rst_adaptive_fill_uses_paragraph_start_prefix() {
    assert_eq!(
        ev(r#"(progn
              (provide 'compile)
              (require 'rst)
              (with-temp-buffer
                (rst-mode)
                (setq-local fill-column 20)
                (insert "Some start of a paragraph\n  continued wrong and more words to wrap here\n")
                (goto-char (point-min))
                (forward-line 1)
                (fill-paragraph nil)
                (buffer-string)))"#),
        "\"Some start of a\nparagraph continued\nwrong and more words\nto wrap here\n\""
    );
}

#[test]
fn rst_insert_list_new_tag_clears_whitespace_only_line() {
    assert_eq!(
        ev(r#"(progn
              (provide 'compile)
              (require 'rst)
              (with-temp-buffer
                (rst-mode)
                (insert "Paragraph\n  \nNext\n")
                (goto-char (point-min))
                (forward-line 1)
                (rst-insert-list-new-tag "*")
                (buffer-string)))"#),
        "\"Paragraph\n\n* \nNext\n\""
    );
}

#[test]
fn rst_convert_bullets_indents_continuation_lines() {
    assert_eq!(
        ev(r#"(progn
              (provide 'compile)
              (require 'rst)
              (list
               (with-temp-buffer
                 (rst-mode)
                 (insert "* item one\n  continuation\n* item two\n")
                 (rst-convert-bullets-to-enumeration (point-min) (point-max))
                 (equal (buffer-string)
                        "1. item one\n   continuation\n2. item two\n"))
               (with-temp-buffer
                 (rst-mode)
                 (insert "  * item one\n    continuation\n  * item two\n")
                 (rst-convert-bullets-to-enumeration (point-min) (point-max))
                 (equal (buffer-string)
                        "  1. item one\n     continuation\n  2. item two\n"))))"#),
        "(t t)"
    );
}

#[test]
fn reftex_combined_citation_keeps_each_entry_comment() {
    assert_eq!(
        ev(r#"(progn
              (require 'cl-lib)
              (require 'reftex-cite)
              (let ((reftex-comment-citations t)
                    (reftex-cite-comment-format " -- %a %y")
                    (reftex-cite-key-separator ", ")
                    (reftex-select-return-marker (make-marker))
                    (reftex-format-cite-function nil))
                (cl-letf (((symbol-function 'reftex-figure-out-cite-format)
                           (lambda (&rest _) "\\cite{%l}"))
                          ((symbol-function 'reftex-offer-bib-menu)
                           (lambda ()
                             '(concat
                               ("foo" ("&key" . "foo")
                                       ("author" . "Alice Example")
                                       ("year" . "2001"))
                               ("bar" ("&key" . "bar")
                                       ("author" . "Bob Example")
                                       ("year" . "2002")))))
                          ((symbol-function 'reftex-kill-buffer)
                           (lambda (&rest _) nil)))
                  (with-temp-buffer
                    (reftex-do-citation)
                    (buffer-string)))))"#),
        "\"\\\\cite{foo, bar -- Alice Example 2001 -- Bob Example 2002}\""
    );
}

#[test]
fn todo_legacy_comment_pattern_matches_multiple_lines() {
    assert_eq!(
        ev(r#"(progn
              (require 'todo-mode)
              (with-temp-buffer
                (let ((item-start (copy-marker (point-min))))
                  (insert "task (first line\ncontinued) ")
                  (set-marker item-start (point-min))
                  (goto-char (point-max))
                  (and (looking-back
                        "(\\(\\(?:.\\|\n\\)*\\)) "
                        (marker-position item-start))
                       (equal (match-string 1) "first line\ncontinued")))))"#),
        "t"
    );
}

#[test]
fn mpc_format_makes_browser_tags_clickable() {
    assert_eq!(
        ev(r#"(progn
              (require 'mpc)
              (let (clickable)
                (dolist (tag '(Genre Performer Artist Album Playlist Date))
                  (with-temp-buffer
                    (mpc-format (format "%%{%s}" tag)
                                (list (cons tag "value")))
                    (push (and (get-text-property (point-min) 'keymap)
                               (eq (get-text-property (point-min) 'follow-link) t))
                          clickable)))
                (nreverse clickable)))"#),
        "(t t t t t nil)"
    );
}

#[test]
fn pcvs_zero_change_merge_keeps_modified_status() {
    assert_eq!(
        ev(r#"(progn
              (require 'cl-lib)
              (require 'pcvs-parse)
              (let ((dont-change-disc nil)
                    (cvs-current-dir "/tmp/")
                    (cvs-current-subdir nil))
                (cl-letf (((symbol-function 'cvs-parsed-fileinfo)
                           (lambda (type &rest _) type)))
                  (with-temp-buffer
                    (insert "RCS file: /tmp/foo,v\n"
                            "retrieving revision 1.1\n"
                            "retrieving revision 1.2\n"
                            "Merging differences between 1.1 and 1.2 into foo\n"
                            "foo already contains the differences between 1.1 and 1.2\n")
                    (goto-char (point-min))
                    (cvs-parse-merge)))))"#),
        "(MODIFIED . MERGED)"
    );
}

#[test]
fn read_passwd_visibility_uses_clicked_mode_line_window() {
    assert_eq!(
        ev(r#"(progn
              (require 'cl-lib)
              (require 'auth-source)
              (let* ((outer (generate-new-buffer " *passwd-outer*"))
                     (inner (generate-new-buffer " *passwd-inner*"))
                     (outer-window 'outer-window)
                     (inner-window 'inner-window)
                     (mode-line-window outer-window)
                     (icon-preference nil))
                (unwind-protect
                    (progn
                      (dolist (buffer (list outer inner))
                        (with-current-buffer buffer
                          (setq-local local-minor-modes '(read-passwd-mode)
                                      read-passwd--password-hidden t)))
                      (cl-letf (((symbol-function 'active-minibuffer-window)
                                 (lambda () inner-window))
                                ((symbol-function 'window-buffer)
                                 (lambda (window)
                                   (if (eq window outer-window) outer inner)))
                                ((symbol-function 'force-mode-line-update)
                                 (lambda (&rest _) nil))
                                ((symbol-function 'read-passwd--hide-password)
                                 (lambda () nil)))
                        (read-passwd-toggle-visibility)
                        (list
                         (buffer-local-value 'read-passwd--password-hidden outer)
                         (buffer-local-value 'read-passwd--password-hidden inner)))
                  (kill-buffer outer)
                  (kill-buffer inner))))"#),
        "(nil t)"
    );
}

#[test]
fn highlight_changes_navigation_skips_contiguous_regions() {
    assert_eq!(
        ev(r#"(progn
              (require 'hilit-chg)
              (list
               (with-temp-buffer
                 (insert "abcde")
                 (setq-local highlight-changes-mode t)
                 (put-text-property 1 2 'hilit-chg 'change-1)
                 (put-text-property 2 3 'hilit-chg 'change-2)
                 (put-text-property 4 5 'hilit-chg 'change-1)
                 (goto-char 1)
                 (highlight-changes-next-change)
                 (point))
               (with-temp-buffer
                 (insert "abcde")
                 (setq-local highlight-changes-mode t)
                 (put-text-property 1 2 'hilit-chg 'change-1)
                 (put-text-property 2 3 'hilit-chg 'change-2)
                 (put-text-property 4 5 'hilit-chg 'change-1)
                 (goto-char 5)
                 (highlight-changes-previous-change)
                 (point))))"#),
        "(4 1)"
    );
}

#[test]
fn nxml_parent_document_provides_internal_dtd_entities() {
    assert_eq!(
        ev(r#"(progn
              (provide 'compile)
              (require 'cl-lib)
              (require 'nxml-mode)
              (require 'rng-valid)
              (let* ((parent (make-temp-file "nxml-parent-" nil ".xml"))
                     (child (make-temp-file "nxml-child-" nil ".xml"))
                     (pbuf nil)
                     (cbuf nil))
                (unwind-protect
                    (progn
                      (with-temp-file parent
                        (insert "<!DOCTYPE p [<!ELEMENT p ANY><!ENTITY shared \"ok\">]><p/>"))
                      (with-temp-file child
                        (insert "<p>&shared;</p>"))
                      (setq pbuf (find-file-noselect parent)
                            cbuf (find-file-noselect child))
                      (set-window-buffer (selected-window) pbuf)
                      (with-current-buffer pbuf
                        (nxml-mode)
                        (rng-validate-mode 1))
                      (set-window-buffer (selected-window) cbuf)
                      (with-current-buffer cbuf
                        (nxml-mode)
                        (rng-validate-mode 1)
                        (nxml-parent-document-set parent)
                        (not
                         (cl-some
                          (lambda (overlay)
                            (let ((help (overlay-get overlay 'help-echo)))
                              (and (stringp help)
                                   (string-match-p
                                    "Referenced entity has not been defined"
                                    help))))
                          (overlays-in (point-min) (point-max)))))
                  (when (buffer-live-p pbuf) (kill-buffer pbuf))
                  (when (buffer-live-p cbuf) (kill-buffer cbuf))
                  (delete-file parent)
                  (delete-file child))))"#),
        "t"
    );
}

#[test]
fn tar_rename_longlink_member_updates_archive_record() {
    assert_eq!(
        ev(r#"(progn
              (require 'tar-mode)
              (let* ((file-name-coding-system 'no-conversion)
                     (tar-file-name-coding-system 'no-conversion)
                     (old-name (make-string 110 ?a))
                     (new-name (make-string 620 ?b))
                     (long-size (1+ (length old-name)))
                     (long-header
                      (make-tar-header nil "././@LongLink" #o644 0 0 long-size
                                       (current-time) nil ?L nil "ustar "
                                       nil nil nil nil))
                     (file-header
                      (make-tar-header nil "short" #o644 0 0 0 (current-time)
                                       nil nil nil "ustar " nil nil nil nil))
                     (long-data
                      (with-temp-buffer
                        (set-buffer-multibyte nil)
                        (insert old-name 0)
                        (insert (make-string
                                 (- (tar-roundup-512 long-size) long-size) 0))
                        (buffer-string)))
                     (archive
                      (concat (tar-header-serialize long-header t)
                              long-data
                              (tar-header-serialize file-header t)
                              (make-string 1024 0))))
                (with-temp-buffer
                  (set-buffer-multibyte nil)
                  (insert archive)
                  (tar-mode)
                  (goto-char (point-min))
                  (let ((inhibit-read-only t))
                    (tar-rename-entry new-name))
                  (let ((renamed-archive
                         (with-current-buffer tar-data-buffer (buffer-string))))
                    (with-temp-buffer
                      (set-buffer-multibyte nil)
                      (insert renamed-archive)
                      (tar-mode)
                      (list
                       (equal (tar-header-name (car tar-parse-info)) new-name)
                       (= (length renamed-archive) (+ (length archive) 512))))))))"#),
        "(t t)"
    );
}

#[test]
fn tar_mode_revert_keeps_listing_visible_for_confirmation() {
    assert_eq!(
        ev(r#"(progn
              (require 'tar-mode)
              (let* ((file-name-coding-system 'no-conversion)
                     (tar-file-name-coding-system 'no-conversion)
                     (file (make-temp-file "tar-revert-" nil ".tar"))
                     (header (tar-new-regular-file-header "file.txt"))
                     (archive (concat (tar-header-serialize header t)
                                      (make-string 1024 0)))
                     (buffer (generate-new-buffer " *tar-revert-test*"))
                     prompt-listing)
                (unwind-protect
                    (progn
                      (write-region archive nil file)
                      (with-current-buffer buffer
                        (setq buffer-file-name file)
                        (set-buffer-multibyte nil)
                        (insert archive)
                        (tar-mode)
                        (set-buffer-modified-p t)
                        (cl-letf (((symbol-function 'yes-or-no-p)
                                   (lambda (&rest _)
                                     (setq prompt-listing
                                           (tar-data-swapped-p))
                                     t)))
                          (tar-mode-revert))
                        (list prompt-listing
                              (tar-data-swapped-p)
                              (not (buffer-modified-p)))))
                  (kill-buffer buffer)
                  (delete-file file))))"#),
        "(t t t)"
    );
}

#[test]
fn sql_mode_does_not_highlight_quoted_identifiers_as_keywords() {
    assert_eq!(
        ev(r#"(progn
              (require 'sql)
              (list
               (with-temp-buffer
                 (sql-mode)
                 (insert "SELECT \"select\" FROM mytable;")
                 (font-lock-ensure)
                 (goto-char (point-min))
                 (search-forward "\"select\"")
                 (backward-char 2)
                 (not (eq (get-text-property (point) 'face)
                          'font-lock-keyword-face)))
               (with-temp-buffer
                 (sql-mode)
                 (insert "SELECT [select] FROM mytable;")
                 (sql-set-product 'ms)
                 (font-lock-ensure)
                 (goto-char (point-min))
                 (search-forward "[select]")
                 (backward-char 2)
                 (not (eq (get-text-property (point) 'face)
                          'font-lock-keyword-face)))))"#),
        "(t t)"
    );
}

#[test]
fn gnus_picon_splits_after_quoted_local_part() {
    assert_eq!(
        ev(r#"(progn
              (require 'gnus-picon)
              (let ((parts
                     (gnus-picon-split-address
                      (concat "\"" "foo@bar" "\"@example.com"))))
                (list (equal (car parts) (concat "\"" "foo@bar" "\""))
                      (equal (cdr parts) '("example" "com"))
                      (equal (gnus-picon-split-address "foo@example.com")
                             '("foo" "example" "com")))))"#),
        "(t t t)"
    );
}

#[test]
fn sh_mode_highlights_in_only_after_list_constructs() {
    assert_eq!(
        ev(r#"(progn
              (require 'sh-script)
              (list
               (with-temp-buffer
                 (sh-mode)
                 (sh-set-shell "sh" nil nil)
                 (insert "echo in\n")
                 (font-lock-ensure)
                 (goto-char 6)
                 (eq (get-text-property (point) 'face) 'font-lock-keyword-face))
               (with-temp-buffer
                 (sh-mode)
                 (sh-set-shell "sh" nil nil)
                 (insert "for x in items; do\n")
                 (font-lock-ensure)
                 (goto-char (point-min))
                 (search-forward "in")
                 (backward-char 2)
                 (eq (get-text-property (point) 'face) 'font-lock-keyword-face))
               (with-temp-buffer
                 (sh-mode)
                 (sh-set-shell "sh" nil nil)
                 (insert "case x in\n  x) echo x;;\nesac\n")
                 (font-lock-ensure)
                 (goto-char (point-min))
                 (search-forward "in")
                 (backward-char 2)
                 (eq (get-text-property (point) 'face) 'font-lock-keyword-face))))"#),
        "(nil t t)"
    );
}

#[test]
fn cus_edit_loads_after_custom_widget_setup_definition() {
    assert_eq!(ev("(progn (require 'cus-edit) t)"), "t");
}

#[test]
fn texinfo_node_pointer_search_skips_excluded_output_blocks() {
    assert_eq!(
        ev(r#"(progn
              (require 'texnfo-upd)
              (list
               (with-temp-buffer
                 (insert "@node One\n@chapter One\n\n"
                         "@ignore\n@node Hidden\n@chapter Hidden\n@end ignore\n\n"
                         "@node Two\n@chapter Two\n")
                 (goto-char (point-min))
                 (search-forward "@node One")
                 (end-of-line)
                 (let ((kind (texinfo-find-pointer
                              (point-min) (point-max) 2 'next)))
                   (list kind (texinfo-pointer-name kind))))
               (with-temp-buffer
                 (insert "@node One\n@chapter One\n"
                         "@iftex\n@node TexOnly\n@chapter TexOnly\n@end iftex\n"
                         "@ifnottex\n@node InfoOnly\n@chapter InfoOnly\n@end ifnottex\n"
                         "@node Two\n@chapter Two\n")
                 (goto-char (point-min))
                 (search-forward "@node One")
                 (end-of-line)
                 (let ((kind (texinfo-find-pointer
                              (point-min) (point-max) 2 'next)))
                   (list kind (texinfo-pointer-name kind))))))"#),
        "((normal \"Two\") (normal \"InfoOnly\"))"
    );
}

#[test]
fn texinfo_menu_copy_preserves_punctuation_descriptions() {
    assert_eq!(
        ev(r#"(progn
              (require 'texnfo-upd)
              (mapcar
               (lambda (description)
                 (with-temp-buffer
                   (insert (format "@menu\n* Node:: %s\n@end menu\n" description))
                   (goto-char (point-min))
                   (let ((menu (list (cons "Node" nil))))
                     (texinfo-incorporate-descriptions menu)
                     (cdr (car menu)))))
               '("?Important" "^Caret" "|Pipe" "{Brace" "*Star" "@code")))"#),
        "(\"?Important\" \"^Caret\" \"|Pipe\" \"{Brace\" \"*Star\" \"@code\")"
    );
}

#[test]
fn texinfo_menu_after_top_is_inserted_inside_top() {
    assert_eq!(
        ev(r#"(progn
              (require 'texnfo-upd)
              (with-temp-buffer
                (insert "@setfilename manual.info\n"
                        "@node Top\n@top Manual\n\n"
                        "@node First\n@section First\n")
                (goto-char (point-min))
                (search-forward "@node First")
                (beginning-of-line)
                (texinfo-make-menu)
                (let* ((output (buffer-string))
                       (top (string-match "^@node Top" output))
                       (menu (string-match "^@menu" output))
                       (first (string-match "^@node First" output)))
                  (list (and top menu first (< top menu) (< menu first))
                        (and (string-match "^\\* First::" output) t)))))"#),
        "(t t)"
    );
}

#[test]
fn org_context_reports_latex_fragment_bounds() {
    assert_eq!(
        ev(r#"(progn
              (require 'org)
              (with-temp-buffer
                (org-mode)
                (insert "$a+b$")
                (goto-char 2)
                (assoc :latex-fragment (org-context))))"#),
        "(:latex-fragment 1 6)"
    );
}

#[test]
fn org_odt_table_closes_textbox_before_list_table() {
    assert_eq!(
        ev(r#"(progn
              (require 'ox-odt)
              (with-temp-buffer
                (org-mode)
                (insert "- List item\n"
                        "  - Sub-item\n"
                        "    #+begin_textbox\n"
                        "    | Table |\n"
                        "    #+end_textbox\n")
                (let* ((xml (org-export-as 'odt nil nil t))
                       (open (string-match "<draw:text-box" xml))
                       (close (and open
                                   (string-match "</draw:text-box>" xml open)))
                       (table (string-match "<table:table " xml))
                       (reopen (and table
                                    (string-match "<draw:text-box" xml table)))
                       (reclose (and reopen
                                     (string-match "</draw:text-box>" xml reopen))))
                  (list (and open close table (< open close) (< close table))
                        (and table reopen reclose (< table reopen)
                             (< reopen reclose))))))"#),
        "(t t)"
    );
}

#[test]
fn artist_detects_shift_changes_from_mouse_movement_events() {
    assert_eq!(
        ev(r#"(progn
              (require 'artist)
              (list
               (artist-shift-has-changed nil '(mouse-movement nil))
               (artist-shift-has-changed nil '(S-mouse-movement nil))
               (artist-shift-has-changed t '(S-mouse-movement nil))
               (artist-shift-has-changed t '(mouse-movement nil))))"#),
        "(nil t nil t)"
    );
}

#[test]
fn eudc_ecomplete_returns_only_requested_attributes() {
    assert_eq!(
        ev(r#"(progn
              (require 'eudcb-ecomplete)
              (cl-letf (((symbol-function 'ecomplete-setup) #'ignore)
                        ((symbol-function 'eudc-translate-attribute-list)
                         (lambda (_) '(mail)))
                        ((symbol-function 'ecomplete-get-matches)
                         (lambda (&rest _) "Alice <alice@example.com>"))
                        ((symbol-function 'mail-header-parse-address)
                         (lambda (&rest _) '("alice@example.com" . "Alice"))))
                (list
                 (eudc-ecomplete-query-internal '((mail . "alice")) '(name))
                 (eudc-ecomplete-query-internal '((mail . "alice")) '(mail))
                 (eudc-ecomplete-query-internal '((mail . "alice")) nil))))"#),
        "((((name . \"Alice\"))) (((mail . \"alice@example.com\"))) (((mail . \"alice@example.com\") (name . \"Alice\"))))"
    );
}

#[test]
fn tempo_o_tag_opens_line_when_inserting_region() {
    assert_eq!(
        ev(r#"(progn
              (require 'tempo)
              (setq tempo-template-test '(o "before " r " after"))
              (with-temp-buffer
                (insert "body")
                (goto-char (point-min))
                (set-mark (point-max))
                (tempo-insert-template 'tempo-template-test t)
                (buffer-string)))"#),
        "\"before \nbody after\""
    );
}

#[test]
fn register_preview_accepts_context_specialized_jump_methods() {
    assert_eq!(
        ev(r#"(progn
              (require 'register)
              (cl-defmethod register-val-jump-to ((val string) arg
                                                   &context (major-mode (eql text-mode)))
                'ok)
              (list
               (with-temp-buffer
                 (text-mode)
                 (list (register--jumpable-p "x")
                       (register-val-jump-to "x" nil)))
               (with-temp-buffer
                 (fundamental-mode)
                 (register--jumpable-p "x"))))"#),
        "((string ok) nil)"
    );
}

#[test]
fn rst_doctest_output_is_literal_and_indented_literal_takes_precedence() {
    assert_eq!(
        ev(r#"(progn
              (provide 'compile)
              (require 'rst)
              (list
               (with-temp-buffer
                 (rst-mode)
                 (font-lock-mode 1)
                 (insert ">>> print('hello')\nhello\n\nordinary text\n")
                 (font-lock-ensure)
                 (let* ((prompt (save-excursion
                                 (goto-char (point-min))
                                 (search-forward ">>>")
                                 (backward-char 3)
                                 (point)))
                       (output (save-excursion
                                 (goto-char (point-min))
                                 (search-forward "\nhello")
                                 (backward-char 5)
                                 (point)))
                       (ordinary (save-excursion
                                   (goto-char (point-min))
                                   (search-forward "ordinary text")
                                   (backward-char 13)
                                   (point)))
                       (prompt-face (get-text-property prompt 'face))
                       (output-face (get-text-property output 'face))
                       (ordinary-face (get-text-property ordinary 'face))
                       (prompt-faces (if (consp prompt-face)
                                         prompt-face (list prompt-face)))
                       (output-faces (if (consp output-face)
                                         output-face (list output-face)))
                       (ordinary-faces (if (consp ordinary-face)
                                           ordinary-face (list ordinary-face))))
                  (list (and (memq 'rst-block prompt-faces) t)
                        (and (memq 'rst-literal output-faces) t)
                        (not (memq 'rst-literal ordinary-faces)))))
               (with-temp-buffer
                 (rst-mode)
                 (font-lock-mode 1)
                 (insert "A literal block::\n\n    >>> This is not a doctest block.\n")
                 (font-lock-ensure)
                 (goto-char (point-min))
                 (search-forward ">>>")
                 (backward-char 3)
                 (let* ((face (get-text-property (point) 'face))
                        (faces (if (consp face) face (list face))))
                   (list (and (memq 'rst-literal faces) t)
                         (and (memq 'rst-block faces) t))))))"#),
        "((t t t) (t nil))"
    );
}

#[test]
fn idlwave_completion_does_not_skip_numeric_literals_as_identifiers() {
    assert_eq!(
        ev(r#"(progn
              (require 'idlwave)
              (cl-letf (((symbol-function 'idlwave-routines) #'ignore)
                        ((symbol-function 'idlwave-start-of-substatement)
                         (lambda (&rest _) (goto-char (point-min))))
                        ((symbol-function 'idlwave-what-function)
                         (lambda (&rest _) '("plot" nil nil 1 1)))
                        ((symbol-function 'idlwave-what-procedure)
                         (lambda (&rest _) '(nil nil nil 0 0))))
                (mapcar
                 (lambda (text)
                   (with-temp-buffer
                     (insert text)
                     (goto-char (point-max))
                     (list (char-to-string (idlwave-last-valid-char))
                           (nth 2 (idlwave-where)))))
                 '("plot(123" "plot(name"))))"#),
        "((\"3\" function) (\"(\" function-keyword))"
    );
}

#[test]
fn python_f_string_expression_highlights_inner_strings() {
    assert_eq!(
        ev(r#"(progn
              (require 'python)
              (with-temp-buffer
                (python-mode)
                (font-lock-mode 1)
                (insert "f\"{'inner'}\"\n")
                (font-lock-ensure)
                (goto-char (point-min))
                (search-forward "'inner'")
                (backward-char 7)
                (let* ((face (get-text-property (point) 'face))
                       (faces (if (consp face) face (list face))))
                  (and (memq 'font-lock-string-face faces) t))))"#),
        "t"
    );
}

#[test]
fn handwrite_form_feed_starts_new_postscript_page() {
    assert_eq!(
        ev(r#"(progn
              (require 'handwrite)
              (let ((handwrite-numlines 100000)
                    (handwrite-pagenumbering nil))
                (fset 'y-or-n-p (lambda (&rest _args) nil))
                (with-temp-buffer
                  (insert "first\fsecond\n")
                  (handwrite)
                  (with-current-buffer "*handwritten1.ps*"
                    (let* ((output (buffer-string))
                           (first (string-match "first" output))
                           (page-two (string-match "%%Page: 2 2" output))
                           (second (string-match "second" output)))
                      (list (how-many "^%%Page:" (point-min) (point-max))
                            (and first page-two second
                                 (< first page-two) (< page-two second))
                            (not (string-match "\f" output))))))))"#),
        "(2 t t)"
    );
}

#[test]
fn pp_fill_uses_visible_width() {
    // Test text display, invisible text, and overlay display strings.
    assert_eq!(
        ev(r#"(progn
              (require 'pp)
              (list
               (with-temp-buffer
                 (emacs-lisp-mode)
                 (insert "(f123456789 bar)")
                 (setq-local fill-column 8)
                 (add-text-properties 2 12 '(display ""))
                 (pp-fill (point-min) (point-max))
                 (not (string-match-p "\n" (buffer-string))))
               (with-temp-buffer
                 (emacs-lisp-mode)
                 (insert "(f123456789 bar)")
                 (setq-local fill-column 8 buffer-invisibility-spec t)
                 (add-text-properties 2 12 '(invisible t))
                 (pp-fill (point-min) (point-max))
                 (not (string-match-p "\n" (buffer-string))))
               (with-temp-buffer
                 (emacs-lisp-mode)
                 (insert "(foo bar)")
                 (let ((overlay (make-overlay 2 5)))
                   (overlay-put overlay 'display "abcdefghijklmnop"))
                 (setq-local fill-column 20)
                 (pp-fill (point-min) (point-max))
                 (and (string-match-p "\n" (buffer-string)) t))))"#),
        "(t t t)"
    );
}

#[test]
fn edebug_frequency_display_drops_noncharacter_events() {
    assert_eq!(
        ev(r#"(progn
              (require 'edebug)
              (require 'cl-lib)
              (let ((unread-command-events nil))
                (cl-letf (((symbol-function 'edebug-display-freq-count)
                           (lambda () nil))
                          ((symbol-function 'read-event)
                           (lambda (&rest _) [left]))
                          ((symbol-function 'undo)
                           (lambda (&rest _) nil)))
                  (edebug-temp-display-freq-count)
                  (null unread-command-events)))"#),
        "t"
    );
}

#[test]
fn calc_pass_errors_bypasses_calc_do_error_handler() {
    assert_eq!(
        ev(r#"(progn
              (require 'calc-prog)
              (let ((calc-pass-errors-enabled nil)
                    (calc-embedded-info nil)
                    (calc-algebraic-mode nil)
                    (calc-timing nil)
                    recorded)
                (cl-letf (((symbol-function 'calc-check-defines)
                           (lambda () nil))
                          ((symbol-function 'calc-select-buffer)
                           (lambda () nil))
                          ((symbol-function 'calc-record)
                           (lambda (&rest args) (setq recorded args)))
                          ((symbol-function 'calc-align-stack-window)
                           (lambda () nil)))
                  (condition-case nil
                      (calc-do (lambda () (error "probe")))
                    (error nil))
                  (let ((caught-before (null recorded)))
                    (setq recorded nil)
                    (calc-pass-errors)
                    (condition-case nil
                        (calc-do (lambda () (error "probe")))
                      (error nil))
                    (list caught-before
                          (equal (car recorded) "<Aborted>")
                          calc-pass-errors-enabled)))))"#),
        "(t t t)"
    );
}

#[test]
fn prog_text_at_point_ignores_comment_start_inside_string() {
    assert_eq!(
        ev(r#"(progn
              (require 'prog-mode)
              (list
               (with-temp-buffer
                 (emacs-lisp-mode)
                 (insert "(message \"; in string\")\n")
                 (goto-char (point-max))
                 (backward-char 1)
                 (and (prog--text-at-point-p) t))
               (with-temp-buffer
                 (emacs-lisp-mode)
                 (insert "(message \"text\") ; comment\n")
                 (goto-char (point-min))
                 (search-forward "text")
                 (search-forward ")")
                 (backward-char 1)
                 (and (prog--text-at-point-p) t))))"#),
        "(nil t)"
    );
}

#[test]
fn comment_indent_new_line_uses_explicit_comment_continue() {
    assert_eq!(
        ev(r#"(progn
              (require 'newcomment)
              (with-temp-buffer
                (emacs-lisp-mode)
                (setq-local comment-start ";; "
                            comment-start-skip ";+ *"
                            comment-continue ";;> ")
                (insert ";; text")
                (goto-char (point-max))
                (comment-indent-new-line)
                (equal (buffer-string) ";; text\n;;> ")))"#),
        "t"
    );
}

#[test]
fn smerge_markers_face_is_applied_inside_strings() {
    assert_eq!(
        ev(r#"(progn
              (require 'smerge-mode)
              (with-temp-buffer
                (emacs-lisp-mode)
                (setq-local font-lock-mode t)
                (insert "(setq x \"\n<<<<<<< ours\na\n=======\nb\n>>>>>>> theirs\n\")\n")
                (smerge-mode 1)
                (font-lock-ensure)
                (goto-char (point-min))
                (search-forward "<<<<<<<")
                (backward-char 1)
                (let* ((face (get-text-property (point) 'face))
                       (faces (if (consp face) face (list face))))
                  (and (memq 'smerge-markers-face faces)
                       (memq 'font-lock-string-face faces)
                       t))))"#),
        "t"
    );
}

#[test]
fn mpc_constraints_pop_empty_ring_reports_no_selection() {
    assert_eq!(
        ev(r#"(progn
              (require 'mpc)
              (condition-case err
                  (mpc-constraints-pop)
                (error (cadr err))))"#),
        "\"No selection to return to\""
    );
}

#[test]
fn org_agenda_next_date_line_handles_leading_whitespace() {
    assert_eq!(
        ev(r#"(progn
              (require 'org-agenda)
              (require 'cl-lib)
              (let ((org-agenda-type 'agenda))
                (with-temp-buffer
                  (insert "  Monday\n  Tuesday\n")
                  (goto-char (point-min))
                  (cl-letf (((symbol-function 'org-agenda-check-type)
                             (lambda (&rest _) nil)))
                    (org-agenda-next-date-line))
                  (= (line-number-at-pos) 2))))"#),
        "t"
    );
}

#[test]
fn doc_view_persists_page_and_slice_per_window() {
    assert_eq!(
        ev(r#"(progn
              (require 'doc-view)
              (let* ((window (selected-window))
                     (image-mode-winprops-alist nil)
                     (winprops (image-mode-winprops window))
                     saved-state serialized)
                (unwind-protect
                    (progn
                      (setq buffer-file-name "test.pdf")
                      (image-mode-window-put 'page 5 winprops)
                      (image-mode-window-put 'slice '(1 2 30 40) winprops)
                      (doc-view--save-window-state-parameters window)
                      (setq saved-state
                            (window-parameter window 'doc-view-window-state)
                            serialized
                            (and (string-match-p
                                  "doc-view-window-state"
                                  (prin1-to-string (window-state-get window t)))
                                 t))
                      (set-window-parameter window 'doc-view-window-state
                                            '("other.pdf" 9 nil))
                      (doc-view--restore-window-state-parameters winprops)
                      (let ((other-file-unchanged
                             (eq (image-mode-window-get 'page winprops) 5)))
                        (set-window-parameter window 'doc-view-window-state
                                              '("test.pdf" 9 nil))
                        (doc-view--restore-window-state-parameters winprops)
                        (list (equal saved-state '("test.pdf" 5 (1 2 30 40)))
                              serialized
                              other-file-unchanged
                              (eq (image-mode-window-get 'page winprops) 9)
                              (null (image-mode-window-get 'slice winprops)))))
                  (set-window-parameter window 'doc-view-window-state nil))))"#),
        "(t t t t t)"
    );
}

#[test]
fn ange_ftp_set_file_modes_rejects_unsupported_nofollow() {
    assert_eq!(
        ev(r#"(progn
              (require 'ange-ftp)
              (let (calls)
                (cl-letf (((symbol-function 'ange-ftp-call-chmod)
                           (lambda (args) (push args calls))))
                  (ange-ftp-set-file-modes "/user@host:/link" #o600 nil)
                  (let ((error-type
                         (condition-case err
                             (progn
                               (ange-ftp-set-file-modes
                                "/user@host:/link" #o600 'nofollow)
                               'no-error)
                           (error (car err)))))
                    (list (nreverse calls) error-type)))))"#),
        "(((\"600\" \"/user@host:/link\")) file-error)"
    );
}

#[test]
fn apply_on_rectangle_normalizes_reversed_line_order() {
    assert_eq!(
        ev(r#"(progn
              (require 'rect)
              (with-temp-buffer
                (insert "aaa\nbbb\nccc")
                (let (lines)
                  (apply-on-rectangle
                   (lambda (_startcol _endcol)
                     (push (line-number-at-pos) lines))
                   (progn (goto-char (point-max)) (point))
                   (point-min))
                  (nreverse lines))))"#),
        "(1 2 3)"
    );
}

#[test]
fn proced_loads_and_reports_attributes() {
    // GNU-verified on 31.1: proced.el loads with zero requires;
    // proced-process-attributes returns (pid . attr-alist) pairs whose
    // key order matches GNU's process-attributes contract.
    assert_eq!(
        ev("(progn (require 'proced)
                  (list (keymapp proced-mode-map)
                        (length proced-grammar-alist)
                        (fboundp 'proced-update)
                        (fboundp 'proced-mode)
                        proced-available))"),
        "(t 33 t t t)"
    );
    assert_eq!(
        ev("(progn (require 'proced)
                  (let* ((p (emacs-pid))
                         (out (proced-process-attributes (list p))))
                    (list (length out)
                          (eq (caar out) p)
                          (mapcar #'car (cdar out)))))"),
        "(1 t (pid args thcount rss vsize etime start nice majflt time stime utime tpgid pgrp ppid state comm group egid user euid))"
    );
}

// ------------------------------------------------------- eieio + registry

#[test]
fn registry_db_insert_lookup_search() {
    // GNU 31.1: `(registry-db)' constructor runs `initialize-instance'
    // :before/:after methods (data/tracker hash setup); lookup returns
    // (key entry) alists and misses yield nil.
    assert_eq!(
        ev("(progn (require 'registry)
                  (let ((db (registry-db)))
                    (registry-insert db \"k1\" 10)
                    (registry-insert db \"k2\" 20)
                    (list (registry-lookup db (list \"k1\"))
                          (registry-lookup db (list \"none\"))
                          (sort (registry-search db :all t)
                                (lambda (a b) (string< a b)))
                          (progn (registry-prune db) t))))"),
        "(((\"k1\" 10)) nil (\"k1\" \"k2\") t)"
    );
}

#[test]
fn cl_loop_hash_iteration() {
    // GNU cl-loop: `for VAR being the hash-keys of TABLE using
    // (hash-values V)' and the hash-values counterpart.
    assert_eq!(
        ev("(require 'cl-lib)
            (let ((h (make-hash-table :test 'equal)))
              (puthash \"a\" 1 h)
              (puthash \"b\" 2 h)
              (list (sort (cl-loop for k being the hash-keys of h
                                   using (hash-values v)
                                   collect (cons k v))
                          (lambda (a b) (string< (car a) (car b))))
                    (sort (cl-loop for v being the hash-values of h
                                   using (hash-keys k)
                                   collect (cons v k))
                          (lambda (a b) (< (car a) (car b))))))"),
        "(((\"a\" . 1) (\"b\" . 2)) ((1 . \"a\") (2 . \"b\")))"
    );
}

#[test]
fn server_local_socket_round_trip() {
    // GNU-verified on 31.1: server-start binds a Unix socket under
    // server-socket-dir, server-running-p probes it, and server-eval-at
    // evals a form through the real -eval/-print protocol.
    assert_eq!(
        ev("(progn (require 'server)
                  (list (fboundp 'server-start)
                        (fboundp 'server-running-p)
                        (fboundp 'server-eval-at)
                        (stringp server-socket-dir)))"),
        "(t t t t)"
    );
    assert_eq!(
        ev("(progn (require 'server)
                  (setq server-name \"remacs-test-sock\")
                  (server-start)
                  (list (server-running-p \"remacs-test-sock\")
                        (processp server-process)
                        (server-eval-at \"remacs-test-sock\" '(+ 20 22))))"),
        "(t t 42)"
    );
}

// -------------------------------------- recentf/widget chain & load args

#[test]
fn recentf_widget_chain_loads() {
    // GNU 31.1 -Q: recentf pulls tree-widget -> wid-edit -> widget; all
    // load verbatim (propertized-string literals #("..." ...) included).
    assert_eq!(
        ev("(progn (require 'recentf)
                  (list (fboundp 'recentf-mode)
                        (fboundp 'recentf-open-files)
                        (boundp 'recentf-mode)
                        (fboundp 'widget-create)
                        (fboundp 'define-widget)))"),
        "(t t t t t)"
    );
    // `recentf-mode' toggles the global minor mode; activation loads
    // the save file and runs a cleanup pass (GNU-verified).
    assert_eq!(
        ev("(progn (require 'recentf)
                  (let ((recentf-save-file (make-temp-file \"remacs-recentf\")))
                    (unwind-protect
                        (progn (recentf-mode 1)
                               (list recentf-mode (listp recentf-list)))
                      (delete-file recentf-save-file))))"),
        "(t t)"
    );
}

#[test]
fn propertized_string_reader_syntax() {
    // GNU reader: #("ab" 0 1 (face bold)) -> string "ab" with
    // (face bold) on chars [0,1).  GNU-verified on 31.1.
    assert_eq!(
        ev(
            "(let ((s (car (read-from-string \"#(\\\"ab\\\" 0 1 (face bold))\"))))
             (list s (get-text-property 0 'face s)))"
        ),
        "(#(\"ab\" 0 1 (face bold)) bold)"
    );
}

#[test]
fn load_optional_arguments() {
    // GNU-verified on 31.1: NOERROR suppresses file-missing.
    assert_eq!(
        ev("(list (load \"/nonexistent/remacs-no-such-lib\" t)
                  (condition-case e (load \"/nonexistent/remacs-no-such-lib\")
                    (file-missing (car e))))"),
        "(nil file-missing)"
    );
    // NOSUFFIX opens the literal name only (no .el/.elc appending).
    assert_eq!(
        ev(
            "(condition-case e (load \"nonexistent/remacs-no-such-lib\" nil nil t)
             (file-missing (car e)))"
        ),
        "file-missing"
    );
    // A successful load returns t (GNU `load' -> t).
    assert_eq!(
        ev("(progn (require 'cl-lib)
                  (let ((f (make-temp-file \"remacs-load\" nil \".el\")))
                    (unwind-protect
                        (progn (with-temp-file f (insert \"(setq remacs--load-arg-test 41)\"))
                               (list (load f) remacs--load-arg-test))
                      (delete-file f))))"),
        "(t 41)"
    );
    // NOMESSAGE controls GNU's `Loading FILE (source)...' message; in
    // batch GNU sends it to stderr (and *Messages*), not stdout.
    let out = ev_out(
        "(let ((f (make-temp-file \"remacs-load\" nil \".el\")))
            (unwind-protect
                (progn (load f) (load f nil t))
              (delete-file f)))",
    );
    assert_eq!(out, "", "load messages must not hit stdout");
    let msgs = ev("(let ((f (make-temp-file \"remacs-load\" nil \".el\")))
            (unwind-protect
                (progn (load f) (load f nil t)
                       (with-current-buffer \"*Messages*\" (buffer-string)))
              (delete-file f)))");
    assert!(
        msgs.contains("Loading ") && msgs.contains("(source)..."),
        "expected `Loading FILE (source)...' in *Messages*, got: {msgs}"
    );
}

// -------------------------------------- compile/comint + process lifecycle

#[test]
fn compile_runs_subprocess_to_exit() {
    // GNU 31.1 -Q: compile.el/comint.el load verbatim; `compile' runs a
    // subprocess under compilation-mode and its sentinel announces
    // "Compilation finished" on exit.
    assert_eq!(
        ev("(progn (require 'compile)
                  (list (fboundp 'compile) (fboundp 'compilation-start)
                        (fboundp 'compilation-mode) (fboundp 'comint-mode)
                        (fboundp 'map-y-or-n-p) (fboundp 'save-some-buffers)
                        (boundp 'other-window-scroll-buffer)))"),
        "(t t t t t t t)"
    );
    // A real compile run: the process exits, the buffer carries GNU's
    // header/output/finish lines, and `process-status' stays `exit'
    // even after `delete-process' (GNU keeps the reported status).
    assert_eq!(
        ev(r#"
            (progn
              (require 'compile)
              (let ((shell-file-name "/bin/sh") (shell-command-switch "-c"))
                ;; Output deliberately arrives before the process exits.
                (compile "printf hello-out; sleep 0.25")
                (let ((process (get-buffer-process "*compilation*"))
                      (deadline (+ (float-time) 5)))
                  (unwind-protect
                      (progn
                        (while (and (process-live-p process) (< (float-time) deadline))
                          (accept-process-output process 0.1))
                        (unless (eq (process-status process) 'exit)
                          (error "Compilation did not exit before the deadline"))
                        (delete-process process)
                        (delete-process process)
                        (list (process-status process)
                              (with-current-buffer "*compilation*"
                                (and (string-match-p "Compilation finished" (buffer-string))
                                     (string-match-p "hello-out" (buffer-string))
                                     t))))
                    (when (process-live-p process) (delete-process process))))))
        "#),
        "(exit t)"
    );
}

// -------------------------------------- float-sup + autoloaded misc libs

#[test]
fn float_sup_is_dumped() {
    // GNU 31.1 -Q: float-sup.el is dumped (`lisp-float-type' feature),
    // so float-pi & the degree/radian converters are bound at startup.
    assert_eq!(
        ev("(list (boundp 'float-pi) float-pi
                  (degrees-to-radians 180)
                  (radians-to-degrees float-pi)
                  (featurep 'lisp-float-type))"),
        "(t 3.141592653589793 3.141592653589793 180.0 t)"
    );
}

#[test]
fn autoloaded_misc_libraries() {
    // GNU 31.1 -Q registers these as autoloads (loaddefs.el); calling
    // an entry point loads the library.  `benchmark-run' & friends are
    // macro autoloads (TYPE = t).
    assert_eq!(
        ev("(list (autoloadp (symbol-function 'benchmark-run))
                  (autoloadp (symbol-function 'animate-string))
                  (autoloadp (symbol-function 'cursor-sensor-mode))
                  (autoloadp (symbol-function 'Helper-help))
                  (autoloadp (symbol-function 'string-edit))
                  (autoloadp (symbol-function 'read-string-from-buffer))
                  (autoloadp (symbol-function 'list-timers)))"),
        "(t t t t t t t)"
    );
    // Calling resolves the autoload and defines the real function.
    assert_eq!(
        ev("(progn (benchmark-run 1 (+ 1 2))
                  (list (fboundp 'benchmark-run)
                        (fboundp 'benchmark-elapse)
                        (progn (animate-string \"x\" 0) t)
                        (fboundp 'animate-sequence)))"),
        "(t t t t)"
    );
    assert_eq!(
        ev("(progn (require 'bib-mode)
                  (list (fboundp 'bib-mode) (fboundp 'bib-find-key)))"),
        "(t t)"
    );
}

// ------------------------------------ shadow/inline/compat & fill modes

#[test]
fn misc_libs_autoloads_and_crm_separator() {
    // GNU 31.1 -Q registers these as autoloads via loaddefs.el.
    assert_eq!(
        ev("(list (autoloadp (symbol-function 'define-inline))
                  (autoloadp (symbol-function 'list-load-path-shadows))
                  (autoloadp (symbol-function 'refill-mode))
                  (autoloadp (symbol-function 'word-wrap-whitespace-mode))
                  (autoloadp (symbol-function 'global-word-wrap-whitespace-mode))
                  (autoloadp (symbol-function 'glyphless-display-mode))
                  (autoloadp (symbol-function 'timeout-debounce))
                  (autoloadp (symbol-function 'timeout-throttle))
                  (autoloadp (symbol-function 'timeout-debounced-func))
                  (autoloadp (symbol-function 'timeout-throttled-func)))"),
        "(t t t t t t t t t t)"
    );
    // GNU -Q: `crm-separator' is declared but unbound; requiring crm
    // binds it to the propertized separator regexp.
    assert_eq!(ev("(boundp 'crm-separator)"), "nil");
    assert_eq!(
        ev("(progn (require 'crm) crm-separator)"),
        "#(\"[ \t]*,[ \t]*\" 0 11 (separator \",\" description \"comma-separated list\"))"
    );
    // compat.el's loaddefs cookie pushes its builtin version.
    assert_eq!(
        ev("(assoc 'compat package--builtin-versions)"),
        "(compat 31 1 9999)"
    );
}

#[test]
fn misc_libs_load_and_entry_points() {
    assert_eq!(
        ev(
            "(progn (dolist (l '(shadow inline compat refill word-wrap-mode
                              glyphless-mode timeout pixel-fill))
                    (require l))
                  (list (fboundp 'load-path-shadows-find)
                        (fboundp 'define-inline)
                        (fboundp 'compat-function)
                        (fboundp 'refill-mode)
                        (fboundp 'word-wrap-whitespace-mode)
                        (fboundp 'glyphless-display-mode)
                        (fboundp 'timeout-throttle)
                        (fboundp 'pixel-fill-region)))"
        ),
        "(t t t t t t t t)"
    );
    // define-inline works (verified against GNU).
    assert_eq!(
        ev("(progn (require 'inline)
                  (define-inline remacs-test-inline (x) (+ x 1))
                  (remacs-test-inline 5))"),
        "6"
    );
}

// ------------------------------------ misc modes & trace (GNU loaddefs autoloads)

#[test]
fn misc_mode_libs_autoloads() {
    // GNU 31.1 -Q: all are autoload cells via loaddefs.el.
    assert_eq!(
        ev("(list (car (symbol-function 'po-find-file-coding-system))
                  (nth 1 (symbol-function 'po-find-file-coding-system))
                  (car (symbol-function 'asm-mode)) (nth 1 (symbol-function 'asm-mode))
                  (car (symbol-function 'm4-mode)) (nth 1 (symbol-function 'm4-mode))
                  (car (symbol-function 'bat-mode)) (nth 1 (symbol-function 'bat-mode))
                  (car (symbol-function 'autoconf-mode)) (nth 1 (symbol-function 'autoconf-mode))
                  (car (symbol-function 'ld-script-mode)) (nth 1 (symbol-function 'ld-script-mode))
                  (car (symbol-function 'bibtex-style-mode)) (nth 1 (symbol-function 'bibtex-style-mode))
                  (car (symbol-function 'emacs-authors-mode)) (nth 1 (symbol-function 'emacs-authors-mode))
                  (car (symbol-function 'cl-font-lock-built-in-mode)) (nth 1 (symbol-function 'cl-font-lock-built-in-mode))
                  (car (symbol-function 'memory-report)) (nth 1 (symbol-function 'memory-report)))"),
        "(autoload \"po\" autoload \"asm-mode\" autoload \"m4-mode\" autoload \"bat-mode\" autoload \"autoconf\" autoload \"ld-script\" autoload \"bibtex-style\" autoload \"emacs-authors-mode\" autoload \"cl-font-lock\" autoload \"memory-report\")"
    );
    // GNU: trace-buffer bound at startup; trace-function is a defalias.
    assert_eq!(
        ev("(list (boundp 'trace-buffer) trace-buffer (symbol-function 'trace-function))"),
        "(t \"*trace-output*\" trace-function-foreground)"
    );
}

#[test]
fn misc_mode_libs_require() {
    assert_eq!(
        ev("(progn (dolist (l '(asm-mode m4-mode bat-mode autoconf
                              ld-script bibtex-style emacs-authors-mode
                              cl-font-lock po trace memory-report))
                    (require l))
                  (list (featurep 'asm-mode) (fboundp 'asm-calculate-indentation)
                        (featurep 'm4-mode) (featurep 'bat-mode)
                        (featurep 'autoconf) (featurep 'ld-script)
                        (featurep 'bibtex-style) (featurep 'emacs-authors-mode)
                        (featurep 'cl-font-lock) (fboundp 'po-find-charset)
                        (featurep 'trace) (fboundp 'untrace-all)
                        (featurep 'memory-report)))"),
        "(t t t t t t t t t t t t t)"
    );
    // trace actually traces (GNU-verified): trace-is-traced reports t.
    assert_eq!(
        ev("(progn (require 'trace)
                  (list (trace-is-traced 'car)
                        (progn (trace-function-background 'car)
                               (not (null (trace-is-traced 'car))))))"),
        "(nil t)"
    );
}

// ------------------------------------ executable/generate-lisp-file/debug-early

#[test]
fn executable_and_debug_early_parity() {
    // GNU 31.1 -Q: debug-early is dumped (fbound, feature stays nil
    // since the file has no `provide'); executable entry points are
    // loaddefs autoloads.
    assert_eq!(
        ev(
            "(list (fboundp 'debug-early) (fboundp 'debug-early-backtrace)
                  (featurep 'debug-early)
                  (car (symbol-function 'executable-interpret))
                  (nth 1 (symbol-function 'executable-interpret))
                  (car (symbol-function 'executable-set-magic))
                  (car (symbol-function 'executable-command-find-posix-p))
                  (car (symbol-function 'executable-make-buffer-file-executable-if-script-p)))"
        ),
        "(t t nil autoload \"executable\" autoload autoload autoload)"
    );
    // GNU: executable-interpret requires its COMMAND argument; the
    // no-arg call signals exactly this arity error (needs call-process
    // MANY arity to reach it).
    assert_eq!(
        ev("(progn (require 'executable)
                  (list (featurep 'executable)
                        (fboundp 'executable-find)
                        (executable-command-find-posix-p \"sh\")
                        (executable-command-find-posix-p \"find\")
                        (condition-case e (executable-interpret)
                          (wrong-number-of-arguments (list 'arity (car e))))))"),
        "(t t nil t (arity wrong-number-of-arguments))"
    );
    // generate-lisp-file is a plain library (no autoloads in GNU).
    assert_eq!(
        ev("(progn (require 'generate-lisp-file)
                  (list (featurep 'generate-lisp-file)
                        (fboundp 'generate-lisp-file-trailer)))"),
        "(t t)"
    );
}

// ------------------------------------ mail/international libraries

#[test]
fn mail_international_libraries_parity() {
    // GNU 31.1 -Q: these entry points are loaddefs autoloads; the
    // libraries themselves are not dumped (featurep nil at startup).
    assert_eq!(
        ev("(list (car (symbol-function 'binhex-decode-region))
                  (car (symbol-function 'binhex-decode-region-external))
                  (car (symbol-function 'binhex-decode-region-internal))
                  (car (symbol-function 'fill-flowed))
                  (car (symbol-function 'fill-flowed-encode))
                  (car (symbol-function 'latexenc-coding-system-to-inputenc))
                  (car (symbol-function 'latexenc-find-file-coding-system))
                  (car (symbol-function 'latexenc-inputenc-to-coding-system))
                  (car (symbol-function 'quoted-printable-decode-region))
                  (car (symbol-function 'uudecode-decode-region))
                  (car (symbol-function 'uudecode-decode-region-external))
                  (car (symbol-function 'uudecode-decode-region-internal))
                  (car (symbol-function 'yenc-decode-region))
                  (car (symbol-function 'yenc-extract-filename))
                  (featurep 'mm-util) (featurep 'rfc2047)
                  (featurep 'ietf-drums) (featurep 'mail-parse)
                  (featurep 'rfc6068) (featurep 'iso-ascii))"),
        "(autoload autoload autoload autoload autoload autoload autoload autoload autoload autoload autoload autoload autoload autoload nil nil nil nil nil nil)"
    );
    // Autoload target library names (GNU-verified).
    assert_eq!(
        ev("(list (nth 1 (symbol-function 'binhex-decode-region))
                  (nth 1 (symbol-function 'fill-flowed))
                  (nth 1 (symbol-function 'quoted-printable-decode-region))
                  (nth 1 (symbol-function 'uudecode-decode-region))
                  (nth 1 (symbol-function 'yenc-decode-region))
                  (nth 1 (symbol-function 'latexenc-inputenc-to-coding-system)))"),
        "(\"binhex\" \"flow-fill\" \"qp\" \"uudecode\" \"yenc\" \"latexenc\")"
    );
    // GNU-verified functional results.
    assert_eq!(
        ev("(progn (dolist (l '(ietf-drums rfc6068 qp rfc2045 rfc2047 rfc2231
                              mail-parse mm-util latexenc flow-fill mail-prsvr
                              iso-ascii mailheader yenc uudecode binhex))
                    (require l))
                  (list (ietf-drums-parse-address \"user@example.com\")
                        (ietf-drums-parse-address \"\\\"Joe\\\" <j@x.org>\")
                        (rfc6068-parse-mailto-url \"mailto:a@b?subject=x\")
                        (with-temp-buffer (insert \"abc=20def=0A\")
                          (quoted-printable-decode-region (point-min) (point-max))
                          (buffer-string))
                        (rfc2045-encode-string \"name\" \"value\")
                        (rfc2047-decode-string \"=?us-ascii?q?hello?=\")
                        (rfc2231-parse-string \"foo=bar; baz=qux\")
                        (mail-header-parse-address \"\\\"Joe\\\" <j@x.org>\")
                        (mm-charset-to-coding-system 'us-ascii)
                        (latexenc-inputenc-to-coding-system \"utf8\")
                        (boundp 'mail-parse-charset) mail-parse-ignored-charsets
                        (fboundp 'iso-ascii-mode)
                        (with-temp-buffer (insert \"f=f! f=\\nf\")
                          (fill-flowed) (buffer-string))
                        (fboundp 'mail-header-extract)
                        (fboundp 'uudecode-decode-region)
                        (fboundp 'binhex-decode-region)
                        (fboundp 'yenc-decode-region)
                        (featurep 'mm-util) (featurep 'rfc2047)
                        (featurep 'rfc2231) (featurep 'mail-parse)))"),
        "((\"user@example.com\") (\"j@x.org\" . \"Joe\") ((\"To\" . \"a@b\") (\"Subject\" . \"x\")) \"abc def
\" \"name=value\" \"hello\" (\"foo\") (\"j@x.org\" . \"Joe\") ascii utf-8 t nil t \"f=f! f=
f\" t t t t t t t t)"
    );
    // rfc1843, ja-dic-utl and mailheader also `provide' their features.
    assert_eq!(
        ev(
            "(progn (dolist (l '(rfc1843 ja-dic-utl mailheader)) (require l))
                  (list (fboundp 'rfc1843-decode-region)
                        (fboundp 'skkdic-lookup-key)
                        (fboundp 'mail-header-extract)
                        (featurep 'rfc1843) (featurep 'ja-dic-utl)
                        (featurep 'mailheader)))"
        ),
        "(t t t t t t)"
    );
}

#[test]
fn inline_alias_and_coding_system_list_parity() {
    // GNU: `inline' is a defalias to the `progn' special form (byte-run.el);
    // eval resolves the alias and calls it with unevaluated arguments.
    assert_eq!(
        ev("(list (inline (+ 1 2))
                  (inline (list 'a 'b) (+ 3 4))
                  (fboundp 'inline))"),
        "(3 7 t)"
    );
    // GNU: (coding-system-list &optional BASE-ONLY) accepts one optional
    // argument; BASE-ONLY keeps base coding systems (utf-8 stays,
    // utf-8-unix is not a base name in GNU either way).
    assert_eq!(
        ev("(list (not (null (memq 'utf-8 (coding-system-list))))
                  (not (null (memq 'utf-8 (coding-system-list t))))
                  (memq 'utf-8-unix (coding-system-list t))
                  (condition-case e (coding-system-list t t)
                    (wrong-number-of-arguments (list 'arity (car e)))))"),
        "(t t nil (arity wrong-number-of-arguments))"
    );
}

// ------------------------- cond-star/derived/generic/range/timer-list/copyright

#[test]
fn mode_macro_autoload_cells_and_lazy_load() {
    // GNU -Q: `define-derived-mode'/`define-generic-mode' are loaddefs
    // autoload cells — GNU's dumped mode definitions were byte-compiled,
    // so nothing resolves them at startup.  Remacs expands its dumped
    // `define-derived-mode' calls with a prelude-local bootstrap macro
    // and restores the autoload cell after the dumped loads, so the
    // -Q observable state matches GNU: autoloads present, features off.
    assert_eq!(
        ev("(list (autoloadp (symbol-function 'define-derived-mode))
                  (autoloadp (symbol-function 'define-generic-mode))
                  (featurep 'derived) (featurep 'generic)
                  (fboundp 'prog-mode) (fboundp 'text-mode)
                  (fboundp 'lisp-mode) (fboundp 'completion-list-mode))"),
        "(t t nil nil t t t t)"
    );
    // First use lazy-loads the real GNU derived.el.
    assert_eq!(
        ev(
            "(progn (define-derived-mode remacs--test-derived prog-mode \"TestDerived\" \"Doc.\")
                  (list (featurep 'derived) (fboundp 'remacs--test-derived)
                        (get 'remacs--test-derived 'derived-mode-parent)))"
        ),
        "(t t prog-mode)"
    );
    // Same for generic.el.
    assert_eq!(
        ev(
            "(progn (define-generic-mode 'remacs--test-generic '(\"#\") nil nil nil nil \"Doc.\")
                  (list (featurep 'generic) (fboundp 'remacs--test-generic)))"
        ),
        "(t t)"
    );
}

#[test]
fn cond_star_range_timer_list_copyright_parity() {
    // GNU -Q: cond*/match*/list-timers/copyright-update are autoloads;
    // `range' and the features stay unloaded.  GNU-verified on 31.1.
    assert_eq!(
        ev("(list (autoloadp (symbol-function 'cond*))
                  (autoloadp (symbol-function 'match*))
                  (autoloadp (symbol-function 'list-timers))
                  (autoloadp (symbol-function 'copyright-update))
                  (fboundp 'copyright))"),
        "(t t t t t)"
    );
    // Functionality after load — byte-identical to GNU 31.1.
    assert_eq!(
        ev("(list (cond* ((> 1 2) 'no) ((< 1 2) 'yes) (t 'other))
                  (progn (require 'range)
                         (list (range-compress-list '(1 2 3 5 7 8 9))
                               (range-difference '(1 . 10) '(4 . 6))))
                  (progn (require 'copyright) (fboundp 'copyright-update))
                  (progn (require 'timer-list) (fboundp 'list-timers)))"),
        "(yes (((1 . 3) 5 (7 . 9)) ((1 . 3) (7 . 10))) t t)"
    );
}

#[test]
fn range_remove_normalizes_bignum_singletons() {
    assert_eq!(
        ev("(progn (require 'range)
                  (let* ((n (lsh 1 80))
                         (result (range-remove (list (cons n (1+ n)))
                                               (list (1+ n)))))
                    (list result (range-uncompress result))))"),
        "((1208925819614629174706176) (1208925819614629174706176))"
    );
}

#[test]
fn msb_rect_xml_and_defsubst_parity() {
    // GNU -Q: msb-mode/xml-parse-region are autoload cells.
    // `rectangle-mark-mode' is a prelude-defined minor mode here (GNU
    // autoloads it from rect.el); the other rect commands are Rust
    // subrs — either way they stay callable.  GNU-verified on 31.1.
    assert_eq!(
        ev("(list (autoloadp (symbol-function 'msb-mode))
                  (autoloadp (symbol-function 'xml-parse-region))
                  (fboundp 'rectangle-mark-mode)
                  (fboundp 'defsubst))"),
        "(t t t t)"
    );
    // defsubst (byte-run.el): an inline function is a defun plus the
    // byte-optimizer property.  GNU-verified on 31.1.
    assert_eq!(
        ev("(progn (defsubst remacs--dbl (x) \"Double X.\" (* 2 x))
                  (list (remacs--dbl 21)
                        (get 'remacs--dbl 'byte-optimizer)
                        (fboundp 'remacs--dbl)))"),
        "(42 byte-compile-inline-expand t)"
    );
    // xml/msb/rect entry points after load — byte-identical to GNU 31.1.
    assert_eq!(
        ev("(list (progn (require 'xml)
                        (let ((x (car (with-temp-buffer
                                        (insert \"<a x='1'>hi<b/></a>\")
                                        (xml-parse-region (point-min) (point-max))))))
                          (list (xml-node-name x) (xml-get-attribute x 'x)
                                (xml-node-children x))))
                  (progn (require 'msb)
                         (list (fboundp 'msb-mode) (boundp 'msb-menu-handles)))
                  (progn (require 'rect)
                         (list (fboundp 'rectangle-mark-mode)
                               (fboundp 'rectangle-exchange-point-and-mark))))"),
        "((a \"1\" (\"hi\" (b nil))) (t nil) (t t))"
    );
}

#[test]
fn dumped_and_autoloaded_misc_libs_parity() {
    // paren.el & rmc.el are in GNU's dump (loadup.el): the features are
    // registered and the entry points are real functions at -Q.
    // GNU-verified on 31.1.
    assert_eq!(
        ev("(list (featurep 'paren) (fboundp 'show-paren-mode)
                  (autoloadp (symbol-function 'show-paren-mode))
                  (featurep 'rmc) (fboundp 'read-multiple-choice)
                  (autoloadp (symbol-function 'read-multiple-choice)))"),
        "(t t nil t t nil)"
    );
    // The rest are loaddefs autoload cells at -Q, even where the prelude
    // keeps early stubs (electric-pair-mode/dcl-mode/find-function).
    assert_eq!(
        ev("(list (autoloadp (symbol-function 'electric-pair-mode))
                  (autoloadp (symbol-function 'electric-pair-local-mode))
                  (autoloadp (symbol-function 'follow-mode))
                  (autoloadp (symbol-function 'dynamic-completion-mode))
                  (autoloadp (symbol-function 'completion-preview-mode))
                  (autoloadp (symbol-function 'glasses-mode))
                  (autoloadp (symbol-function 'dcl-mode))
                  (autoloadp (symbol-function 'cpp-highlight-buffer))
                  (autoloadp (symbol-function 'bug-reference-mode))
                  (autoloadp (symbol-function 'faceup-view-buffer))
                  (autoloadp (symbol-function 're-builder))
                  (autoloadp (symbol-function 'check-declare-file))
                  (autoloadp (symbol-function 'common-lisp-indent-function))
                  (autoloadp (symbol-function 'find-function))
                  (fboundp 'hierarchy-new)
                  (fboundp 'define-multisession-variable)
                  (fboundp 'lm-verify)
                  (fboundp 'make-vtable))"),
        "(t t t t t t t t t t t t t t nil nil nil nil)"
    );
    // require paths land on the features, and hierarchy/lisp-mnt behave
    // byte-identically to GNU 31.1.
    assert_eq!(
        ev("(list (featurep (require 'elec-pair))
                  (featurep (require 'follow))
                  (featurep (require 'glasses))
                  (featurep (require 'bug-reference))
                  (featurep (require 'hierarchy))
                  (featurep (require 'multisession))
                  (featurep (require 're-builder))
                  (featurep (require 'check-declare))
                  (featurep (require 'cl-indent))
                  (featurep (require 'find-func))
                  (featurep (require 'lisp-mnt))
                  (featurep (require 'vtable))
                  (featurep (require 'completion))
                  (featurep (require 'completion-preview))
                  (featurep (require 'dcl-mode))
                  (featurep (require 'cpp))
                  (featurep (require 'faceup)))"),
        "(t t t t t t t t t t t t t t t t t)"
    );
    assert_eq!(
        ev("(progn (require 'lisp-mnt)
                  (with-temp-buffer
                    (insert \";; foo.el --- test\\n;; Version: 1.2\\n\")
                    (lm-version)))"),
        "\"1.2\""
    );
}

#[test]
fn dumped_and_autoloaded_misc_libs2_parity() {
    // format.el, composite.el, iso-transl.el, mule-util.el and
    // epa-hook.el are in GNU's dump (loadup.el): features registered,
    // entry points real functions at -Q.  GNU-verified on 31.1.
    assert_eq!(
        ev("(list (featurep 'format) (fboundp 'format-decode)
                  (autoloadp (symbol-function 'format-decode))
                  (featurep 'composite) (fboundp 'compose-region)
                  (char-table-p composition-function-table)
                  (featurep 'iso-transl) (fboundp 'iso-transl-set-language)
                  (featurep 'mule-util) (featurep 'epa-hook)
                  (boundp 'format-alist)
                  (boundp 'customize-package-emacs-version-alist))"),
        "(t t nil t t t t t t t t t)"
    );
    // Loaddefs autoload cells at -Q (GNU-verified on 31.1).
    assert_eq!(
        ev("(list (autoloadp (symbol-function 'blackbox))
                  (autoloadp (symbol-function 'decipher-mode))
                  (autoloadp (symbol-function 'dig))
                  (autoloadp (symbol-function 'dns-mode))
                  (autoloadp (symbol-function 'dunnet))
                  (autoloadp (symbol-function 'forms-mode))
                  (autoloadp (symbol-function 'icon-mode))
                  (autoloadp (symbol-function 'mixal-mode))
                  (autoloadp (symbol-function 'opascal-mode))
                  (autoloadp (symbol-function 'pascal-mode))
                  (autoloadp (symbol-function 'picture-mode))
                  (autoloadp (symbol-function 'proced))
                  (autoloadp (symbol-function 'robin-use-package))
                  (autoloadp (symbol-function 'sieve-mode))
                  (autoloadp (symbol-function 'simula-mode))
                  (autoloadp (symbol-function 'subword-mode))
                  (autoloadp (symbol-function 'type-break-mode))
                  (autoloadp (symbol-function 'shortdoc-display-group))
                  ;; prelude auto-mode stubs restored to autoload cells
                  (autoloadp (symbol-function 'dns-mode))
                  (autoloadp (symbol-function 'icon-mode))
                  (autoloadp (symbol-function 'mixal-mode))
                  (autoloadp (symbol-function 'opascal-mode))
                  (autoloadp (symbol-function 'pascal-mode))
                  (autoloadp (symbol-function 'sieve-mode))
                  (autoloadp (symbol-function 'simula-mode))
                  (autoloadp (symbol-function 'metafont-mode))
                  (autoloadp (symbol-function 'metapost-mode)))"),
        "(t t t t t t t t t t t t t t t t t t t t t t t t t t t)"
    );
    // require paths land on the features (longlines needs format-alist,
    // trampver needs customize-package-emacs-version-alist,
    // shortdoc-doc needs the shortdoc macro autoload).
    assert_eq!(
        ev("(list (featurep (require 'longlines))
                  (featurep (require 'trampver))
                  (featurep (require 'shortdoc-doc))
                  (featurep (require 'shortdoc))
                  (featurep (require 'dns))
                  (featurep (require 'mail-extr))
                  (featurep (require 'netrc))
                  (featurep (require 'picture))
                  (featurep (require 'proced))
                  (featurep (require 'subword))
                  (featurep (require 'dunnet))
                  (featurep (require 'editorconfig-core-handle)))"),
        "(t t t t t t t t t t t t)"
    );
}

#[test]
fn r7_autoload_libs_parity() {
    // 61 GNU 31.1 libraries added as embedded require/autoload targets:
    // add-log, arc-mode, bruce, bubbles, cfengine, compare-w, diff,
    // dos-w32, echistory, ehelp, elint, emerge, epg, facemenu, filesets,
    // find-dired, find-lisp, flymake-cc, footnote, goto-addr, hashcash,
    // help-mode, hideshow, hmac-md5, ido, iswitchb, landmark, ldap,
    // loaddefs-gen, make-mode, man, misearch, nroff-mode, nsm, perl-mode,
    // pixel-scroll, pong, puny, reftex-vars, reporter, rfc2104, scheme,
    // sendmail, snake, snmp-mode, socks, strokes, supercite, svg, tetris,
    // thumbs, touch-screen, tutorial, two-column, utf7, vc-bzr, vc-hg,
    // vc-svn, wdired, which-key, x-dnd.  GNU-verified on 31.1 -Q: none
    // are dumped; their entry points are loaddefs autoload cells.
    assert_eq!(
        ev("(list (featurep 'add-log) (featurep 'arc-mode)
                  (featurep 'diff) (featurep 'epg) (featurep 'help-mode)
                  (featurep 'ido) (featurep 'man) (featurep 'perl-mode)
                  (featurep 'scheme) (featurep 'sendmail)
                  (featurep 'tetris) (featurep 'which-key))"),
        "(nil nil nil nil nil nil nil nil nil nil nil nil)"
    );
    // Loaddefs autoload cells at -Q, including the prelude stubs that
    // are restored to GNU's autoload cells after the prelude runs
    // (archive-mode, 2C-*, dsssl-mode, diff-latest-backup-file,
    // help-*, pixel-scroll-*, snmp modes).  GNU-verified on 31.1.
    assert_eq!(
        ev("(list (autoloadp (symbol-function 'add-change-log-entry))
                  (autoloadp (symbol-function 'archive-mode))
                  (autoloadp (symbol-function 'bubbles))
                  (autoloadp (symbol-function 'cfengine3-mode))
                  (autoloadp (symbol-function 'compare-windows))
                  (autoloadp (symbol-function 'diff-latest-backup-file))
                  (autoloadp (symbol-function '2C-command))
                  (autoloadp (symbol-function 'dsssl-mode))
                  (autoloadp (symbol-function 'epg-make-context))
                  (autoloadp (symbol-function 'facemenu-menu))
                  (autoloadp (symbol-function 'find-dired))
                  (autoloadp (symbol-function 'find-lisp-find-dired))
                  (autoloadp (symbol-function 'footnote-mode))
                  (autoloadp (symbol-function 'goto-address-mode))
                  (autoloadp (symbol-function 'help-mode))
                  (autoloadp (symbol-function 'help-with-tutorial))
                  (autoloadp (symbol-function 'turn-off-hideshow))
                  (autoloadp (symbol-function 'ido-mode))
                  (autoloadp (symbol-function 'iswitchb-mode))
                  (autoloadp (symbol-function 'makefile-mode))
                  (autoloadp (symbol-function 'man))
                  (autoloadp (symbol-function 'multi-isearch-buffers))
                  (autoloadp (symbol-function 'nroff-mode))
                  (autoloadp (symbol-function 'perl-mode))
                  (autoloadp (symbol-function 'pixel-scroll-mode))
                  (autoloadp (symbol-function 'pong))
                  (autoloadp (symbol-function 'reftex-mode))
                  (autoloadp (symbol-function 'reporter-submit-bug-report))
                  (autoloadp (symbol-function 'scheme-mode))
                  (autoloadp (symbol-function 'mail))
                  (autoloadp (symbol-function 'snake))
                  (autoloadp (symbol-function 'snmp-mode))
                  (autoloadp (symbol-function 'strokes-mode))
                  (autoloadp (symbol-function 'sc-cite-original))
                  (autoloadp (symbol-function 'tetris))
                  (autoloadp (symbol-function 'touch-screen-hold))
                  (autoloadp (symbol-function 'utf7-encode))
                  (autoloadp (symbol-function 'wdired-change-to-wdired-mode))
                  (autoloadp (symbol-function 'which-key-mode))
                  ;; no -Q autoload cells in GNU for these: require-only
                  (fboundp 'svg-image) (fboundp 'ldap-search)
                  (fboundp 'thumbs) (fboundp 'x-dnd-drag-begin)
                  (fboundp 'socks-open-network-stream)
                  ;; vc-bzr-registered is a real loaddefs defun in GNU
                  (fboundp 'vc-bzr-registered)
                  (autoloadp (symbol-function 'vc-bzr-registered)))"),
        "(t t t t t t t t t t t t t t t t t t t t t t t t t t t t t t t t t t t t t t t nil nil nil nil nil t nil)"
    );
}

#[test]
fn r8_dumped_libs_parity() {
    // abbrev, cconv, cus-face, ediff-hook, eldoc, mouse, prog-mode,
    // regexp-opt, register, replace, scroll-bar, text-mode and timer
    // are in GNU's dump (loadup.el): features and real definitions
    // present at -Q.  GNU-verified on 31.1.
    assert_eq!(
        ev("(mapcar (lambda (f) (featurep f))
                    '(abbrev cconv cus-face ediff-hook eldoc mouse
                      prog-mode regexp-opt register replace
                      scroll-bar text-mode timer))"),
        "(t t t t t t t t t t t t t)"
    );
    // Entry points are real functions, not autoload cells.
    assert_eq!(
        ev("(list (autoloadp (symbol-function 'regexp-opt))
                  (fboundp 'regexp-opt)
                  (fboundp 'define-abbrev)
                  (fboundp 'prog-mode)
                  (autoloadp (symbol-function 'prog-mode))
                  (fboundp 'text-mode)
                  (fboundp 'scroll-bar-mode)
                  (fboundp 'eldoc-mode)
                  (fboundp 'run-at-time)
                  (fboundp 'increment-register)
                  (fboundp 'replace-regexp)
                  (fboundp 'mouse-drag-region)
                  (fboundp 'cconv-convert)
                  (boundp 'glyphless-char-display))"),
        "(nil t t t nil t t t t t t t t t)"
    );
    // Functional spot checks — GNU-verified on 31.1.
    assert_eq!(
        ev("(list (with-temp-buffer (text-mode) major-mode)
                  (progn (define-abbrev-table 'mytab
                           '((\"abc\" \"alphabet\")))
                         (abbrev-table-p mytab))
                  (timerp (run-at-time \"1 hour\" nil 'ignore))
                  (regexp-opt-depth \"\\\\(foo\\\\)\")
                  (replace-regexp-in-string \"a\" \"X\" \"banana\")
                  (with-temp-buffer
                    (insert \"ab\")
                    (set-register ?x (buffer-substring 1 3))
                    (get-register ?x)))"),
        "(text-mode t t 1 \"bXnXnX\" \"ab\")"
    );
    assert_eq!(
        ev("(regexp-opt '(\"foo\" \"bar\" \"baz\"))"),
        "\"\\\\(?:ba[rz]\\\\|foo\\\\)\""
    );
}

#[test]
fn r9_sort_help_fns_libs_parity() {
    // sort, dired-x, find-file, help-fns, chart, tar-mode and
    // elisp-scope are GNU 31.1 require/autoload libraries: no features
    // at -Q, loaddefs autoload cells for entry points.  GNU-verified.
    assert_eq!(
        ev("(mapcar #'featurep '(sort dired-x find-file help-fns
                              chart tar-mode elisp-scope))"),
        "(nil nil nil nil nil nil nil)"
    );
    // Autoload cells for their entry points — including the prelude
    // stubs restored to GNU's cells (sort-*, describe-*, tar-mode).
    assert_eq!(
        ev("(list (autoloadp (symbol-function 'sort-lines))
                  (autoloadp (symbol-function 'sort-subr))
                  (autoloadp (symbol-function 'delete-duplicate-lines))
                  (autoloadp (symbol-function 'reverse-region))
                  (autoloadp (symbol-function 'describe-function))
                  (autoloadp (symbol-function 'describe-mode))
                  (autoloadp (symbol-function 'describe-variable))
                  (autoloadp (symbol-function 'variable-at-point))
                  ;; find-file is a C subr in both (autoloadp=nil)
                  (subrp (symbol-function 'find-file))
                  (autoloadp (symbol-function 'tar-mode))
                  (autoloadp (symbol-function 'elisp-scope-analyze-form)))"),
        "(t t t t t t t t t t t)"
    );
    // `password-word-equivalents'/`password-colon-equivalents' are
    // GNU-dumped mule-conf.el defcustoms (comint needs them at eval).
    assert_eq!(
        ev("(list (boundp 'password-word-equivalents)
                  (length password-word-equivalents)
                  (boundp 'password-colon-equivalents))"),
        "(t 49 t)"
    );
    // Functional spot check — GNU-verified on 31.1.
    assert_eq!(
        ev("(with-temp-buffer (insert \"c\\nb\\na\\n\")
                             (sort-lines nil 1 7) (buffer-string))"),
        "\"a\nb\nc\n\""
    );
}

#[test]
fn r10_compat_libs_parity() {
    // Round-10 batch: 29 more GNU 31.1 libraries plus the missing
    // startup state they need.  GNU-verified on 31.1:
    //   dumped:    newcomment, image, tab-bar (features at -Q)
    //   require:   the rest (no features at -Q)
    // select was already provided by subrs + the feature list.
    assert_eq!(
        ev("(mapcar #'featurep '(newcomment image tab-bar select json
                              bookmark char-fold tab-line time woman
                              f90 fortran which-func etags-regen
                              vc-annotate cua-base keypad edt viper-init
                              plstore profiler vcursor epa artist
                              enriched sgml-mode xscheme ebrowse
                              vc-filewise vc-src pcvs-util))"),
        "(t t t t nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil nil)"
    );
    // GNU loaddefs autoload cells for the new libraries' entry points.
    assert_eq!(
        ev("(list (autoloadp (symbol-function 'bookmark-set))
                  (autoloadp (symbol-function 'artist-mode))
                  (autoloadp (symbol-function 'cua-mode))
                  (autoloadp (symbol-function 'display-time-mode))
                  (autoloadp (symbol-function 'fortran-mode))
                  (autoloadp (symbol-function 'sgml-mode))
                  (autoloadp (symbol-function 'etags-regen-mode))
                  (autoloadp (symbol-function 'image-dired-show-all-from-dir))
                  ;; `image-dired' is a GNU loaddefs defalias.
                  (symbol-function 'image-dired)
                  ;; `which-func-mode' has no cell in GNU -Q.
                  (symbol-function 'which-func-mode))"),
        "(t t t t t t t t image-dired-show-all-from-dir nil)"
    );
    // Minor-mode defvars + custom-autoloads from GNU loaddefs.
    assert_eq!(
        ev("(list (boundp 'cua-mode) (get 'cua-mode 'custom-autoload)
                  (boundp 'global-tab-line-mode)
                  (get 'global-tab-line-mode 'custom-autoload)
                  (boundp 'etags-regen-mode)
                  (get 'etags-regen-mode 'custom-autoload))"),
        "(t t t t t t)"
    );
    // Startup vars used by the new libs (GNU-verified values).
    assert_eq!(
        ev("(list (consp (get 'json-end-of-file 'error-conditions))
                  (boundp 'char-script-table)
                  (boundp 'window-persistent-parameters)
                  (boundp 'menu-bar-manuals-menu)
                  (boundp 'frame-background-mode)
                  (boundp 'mode-line-misc-info)
                  (boundp 'system-configuration-options)
                  (fboundp 'set-selection-coding-system)
                  (fboundp 'find-image)
                  (fboundp 'custom-add-choice))"),
        "(t t t t t t t t t t)"
    );
    // GNU `define-error' accepts a list of parents.
    assert_eq!(
        ev("(progn (define-error 'my-e1 \"m1\" 'my-e2)
                  (define-error 'my-e2 \"m2\")
                  (list (get 'my-e1 'error-conditions)
                        (get 'my-e2 'error-conditions)))"),
        "((my-e1 my-e2) (my-e2 error))"
    );
    // GNU `map-char-table' skips nil-valued ranges.
    assert_eq!(
        ev("(let ((tbl (make-char-table nil)) (n 0) (keys nil))
             (set-char-table-range tbl 65 'x)
             (map-char-table (lambda (k v) (setq n (1+ n))
                              (push (list k v) keys)) tbl)
             (list n keys))"),
        "(1 ((65 x)))"
    );
    // unicode property tables carry GNU's 3 extra slots: slot 0 is the
    // property symbol; slots 1/2 hold the unidata-gen mapper functions
    // for computed properties (functionp t in GNU) or nil/int markers
    // for direct ones.
    assert_eq!(
        ev("(let ((tbl (unicode-property-table-internal 'lowercase)))
             (list (char-table-extra-slot tbl 0)
                   (char-table-extra-slot tbl 1)
                   (char-table-extra-slot tbl 2)))"),
        "(lowercase nil 0)"
    );
    assert_eq!(
        ev(
            "(let ((tbl (unicode-property-table-internal 'decomposition)))
             (list (char-table-extra-slot tbl 0)
                   (functionp (char-table-extra-slot tbl 1))
                   (functionp (char-table-extra-slot tbl 2))))"
        ),
        "(decomposition t t)"
    );
    // Functional spot checks — GNU-verified.
    assert_eq!(
        ev("(with-temp-buffer (emacs-lisp-mode) (insert \"x\\ny\\n\")
                             (comment-region (point-min) (point-max))
                             (buffer-string))"),
        "\";; x\n;; y\n\""
    );
    assert_eq!(
        ev("(progn (require 'sgml-mode) (list (fboundp 'sgml-mode)
                  (featurep 'sgml-mode)))"),
        "(t t)"
    );
    assert_eq!(
        ev("(progn (require 'bookmark) (list (fboundp 'bookmark-set)
                  (featurep 'bookmark)))"),
        "(t t)"
    );
}
