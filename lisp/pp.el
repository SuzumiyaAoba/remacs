;;; pp.el --- pretty printer, port of GNU pp.el (pp-fill path) -*- lexical-binding: t -*-

(defcustom pp-escape-newlines t
  "Value of `print-escape-newlines' used by pp-* functions.")

(defcustom pp-max-width t
  "Max width to use.")

(defcustom pp-use-max-width nil
  "Non-nil means `pp' tries to fit the whole object within `pp-max-width'.")

(defcustom pp-default-function #'pp-fill
  "Function called by `pp' to do the actual pretty-printing.")

;; Stubs for the elisp-mode environment GNU's pp expects in its temp
;; buffers.  Our default syntax table already has Lisp paren syntax.
(defvar emacs-lisp-mode-syntax-table nil)
(unless emacs-lisp-mode-syntax-table
  (setq emacs-lisp-mode-syntax-table (syntax-table)))

(defun lisp-mode-variables (&optional _lisp-mode)
  "Subset: syntax table is already Lisp-compatible."
  nil)

(defun pp--lisp-indent ()
  "Indent current line per Emacs-Lisp conventions (subset).
Continuation lines align under the first argument when one exists on
the same line as the open paren, else at open-column + 1."
  (let ((target
         (save-excursion
           (beginning-of-line)
           (condition-case nil
               (let ((open (scan-lists (point) -1 1)))
                 (goto-char open)
                 (let ((ocol (current-column)))
                   (forward-char 1)
                   (skip-chars-forward " \t")
                   (if (eolp)
                       (1+ ocol)
                     (condition-case nil
                         (progn
                           (forward-sexp 1)
                           (skip-chars-forward " \t")
                           (if (eolp) (1+ ocol) (current-column)))
                       (error (1+ ocol))))))
             (error 0)))))
    (indent-line-to (or target 0))))

(defun pp-to-string (object &optional pp-function)
  "Return a string containing the pretty-printed representation of OBJECT."
  (with-temp-buffer
    (lisp-mode-variables nil)
    (set-syntax-table emacs-lisp-mode-syntax-table)
    (let ((indent-line-function #'pp--lisp-indent))
      (funcall (or pp-function pp-default-function) object)
      ;; Preserve old behavior of (usually) finishing with a newline.
      (unless (bolp) (insert "\n"))
      (buffer-string))))

(defun pp--within-fill-column-p ()
  "Return non-nil if point is within `fill-column'."
  (and (save-excursion
         (re-search-backward
          "^\\|\n" (max (point-min) (- (point) fill-column)) t))
       (<= (current-column) fill-column)))

(defun pp-fill (beg &optional end)
  "Break lines in Lisp code between BEG and END so it fits within `fill-column'."
  (interactive "r")
  (if (null end) (pp--object beg #'pp-fill)
    (goto-char beg)
    (let* ((end (copy-marker end t))
           (avoid-unbreakable
            (lambda ()
              (and (memq (char-before) '(?# ?s ?f))
                   (memq (char-after) '(?\[ ?\())
                   (looking-back "#[sf]?" (- (point) 2))
                   (goto-char (match-beginning 0)))))
           (newline (lambda ()
                      (skip-chars-forward ")]}")
                      (unless (save-excursion (skip-chars-forward " \t") (eolp))
                        (funcall avoid-unbreakable)
                        (insert "\n")
                        (indent-according-to-mode)))))
      (while (progn (forward-comment (point-max))
                    (< (point) end))
        (let ((beg (point))
              (paired (when (looking-at "['`,#]*[[:alpha:]^]*\\([({[\"]\\)")
                        (match-beginning 1))))
          (goto-char (or (scan-sexps (or paired (point)) 1) end))
          (unless
              (and
               (save-excursion (not (search-backward "\n" beg t)))
               (or (pp--within-fill-column-p)
                   (and
                    (save-excursion
                      (goto-char beg)
                      (while
                          (progn
                            (funcall avoid-unbreakable)
                            (let ((pos (point)))
                              (skip-chars-backward " \t({[',.")
                              (while (and (memq (char-after) '(?\. ?\{))
                                          (not (memq (char-before)
                                                     '(nil ?\n ?\) \" ?\]))))
                                (forward-char 1))
                              (not (eql pos (point))))))
                      (if (bolp)
                          (progn (goto-char beg) nil)
                        (setq beg (copy-marker beg t))
                        (if paired (setq paired (copy-marker paired t)))
                        (insert "\n") (indent-according-to-mode)
                        t))
                    (pp--within-fill-column-p))))
            (when (and paired (not (eq ?\" (char-after paired))))
              (save-excursion
                (goto-char beg)
                (when (looking-at "(\\([^][()\" \t\n;']+\\)")
                  (let* ((sym (intern-soft (match-string 1)))
                         (lif (and sym (get sym 'lisp-indent-function))))
                    (if (eq lif 'defun) (setq lif 2))
                    (when (natnump lif)
                      (goto-char (match-end 0))
                      (ignore-error scan-error
                        (forward-sexp lif)
                        (funcall newline))))))
              (save-excursion
                (pp-fill (1+ paired) (1- (point)))))
            (funcall newline)))))))

(defun pp--object (object region-function)
  "Pretty-print OBJECT at point via REGION-FUNCTION."
  (let ((print-escape-newlines pp-escape-newlines)
        (print-quoted t)
        (beg (point)))
    (prin1 object (current-buffer))
    (funcall region-function beg (point))))

(defun pp-buffer ()
  "Prettify the current buffer with printed representation of a Lisp object."
  (interactive)
  (funcall pp-default-function (point-min) (point-max))
  (goto-char (point-max))
  (unless (bolp) (insert "\n"))
  (goto-char (point-min)))

(defun pp (object &optional stream)
  "Output the pretty-printed representation of OBJECT, any Lisp object.
Output stream is STREAM, or value of `standard-output' (which see)."
  (let ((stream (or stream standard-output)))
    (save-current-buffer
      (when (bufferp stream) (set-buffer stream))
      (let ((begin (point))
            (cols (current-column)))
        (princ (pp-to-string object) (or stream standard-output))
        (when (and (> cols 0) (bufferp stream))
          (indent-rigidly begin (point) cols))))))

(defun pp-last-sexp ()
  "Read sexp before point.  Ignore leading comment characters."
  (let ((pt (point)))
    (save-excursion
      (forward-sexp -1)
      (when (looking-at ",@?")
        (goto-char (match-end 0)))
      (read (current-buffer)))))

(defun pp-macroexpand-expression (expression)
  "Macroexpand EXPRESSION and pretty-print its value."
  (interactive
   (list (read--expression "Macroexpand: ")))
  (pp-display-expression (macroexpand-1 expression) "*Pp Macroexpand Output*"
                         pp-use-max-width))

(defun pp-display-expression (expression out-buffer-name &optional _lisp)
  "Prettify and display EXPRESSION in buffer OUT-BUFFER-NAME."
  (with-output-to-temp-buffer out-buffer-name
    (pp expression)))

(defun pp-eval-expression (expression &optional insert-value)
  "Evaluate EXPRESSION and pretty-print its value."
  (interactive
   (list (read--expression "Eval: ") current-prefix-arg))
  (message "Evaluating...")
  (let ((result (eval expression lexical-binding)))
    (values--store-value result)
    (if insert-value
        (insert (pp-to-string result))
      (pp-display-expression result "*Pp Eval Output*" pp-use-max-width))))

(defun pp-eval-last-sexp (arg)
  "Run `pp-eval-expression' on sexp before point."
  (interactive "P")
  (if arg
      (insert (pp-to-string (eval (macroexpand (pp-last-sexp))
                                lexical-binding)))
    (pp-eval-expression (macroexpand (pp-last-sexp)))))

(defun pp-macroexpand-last-sexp (arg)
  "Run `pp-macroexpand-expression' on sexp before point."
  (interactive "P")
  (if arg
      (insert (pp-to-string (macroexpand-1 (pp-last-sexp))))
    (pp-macroexpand-expression (pp-last-sexp))))

(provide 'pp)
