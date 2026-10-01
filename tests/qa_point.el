;; point/char movement + buffer size — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - point is 1-based; point-min = 1, point-max = size+1.
;;   - forward-char/backward-char move by N chars.
;;   - goto-char clamps to [point-min, point-max].
;;   - char-before/char-after return nil at the boundary.

;; `qa' evals its form in DYNAMIC scope — `m' must be special.
(defvar m)

;; == surface ==
(qa (fboundp 'point) t)
(qa (fboundp 'point-min) t)
(qa (fboundp 'point-max) t)
(qa (fboundp 'goto-char) t)
(qa (fboundp 'forward-char) t)
(qa (fboundp 'backward-char) t)
(qa (fboundp 'char-after) t)
(qa (fboundp 'char-before) t)
(qa (fboundp 'following-char) t)
(qa (fboundp 'preceding-char) t)
(qa (fboundp 'point-marker) t)
(qa (fboundp 'buffer-size) t)
(qa (fboundp 'point-min-marker) t)
(qa (fboundp 'point-max-marker) t)
(qa (fboundp 'point-at-bol) t)
(qa (fboundp 'point-at-eol) t)
(qa (fboundp 'line-beginning-position) t)
(qa (fboundp 'line-end-position) t)
(qa (fboundp 'forward-line) t)
(qa (fboundp 'end-of-line) t)
(qa (fboundp 'beginning-of-line) t)
(qa (fboundp 'line-number-at-pos) t)

;; == point positions are 1-based ==
(let ((buf (generate-new-buffer " *b*")))
  (with-current-buffer buf
    (insert "abc\ndef\nghi")
    ;; empty-region → point=1
    (qa (point-min) 1)
    (qa (point-max) 12)
    ;; point starts at 1
    (goto-char (point-min))
    (qa (point) 1))
  (kill-buffer buf))

;; == forward-char / backward-char ==
(let ((buf (generate-new-buffer " *b*")))
  (with-current-buffer buf
    (insert "abc\ndef\nghi")
    (goto-char (point-min))
    (forward-char 2)
    (qa (point) 3)
    (backward-char 1)
    (qa (point) 2)
    ;; negative forward = backward
    (forward-char -1)
    (qa (point) 1))
  (kill-buffer buf))

;; == goto-char clamps to bounds ==
(let ((buf (generate-new-buffer " *b*")))
  (with-current-buffer buf
    (insert "abc")
    (goto-char 0)
    (qa (point) 1)
    (goto-char 99)
    (qa (point) 4)
    ;; exact
    (goto-char 2)
    (qa (point) 2))
  (kill-buffer buf))

;; == char-after / char-before ==
(let ((buf (generate-new-buffer " *b*")))
  (with-current-buffer buf
    (insert "abc")
    (goto-char 1)
    (qa (char-after) ?a)
    (qa (char-before) nil)
    (goto-char 4)
    (qa (char-after) nil)
    (qa (char-before) ?c)
    (goto-char 2)
    (qa (char-after) ?b)
    (qa (char-before) ?a))
  (kill-buffer buf))

;; == char-after with explicit position ==
(let ((buf (generate-new-buffer " *b*")))
  (with-current-buffer buf
    (insert "abc")
    (qa (char-after 2) ?b)
    (qa (char-before 3) ?b)
    ;; out of bounds → nil
    (qa (char-after 99) nil)
    (qa (char-before 0) nil))
  (kill-buffer buf))

;; == following-char / preceding-char ==
(let ((buf (generate-new-buffer " *b*")))
  (with-current-buffer buf
    (insert "abc")
    (goto-char 2)
    (qa (following-char) ?b)
    (qa (preceding-char) ?a))
  (kill-buffer buf))

;; == buffer-size ==
(let ((buf (generate-new-buffer " *b*")))
  (with-current-buffer buf
    (qa (buffer-size) 0)
    (insert "hello")
    (qa (buffer-size) 5)
    (erase-buffer)
    (qa (buffer-size) 0))
  (kill-buffer buf))

;; == line-beginning-position / line-end-position ==
(let ((buf (generate-new-buffer " *b*")))
  (with-current-buffer buf
    (insert "line1\nline2\nline3")
    (goto-char 8) ;; inside line2
    (qa (line-beginning-position) 7)
    (qa (line-end-position) 12)
    ;; line-number-at-pos
    (qa (line-number-at-pos) 2)
    (goto-char 1)
    (qa (line-number-at-pos) 1))
  (kill-buffer buf))

;; == forward-line moves to next line start ==
(let ((buf (generate-new-buffer " *b*")))
  (with-current-buffer buf
    (insert "a\nb\nc\n")
    (goto-char 1)
    (forward-line 1)
    (qa (point) 3)
    (forward-line 2)
    ;; last line — position past "c\n"
    (qa (point) 7))
  (kill-buffer buf))

;; == beginning-of-line / end-of-line ==
(let ((buf (generate-new-buffer " *b*")))
  (with-current-buffer buf
    (insert "abc\ndef\n")
    (goto-char 5)
    (end-of-line)
    (qa (point) 8)
    (beginning-of-line)
    (qa (point) 5))
  (kill-buffer buf))

;; == point-marker returns a marker ==
(let ((buf (generate-new-buffer " *b*")))
  (with-current-buffer buf
    (insert "abc")
    (goto-char 2)
    (let ((m (point-marker)))
      (qa (markerp m) t)
      (qa (marker-position m) 2)
      ;; marker tracks buffer
      (qa (eq (marker-buffer m) (current-buffer)) t)))
  (kill-buffer buf))

;; == save-excursion restores point ==
(let ((buf (generate-new-buffer " *b*")))
  (with-current-buffer buf
    (insert "abc")
    (goto-char 2)
    (save-excursion
      (goto-char 3)
      (qa (point) 3))
    (qa (point) 2))
  (kill-buffer buf))

;; == char-to-string ==
(qa (char-to-string ?a) "a")
(qa (char-to-string ?x) "x")
;; non-char signal
(qa-err (char-to-string "a") 'wrong-type-argument)

;; == eobp / bobp ==
(let ((buf (generate-new-buffer " *b*")))
  (with-current-buffer buf
    (insert "ab")
    (goto-char 1)
    (qa (bobp) t)
    (qa (eobp) nil)
    (goto-char 3)
    (qa (bobp) nil)
    (qa (eobp) t))
  (kill-buffer buf))

;; == insert-char ==
(let ((buf (generate-new-buffer " *b*")))
  (with-current-buffer buf
    (insert-char ?x 3)
    (qa (buffer-string) "xxx")
    ;; insert-char with COUNT & inherit
    (erase-buffer)
    (insert-char ?- 5)
    (qa (buffer-string) "-----"))
  (kill-buffer buf))

;; == delete-char ==
(let ((buf (generate-new-buffer " *b*")))
  (with-current-buffer buf
    (insert "abc")
    (goto-char 1)
    (delete-char 1)
    (qa (buffer-string) "bc")
    (qa (point) 1))
  (kill-buffer buf))

;; == delete-backward-char ==
(let ((buf (generate-new-buffer " *b*")))
  (with-current-buffer buf
    (insert "abc")
    (goto-char 4)
    (delete-backward-char 1)
    (qa (buffer-string) "ab")
    (qa (point) 3))
  (kill-buffer buf))

;; == erase-buffer empties ==
(let ((buf (generate-new-buffer " *b*")))
  (with-current-buffer buf
    (insert "abcdef")
    (erase-buffer)
    (qa (buffer-string) "")
    (qa (point-min) 1)
    (qa (point-max) 1))
  (kill-buffer buf))

(princ (qa--done))
