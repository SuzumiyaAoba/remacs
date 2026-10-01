;; syntax-ppss + parse state — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - syntax-ppss returns a 11-element list.
;;   - Slot 0 = paren depth, slot 3 = in-string (t or char), slot 4 = in-comment.

(require 'syntax)

;; == surface ==
(qa (fboundp 'syntax-ppss) t)
(qa (fboundp 'syntax-ppss-context) t)
(qa (fboundp 'syntax-ppss-depth) t)
(qa (fboundp 'syntax-ppss-toplevel-pos) t)
(qa (fboundp 'syntax-ppss-flush-cache) t)
(qa (fboundp 'syntax-propertize) t)
(qa (fboundp 'syntax-propertize-rules) t)
(qa (fboundp 'syntax-propertize-precompile-rules) t)
;; NOT fns
(qa (fboundp 'syntax-propertize-extend-region-functions) nil)

;; == syntax-ppss at point-max outside any list ==
(let ((buf (generate-new-buffer " *s*")))
  (unwind-protect
      (with-current-buffer buf
        (insert "(a b)")
        (goto-char (point-max))
        (let ((pp (syntax-ppss)))
          ;; depth 0, no string, no comment
          (qa (syntax-ppss-depth pp) 0)
          (qa (nth 3 pp) nil)
          (qa (nth 4 pp) nil)))
    (kill-buffer buf)))

;; == inside a paren — depth 1 ==
(let ((buf (generate-new-buffer " *s*")))
  (unwind-protect
      (with-current-buffer buf
        (insert "(a")
        (goto-char (point-max))
        (let ((pp (syntax-ppss)))
          (qa (syntax-ppss-depth pp) 1)))
    (kill-buffer buf)))

;; == nested parens — depth 3 ==
(let ((buf (generate-new-buffer " *s*")))
  (unwind-protect
      (with-current-buffer buf
        (insert "(a (b (c")
        (goto-char (point-max))
        (qa (syntax-ppss-depth (syntax-ppss)) 3))
    (kill-buffer buf)))

;; == inside a string — slot 3 is the open-quote char ==
(let ((buf (generate-new-buffer " *s*")))
  (unwind-protect
      (with-current-buffer buf
        (insert "(a \"str\" b)")
        ;; inside the "str" — pos 5
        (goto-char 5)
        (let ((pp (syntax-ppss)))
          (qa (syntax-ppss-depth pp) 1)
          ;; slot 3 = the opening " char
          (qa (nth 3 pp) 34)))
    (kill-buffer buf)))

;; == outside string —
(let ((buf (generate-new-buffer " *s*")))
  (unwind-protect
      (with-current-buffer buf
        (insert "(a \"str\" b)")
        (goto-char (point-max))
        (let ((pp (syntax-ppss)))
          (qa (nth 3 pp) nil)))
    (kill-buffer buf)))

;; == inside a comment — slot 4 ==
(let ((buf (generate-new-buffer " *s*")))
  (unwind-protect
      (with-current-buffer buf
        (emacs-lisp-mode)
        (insert "; comment\nx")
        ;; inside the comment
        (goto-char 3)
        (let ((pp (syntax-ppss)))
          ;; slot 4 = t for end-of-line comment
          (qa (nth 4 pp) t)))
    (kill-buffer buf)))

;; == after comment — outside comment ==
(let ((buf (generate-new-buffer " *s*")))
  (unwind-protect
      (with-current-buffer buf
        (emacs-lisp-mode)
        (insert "; comment\nx")
        (goto-char (point-max))
        (let ((pp (syntax-ppss)))
          (qa (nth 4 pp) nil)))
    (kill-buffer buf)))

;; == syntax-ppss-toplevel-pos — position of enclosing ( ==
(let ((buf (generate-new-buffer " *s*")))
  (unwind-protect
      (with-current-buffer buf
        (insert "(a (b)")
        (goto-char 4)
        ;; toplevel is the outer (
        (qa (syntax-ppss-toplevel-pos (syntax-ppss)) 1))
    (kill-buffer buf)))

;; == toplevel of nested —
(let ((buf (generate-new-buffer " *s*")))
  (unwind-protect
      (with-current-buffer buf
        (insert "(a (b)")
        (goto-char 5)
        ;; inside the inner (, toplevel is still the outer
        (qa (syntax-ppss-toplevel-pos (syntax-ppss)) 1))
    (kill-buffer buf)))

;; == syntax-ppss-context — 'string / 'comment / nil ==
(let ((buf (generate-new-buffer " *s*")))
  (unwind-protect
      (with-current-buffer buf
        (insert "(a \"str\" b)")
        (goto-char 5)
        (qa (syntax-ppss-context (syntax-ppss)) 'string))
    (kill-buffer buf)))

;; == outside → nil ==
(let ((buf (generate-new-buffer " *s*")))
  (unwind-protect
      (with-current-buffer buf
        (insert "(a \"str\" b)")
        (goto-char (point-max))
        (qa (syntax-ppss-context (syntax-ppss)) nil))
    (kill-buffer buf)))

;; == comment context ==
(let ((buf (generate-new-buffer " *s*")))
  (unwind-protect
      (with-current-buffer buf
        (emacs-lisp-mode)
        (insert "; hi\nx")
        (goto-char 3)
        (qa (syntax-ppss-context (syntax-ppss)) 'comment))
    (kill-buffer buf)))

;; == syntax-ppss-flush-cache — clears the cache ==
(let ((buf (generate-new-buffer " *s*")))
  (unwind-protect
      (with-current-buffer buf
        (insert "(a)")
        (syntax-ppss)  ;; populate cache
        (syntax-ppss-flush-cache 1)
        ;; still works
        (qa (syntax-ppss-depth (syntax-ppss)) 0))
    (kill-buffer buf)))

(princ (qa--done))
