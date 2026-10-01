;; narrow/widen + save-restriction — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - narrow-to-region shrinks point-min/point-max to the given BEG END
;;     positions (1-based, NOT re-anchored).
;;   - widen restores 1 / (1+ size).
;;   - save-restriction restores narrowing on exit (including errors).
;;   - narrow-to-page splits on ^L (form feed).

;; == surface ==
(qa (fboundp 'narrow-to-region) t)
(qa (fboundp 'narrow-to-defun) t)
(qa (fboundp 'narrow-to-page) t)
(qa (fboundp 'widen) t)

;; == narrow-to-region — BEG END are absolute positions ==
(let ((buf (generate-new-buffer " *n*")))
  (with-current-buffer buf
    (insert "line1\nline2\nline3\n")
    (goto-char (point-min))
    (forward-line 1)
    ;; point is now at position 7 (1-based; "line2" starts at 7)
    (qa (point) 7)
    ;; narrow to line2: positions 7..12 ("line2")
    (narrow-to-region (point) (line-end-position))
    (qa (buffer-string) "line2")
    (qa (point-min) 7)
    (qa (point-max) 12)
    ;; widen restores
    (widen)
    (qa (buffer-string) "line1\nline2\nline3\n")
    (qa (point-min) 1)
    (qa (point-max) 19))
  (kill-buffer buf))

;; == save-restriction restores narrowing ==
(let ((buf (generate-new-buffer " *n*")))
  (with-current-buffer buf
    (insert "abc\ndef\n")
    (save-restriction
      (narrow-to-region 1 4)
      (qa (buffer-string) "abc"))
    (qa (buffer-string) "abc\ndef\n"))
  (kill-buffer buf))

;; == save-restriction unwinds on signal ==
(let ((buf (generate-new-buffer " *n*")))
  (with-current-buffer buf
    (insert "abc\ndef\n")
    (ignore-errors
      (save-restriction
        (narrow-to-region 1 4)
        (error "boom")))
    (qa (buffer-string) "abc\ndef\n"))
  (kill-buffer buf))

;; == narrow-to-page — ^L is the page separator ==
(let ((buf (generate-new-buffer " *n*")))
  (with-current-buffer buf
    (insert "p1\n\fp2\n\fp3\n")
    (goto-char (point-min))
    (forward-line 2) ;; into "p2" page
    (narrow-to-page)
    ;; GNU narrows to "p2\n" (page body, without surrounding ^L)
    (qa (buffer-string) "p2\n")
    (widen))
  (kill-buffer buf))

;; == narrow-to-region with no args → arity error ==
(qa-err (narrow-to-region) 'wrong-number-of-arguments)

;; == narrow-to-defun ==
(let ((buf (generate-new-buffer " *n*")))
  (with-current-buffer buf
    (insert "(defun f1 () 1)\n\n(defun f2 () 2)\n")
    ;; inside f2's defun (position 20 is past "d")
    (goto-char 20)
    (narrow-to-defun)
    ;; GNU narrows to "(defun f2 () 2)\n" — string-match starts at 1
    (qa (string-match-p "defun f2" (buffer-string)) 1)
    (qa (string-match-p "defun f1" (buffer-string)) nil)
    (widen))
  (kill-buffer buf))

;; == narrow + insert — insert at position goes BEFORE the position ==
(let ((buf (generate-new-buffer " *n*")))
  (with-current-buffer buf
    (insert "hello")
    ;; narrow to positions 2..4 ("el")
    (narrow-to-region 2 4)
    ;; goto-char 2 + insert → X goes before 'e'
    (goto-char 2)
    (insert "X")
    (qa (buffer-string) "Xel")
    (widen)
    ;; after widen: "h" + "Xel" + "lo"
    (qa (buffer-string) "hXello"))
  (kill-buffer buf))

;; == char-after past end-of-narrow is nil ==
(let ((buf (generate-new-buffer " *n*")))
  (with-current-buffer buf
    (insert "abcdefgh")
    (narrow-to-region 3 6)
    (qa (point-min) 3)
    (qa (point-max) 6)
    (goto-char (point-max))
    (qa (char-after) nil)
    (widen)
    (goto-char 4)
    (qa (char-after) ?d))
  (kill-buffer buf))

;; == point is clamped to narrowed bounds ==
(let ((buf (generate-new-buffer " *n*")))
  (with-current-buffer buf
    (insert "0123456789")
    (narrow-to-region 4 7)
    ;; goto-char clamps to [4,7]
    (goto-char 1)
    (qa (point) 4)
    (goto-char 10)
    (qa (point) 7)
    (widen))
  (kill-buffer buf))

(princ (qa--done))
