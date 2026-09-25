//! Tests for interactive commands added for GNU binding compatibility:
//! occur, append-next-kill, repeat, list navigation, transpose-*, etc.

mod common;
use common::{ev, ev_err, ev_out};

#[test]
fn occur_buffer_contents() {
    let out = ev_out(
        "(with-temp-buffer
           (insert \"x\\nfoo a\\ny\\nfoo b\\n\")
           (occur \"foo\")
           (with-current-buffer \"*Occur*\" (princ (buffer-string))))",
    );
    assert_eq!(
        out,
        "Searched 1 buffer; 2 matches for \"foo\"\n2 matches for \"foo\" in buffer:  *temp*\n      2:foo a\n      4:foo b\n"
    );
}

#[test]
fn occur_does_not_select_buffer() {
    assert_eq!(
        ev("(with-temp-buffer
             (insert \"foo\")
             (occur \"foo\")
             (buffer-name))"),
        "\" *temp*\""
    );
}

#[test]
fn occur_single_match_grammar() {
    let out = ev_out(
        "(with-temp-buffer
           (insert \"x\\nfoo\\n\")
           (occur \"foo\")
           (with-current-buffer \"*Occur*\" (princ (buffer-string))))",
    );
    assert!(out.contains("1 match for \"foo\""));
}

#[test]
fn append_next_kill_merges_entries() {
    // append-next-kill makes the next kill append to the current entry.
    assert_eq!(
        ev("(progn
             (kill-new \"one\")
             (append-next-kill)
             (kill-new \"two\")
             (list (car kill-ring) (length kill-ring)))"),
        "(\"onetwo\" 1)"
    );
}

#[test]
fn append_next_kill_consumed_once() {
    assert_eq!(
        ev("(progn
             (kill-new \"one\")
             (append-next-kill)
             (kill-new \"two\")
             (kill-new \"three\")
             (list (car kill-ring) (cadr kill-ring) (length kill-ring)))"),
        "(\"three\" \"onetwo\" 2)"
    );
}

#[test]
fn default_key_bindings() {
    // Spot-check bindings corrected to match GNU.
    assert_eq!(ev("(key-binding \"\\C-o\")"), "open-line");
    assert_eq!(ev("(key-binding \"\\C-xz\")"), "repeat");
    assert_eq!(ev("(key-binding \"\\M-gg\")"), "goto-line");
    assert_eq!(ev("(key-binding \"\\M-so\")"), "occur");
    assert_eq!(ev("(key-binding \"\\C-\\M-t\")"), "transpose-sexps");
    assert_eq!(ev("(key-binding \"\\C-\\M-u\")"), "backward-up-list");
    assert_eq!(ev("(key-binding \"\\C-\\M-d\")"), "down-list");
    assert_eq!(ev("(key-binding \"\\C-\\M-n\")"), "forward-list");
    assert_eq!(ev("(key-binding \"\\C-\\M-p\")"), "backward-list");
    assert_eq!(ev("(key-binding \"\\C-\\M-v\")"), "scroll-other-window");
    assert_eq!(ev("(key-binding \"\\C-\\M-w\")"), "append-next-kill");
    assert_eq!(ev("(key-binding \"\\M-=\")"), "count-words-region");
    assert_eq!(ev("(key-binding \"\\M-^\")"), "delete-indentation");
    assert_eq!(ev("(key-binding \"\\C-x\\C-t\")"), "transpose-lines");
    assert_eq!(ev("(key-binding \"\\C-x\\C-v\")"), "find-alternate-file");
    assert_eq!(ev("(key-binding \"\\C-x\\C-n\")"), "set-goal-column");
}

#[test]
fn ctrl_punctuation_keys() {
    // GNU keeps the control bit on punctuation keys: C-/ is 67108911,
    // not folded like C-a..C-z. C-o is 15 and must not collide.
    assert_eq!(ev("(aref (kbd \"C-/\") 0)"), "67108911");
    assert_eq!(ev("(aref (kbd \"C-o\") 0)"), "15");
    assert_eq!(ev("(aref (kbd \"C-_\") 0)"), "31");
    assert_eq!(ev("(key-binding (kbd \"C-/\"))"), "undo");
    assert_eq!(ev("(key-binding (kbd \"C-_\"))"), "undo");
}

#[test]
fn goto_line() {
    assert_eq!(
        ev("(with-temp-buffer
             (insert \"a\\nb\\nc\\n\")
             (goto-line 2)
             (point))"),
        "3"
    );
}

#[test]
fn transpose_sexps() {
    // Between sexps: swap them, point lands at end (GNU).
    assert_eq!(
        ev("(with-temp-buffer
             (insert \"(aa) (bb)\")
             (goto-char 6)
             (transpose-sexps 1)
             (list (buffer-string) (point)))"),
        "(\"(bb) (aa)\" 10)"
    );
    // At point-min there is no previous sexp: buffer unchanged,
    // point moves past the first sexp (GNU).
    assert_eq!(
        ev("(with-temp-buffer
             (insert \"(aa) (bb)\")
             (goto-char (point-min))
             (transpose-sexps 1)
             (list (buffer-string) (point)))"),
        "(\"(aa) (bb)\" 5)"
    );
    // At point-max: unchanged, point stays (GNU).
    assert_eq!(
        ev("(with-temp-buffer
             (insert \"(aa) (bb)\")
             (goto-char (point-max))
             (transpose-sexps 1)
             (list (buffer-string) (point)))"),
        "(\"(aa) (bb)\" 10)"
    );
}

#[test]
fn list_navigation() {
    assert_eq!(
        ev("(with-temp-buffer
             (insert \"(a (b) c)\")
             (goto-char (point-min))
             (down-list 1)
             (char-after))"),
        "97" // 'a'
    );
    assert_eq!(
        ev("(with-temp-buffer
             (insert \"(a) (b)\")
             (goto-char (point-min))
             (forward-list 1)
             (point))"),
        "4"
    );
    assert_eq!(
        ev("(with-temp-buffer
             (insert \"(a) (b)\")
             (goto-char (point-max))
             (backward-list 1)
             (point))"),
        "5"
    );
    assert_eq!(
        ev("(with-temp-buffer
             (insert \"((a))\")
             (goto-char 3)
             (backward-up-list 1)
             (point))"),
        "2"
    );
}

#[test]
fn circular_member_assoc_error() {
    let circ = "(let ((l (list 1 2 3))) (setcdr (cddr l) l) %s)";
    assert_eq!(ev_err(&circ.replace("%s", "(member 9 l)")), "circular-list");
    assert_eq!(ev_err(&circ.replace("%s", "(memq 9 l)")), "circular-list");
    assert_eq!(ev_err(&circ.replace("%s", "(assoc 9 l)")), "circular-list");
    assert_eq!(ev_err(&circ.replace("%s", "(assq 9 l)")), "circular-list");
    assert_eq!(ev_err(&circ.replace("%s", "(rassoc 9 l)")), "circular-list");
    assert_eq!(
        ev_err(&circ.replace("%s", "(assoc 9 l (lambda (a b) nil))")),
        "circular-list"
    );
    assert_eq!(ev_err(&circ.replace("%s", "(nreverse l)")), "circular-list");
}

#[test]
fn circular_safe_length_and_proper_list_p() {
    // GNU safe-length returns the detection point count (5 for a
    // 3-cycle); proper-list-p returns nil rather than erroring.
    assert_eq!(
        ev("(let ((l (list 1 2 3))) (setcdr (cddr l) l) (safe-length l))"),
        "5"
    );
    assert_eq!(
        ev("(let ((l (list 1 2 3))) (setcdr (cddr l) l) (proper-list-p l))"),
        "nil"
    );
    assert_eq!(ev("(safe-length '(1 2 3 4))"), "4");
    assert_eq!(ev("(safe-length '(1 2 . 3))"), "2");
}

#[test]
fn circular_list_printing() {
    // Printers terminate on cdr-chain cycles: GNU repeats the list's
    // elements until re-hitting the cons where the cycle was entered,
    // then prints ". #N" (its `being_printed' stack index).
    assert_eq!(
        ev("(let ((l (list 1 2 3))) (setcdr (cddr l) l) (prin1-to-string l))"),
        "\"(1 2 3 1 2 . #2)\""
    );
    assert_eq!(
        ev("(let ((l (list 1 2))) (setcdr (cdr l) l) (format \"%s\" l))"),
        "\"(1 2 1 2 . #2)\""
    );
}

#[test]
fn print_circle_labels() {
    // `print-circle': shared/cyclic objects get `#N=' at first
    // occurrence and `#N#' afterwards; numbers are assigned at the
    // object's second encounter during GNU's print_preprocess walk
    // (car-first DFS), all verified against GNU 31.1.
    let cases: &[(&str, &str)] = &[
        (
            "(let ((l (list 1 2))) (setcdr (cdr l) l) l)",
            "\"#1=(1 2 . #1#)\"",
        ),
        ("(let ((x (list 5))) (list x x))", "\"(#1=(5) #1#)\""),
        (
            "(let ((x (list 5))) (cons 1 (cons x x)))",
            "\"(1 #1=(5) . #1#)\"",
        ),
        (
            "(let* ((x (list 1 2)) (y (list x x))) (list y y))",
            "\"(#2=(#1=(1 2) #1#) #2#)\"",
        ),
        (
            "(let* ((x (list 2)) (y (cons 1 x))) (list y x))",
            "\"((1 . #1=(2)) #1#)\"",
        ),
        (
            "(let* ((v (vector nil nil)) (l (list v)))
              (aset v 0 l) (aset v 1 l) (list v l))",
            "\"(#1=[#2=(#1#) #2#] #2#)\"",
        ),
        ("(let ((l (list 1))) (setcar l l) l)", "\"#1=(#1#)\""),
        ("(let ((s \"ab\")) (list s s))", "\"(#1=\\\"ab\\\" #1#)\""),
        (
            "(let ((l (list 1 2 3))) (setcdr (cdr l) (cdr l)) l)",
            "\"(1 . #1=(2 . #1#))\"",
        ),
    ];
    for (form, want) in cases {
        assert_eq!(
            ev(&format!(
                "(let ((print-circle t)) (prin1-to-string {form}))"
            )),
            *want,
            "{form}"
        );
    }
    // Without `print-circle', shared (non-cyclic) objects print plain.
    assert_eq!(
        ev("(let ((print-circle nil) (x (list 5))) (prin1-to-string (list x x)))"),
        "\"((5) (5))\""
    );
}

#[test]
fn record_constructor() {
    assert_eq!(ev("(record 'a 1 2)"), "#s(a 1 2)");
    assert_eq!(ev("(recordp (record 'a))"), "t");
    assert_eq!(ev("(recordp '(a))"), "nil");
    assert_eq!(ev("(type-of (record 'a))"), "a");
}

#[test]
fn delete_blank_lines_variants() {
    assert_eq!(
        ev("(with-temp-buffer
             (insert \"a\\n\\n\\nb\\n\")
             (goto-char 3)
             (delete-blank-lines)
             (buffer-string))"),
        "\"a\n\nb\n\""
    );
    // Point on a non-blank line: GNU deletes only the following
    // blank lines; preceding blanks are untouched.
    assert_eq!(
        ev("(with-temp-buffer
             (insert \"\\n\\na\\n\\n\\nb\\n\")
             (goto-char 3)
             (delete-blank-lines)
             (buffer-string))"),
        "\"\n\na\nb\n\""
    );
}
