;; Indent-command domain — GNU 31.1 pins (verified).

;; == vars ==
(qa tab-width 8)
(qa (boundp 'indent-tabs-mode) t)

;; == indent-line-to: GNU returns the new column ==
(qa (with-temp-buffer
         (insert "x")
         (list (indent-line-to 4) (current-column)))
    '(4 4))
(qa (with-temp-buffer
      (insert "x")
      (indent-line-to 2)
      (buffer-string))
    "  x")
(qa (with-temp-buffer
      (insert "  x")
      (indent-line-to 4)
      (buffer-string))
    "    x")
(qa-err (with-temp-buffer (insert "x") (indent-line-to -2))
          'wrong-type-argument)
(qa (with-temp-buffer
      (insert "   x")
      (indent-line-to 0)
      (buffer-string))
    "x")

;; == indent-to ==
(qa (with-temp-buffer
      (insert "ab")
      (goto-char 3)
      (list (indent-to 5) (buffer-string)))
    '(5 "ab   "))
(qa (with-temp-buffer
      (insert "ab")
      (goto-char 3)
      (indent-to 2 3)
      (buffer-string))
    "ab   ")
(qa (with-temp-buffer
      (let ((indent-tabs-mode nil))
        (insert "x")
        (list (indent-to 10) (current-column))))
    '(10 10))

;; == indent-to-left-margin ==
(qa (with-temp-buffer
      (let ((left-margin 4))
        (insert "x")
        (list (indent-to-left-margin) (buffer-string))))
    '(nil "    x"))

;; == just-one-space ==
(qa (with-temp-buffer
      (insert "a   b")
      (goto-char 3)
      (list (just-one-space) (buffer-string)))
    '(nil "a b"))
(qa (with-temp-buffer
      (insert "a   b")
      (goto-char 3)
      (list (just-one-space 3) (buffer-string)))
    '(nil "a   b"))
(qa (with-temp-buffer
      (insert "a   b")
      (goto-char 3)
      (list (just-one-space 0) (buffer-string)))
    '(nil "ab"))
(qa (with-temp-buffer
      (insert "a\n  b")
      (goto-char 2)
      (list (just-one-space -1) (buffer-string)))
    '(nil "a b"))

;; == delete-horizontal-space ==
(qa (with-temp-buffer
      (insert "a   b")
      (goto-char 3)
      (list (delete-horizontal-space) (buffer-string)))
    '(nil "ab"))
(qa (with-temp-buffer
      (insert "a   b")
      (goto-char 3)
      (list (delete-horizontal-space t) (buffer-string)))
    '(nil "a  b"))

;; == fixup-whitespace ==
(qa (with-temp-buffer
         (insert "a   b")
         (goto-char 3)
         (list (fixup-whitespace) (buffer-string)))
       '(nil "a b"))

;; == cycle-spacing ==
(qa (with-temp-buffer
      (let ((indent-tabs-mode nil))
        (insert "x")
        (goto-char (point-min))
        (cycle-spacing)
        (buffer-string)))
    " x")
;; bare cycle-spacing returns (just-one-space "a b" POINT)
(qa (with-temp-buffer
      (insert "a   b")
      (goto-char 3)
      (list (cycle-spacing) (buffer-string) (point)))
    '(delete-all-space "ab" 1))
(qa (with-temp-buffer
      (insert "a   b")
      (goto-char 3)
      (list (cycle-spacing -1) (buffer-string)))
    '(just-one-space "a b"))

;; == move-to-column ==
(qa (with-temp-buffer
      (insert "x y")
      (goto-char 2)
      (move-to-column 3))
    3)

;; == current-indentation ==
(qa (with-temp-buffer
      (insert "    x")
      (current-indentation))
    4)
(qa (with-temp-buffer
      (insert "x")
      (current-indentation))
    0)
(qa (with-temp-buffer
      (insert "\tx")
      (current-indentation))
    8)

;; == delete-indentation ==
;; GNU: insert a space; remacs joins without one
(qa (with-temp-buffer
      (insert "foo\n  bar")
      (goto-char (point-max))
      (list (delete-indentation) (buffer-string)))
    '(nil "foo bar"))

;; == indent-region in elisp ==
(qa (with-temp-buffer
         (emacs-lisp-mode)
         (insert "(a\nb)")
         (list (indent-region (point-min) (point-max))
               (buffer-string)))
    '(t "(a\n b)"))
(qa (with-temp-buffer
      (emacs-lisp-mode)
      (insert "(a\nb)")
      (indent-region (point-min) (point-max) 3)
      (buffer-string))
    "   (a\n   b)")

;; == indent-according-to-mode ==
(qa (with-temp-buffer
      (insert "x")
      (list (indent-according-to-mode) (buffer-string)))
    '(nil "x"))
(qa (with-temp-buffer
         (insert "  x\ny")
         (goto-char (point-max))
         (list (indent-according-to-mode) (buffer-string)))
    '(2 "  x\n  y"))

;; == indent-relative ==
;; GNU: markerp indent-relative + trailing space
(qa (with-temp-buffer
      (insert "foo bar\nbaz")
      (goto-char (point-max))
      (list (markerp (indent-relative)) (buffer-string)))
    '(t "foo bar\nbaz "))
(qa (with-temp-buffer
      (insert "baz")
      (goto-char (point-max))
      (let ((r (indent-relative)))
        (list (markerp r) (buffer-string))))
    '(nil "baz\t"))
(qa (with-temp-buffer
      (goto-char (point-min))
      (list (indent-relative) (buffer-string)))
    '(8 "\t"))

;; == move-to-tab-stop ==
(qa (with-temp-buffer
      (insert "a")
      (list (move-to-tab-stop) (current-column)))
    '(nil 8))
(qa (with-temp-buffer
      (let ((indent-tabs-mode nil))
        (insert "a")
        (list (move-to-tab-stop) (current-column))))
    '(nil 8))
(qa (with-temp-buffer
      (let ((tab-stop-list '(3 7 12)))
        (insert "a")
        (list (move-to-tab-stop) (current-column))))
    '(nil 3))

;; == indent-line-function / indent-region-function ==
(qa (with-temp-buffer
      (emacs-lisp-mode)
      indent-line-function)
    'lisp-indent-line)
;; GNU: elisp mode sets indent-region-function
(qa (with-temp-buffer
         (emacs-lisp-mode)
         indent-region-function)
       'lisp-indent-region)

;; == indent-for-tab-command ==
(qa (with-temp-buffer
      (insert "x")
      (goto-char (point-max))
      (list (indent-for-tab-command) (buffer-string)))
    '(nil "x\t"))
(qa (with-temp-buffer
      (let ((tab-always-indent nil))
        (insert "x")
        (list (indent-for-tab-command) (buffer-string))))
    '(nil "x\t"))

;; == comment-indent ==
(qa (with-temp-buffer
      (let ((comment-start ";")
            (comment-column 24))
        (insert "x ; c")
        (goto-char (point-max))
        (list (markerp (comment-indent)) (buffer-string))))
    '(t "x\t\t\t; c"))

;; == reindent-then-newline-and-indent ==
(qa (with-temp-buffer
         (insert "   abc\nxy")
         (goto-char (point-max))
         (list (reindent-then-newline-and-indent) (buffer-string)))
    '(3 "   abc\n   xy\n   "))

;; == indent-sexp / indent-pp-sexp ==
(qa (with-temp-buffer
         (emacs-lisp-mode)
         (insert "(foo\nbar\nbaz)")
         (goto-char (point-min))
         (list (markerp (indent-sexp)) (buffer-string)))
       '(t "(foo\n bar\n baz)"))
;; GNU: pp-sexp aligns under car
(qa (with-temp-buffer
         (emacs-lisp-mode)
         (insert "(foo\nbar)")
         (goto-char (point-min))
         (list (markerp (indent-pp-sexp)) (buffer-string)))
       '(t "(foo\n bar)"))
(qa-err (with-temp-buffer
          (emacs-lisp-mode)
          (insert "(foo\nbar")
          (goto-char (point-min))
          (indent-sexp))
        'scan-error)

;; == indent-code-rigidly ==
(qa (with-temp-buffer
      (emacs-lisp-mode)
      (insert "(a\nb)")
      (list (indent-code-rigidly (point-min) (point-max) 2)
            (buffer-string)))
    '(nil "  (a\n  b)"))
(qa (with-temp-buffer
      (emacs-lisp-mode)
      (insert "  (a\n  b)")
      (indent-code-rigidly (point-min) (point-max) -2)
      (buffer-string))
    "(a\nb)")

;; == indent-new-comment-line ==
(qa (with-temp-buffer
      (let ((comment-start "# "))
        (insert "foo")
        (list (indent-new-comment-line) (buffer-string))))
    '(nil "foo\n"))
(qa (with-temp-buffer
      (emacs-lisp-mode)
      (insert "(foo) ;cmt")
      (goto-char 9)
      (list (indent-new-comment-line) (buffer-string)))
    '(nil "(foo) ;c\n      ;mt"))

(princ (qa--done))
