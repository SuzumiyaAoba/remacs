use remacs_runtime::lisp::{Flow, Interp};

const CASES: &str = r#"
(progn
  (require 'ffap)
  (defun remacs-test-ffap-url (text position expected &rest options)
    (with-temp-buffer
      (insert (if (plist-get options :properties)
                  (propertize text 'face 'bold) text))
      (when (plist-get options :narrow)
        (narrow-to-region 6 (- (point-max) 5)))
      (when (plist-get options :major-url) (setq-local major-mode 'url))
      (when (eq (plist-get options :mode) 'raw-url)
        (setq-local ffap-string-at-point-mode-alist
                    (cons (cons 'raw-url
                                (cdr (assq 'url ffap-string-at-point-mode-alist)))
                          ffap-string-at-point-mode-alist)))
      (goto-char position)
      (when (plist-get options :region)
        (set-mark (point-min))
        (goto-char (point-max))
        (setq transient-mark-mode t mark-active t))
      (let* ((before (point))
             (mode (if (plist-member options :mode)
                       (plist-get options :mode) 'url))
             (ffap-max-region-length
              (if (plist-member options :max)
                  (let ((max (plist-get options :max)))
                    (if (eq max t) (length expected) max))
                1024))
             (value (ffap-string-at-point mode))
             (start (and (> (length expected) 0)
                         (1+ (string-match (regexp-quote expected) text))))
             (bounds (if start (list start (+ start (length expected))) '(1 1))))
        (unless (and (equal value expected)
                     (equal ffap-string-at-point-region bounds)
                     (eq value ffap-string-at-point)
                     (= (point) before)
                     (or (= (length value) 0) (null (text-properties-at 0 value))))
          (error "FFAP %S %S %S: expected %S %S, got %S %S at %S"
                 text position options expected bounds value
                 ffap-string-at-point-region (point))))))
  (let ((cases
         '(("(https://example.com)." 10 "https://example.com")
           ("https://example.com)" 10 "https://example.com")
           ("((https://example.com))." 10 "https://example.com")
           ("(https://example.com)." 21 "https://example.com")
           ("(https://example.com)." 22 "https://example.com")
           ("https://example.com" 10 "https://example.com")
           ("https://example.com/a(b)" 10 "https://example.com/a(b)")
           ("(https://example.com/a(b))." 10 "https://example.com/a(b)")
           ("https://example.com/a(b)))" 10 "https://example.com/a(b)")
           ("((https://example.com/a((b))))." 10 "https://example.com/a((b))")
           ("https://example.com/a)b(c)" 10 "https://example.com/a)b(c)")
           ("https://example.com/a(b" 10 "https://example.com/a(b")
           ("https://example.com/%28name%29)" 10 "https://example.com/%28name%29")
           ("<https://example.com/a(b)>" 10 "https://example.com/a(b)")
           ("(https://example.com/?q=(x))." 10 "https://example.com/?q=(x)")
           ("(https://例え.jp/項目(一))." 10 "https://例え.jp/項目(一)")
           ("user@example.com)" 5 "user@example.com")
           ("<message-id@example.com>" 10 "message-id@example.com")
           ("(https://example.com)." 10 "https://example.com" :properties t)
           ("lead (https://example.com). tail" 12 "https://example.com" :narrow t)
           ("(https://example.com)." 10 "https://example.com"
            :major-url t :mode nil)
           ("(https://example.com)." 10 "https://example.com" :max t)
           ("(https://example.com)." 10 "" :max 3)
           ("(https://example.com)" 10 "(https://example.com)" :region t)
           ("(https://example.com)" 10 "" :region t :max 3)
           ("(https://example.com)." 10 "https://example.com)" :mode raw-url)
           ("(report.txt)" 5 "report.txt" :mode file))))
    (dolist (case cases) (apply #'remacs-test-ffap-url case))
    (length cases)))
"#;

#[test]
fn ffap_url_extraction_trims_only_unmatched_trailing_parentheses() {
    let mut interp = Interp::new();
    let result = interp.eval_str(CASES).unwrap_or_else(|flow| match flow {
        Flow::Signal(signal) => panic!(
            "{} {}",
            interp.prin1_to_string(&signal.symbol()),
            interp.prin1_to_string(&signal.data())
        ),
        other => panic!("{other:?}"),
    });
    assert_eq!(result.int(), Some(27));
}
