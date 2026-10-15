;;; -*- lexical-binding: nil -*-
(defun qa62-lisp-tab (text end interactive arg)
  (with-temp-buffer
    (emacs-lisp-mode)
    (insert text)
    (goto-char 1) (forward-line 1)
    (when end (end-of-line))
    (let ((current-prefix-arg arg))
      (if interactive (call-interactively 'indent-for-tab-command)
        (indent-for-tab-command arg)))
    (list (buffer-string) (point) (current-column))))
(qa (qa62-lisp-tab "(defun f ()
x)" nil nil nil) '("(defun f ()
  x)" 15 2))
(qa (qa62-lisp-tab "(defun f ()
x)" nil t nil) '("(defun f ()
  x)" 15 2))
(qa (qa62-lisp-tab "(defun f ()
x)" t nil nil) '("(defun f ()
  x)" 17 4))
(qa (qa62-lisp-tab "(defun f ()
x)" t t nil) '("(defun f ()
  x)" 17 4))
(qa (qa62-lisp-tab "(if c
a" nil nil nil) '("(if c
    a" 11 4))
(qa (qa62-lisp-tab "(if c
a" nil t nil) '("(if c
    a" 11 4))
(qa (qa62-lisp-tab "(if c
a" t nil nil) '("(if c
    a" 12 5))
(qa (qa62-lisp-tab "(if c
a" t t nil) '("(if c
    a" 12 5))
(qa (qa62-lisp-tab "(let ((x 1))
x" nil nil nil) '("(let ((x 1))
  x" 16 2))
(qa (qa62-lisp-tab "(let ((x 1))
x" nil t nil) '("(let ((x 1))
  x" 16 2))
(qa (qa62-lisp-tab "(let ((x 1))
x" t nil nil) '("(let ((x 1))
  x" 17 3))
(qa (qa62-lisp-tab "(let ((x 1))
x" t t nil) '("(let ((x 1))
  x" 17 3))
(qa (qa62-lisp-tab "(dolist (e l)
x" nil nil nil) '("(dolist (e l)
  x" 17 2))
(qa (qa62-lisp-tab "(dolist (e l)
x" nil t nil) '("(dolist (e l)
  x" 17 2))
(qa (qa62-lisp-tab "(dolist (e l)
x" t nil nil) '("(dolist (e l)
  x" 18 3))
(qa (qa62-lisp-tab "(dolist (e l)
x" t t nil) '("(dolist (e l)
  x" 18 3))
(qa (qa62-lisp-tab "(cond
(a" nil nil nil) '("(cond
 (a" 8 1))
(qa (qa62-lisp-tab "(cond
(a" nil t nil) '("(cond
 (a" 8 1))
(qa (qa62-lisp-tab "(cond
(a" t nil nil) '("(cond
 (a" 10 3))
(qa (qa62-lisp-tab "(cond
(a" t t nil) '("(cond
 (a" 10 3))
(qa (qa62-lisp-tab "(save-excursion
x" nil nil nil) '("(save-excursion
  x" 19 2))
(qa (qa62-lisp-tab "(save-excursion
x" nil t nil) '("(save-excursion
  x" 19 2))
(qa (qa62-lisp-tab "(save-excursion
x" t nil nil) '("(save-excursion
  x" 20 3))
(qa (qa62-lisp-tab "(save-excursion
x" t t nil) '("(save-excursion
  x" 20 3))
(qa (commandp 'indent-for-tab-command) t)
(qa (eval (car (get 'lisp-body-indent 'standard-value))) 2)
(qa (get 'lisp-body-indent 'custom-type) 'integer)
(qa (get 'lisp-body-indent 'safe-local-variable) 'integerp)
(defvar qa62-calls 0)
(with-temp-buffer
  (insert "x") (goto-char 1)
  (let ((indent-line-function (lambda () (setq qa62-calls (1+ qa62-calls)) (indent-line-to 3)))
        (tab-always-indent t) (qa62-calls 0))
    (indent-for-tab-command)
    (qa qa62-calls 1)
    (qa (buffer-string) "   x")
    (qa (current-column) 3)))
;; Explicit tab-insertion routes respect indent-tabs-mode and prefix count.
(dolist (tabs '(nil t))
  (with-temp-buffer
    (insert "x")
    (let ((indent-line-function 'indent-to-left-margin)
          (indent-tabs-mode tabs) (tab-width 4))
      (indent-for-tab-command 2)
      (qa (buffer-string) (if tabs "x\t\t" "x       "))
      (qa (current-column) 8))))
(with-temp-buffer
  (insert "x")
  (let ((indent-line-function (lambda () (error "should not indent")))
        (tab-always-indent nil) (indent-tabs-mode t))
    (indent-for-tab-command)
    (qa (buffer-string) "x\t")))
;; Completion occurs only after indentation makes no text/point change.
(defvar qa62-completions 0)
(let ((old-complete (symbol-function 'completion-at-point)))
  (unwind-protect
      (progn
        (fset 'completion-at-point (lambda () (setq qa62-completions (1+ qa62-completions))))
        (with-temp-buffer
          (insert "abc")
          (let ((indent-line-function #'ignore) (tab-always-indent 'complete)
                (tab-first-completion nil) (qa62-completions 0)
                (this-command 'indent-for-tab-command) (last-command 'other))
            (indent-for-tab-command)
            (qa qa62-completions 1)
            (qa (buffer-string) "abc")))
        (with-temp-buffer
          (insert "abc") (goto-char 1)
          (let ((indent-line-function (lambda () (indent-line-to 2)))
                (tab-always-indent 'complete) (tab-first-completion nil)
                (qa62-completions 0))
            (indent-for-tab-command)
            (qa qa62-completions 0)
            (qa (buffer-string) "  abc"))))
    (fset 'completion-at-point old-complete)))
;; Region dispatch takes precedence over a prefix and line indentation.
(defvar qa62-region nil)
(with-temp-buffer
  (insert "a\nb") (set-mark 1)
  (let ((transient-mark-mode t) (mark-active t)
        (indent-region-function (lambda (start end) (setq qa62-region (list start end))))
        (indent-line-function (lambda () (error "unexpected line indentation")))
        (qa62-region nil))
    (indent-for-tab-command '(4))
    (qa qa62-region '(1 4))))
;; The line function sees the widened buffer; caller narrowing is restored.
(with-temp-buffer
  (insert "a\nb\nc") (narrow-to-region 3 4) (goto-char 3)
  (let ((indent-line-function (lambda () (setq qa62-region (list (point-min) (point-max)))))
        (qa62-region nil) (tab-always-indent t))
    (indent-for-tab-command)
    (qa qa62-region '(1 6))
    (qa (list (point-min) (point-max)) '(3 4))))
(with-temp-buffer
  (let ((indent-line-function (lambda () (error "qa62-indent-error")))
        (tab-always-indent t))
    (qa (condition-case e (indent-for-tab-command) (error e)) '(error "qa62-indent-error"))))

(qa (qa62-lisp-tab "(defun f ()
(foo
bar))" nil nil '(4)) '("(defun f ()
  (foo
  bar))" 15 2))
(qa (qa62-lisp-tab "(let ((x 1))
(foo
bar))" nil nil '(4)) '("(let ((x 1))
  (foo
  bar))" 16 2))
(defun qa62-complete-case (style pos repeat &optional text)
  (let ((old-complete (symbol-function 'completion-at-point)))
    (unwind-protect
        (progn
          (fset 'completion-at-point (lambda () (setq qa62-completions (1+ qa62-completions))))
          (with-temp-buffer
            (insert (or text "abc")) (goto-char pos)
            (let ((indent-line-function #'ignore) (tab-always-indent 'complete)
                  (tab-first-completion style) (qa62-completions 0)
                  (this-command 'indent-for-tab-command)
                  (last-command (if repeat 'indent-for-tab-command 'other)))
              (indent-for-tab-command)
              qa62-completions)))
      (fset 'completion-at-point old-complete))))
(qa (qa62-complete-case 'nil 1 nil) 1)
(qa (qa62-complete-case 'nil 1 t) 1)
(qa (qa62-complete-case 'nil 4 nil) 1)
(qa (qa62-complete-case 'nil 4 t) 1)
(qa (qa62-complete-case 'eol 1 nil) 0)
(qa (qa62-complete-case 'eol 1 t) 1)
(qa (qa62-complete-case 'eol 4 nil) 1)
(qa (qa62-complete-case 'eol 4 t) 1)
(qa (qa62-complete-case 'word 1 nil) 0)
(qa (qa62-complete-case 'word 1 t) 1)
(qa (qa62-complete-case 'word 4 nil) 1)
(qa (qa62-complete-case 'word 4 t) 1)
(qa (qa62-complete-case 'word-or-paren 1 nil) 0)
(qa (qa62-complete-case 'word-or-paren 1 t) 1)
(qa (qa62-complete-case 'word-or-paren 4 nil) 1)
(qa (qa62-complete-case 'word-or-paren 4 t) 1)
(qa (qa62-complete-case 'word-or-paren-or-punct 1 nil) 0)
(qa (qa62-complete-case 'word-or-paren-or-punct 1 t) 1)
(qa (qa62-complete-case 'word-or-paren-or-punct 4 nil) 1)
(qa (qa62-complete-case 'word-or-paren-or-punct 4 t) 1)
;; A mode can decline indentation, allowing the default line function.
(let ((old-default (default-value 'indent-line-function)))
  (unwind-protect
      (progn
        (set-default 'indent-line-function (lambda () (indent-line-to 4)))
        (with-temp-buffer
          (insert "x") (goto-char 1)
          (setq-local indent-line-function (lambda () 'noindent))
          (let ((tab-always-indent t))
            (indent-for-tab-command)
            (qa (buffer-string) "    x")
            (qa (current-column) 4))))
    (set-default 'indent-line-function old-default)))
;; If the mode declines inside a multiline comment, align with its text.
(with-temp-buffer
  (set-syntax-table (copy-syntax-table))
  (modify-syntax-entry ?/ ". 14")
  (modify-syntax-entry ?* ". 23")
  (insert "/* comment\n x\n */")
  (goto-char 1) (forward-line 1)
  (let ((indent-line-function (lambda () 'noindent))
        (tab-always-indent t) (comment-start-skip "/\\*+[ \t]*")
        (this-command 'indent-for-tab-command) (last-command 'other))
    (indent-for-tab-command)
    (qa (buffer-string) "/* comment\n   x\n */")
    (qa (current-column) 3)))
;; A point-only movement suppresses completion too.
(let ((old-complete (symbol-function 'completion-at-point)))
  (unwind-protect
      (progn
        (fset 'completion-at-point (lambda () (error "unexpected completion")))
        (with-temp-buffer
          (insert "abc") (goto-char 1)
          (let ((indent-line-function (lambda () (forward-char)))
                (tab-always-indent 'complete))
            (indent-for-tab-command)
            (qa (point) 2))))
    (fset 'completion-at-point old-complete)))
(with-temp-buffer
  (let ((indent-line-function (lambda () (error "unexpected repeated indent")))
        (tab-always-indent nil) (indent-tabs-mode t)
        (this-command 'indent-for-tab-command) (last-command 'indent-for-tab-command))
    (indent-for-tab-command)
    (qa (buffer-string) "\t")))
(qa (qa62-complete-case 'word 1 nil "(") 1)
(qa (qa62-complete-case 'word-or-paren 1 nil "(") 0)
(qa (qa62-complete-case 'word-or-paren-or-punct 1 nil "(") 0)
(qa (qa62-complete-case 'word-or-paren 1 nil ".") 1)
(qa (qa62-complete-case 'word-or-paren-or-punct 1 nil ".") 0)
(qa (qa62-complete-case 'word-or-paren-or-punct 1 t ".") 1)
(princ (qa--done)) (terpri)
