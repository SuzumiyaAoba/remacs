;; Probe: misc.rs remaining branches — bool-vector ops, file times,
;; events, keymap iteration, coding systems, arity, time conversion.
(progn
  ;; ---- bool-vector binops ----------------------------------------
  (let ((a (bool-vector t nil t nil))
        (b (bool-vector t t nil nil)))
    (prin1 (bool-vector-union a b))
    (prin1 (bool-vector-intersection a b))
    (prin1 (bool-vector-exclusive-or a b))
    (prin1 (bool-vector-subsetp a b))
    (prin1 (bool-vector-subsetp b a))
    (prin1 (bool-vector-not a))
    (prin1 (bool-vector-not a t))
    (condition-case e (bool-vector-union a "s") (error (prin1 (car e)))))
  ;; ---- make-temp-file-internal ------------------------------------
  (let ((f (make-temp-file-internal "mti" nil ".x" nil)))
    (prin1 (stringp f))
    (prin1 (file-exists-p f))
    (delete-file f))
  (let ((d (make-temp-file-internal "mtd" t "" nil)))
    (prin1 (file-directory-p d))
    (delete-directory d))
  ;; ---- set-file-times ----------------------------------------------
  (let ((f (concat (file-name-as-directory (file-truename temporary-file-directory)) "sft.txt")))
    (with-temp-file f (insert "x"))
    (set-file-times f)
    (set-file-times f '(100 200))
    (condition-case e (set-file-times "/nonexistent-xyz") (error (prin1 (car e))))
    (delete-file f))
  ;; ---- uptime / times ------------------------------------------------
  (prin1 (listp (emacs-uptime)))
  (prin1 (emacs-uptime "%S"))
  (prin1 (listp (time-convert (current-time) 'list)))
  (prin1 (time-convert '(1 2) 'integer))
  (prin1 (float-time '(1000000000 0)))
  (prin1 (listp (encode-time 0 0 0 15 6 2024)))
  (prin1 (listp (encode-time (list 0 0 0 15 6 2024))))
  ;; ---- help-function-arglist ---------------------------------------------
  (prin1 (help-function-arglist 'car))
  (prin1 (help-function-arglist 'when t))
  (prin1 (help-function-arglist (lambda (a &optional b) a)))
  ;; ---- events -----------------------------------------------------------
  (prin1 (event-modifiers ?\C-a))
  (prin1 (event-modifiers 'C-down))
  (prin1 (event-modifiers 'M-S-f5))
  (prin1 (event-basic-type ?\C-a))
  (prin1 (event-basic-type 'M-S-f5))
  (prin1 (event-basic-type 65))
  ;; ---- window normalize -------------------------------------------------
  (prin1 (windowp (window-normalize-window (selected-window))))
  (prin1 (windowp (window-normalize-window nil)))
  (condition-case e (window-normalize-window 'x) (error (prin1 (car e))))
  (prin1 (framep (window-normalize-frame nil)))
  (prin1 (bufferp (window-normalize-buffer nil)))
  (prin1 (bufferp (window-normalize-buffer "*scratch*")))
  ;; ---- func-arity ----------------------------------------------------------
  (prin1 (func-arity 'car))
  (prin1 (func-arity 'when))
  (prin1 (func-arity 'quote))
  (prin1 (func-arity (lambda (a b &optional c &rest d) a)))
  (condition-case e (func-arity 5) (error (prin1 (car e))))
  ;; ---- map-keymap -------------------------------------------------------------
  (let ((m (make-sparse-keymap)) (seen nil))
    (define-key m "a" 'ignore)
    (define-key m "b" 'self-insert-command)
    (map-keymap (lambda (k v) (push (cons k v) seen)) m)
    (prin1 (length seen)))
  ;; ---- format-message -------------------------------------------------------
  (prin1 (format-message "x `%s' y" 'q))
  (prin1 (format-message "%s" 5))
  ;; ---- coding-system ------------------------------------------------------------
  (prin1 (coding-system-eol-type 'utf-8-unix))
  (prin1 (coding-system-eol-type 'utf-8-dos))
  (prin1 (coding-system-eol-type 'utf-8-mac))
  (prin1 (coding-system-eol-type 'undecided))
  (prin1 (listp (coding-system-plist 'utf-8)))
  ;; ---- pos-visible-in-window-p ---------------------------------------------------
  (with-temp-buffer
    (insert "a\nb\nc")
    (prin1 (pos-visible-in-window-p (point-min)))
    (prin1 (pos-visible-in-window-p (point-min) (selected-window))))
  ;; ---- value< ------------------------------------------------------------------
  (prin1 (value< 1 2))
  (prin1 (value< "a" "b"))
  (prin1 (value< 'a 'b))
  (condition-case e (value< 1 "a") (error (prin1 (car e))))
  ;; ---- locale-info -----------------------------------------------------------------
  (mapc (lambda (it) (prin1 (locale-info it)))
        '(codeset days months paper))
  (prin1 (locale-info 'bogus-item))
  ;; ---- make-interpreted-closure -------------------------------------------------------
  (let ((c (make-interpreted-closure '(x) '((+ x 1)) nil)))
    (prin1 (funcall c 5)))
  (let ((c (make-interpreted-closure '(x) '(x) '((x . 7)))))
    (prin1 (funcall c 5)))
  ;; ---- length on types ----------------------------------------------------------------
  (prin1 (length "abc"))
  (prin1 (length [1 2 3]))
  (prin1 (length '(1 2)))
  (prin1 (length (bool-vector t nil)))
  (condition-case e (length 5) (error (prin1 (car e))))
  (prin1 'done))
