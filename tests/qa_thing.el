;; thing-at-point / forward-thing / replace-regexp-in-region — GNU 31.1
;; pins (verified).
;;
;; GNU truths pinned:
;;   - forward-thing returns t (not a marker, not the new point).
;;   - bounds-of-thing-inside / bounds-of-thing-internal /
;;     thing-at-point-looking-at are NOT fns.
;;   - replace-regexp-in-region returns count on hit, nil on miss.

;; == fns ==
(qa (fboundp 'forward-thing) t)
(qa (fboundp 'thing-at-point) t)
(qa (fboundp 'bounds-of-thing-at-point) t)
(qa (fboundp 'bounds-of-thing-inside) nil)
(qa (fboundp 'bounds-of-thing-internal) nil)
(qa (fboundp 'thing-at-point-looking-at) nil)
(qa (fboundp 'match-data--translate) t)
(qa (fboundp 'replace-regexp-in-region) t)

;; == thing-at-point 'word / 'symbol / 'line ==
(with-temp-buffer
  (insert "hello world")
  (goto-char 2)
  (qa (thing-at-point 'word) "hello")
  (qa (thing-at-point 'symbol) "hello")
  (qa (thing-at-point 'line) "hello world"))

;; == bounds-of-thing-at-point ==
(with-temp-buffer
  (insert "hello world")
  (goto-char 2)
  (qa (bounds-of-thing-at-point 'word) '(1 . 6)))

;; == forward-thing returns t; sets point ==
(with-temp-buffer
  (insert "hello world")
  (goto-char 1)
  (qa (forward-thing 'word) t)
  (qa (point) 6))
;; sexp inside a paren → nil (moves past close)
(with-temp-buffer
  (emacs-lisp-mode)
  (insert "(a b c)\nd e f")
  (goto-char 1)
  (qa (forward-thing 'sexp) nil)
  (qa (point) 8))
;; 'line goes via forward-line → returns 0
(with-temp-buffer
  (insert "line1\nline2\nline3")
  (goto-char 1)
  (qa (forward-thing 'line) 0)
  (qa (point) 7))
;; negative N
(with-temp-buffer
  (insert "one two three")
  (goto-char (point-max))
  (qa (forward-thing 'word -1) t)
  (qa (point) 9))
;; forward-thing returns t, not a marker
(with-temp-buffer
  (insert "abc")
  (goto-char 1)
  (qa (markerp (forward-thing 'word)) nil))

;; == match-string via looking-at ==
(with-temp-buffer
  (insert "hello world")
  (goto-char 1)
  (qa (looking-at "h\\(e\\)llo") t)
  (qa (match-string 1) "e"))

;; == replace-regexp-in-region ==
(with-temp-buffer
  (insert "foo bar foo")
  (qa (replace-regexp-in-region "foo" "X" 1 12) 2)
  (qa (buffer-string) "X bar X"))
;; no match → nil, not 0
(with-temp-buffer
  (insert "abc")
  (qa (replace-regexp-in-region "zzz" "X" 1 4) nil))
(with-temp-buffer
  (insert "foo\nbar")
  (qa (replace-regexp-in-region "o" "0" 1 5) 2)
  (qa (buffer-string) "f00\nbar"))

(princ (qa--done))
