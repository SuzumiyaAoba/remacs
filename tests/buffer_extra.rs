//! Tests for region/text-property/motion subrs (src/buffer/extra.rs)
//! and core buffer primitives.

mod common;
use common::{ev, ev_out};

#[test]
fn subst_char_in_region() {
    assert_eq!(
        ev_out(
            "(with-temp-buffer
               (insert \"hello world\")
               (subst-char-in-region (point-min) (point-max) ?o ?0)
               (princ (buffer-string)))"
        ),
        "hell0 w0rld"
    );
}

#[test]
fn current_indentation() {
    assert_eq!(
        ev("(with-temp-buffer
              (insert \"abc\\n   indented\\n\")
              (goto-char (point-min))
              (forward-line 1)
              (current-indentation))"),
        "3"
    );
    assert_eq!(
        ev("(with-temp-buffer
              (insert \"\\tfoo\")
              (goto-char (point-min))
              (current-indentation))"),
        "8"
    );
    assert_eq!(
        ev("(with-temp-buffer
              (insert \"nope\")
              (current-indentation))"),
        "0"
    );
}

#[test]
fn text_properties() {
    assert_eq!(
        ev("(with-temp-buffer
              (insert \"hello\")
              (put-text-property 1 4 'face 'bold)
              (get-text-property 2 'face))"),
        "bold"
    );
    assert_eq!(
        ev("(with-temp-buffer
              (insert \"hello\")
              (get-text-property 2 'face))"),
        "nil"
    );
    assert_eq!(
        ev("(with-temp-buffer
              (insert \"hello\")
              (put-text-property 1 4 'face 'bold)
              (add-text-properties 1 4 '(underline t))
              (list (get-text-property 2 'face)
                    (get-text-property 2 'underline)))"),
        "(bold t)"
    );
    assert_eq!(
        ev("(with-temp-buffer
              (insert \"hello\")
              (put-text-property 1 4 'face 'bold)
              (remove-text-properties 1 4 '(face))
              (get-text-property 2 'face))"),
        "nil"
    );
    assert_eq!(
        ev("(with-temp-buffer
              (insert \"hello\")
              (put-text-property 1 3 'x 1)
              (put-text-property 3 5 'x 2)
              (next-property-change 1))"),
        "3"
    );
    assert_eq!(
        ev("(with-temp-buffer
              (insert \"hello\")
              (put-text-property 2 4 'face 'bold)
              (text-properties-at 2))"),
        "(face bold)"
    );
}

#[test]
fn fields() {
    // No field property → field is whole accessible region.
    assert_eq!(
        ev("(with-temp-buffer
              (insert \"abcdef\")
              (field-beginning))"),
        "1"
    );
    assert_eq!(
        ev("(with-temp-buffer
              (insert \"abcdef\")
              (field-end))"),
        "7"
    );
}

#[test]
fn indent_column() {
    assert_eq!(
        ev("(with-temp-buffer
              (insert \"ab\")
              (current-column))"),
        "2"
    );
    assert_eq!(
        ev("(with-temp-buffer
              (insert \"\\t\")
              (current-column))"),
        "8"
    );
}

#[test]
fn delete_and_extract() {
    assert_eq!(
        ev("(with-temp-buffer
              (insert \"hello\")
              (delete-and-extract-region 2 4))"),
        "\"el\""
    );
    assert_eq!(
        ev_out(
            "(with-temp-buffer
                  (insert \"hello\")
                  (delete-and-extract-region 2 4)
                  (princ (buffer-string)))"
        ),
        "hlo"
    );
}

#[test]
fn region_encoding() {
    assert_eq!(
        ev_out(
            "(with-temp-buffer
                  (insert \"hi\")
                  (base64-encode-region 1 3)
                  (princ (buffer-string)))"
        ),
        "aGk="
    );
    assert_eq!(
        ev_out(
            "(with-temp-buffer
                  (insert \"aGk=\")
                  (base64-decode-region 1 5)
                  (princ (buffer-string)))"
        ),
        "hi"
    );
}

#[test]
fn buffer_swap_text() {
    assert_eq!(
        ev_out(
            "(let ((a (generate-new-buffer \"*a*\"))
                     (b (generate-new-buffer \"*b*\")))
                 (with-current-buffer a (insert \"AAA\"))
                 (with-current-buffer b (insert \"BBB\"))
                 (with-current-buffer a (buffer-swap-text b))
                 (princ (with-current-buffer a (buffer-string)))
                 (princ \"|\")
                 (princ (with-current-buffer b (buffer-string))))"
        ),
        "BBB|AAA"
    );
}

#[test]
fn transpose_regions() {
    assert_eq!(
        ev_out(
            "(with-temp-buffer
                  (insert \"abcdef\")
                  (transpose-regions 1 3 4 6)
                  (princ (buffer-string)))"
        ),
        "decabf"
    );
}

#[test]
fn translate_region() {
    // Chars outside the table range are unchanged (matches Emacs).
    assert_eq!(
        ev_out(
            "(with-temp-buffer
                  (insert \"abcabc\")
                  (translate-region 1 7 \"cba\")
                  (princ (buffer-string)))"
        ),
        "abcabc"
    );
    // Char code i maps to table[i].
    assert_eq!(
        ev("(with-temp-buffer
              (insert 0 1 2)
              (translate-region 1 4 \"abc\")
              (mapcar #'identity (buffer-string)))"),
        "(97 98 99)"
    );
}

#[test]
fn narrow_to_indirect() {
    assert_eq!(
        ev("(with-temp-buffer
              (insert \"0123456789\")
              (narrow-to-region 3 6)
              (list (point-min) (point-max) (buffer-size)))"),
        "(3 6 3)"
    );
    assert_eq!(
        ev("(with-temp-buffer
              (insert \"0123456789\")
              (narrow-to-region 3 6)
              (widen)
              (list (point-min) (point-max)))"),
        "(1 11)"
    );
}
