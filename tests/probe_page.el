;;; probe_page.el — page motion + vertical-motion/goal-column cluster vs GNU.
;;; Self-asserting: signals on mismatch.  -*- lexical-binding: t -*-

;; forward-page / backward-page (GNU textmodes/page.el)
(with-temp-buffer
  (insert "p1a\np1b\n\fp2a\np2b\np2c\n\fp3a\n")
  (goto-char 1)
  (forward-page) (cl-assert (= (point) 10))
  (goto-char 8)                      ; standing on the delimiter
  (forward-page) (cl-assert (= (point) 10))
  (forward-page -1) (cl-assert (= (point) 1))
  (backward-page) (cl-assert (= (point) 1))
  (cl-assert (equal (page--count-lines-page) '(2 0 2)))
  (goto-char (point-min)) (forward-line 1)
  (cl-assert (equal (page--count-lines-page) '(2 1 1))))

;; GNU forward-page counts / overshoot / negative
(with-temp-buffer
  (insert "h1a\nh1b\n\fh2a\n\fh3a\nh3b\nh3c\n")
  (goto-char 1)
  (forward-page 0) (cl-assert (= (point) 1))
  (forward-page 2) (cl-assert (= (point) 15))
  (forward-page 99) (cl-assert (= (point) 27))
  (backward-page 2) (cl-assert (= (point) 10))
  (forward-page -1) (cl-assert (= (point) 1)))

;; narrow-to-page restriction bounds (GNU semantics)
(with-temp-buffer
  (insert "h1a\nh1b\n\fh2a\n\fh3a\nh3b\nh3c\n")
  (goto-char 15)
  (narrow-to-page -1)
  (cl-assert (= (point-min) 10)) (cl-assert (= (point-max) 14))
  (widen)
  (goto-char 20) (narrow-to-page 0)
  (cl-assert (= (point-min) 15)) (cl-assert (= (point-max) 27)))

;; page--what-page: (page . line-in-page), bol/looking-back adjust
(with-temp-buffer
  (insert "h1a\nh1b\n\fh2a\n\fh3a\nh3b\nh3c\n")
  (goto-char 3)  (cl-assert (equal (page--what-page) '(1 1)))
  (goto-char 14) (cl-assert (equal (page--what-page) '(2 2)))
  (goto-char 8)  (cl-assert (equal (page--what-page) '(1 2))))

;; Zero-length page-delimiter must still make progress
(with-temp-buffer
  (insert "aaa\nbbb\n")
  (let ((page-delimiter ""))
    (goto-char 1)
    (forward-page 2)
    (cl-assert (= (point) 2))))

;; Delimiter must match at bol, not mid-line
(with-temp-buffer
  (insert "x\fy\n")
  (goto-char 1)
  (forward-page)
  (cl-assert (= (point) 5)))

;; mark-page with numeric args
(with-temp-buffer
  (insert "a\n\fb\n\fc\n\fd\n")
  (goto-char 6)
  (mark-page 2)
  (cl-assert (= (point) 10)) (cl-assert (= (mark) 12))
  (goto-char 6)
  (mark-page -1)
  (cl-assert (= (point) 1)) (cl-assert (= (mark) 4)))

;; count-lines: GNU counts a trailing partial line
(with-temp-buffer
  (insert "h1a\nh1b\n\fx\n")
  (cl-assert (= (count-lines 1 3) 1))
  (cl-assert (= (count-lines 1 7) 2))
  (cl-assert (= (count-lines 1 8) 2))
  (cl-assert (= (count-lines 2 4) 1))
  (cl-assert (= (count-lines 4 4) 0))
  (cl-assert (= (count-lines 1 5) 1))
  (cl-assert (= (count-lines 5 12) 2)))

;; vertical-motion: batch lands at bol, signed return, (COLS . LINES)
(with-temp-buffer
  (insert "abcdef\nxyzq\n")
  (goto-char 10)
  (cl-assert (equal (list (vertical-motion 1) (point)) '(1 13)))
  (goto-char 3)
  (cl-assert (equal (list (vertical-motion 1) (point)) '(1 8)))
  (goto-char 10)
  (cl-assert (equal (list (vertical-motion -1) (point)) '(-1 1)))
  (cl-assert (= (vertical-motion -5) 0))
  (goto-char 10)
  (cl-assert (equal (list (vertical-motion (cons 2 0)) (point)) '(0 8))))

;; next-line/previous-line keep column via temporary-goal-column
(with-temp-buffer
  (insert "abcdef\nxy\npqr\n")
  (goto-char 3)
  (next-line) (cl-assert (= (point) 10))
  (next-line) (cl-assert (= (point) 13))
  (previous-line) (cl-assert (= (point) 10)))

;; goal-column set/clear via set-goal-column
(with-temp-buffer
  (insert "abcdef\nxy\npqr\n")
  (goto-char 2)
  (set-goal-column nil)
  (cl-assert (= goal-column 1))
  (next-line 1)
  (cl-assert (= (point) 9))
  (set-goal-column t)
  (cl-assert (null goal-column)))

;; temporary-goal-column persists across a line-motion run
(with-temp-buffer
  (insert "abcdef\nxy\npqr\n")
  (goto-char 5)
  (let ((temporary-goal-column 99) (last-command 'next-line))
    (next-line 1)
    (cl-assert (= (point) 10))))

;; track-eol: eol stays eol
(with-temp-buffer
  (insert "abcdef\nxy\npqr\n")
  (goto-char 7)
  (let ((track-eol t) (last-command 'other))
    (next-line 1)
    (cl-assert (= (point) 10))))

;; end-of-buffer: errors without noerror, quiet with
(with-temp-buffer
  (insert "abcdef\nxy\npqr\n")
  (goto-char (point-max))
  (cl-assert (eq (condition-case nil (next-line 1) (error 'end-of-buffer))
                 'end-of-buffer))
  (cl-assert (= (point) 15))
  (cl-assert (not (line-move-1 5 t))))

;; comment-beginning (newcomment.el): inside -> start, outside -> nil
(with-temp-buffer
  (emacs-lisp-mode)
  (insert "a ; comment\nb")
  (goto-char 10)
  (cl-assert (= (comment-beginning) 3))
  (cl-assert (= (point) 5))             ; past comment-start-skip
  (goto-char 13)
  (cl-assert (null (comment-beginning)))
  (goto-char 1)
  (cl-assert (null (comment-beginning))))

(princ "page-ok")
