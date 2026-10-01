;; thingatpt.el — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - word-at-point returns a STRING ("" past end).
;;   - symbol-at-point returns a SYMBOL (or nil when no symbol).
;;   - at a space between words → returns the word STARTED there.
;;   - number-at-point returns a NUMBER.
;;   - sexp-at-point returns the SEXP form.
;;   - bounds-of-thing-at-point returns (BEG . END).
;;   - url-at-point / filename-at-point are NOT fns in 31.1.

(require 'thingatpt)

;; == surface ==
(qa (fboundp 'thing-at-point) t)
(qa (fboundp 'forward-thing) t)
(qa (fboundp 'bounds-of-thing-at-point) t)
(qa (fboundp 'form-at-point) t)
(qa (fboundp 'symbol-at-point) t)
(qa (fboundp 'word-at-point) t)
(qa (fboundp 'number-at-point) t)
(qa (fboundp 'sexp-at-point) t)
(qa (fboundp 'list-at-point) t)
(qa (fboundp 'thing-at-point-bounds-of-list-at-point) t)
(qa (fboundp 'thing-at-point-looking-at) t)
;; NOT fns
(qa (fboundp 'url-at-point) nil)
(qa (fboundp 'filename-at-point) nil)
(qa (fboundp 'thing-at-point-property-changed) nil)
(qa (fboundp 'thing-at-point--bounds-of-well-formed-sexp) nil)
(qa (fboundp 'thing-at-point--read-from-whole-string-limit) nil)
(qa (fboundp 'thing-at-point-beginning-of-sexp) nil)
(qa (fboundp 'thing-at-point-end-of-sexp) nil)
(qa (fboundp 'sentence-end-double-space) nil)
(qa (fboundp 'thing-at-point-url-path) nil)

;; == word-at-point ==
(with-temp-buffer
  (insert "hello world")
  (goto-char 2)
  (qa (word-at-point) "hello"))
(with-temp-buffer
  (insert "hello world")
  (goto-char 7)
  (qa (word-at-point) "world"))
;; at a space — returns the word ENDING at point
(with-temp-buffer
  (insert "hello world")
  (goto-char 6)
  (qa (word-at-point) "hello"))

;; == symbol-at-point — returns a SYMBOL ==
(with-temp-buffer
  (insert "foo-bar baz")
  (goto-char 2)
  (qa (symbol-at-point) 'foo-bar))
(with-temp-buffer
  (insert "a_b c")
  (goto-char 1)
  (qa (symbol-at-point) 'a_b))
;; at whitespace → symbol STARTING at point (first atom)
(with-temp-buffer
  (insert "a b")
  (goto-char 2)
  (qa (symbol-at-point) 'a))

;; == number-at-point ==
(with-temp-buffer
  (insert "12345")
  (goto-char 3)
  (qa (number-at-point) 12345))
(with-temp-buffer
  (insert "x3.14")
  (goto-char 4)
  (qa (number-at-point) 3.14))
(with-temp-buffer
  (insert "x -7")
  (goto-char 4)
  (qa (number-at-point) -7))
;; no number → nil
(with-temp-buffer
  (insert "abc")
  (goto-char 2)
  (qa (number-at-point) nil))

;; == sexp-at-point ==
(with-temp-buffer
  (insert "(a b c)")
  (goto-char 2)
  (qa (sexp-at-point) 'a))
(with-temp-buffer
  (insert "(a b c)")
  (goto-char 1)
  (qa (sexp-at-point) '(a b c)))

;; == list-at-point ==
(with-temp-buffer
  (insert "(x y)")
  (goto-char 1)
  (qa (listp (list-at-point)) t))

;; == bounds-of-thing-at-point ==
(with-temp-buffer
  (insert "abc def")
  (goto-char 2)
  (qa (bounds-of-thing-at-point 'word) '(1 . 4)))
(with-temp-buffer
  (insert "hello world")
  (goto-char 8)
  (qa (bounds-of-thing-at-point 'word) '(7 . 12)))

;; == forward-thing ==
(with-temp-buffer
  (insert "a b c")
  (goto-char 1)
  (forward-thing 'word)
  (qa (point) 2))
(with-temp-buffer
  (insert "a b c")
  (goto-char 1)
  (forward-thing 'word 2)
  (qa (point) 4))

;; == thing-at-point returns raw text for 'word ==
(with-temp-buffer
  (insert "abc def")
  (goto-char 2)
  (qa (thing-at-point 'word) "abc"))
;; 'symbol returns a string too
(with-temp-buffer
  (insert "foo-bar")
  (goto-char 3)
  (qa (thing-at-point 'symbol) "foo-bar"))

;; == form-at-point returns the list ==
(with-temp-buffer
  (insert "(+ 1 2)")
  (goto-char 1)
  (qa (form-at-point) '(+ 1 2)))

;; == thing-at-point-looking-at returns t when it matches ==
(with-temp-buffer
  (insert "abc123")
  (goto-char 1)
  (qa (thing-at-point-looking-at "[a-z]+[0-9]+") t))

;; == empty buffer → nil ==
(with-temp-buffer
  (qa (bounds-of-thing-at-point 'word) nil)
  (qa (thing-at-point 'word) nil)
  (qa (symbol-at-point) nil)
  (qa (word-at-point) nil)
  (qa (number-at-point) nil))

;; == url-at-point not a fn in 31.1 ==
(qa (fboundp 'url-at-point) nil)

(princ (qa--done))
