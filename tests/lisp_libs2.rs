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
        ev("(progn (require 'tabify) (require 'obarray) (require 'scroll-lock)
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
                        (fboundp 'scroll-lock-mode)))"),
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
        ev("(progn (require 'ansi-osc) (require 'help-macro) (require 'master)
                  (list (fboundp 'ansi-osc-apply-on-region)
                        (keymapp ansi-osc-hyperlink-map)
                        (fboundp 'make-help-screen)
                        (fboundp 'master-mode)
                        (fboundp 'master-says)))"),
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
        ev("(list (find-file-name-handler \"foo.gz\" 'insert-file-contents)
                  (find-file-name-handler \"foo.gz\" 'load)
                  (find-file-name-handler \"foo.gz\" 'file-name-sans-versions)
                  (let ((inhibit-file-name-handlers '(jka-compr-handler)))
                    (find-file-name-handler \"foo.gz\" 'insert-file-contents))
                  (file-name-sans-versions \"foo.gz\"))"),
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
        ev("(list (cl-destructuring-bind (a . b) '(1 2 3) (list a b))
                 (cl-destructuring-bind (a b . c) '(1 2 3 4) (list a b c)))"),
        "((1 (2 3)) (1 2 (3 4)))"
    );
}

#[test]
fn cl_defstruct_accepts_docstring() {
    // GNU-verified on 31.1: `cl-defstruct' accepts a docstring
    // between the name and the slot list.
    assert_eq!(
        ev("(progn (cl-defstruct ec-probe-struct \"A struct doc.\" one (two 2))
                  (let ((s (make-ec-probe-struct :one 1)))
                    (list (ec-probe-struct-one s)
                          (ec-probe-struct-two s)
                          (ec-probe-struct-p s))))"),
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
        ev("(progn (require 'array) (require 'dos-vars) (require 'fringe)
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
                        (boundp 'special-event-map)))"),
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
    assert_eq!(
        ev("(list (fboundp 'rx) (featurep 'rx))"),
        "(t nil)"
    );
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
        ev("(let ((h (make-hash-table :test 'equal)))
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
        ev("(progn (dolist (l '(shadow inline compat refill word-wrap-mode
                              glyphless-mode timeout pixel-fill))
                    (require l))
                  (list (fboundp 'load-path-shadows-find)
                        (fboundp 'define-inline)
                        (fboundp 'compat-function)
                        (fboundp 'refill-mode)
                        (fboundp 'word-wrap-whitespace-mode)
                        (fboundp 'glyphless-display-mode)
                        (fboundp 'timeout-throttle)
                        (fboundp 'pixel-fill-region)))"),
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
        ev("(list (fboundp 'debug-early) (fboundp 'debug-early-backtrace)
                  (featurep 'debug-early)
                  (car (symbol-function 'executable-interpret))
                  (nth 1 (symbol-function 'executable-interpret))
                  (car (symbol-function 'executable-set-magic))
                  (car (symbol-function 'executable-command-find-posix-p))
                  (car (symbol-function 'executable-make-buffer-file-executable-if-script-p)))"),
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
        ev("(progn (dolist (l '(rfc1843 ja-dic-utl mailheader)) (require l))
                  (list (fboundp 'rfc1843-decode-region)
                        (fboundp 'skkdic-lookup-key)
                        (fboundp 'mail-header-extract)
                        (featurep 'rfc1843) (featurep 'ja-dic-utl)
                        (featurep 'mailheader)))"),
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
        ev("(progn (define-derived-mode remacs--test-derived prog-mode \"TestDerived\" \"Doc.\")
                  (list (featurep 'derived) (fboundp 'remacs--test-derived)
                        (get 'remacs--test-derived 'derived-mode-parent)))"),
        "(t t prog-mode)"
    );
    // Same for generic.el.
    assert_eq!(
        ev("(progn (define-generic-mode 'remacs--test-generic '(\"#\") nil nil nil nil \"Doc.\")
                  (list (featurep 'generic) (fboundp 'remacs--test-generic)))"),
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
    assert_eq!(ev("(regexp-opt '(\"foo\" \"bar\" \"baz\"))"),
               "\"\\\\(?:ba[rz]\\\\|foo\\\\)\"");
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
