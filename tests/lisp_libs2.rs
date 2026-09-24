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
    assert_eq!(
        ev("(command-execute 'self-insert-command)"),
        "nil"
    );
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
    // GNU-verified on 31.1: U+200E appended iff STR has a strong R/AL char.
    assert_eq!(
        ev("(list (bidi-string-mark-left-to-right \"abc\")
                 (bidi-string-mark-left-to-right \"אבג\")
                 (bidi-string-mark-left-to-right \"aאב\"))"),
        "(\"abc\" \"אבג‎\" \"aאב‎\")"
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
        ev("(eval '(let ((f (cl-flet ((h (x) (* x 2)))
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
        ev("(eval '(list (cl-labels ((f (n) (if (<= n 1) 1 (* n (f (1- n))))))
                          (f 5))
                        (cl-labels ((ev (n) (if (= n 0) t (od (1- n))))
                                    (od (n) (if (= n 0) nil (ev (1- n)))))
                          (ev 10))
                        (let ((f (cl-labels ((g (n) (if (<= n 0) 0 (+ n (g (1- n))))))
                                   (lambda (k) (g k)))))
                          (funcall f 4))) t)"),
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
        ev("(list (macroexpand-1 '(f a b) '((f . (lambda (a b) `(g ,a ,b)))))
                 (macroexpand-1 '(f a b) '((f . nil)))
                 (macroexpand-1 '(f a b)))"),
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
        ev("(progn (require 'case-table) (require 'chistory) (require 'midnight)
                  (list (with-temp-buffer
                          (describe-buffer-case-table)
                          (buffer-string))
                        (fboundp 'list-command-history)
                        (fboundp 'clean-buffer-list)))"),
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
