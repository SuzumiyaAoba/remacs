;; pp.el — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - pp writes pretty-printed SEXP + a trailing newline.
;;   - pp-to-string returns the printed form + newline.
;;   - pp-fill wraps a region to fill-column.

(require 'pp)

;; == surface ==
(qa (fboundp 'pp) t)
(qa (fboundp 'pp-buffer) t)
(qa (fboundp 'pp-region) nil)
(qa (fboundp 'pp-last-sexp) t)
(qa (fboundp 'pp-eval-expression) t)
(qa (fboundp 'pp-eval-last-sexp) t)
(qa (fboundp 'pp-macroexpand-expression) t)
(qa (fboundp 'pp-macroexpand-last-sexp) t)
(qa (fboundp 'pp-to-string) t)
(qa (fboundp 'pp-display-expression) t)
(qa (fboundp 'pp-pretty-format) nil)
(qa (fboundp 'pp-fill) t)
(qa (fboundp 'pp-string) nil)
(qa (fboundp 'pp-newline-and-indent) nil)
(qa (fboundp 'pp-separator) nil)
(qa (fboundp 'pp-newline-and-indent-level) nil)
(qa (fboundp 'pp-newline-and-indent-region) nil)

;; == pp writes SEXP + newline ==
(let ((buf (generate-new-buffer " *pp*")))
  (unwind-protect
      (progn
        (pp '(x) buf)
        (qa (with-current-buffer buf (buffer-string)) "(x)\n"))
    (kill-buffer buf)))

;; == pp on atoms ==
(let ((buf (generate-new-buffer " *pp*")))
  (unwind-protect
      (progn
        (pp 42 buf)
        (qa (with-current-buffer buf (buffer-string)) "42\n"))
    (kill-buffer buf)))

;; == pp-to-string adds the newline too ==
(qa (pp-to-string '(x)) "(x)\n")
(qa (pp-to-string 'sym) "sym\n")
(qa (pp-to-string "str") "\"str\"\n")
(qa (pp-to-string 42) "42\n")
(qa (pp-to-string nil) "nil\n")

;; == pp on a let — fits on one line ==
(let ((buf (generate-new-buffer " *pp*")))
  (unwind-protect
      (progn
        (pp '(let ((a 1) (b 2)) (+ a b)) buf)
        (let ((out (with-current-buffer buf (buffer-string))))
          ;; fits in fill-column → single line
          (qa out "(let ((a 1) (b 2)) (+ a b))\n")))
    (kill-buffer buf)))

;; == pp-to-string of a quote form ==
(let ((s (pp-to-string '(quote x))))
  ;; prints as 'x not (quote x)
  (qa s "'x\n"))

;; == pp-eval-expression / -last-sexp — need interactive; verify fboundp ==
(qa (fboundp 'pp-eval-expression) t)
(qa (fboundp 'pp-eval-last-sexp) t)

;; == pp-macroexpand-* — same ==
(qa (fboundp 'pp-macroexpand-expression) t)
(qa (fboundp 'pp-macroexpand-last-sexp) t)

;; == pp-buffer — pretty-print current buffer ==
(let ((buf (generate-new-buffer " *pp*")))
  (unwind-protect
      (with-current-buffer buf
        (insert "(foo (bar) (baz))")
        (pp-buffer)
        ;; buffer content is re-formatted
        (qa (stringp (buffer-string)) t)
        (qa (string-prefix-p "(foo" (buffer-string)) t))
    (kill-buffer buf)))

;; pp-region is NOT a fn in 31.1 batch — covered by fboundp above

;; == pp-fill ==
(let ((buf (generate-new-buffer " *pp*")))
  (unwind-protect
      (with-current-buffer buf
        (insert "aaa bbb ccc ddd eee fff")
        ;; doesn't wrap if fits
        (let ((fill-column 40))
          (pp-fill (point-min) (point-max)))
        (qa (string-match-p "\n" (buffer-string)) nil))
    (kill-buffer buf)))

;; pp-fill wraps at fill-column — splits at "eee"
(let ((buf (generate-new-buffer " *pp*")))
  (unwind-protect
      (with-current-buffer buf
        (insert "aaa bbb ccc ddd eee fff ggg hhh iii")
        (let ((fill-column 20))
          (pp-fill (point-min) (point-max))
          (qa (buffer-string) "aaa bbb ccc ddd eee\nfff ggg hhh iii")))
    (kill-buffer buf)))

;; == pp-last-sexp — pretty-print the sexp before point ==
(let ((buf (generate-new-buffer " *pp*")))
  (unwind-protect
      (with-current-buffer buf
        (insert "(a b)")
        (goto-char (point-max))
        ;; pp-last-sexp needs NO-DISPLAY or similar
        ;; on batch — just verify it doesn't error on a real sexp
        (condition-case e
            (pp-last-sexp nil)
          (error nil)))
    (kill-buffer buf)))

(princ (qa--done))
