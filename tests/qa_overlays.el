;; Overlays/invisibility/motion — GNU 31.1 pins:
;;   - `next-overlay-change' returns the position of the next overlay
;;     boundary; past the last → point-max.  `previous-overlay-change'
;;     symmetric; before the first → point-min.
;;   - `overlays-at' order: most-recently-created FIRST (reverse
;;     creation order at the same position); overlay `priority' does NOT
;;     reorder overlays-at (it only affects display).
;;   - `buffer-invisibility-spec' defaults to t: ANY non-nil `invisible'
;;     prop hides text.  A list spec makes only LISTED symbols invisible.
;;   - `invisible-p' checks the effective invisibility at a char pos.
;;   - `count-words' counts whitespace-separated words.
;;   - `bounds-of-thing-at-point'/'thing-at-point' for word/symbol/sexp/
;;     line: word stops at `-', symbol doesn't.
;;   - `narrow-to-defun' narrows to the surrounding top-level form
;;     (in emacs-lisp-mode).
;;   - `position-bytes'/'byte-to-position' convert between char and byte
;;     positions on multibyte text (UTF-8: あ = 3 bytes).
;;   - `line-move' returns the # of lines actually moved (t = count>0).

;; == next-overlay-change / previous-overlay-change ==
(qa (with-temp-buffer
      (insert "abcdefgh")
      (make-overlay 2 5)
      (make-overlay 4 7)
      (list (next-overlay-change 1)
            (next-overlay-change 2)
            (next-overlay-change 5)
            (next-overlay-change 7)))
    '(2 4 7 9))
(qa (with-temp-buffer
      (insert "abcdefgh")
      (make-overlay 2 5)
      (make-overlay 4 7)
      (list (previous-overlay-change 9)
            (previous-overlay-change 7)
            (previous-overlay-change 5)
            (previous-overlay-change 1)))
    '(7 5 4 1))
;; single overlay
(qa (with-temp-buffer
      (insert "abcdefgh")
      (make-overlay 2 5)
      (list (next-overlay-change 1)
            (next-overlay-change 5)
            (previous-overlay-change 9)
            (previous-overlay-change 2)))
    '(2 9 5 1))
;; no overlays → boundaries
(qa (with-temp-buffer
      (insert "abc")
      (list (next-overlay-change 1)
            (previous-overlay-change 4)))
    '(4 1))

;; == overlays-at ordering: newest first, priority ignored ==
(qa (with-temp-buffer
      (insert "abcdef")
      (let ((o1 (make-overlay 2 5)) (o2 (make-overlay 2 5)))
        (list (eq (car (overlays-at 3)) o2)
              (eq (cadr (overlays-at 3)) o1))))
    '(t t))
(qa (with-temp-buffer
      (insert "abcdef")
      (let ((o1 (make-overlay 2 5)) (o2 (make-overlay 2 5)))
        (overlay-put o1 'priority 10)
        (overlay-put o2 'priority 1)
        (eq (car (overlays-at 3)) o1)))
    nil) ;; GNU: newest-first regardless of priority
;; overlays-at outside/edge
(qa (with-temp-buffer
      (insert "abcdef")
      (make-overlay 2 5)
;; overlays-in covers overlays intersecting [START,END)
(qa (with-temp-buffer
      (insert "abcdefgh")
      (make-overlay 2 5)
      (make-overlay 4 7)
      (list (length (overlays-in 1 8))
            (length (overlays-in 5 7))
            (length (overlays-in 7 8))))
    '(2 1 0))
      (list (length (overlays-at 2))
            (length (overlays-at 4))))
    '(1 1))

;; == buffer-invisibility-spec: default t ==
(qa (with-temp-buffer
      (insert "abcdefgh")
      buffer-invisibility-spec)
    t)
(qa (with-temp-buffer
      (insert "abcdefgh")
      (put-text-property 2 5 'invisible t)
      (list (invisible-p 3) (invisible-p 1) (invisible-p 5)))
    '(t nil nil))
;; spec list: only listed symbols hide
(qa (with-temp-buffer
        (insert "abcdefgh")
        (let ((buffer-invisibility-spec '(x)))
          (put-text-property 6 8 'invisible 'x)
          (list (invisible-p 7) (invisible-p 3))))
      '(t nil))
;; unlisted symbol not hidden under list spec
(qa (with-temp-buffer
        (insert "abcdefgh")
        (let ((buffer-invisibility-spec '(x)))
          (put-text-property 6 8 'invisible 'y)
          (invisible-p 7)))
      nil)
;; spec t: any non-nil value hides
(qa (with-temp-buffer
        (insert "abcdefgh")
        (put-text-property 6 8 'invisible 'whatever)
        (invisible-p 7))
      t)
;; spec t + invisible nil → not hidden
(qa (with-temp-buffer
      (insert "abcdefgh")
      (put-text-property 6 8 'invisible nil)
      (invisible-p 7))
    nil)

;; Issue #161: ellipsis entries, list-valued properties, and positions.
(qa (with-temp-buffer
      (insert "ab")
      (add-to-invisibility-spec '(foo . t))
      (put-text-property 1 2 'invisible 'foo)
      (invisible-p 1))
    2)
(qa (with-temp-buffer
      (insert "ab")
      (setq buffer-invisibility-spec '((foo . t)))
      (put-text-property 1 2 'invisible 'foo)
      (invisible-p 1))
    2)
(qa (let ((buffer-invisibility-spec '((foo)))) (invisible-p 'foo)) t)
(qa (let ((buffer-invisibility-spec '((foo . ellipsis))))
      (invisible-p 'foo))
    2)
(qa (with-temp-buffer
      (insert "ab")
      (add-to-invisibility-spec 'foo)
      (put-text-property 1 2 'invisible '(x foo))
      (invisible-p 1))
    t)
(qa (let ((buffer-invisibility-spec '((foo . t))))
      (invisible-p '(x foo)))
    2)
(qa (let ((buffer-invisibility-spec '(foo))) (invisible-p '(x foo . y))) t)
(qa (let ((buffer-invisibility-spec '(foo . rest))) (invisible-p 'foo)) t)
(qa (let ((buffer-invisibility-spec '((foo . t) foo))) (invisible-p 'foo)) 2)
(qa (let ((buffer-invisibility-spec '(foo (foo . t)))) (invisible-p 'foo)) t)
(qa (let ((buffer-invisibility-spec '((foo . t) bar)))
      (invisible-p '(bar foo)))
    t)
(qa (let ((prop (list 'foo)))
      (let ((buffer-invisibility-spec (list prop))) (invisible-p prop)))
    t)
(qa (let* ((prop (list 'foo))
           (buffer-invisibility-spec (list '(foo . t) prop)))
      (invisible-p prop))
    t)
(qa (let ((buffer-invisibility-spec '(x . foo))) (invisible-p 'foo)) nil)
(qa (let ((buffer-invisibility-spec '(foo))) (invisible-p '(x . foo))) nil)
(qa (let ((buffer-invisibility-spec '(nil))) (invisible-p nil)) t)
(qa (with-temp-buffer (insert "ab") (invisible-p -1)) t)
(qa (let ((buffer-invisibility-spec nil)) (invisible-p -1)) nil)
(qa (let ((buffer-invisibility-spec t)) (invisible-p (1+ most-positive-fixnum))) t)
(qa-err-data (with-temp-buffer (insert "ab") (invisible-p 0))
             'args-out-of-range '(0))
(qa-err-data (invisible-p (make-marker))
             'error '("Marker does not point anywhere"))
(qa (with-temp-buffer
      (insert "ab")
      (setq buffer-invisibility-spec '((foo . t)))
      (let ((overlay (make-overlay 1 2)))
        (overlay-put overlay 'invisible 'foo)
        (invisible-p (copy-marker 1))))
    2)

;; == count-words ==
(qa (with-temp-buffer
      (insert "hello world  foo\tbar")
      (list (count-words (point-min) (point-max))
            (count-words 1 12)))
    '(4 2))
(qa (with-temp-buffer
      (insert "")
      (count-words (point-min) (point-max)))
    0)
(qa (with-temp-buffer
      (insert "one")
      (count-words 1 4))
    1)

;; == bounds-of-thing-at-point / thing-at-point ==
(qa (with-temp-buffer
      (insert "hello-world (a b) \"str\"")
      (goto-char 3)
      (list (bounds-of-thing-at-point 'word)
            (thing-at-point 'word)
            (bounds-of-thing-at-point 'symbol)
            (thing-at-point 'symbol)))
    '((1 . 6) "hello" (1 . 12) "hello-world"))
(qa (with-temp-buffer
      (insert "(a b) \"str\"")
      (goto-char 2)
      (list (bounds-of-thing-at-point 'sexp)
            (thing-at-point 'sexp)))
    '((2 . 3) "a"))
(qa (with-temp-buffer
      (insert "hello world")
      (goto-char 3)
      (thing-at-point 'line))
    "hello world")
(qa (with-temp-buffer
      (insert "hi")
      (goto-char 2)
      (list (thing-at-point 'symbol) (thing-at-point 'word)))
    '("hi" "hi"))
;; thing-at-point on empty position → nil
(qa (with-temp-buffer
      (thing-at-point 'word))
    nil)

;; == narrow-to-defun ==
(qa (with-temp-buffer
      (emacs-lisp-mode)
      (insert "(defun a ()\n  1)\n\n(defun b ()\n  2)\n")
      (goto-char (point-max))
      (narrow-to-defun)
      (list (buffer-string) (point-min) (point-max)))
    '("(defun b ()\n  2)\n" 19 36))
;; point at the FIRST defun narrows to it (incl. trailing blank line)
(qa (with-temp-buffer
      (emacs-lisp-mode)
      (insert "(defun a ()\n  1)\n\n(defun b ()\n  2)\n")
      (goto-char 3)
      (narrow-to-defun)
      (list (point-min) (point-max)))
    '(1 18))
(qa (with-temp-buffer
      (insert "abあcd")
      (list (position-bytes 1) (position-bytes 3) (position-bytes 4)))
    '(1 3 6)) ;; あ occupies bytes 3-5
(qa (with-temp-buffer
      (insert "abあcd")
      (list (byte-to-position 3) (byte-to-position 6)))
    '(3 4))
(qa (with-temp-buffer
      (insert "abcd")
      (list (position-bytes 3) (byte-to-position 3)))
    '(3 3))

;; == line-move ==
(qa (with-temp-buffer
      (insert "ab\ncd\nef")
      (goto-char 1)
      (list (line-move 1) (point)))
    '(t 4))
(qa (with-temp-buffer
      (insert "ab\ncd\nef")
      (goto-char 4)
      (list (line-move -1) (point)))
    '(t 1))
;; moving past the last line signals end-of-buffer
(qa-err (with-temp-buffer
          (insert "ab\ncd")
          (goto-char 1)
          (line-move 5))
        'end-of-buffer)
;; no-newline buffer: can't move at all
(qa-err (with-temp-buffer
          (insert "abc")
          (goto-char 1)
          (line-move 1))
        'end-of-buffer)

;; == line-number-at-pos ==
(qa (with-temp-buffer
      (insert "a\nb\nc")
      (list (line-number-at-pos 1)
            (line-number-at-pos 3)
            (line-number-at-pos 5)))
    '(1 2 3))
;; ABSOLUTE arg ignores narrowing
(qa (with-temp-buffer
      (insert "a\nb\nc\nd")
      (narrow-to-region 5 7)
      (list (line-number-at-pos 5)
            (line-number-at-pos 5 t)))
    '(1 3))

;; == count-lines: counts LINES (newlines + 1 when non-empty) ==
(qa (with-temp-buffer
      (insert "a\nb\nc")
      (count-lines (point-min) (point-max)))
    3)
(qa (with-temp-buffer
      (insert "a\nb\nc\n")
      (count-lines (point-min) (point-max)))
    3)
(qa (with-temp-buffer
      (insert "x")
      (count-lines 1 2))
    1)

(princ (qa--done))
