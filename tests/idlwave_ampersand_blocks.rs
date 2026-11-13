//! Regression coverage for IDLWAVE block indentation across `&` statements.
mod common;

use common::ev;

#[test]
fn nested_blocks_on_ampersand_lines_indent_as_separate_statements() {
    assert_eq!(
        ev(r#"(progn
              ;; `idlw-help' pulls in unrelated browser libraries.
              (provide 'idlw-help)
              (defun idlwave-help-check-locations () nil)
              (require 'idlwave)
              (let ((idlwave-config-directory
                     (make-temp-file "remacs-idlwave-" t))
                    (idlwave-block-indent 2)
                    (idlwave-end-offset -2))
                (unwind-protect
                    (with-temp-buffer
                      (idlwave-mode)
                      (insert "if outer then begin\n"
                              "if condition1 then begin & if condition2 then begin\n"
                              "do_something\n"
                              "endif & endif\n"
                              "after_inner\n"
                              "endif\n"
                              "outside\n")
                      (indent-region (point-min) (point-max))
                      (goto-char (point-min))
                      (let (indents)
                        (while (< (point) (point-max))
                          (push (current-indentation) indents)
                          (forward-line 1))
                        (nreverse indents)))
                  (delete-directory idlwave-config-directory t))))"#),
        "(0 2 6 4 2 0 0)"
    );
}
