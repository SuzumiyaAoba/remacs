;; indent.el — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - indent-line-to sets indentation to column.
;;   - indent-rigidly shifts every line by N.
;;   - tab-to-tab-stop moves to next tab-stop.
;;   - indent-to-left-margin pads to left-margin.

;; == surface ==
(qa (fboundp 'indent-for-tab-command) t)
(qa (fboundp 'indent-relative) t)
(qa (fboundp 'indent-relative-maybe) t)
(qa (fboundp 'indent-rigidly) t)
(qa (fboundp 'indent-rigidly-left) t)
(qa (fboundp 'indent-rigidly-right) t)
(qa (fboundp 'indent-code-rigidly) t)
(qa (fboundp 'indent-line-to) t)
(qa (fboundp 'indent-to-left-margin) t)
(qa (fboundp 'move-to-left-margin) t)
(qa (fboundp 'indent-region) t)
(qa (fboundp 'indent-tabs-mode) t)
(qa (fboundp 'tab-to-tab-stop) t)
(qa (fboundp 'indent-according-to-mode) t)
(qa (fboundp 'indent-for-comment) t)
(qa (fboundp 'insert-parentheses) t)
(qa (fboundp 'indent-new-comment-line) t)
(qa (fboundp 'indent-pp-sexp) t)
;; NOT fns in 31.1
(qa (fboundp 'indent-region-function) nil)
(qa (fboundp 'indent-line-function) nil)
(qa (fboundp 'indent-ffap-indent-previous) nil)

;; == indent-line-to pads to column ==
(let ((buf (generate-new-buffer " *i*")))
  (unwind-protect
      (with-current-buffer buf
        (insert "abc")
        (goto-char (point-min))
        (indent-line-to 4)
        (qa (buffer-string) "    abc"))
    (kill-buffer buf)))

;; == indent-line-to 0 — flush-left ==
(let ((buf (generate-new-buffer " *i*")))
  (unwind-protect
      (with-current-buffer buf
        (insert "   abc")
        (goto-char (point-min))
        (indent-line-to 0)
        (qa (buffer-string) "abc"))
    (kill-buffer buf)))

;; == indent-rigidly 2 — shifts all lines right ==
(let ((buf (generate-new-buffer " *i*")))
  (unwind-protect
      (with-current-buffer buf
        (insert "abc\ndef\nghi")
        (indent-rigidly (point-min) (point-max) 2)
        (qa (buffer-string) "  abc\n  def\n  ghi"))
    (kill-buffer buf)))

;; == indent-rigidly -2 — shifts left ==
(let ((buf (generate-new-buffer " *i*")))
  (unwind-protect
      (with-current-buffer buf
        (insert "    abc\n    def")
        (indent-rigidly (point-min) (point-max) -2)
        (qa (buffer-string) "  abc\n  def"))
    (kill-buffer buf)))

;; == tab-to-tab-stop — moves to next stop ==
(let ((buf (generate-new-buffer " *t*")))
  (unwind-protect
      (with-current-buffer buf
        (insert "a")
        (goto-char (point-max))
        (let ((tab-stop-list '(4 8 12)))
          (tab-to-tab-stop)
          ;; GNU: tab-stop-list stops; first stop after col 1 = 4
          (qa (current-column) 4)))
    (kill-buffer buf)))

;; == tab-to-tab-stop again ==
(let ((buf (generate-new-buffer " *t*")))
  (unwind-protect
      (with-current-buffer buf
        (insert "a")
        (goto-char (point-max))
        (let ((tab-stop-list '(4 8 12)))
          (tab-to-tab-stop)
          (tab-to-tab-stop)
          (qa (current-column) 8)))
    (kill-buffer buf)))

;; == indent-to-left-margin ==
(let ((buf (generate-new-buffer " *i*")))
  (unwind-protect
      (with-current-buffer buf
        (insert "abc")
        (goto-char (point-min))
        (let ((left-margin 5))
          (indent-to-left-margin))
        (qa (buffer-string) "     abc"))
    (kill-buffer buf)))

;; == indent-tabs-mode is a var ==
(qa (boundp 'indent-tabs-mode) t)
;; default is t in fundamental-mode
(let ((buf (generate-new-buffer " *i*")))
  (unwind-protect
      (with-current-buffer buf
        (qa indent-tabs-mode t))
    (kill-buffer buf)))

;; == tab-stop-list var ==
(qa (boundp 'tab-stop-list) t)

;; == left-margin var ==
(qa (boundp 'left-margin) t)
(qa left-margin 0)

;; == indent-relative-first-indent-point inserts a tab on first line ==
(let ((buf (generate-new-buffer " *i*")))
  (unwind-protect
      (with-current-buffer buf
        (insert "abc")
        (goto-char (point-min))
        (indent-relative-first-indent-point)
        (qa (buffer-string) "\tabc"))
    (kill-buffer buf)))

;; == indent-according-to-mode in fundamental-mode is a no-op ==
(let ((buf (generate-new-buffer " *i*")))
  (unwind-protect
      (with-current-buffer buf
        (insert "abc")
        (goto-char (point-min))
        (indent-according-to-mode)
        (qa (buffer-string) "abc"))
    (kill-buffer buf)))

(princ (qa--done))
