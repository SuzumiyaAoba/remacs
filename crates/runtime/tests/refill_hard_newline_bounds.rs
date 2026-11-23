use remacs_runtime::lisp::{Flow, Interp};
use std::path::PathBuf;

const CASES: &str = r###"(progn
  (defvar use-hard-newlines nil)
  (defvar paragraph-start "[ \t\f]*$")
  (defvar paragraph-ignore-fill-prefix nil)
  (defvar adaptive-fill-mode nil)
  (defvar fill-prefix nil)
  (defvar refill-ignorable-overlay nil)
  (defvar remacs-2955-hard-beg nil)
  (defvar remacs-2955-hard-end nil)
  (defvar remacs-2955-paragraph-settings nil)
  (defvar remacs-2955-prefix-bounds nil)
  (defvar remacs-2955-region-call nil)
  (defun remacs-2955-check (label condition)
    (unless condition
      (error "Refill hard-newline bounds #2955 failed: %S" label))
    t)
  (let ((buffer (generate-new-buffer " *refill-hard-newline*")))
    (unwind-protect
        (with-current-buffer buffer
          (let ((use-hard-newlines t)
                (adaptive-fill-mode t)
                (fill-prefix nil))
            (insert "first paragraph\nsecond paragraph")
            (let* ((hard-newline
                    (progn
                      (goto-char (point-min))
                      (search-forward "\n")
                      (1- (point))))
                   (second-beg (1+ hard-newline))
                   (paragraph-end (point-max))
                   (pos (+ second-beg 2))
                   (refill-ignorable-overlay
                    (make-overlay second-beg (1+ second-beg) buffer))
                   (had-forward (fboundp 'fill-forward-paragraph))
                   (old-forward
                    (and had-forward (symbol-function 'fill-forward-paragraph)))
                   (had-context (fboundp 'fill-context-prefix))
                   (old-context
                    (and had-context (symbol-function 'fill-context-prefix)))
                   (had-region (fboundp 'fill-region))
                   (old-region (and had-region (symbol-function 'fill-region))))
              (setq remacs-2955-hard-beg second-beg
                    remacs-2955-hard-end paragraph-end
                    remacs-2955-paragraph-settings nil
                    remacs-2955-prefix-bounds nil
                    remacs-2955-region-call nil)
              (put-text-property hard-newline (1+ hard-newline) 'hard t)
              (unwind-protect
                  (progn
                    (fset 'fill-forward-paragraph
                          (lambda (arg)
                            (setq remacs-2955-paragraph-settings
                                  (cons (list arg paragraph-start
                                              paragraph-ignore-fill-prefix)
                                        remacs-2955-paragraph-settings))
                            (goto-char (if (> arg 0)
                                           remacs-2955-hard-end
                                         remacs-2955-hard-beg))
                            0))
                    (fset 'fill-context-prefix
                          (lambda (beg end &optional _)
                            (setq remacs-2955-prefix-bounds (cons beg end))
                            "  "))
                    (fset 'fill-region
                          (lambda (beg end &optional _)
                            (setq remacs-2955-region-call
                                  (list beg end fill-prefix))))
                    (refill-fill-paragraph-at pos))
                (if had-forward
                    (fset 'fill-forward-paragraph old-forward)
                  (fmakunbound 'fill-forward-paragraph))
                (if had-context
                    (fset 'fill-context-prefix old-context)
                  (fmakunbound 'fill-context-prefix))
                (if had-region
                    (fset 'fill-region old-region)
                  (fmakunbound 'fill-region)))
              (delete-overlay refill-ignorable-overlay)
              (remacs-2955-check
               'hard-newline-paragraph-settings
               (equal (nreverse remacs-2955-paragraph-settings)
                      '((1 "." t) (-1 "." t))))
              (remacs-2955-check
               'prefix-uses-current-hard-newline-paragraph
               (equal remacs-2955-prefix-bounds
                      (cons second-beg paragraph-end)))
              (remacs-2955-check
               'fill-only-current-hard-newline-paragraph
               (equal remacs-2955-region-call
                      (list (1+ second-beg) paragraph-end "  ")))))
      (kill-buffer buffer))))
  t)"###;

#[test]
fn refill_uses_hard_newlines_as_paragraph_boundaries() {
    let source = PathBuf::from(env!("REMACS_SOURCE_ROOT"))
        .join("lisp/refill.el")
        .canonicalize()
        .unwrap();
    let source = std::fs::read_to_string(source).unwrap();
    let start = source.find("(defun refill-fill-paragraph-at").unwrap();
    let end = start
        + source[start..]
            .find("\n(defun refill-fill-paragraph (")
            .unwrap();
    let definition = &source[start..end];
    let code = format!("(progn {} {})", definition, CASES);
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
