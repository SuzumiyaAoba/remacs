use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r###"(progn
  (defun remacs-2966-check (label condition actual)
    (unless condition
      (error "Makefile continued recipe font-lock #2966 failed: %S: %S"
             label actual))
    t)
  (defvar makefile-rule-action-regex
    "^\t[ \t]*\\(?:\\([-@]+\\)[ \t]*\\)?\\(\\(?:.*\\\\\n\\)*.*\\)")
  (with-temp-buffer
    (insert "all:\n\techo first \\\n\t  && echo second\n")
    (setq-local font-lock-multiline t)
    (setq-local font-lock-keywords
                '(t nil
                    (makefile-match-action (2 'makefile-shell prepend))))
    (let* ((continuation
            (progn
              (goto-char (point-min))
              (search-forward "&&")
              (- (point) 2)))
           (action-match
            (save-match-data
              (save-excursion
                (goto-char (point-min))
                (and (makefile-match-action (point-max))
                     (list (match-beginning 0) (match-end 0)
                           (match-beginning 2) (match-end 2)))))))
      (remacs-2966-check
       'matcher-spans-continuation
       (and action-match (> (nth 3 action-match) continuation))
       (list action-match continuation))
      (font-lock-fontify-keywords-region (point-min) (point-max))
      (remacs-2966-check
       'font-lock-multiline
       (get-text-property continuation 'font-lock-multiline)
       continuation)
      (let ((face (get-text-property continuation 'face)))
        (remacs-2966-check
         'makefile-shell-continuation
         (or (eq face 'makefile-shell)
             (and (listp face) (memq 'makefile-shell face)))
         (list continuation face)))))
  t)"###;

#[test]
fn makefile_continued_recipe_is_fontified_as_one_multiline_match() {
    let source = PathBuf::from(env!("REMACS_SOURCE_ROOT"))
        .join("lisp/make-mode.el")
        .canonicalize()
        .unwrap();
    let source = std::fs::read_to_string(source).unwrap();
    assert!(source.contains("(setq-local font-lock-multiline t)"));
    let start = source
        .find("(defun makefile-match-action")
        .expect("makefile action matcher");
    let end = start
        + source[start..]
            .find("\n(defun makefile-do-macro-insertion")
            .expect("end of makefile action matcher");
    let code = format!("(progn {} {})", &source[start..end], CASES);
    let mut interp = Interp::new();
    let result = interp.eval_str(&code).unwrap_or_else(|flow| match flow {
        Flow::Signal(signal) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        other => panic!("{other:?}"),
    });
    assert!(result.truthy());
}
