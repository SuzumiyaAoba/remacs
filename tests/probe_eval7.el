;; Probe: file/process misc branches, interactive batch fallbacks.
(progn
  ;; ---- expand-file-name env vars --------------------------------------
  (prin1 (expand-file-name "$HOME/rtk"))
  (prin1 (expand-file-name "${HOME}/rtk"))
  (prin1 (expand-file-name "$REMACS_UNDEF_XYZ/a"))
  (prin1 (expand-file-name "x" "$HOME"))
  ;; ---- default-directory as buffer-local --------------------------------
  (with-temp-buffer
    (setq-local default-directory "/usr/")
    (prin1 (expand-file-name "z")))
  ;; ---- file predicates ---------------------------------------------------
  (prin1 (file-accessible-directory-p "/"))
  (prin1 (file-accessible-directory-p "/no-such-dir-xyz"))
  (prin1 (file-accessible-directory-p "/etc/passwd"))
  (prin1 (file-writable-p "/"))
  (prin1 (file-readable-p "/etc/passwd"))
  (prin1 (file-executable-p "/bin/sh"))
  (prin1 (file-regular-p "/etc/passwd"))
  (prin1 (file-regular-p "/"))
  (prin1 (file-symlink-p "/etc/passwd"))
  ;; ---- write-region variants ----------------------------------------------
  (let ((f (expand-file-name "rtk-wr" temporary-file-directory)))
    (write-region "abc" nil f nil 'quiet)
    (write-region "def" nil f 'append 'quiet)
    (prin1 (with-temp-buffer (insert-file-contents f) (buffer-string)))
    (condition-case e
        (write-region "x" nil f nil nil 'excl)
      (error (prin1 (car e))))
    (write-region nil nil f nil 'quiet)
    (delete-file f))
  ;; ---- save-buffer on a file-visiting buffer -------------------------------
  (let ((f (expand-file-name "rtk-sb" temporary-file-directory)))
    (with-current-buffer (find-file-noselect f)
      (insert "saved")
      (save-buffer)
      (prin1 (buffer-modified-p)))
    (with-temp-buffer (insert-file-contents f) (prin1 (buffer-string)))
    (delete-file f))
  ;; ---- call-process destinations --------------------------------------------
  (with-temp-buffer
    (prin1 (call-process "echo" nil t nil "hi"))
    (prin1 (buffer-string)))
  (with-temp-buffer
    (prin1 (call-process "echo" nil (list t nil) nil "hi"))
    (prin1 (buffer-string)))
  (let ((f (expand-file-name "rtk-cp" temporary-file-directory)))
    (prin1 (call-process "echo" nil (list (list :file f) nil) nil "hi"))
    (prin1 (with-temp-buffer (insert-file-contents f) (buffer-string)))
    (delete-file f))
  (prin1 (call-process "echo" nil nil nil "a" "b" "c"))
  (condition-case e (call-process "no-such-cmd-xyz" nil nil nil)
    (error (prin1 (car e))))
  ;; ---- call-process-region ---------------------------------------------------
  (with-temp-buffer
    (insert "abc")
    (prin1 (call-process-region 1 4 "cat" nil nil nil))
    (prin1 (buffer-string)))
  ;; ---- minibuffer fns ----------------------------------------------------------
  (prin1 (minibuffer-window))
  (prin1 (minibuffer-prompt-end))
  (prin1 (minibuffer-contents))
  (prin1 (active-minibuffer-window))
  ;; ---- y-or-n-p batch ------------------------------------------------------------
  (prin1 (y-or-n-p "q? "))
  ;; ---- syntax table ----------------------------------------------------------------
  (let ((st (copy-syntax-table)))
    (set-syntax-table st)
    (prin1 (syntax-table-p (syntax-table)))
    (prin1 (char-syntax ?a))
    (modify-syntax-entry ?b "w" st)
    (prin1 (char-syntax ?b)))
  ;; ---- interactive specs batch fallbacks --------------------------------------------
  (prin1 (call-interactively (lambda (n) (interactive "nN: ") n)))
  (prin1 (call-interactively (lambda (k) (interactive "kK: ") k)))
  (prin1 (call-interactively (lambda (x) (interactive "xE: ") x)))
  (prin1 (call-interactively (lambda (c) (interactive "cC: ") c)))
  (prin1 (call-interactively (lambda (e) (interactive "eE: ") e)))
  ;; ---- condition-case multi-symbol handler ------------------------------------------
  (prin1 (condition-case v
             (signal 'arith-error '(x))
           ((arith-error file-error) (list 'caught v))))
  ;; ---- standard-output destinations ---------------------------------------------------
  (let ((standard-output (get-buffer-create " *so-buf*")))
    (princ "to-buf")
    (prin1 (with-current-buffer standard-output (buffer-string))))
  (let ((standard-output (lambda (s) (setq so-seen (concat so-seen s))))
        (so-seen ""))
    (princ "to-fn")
    (prin1 so-seen))
  ;; ---- message / error paths ------------------------------------------------------------
  (message "msg %d" 7)
  (prin1 (condition-case e (error "oops %s" "v") (error (cdr e))))
  (prin1 (condition-case e (signal 'my-err '(d1 d2)) (error e)))
  ;; ---- bool-vector-not with target ------------------------------------------------------
  (let ((t_ (make-bool-vector 3 nil)))
    (prin1 (bool-vector-not (bool-vector t nil t) t_))
    (prin1 t_))
  ;; ---- emacs-uptime day format -------------------------------------------------------------
  (prin1 (stringp (emacs-uptime "%d")))
  ;; ---- window/misc --------------------------------------------------------------------------
  (prin1 (window-minibuffer-p))
  (prin1 (window-minibuffer-p (selected-window)))
  (prin1 (minibuffer-window-active-p (minibuffer-window)))
  ;; ---- get-buffer-window / buf_of error paths -------------------------------------------------
  (prin1 (get-buffer-window (current-buffer)))
  (prin1 (get-buffer-window " *no-such-buf*"))
  (condition-case e (buffer-local-value 'zzz (current-buffer))
    (error (prin1 (car e))))
  ;; ---- copy-to-buffer / append-to-buffer -------------------------------------------------------
  (with-temp-buffer (insert "src")
    (let ((dst (get-buffer-create " *dst*")))
      (copy-to-buffer dst 1 4)
      (prin1 (with-current-buffer dst (buffer-string)))))
  (with-temp-buffer (insert "ap")
    (append-to-buffer " *dst*" 1 3)
    (prin1 (with-current-buffer " *dst*" (buffer-string))))
  ;; ---- syntax-after / forward-comment / sexp moves ---------------------------------------------
  (with-temp-buffer
    (insert "a \"str\" (p)")
    (prin1 (syntax-after 1))
    (goto-char 3)
    (prin1 (forward-sexp 1))
    (prin1 (forward-sexp 1)))
  ;; ---- read-expression-ish paths -----------------------------------------------------------------
  (prin1 (read-from-string ";;c\n42"))
  (prin1 (read-from-string "  x"))
  ;; ---- time / encode-decode ----------------------------------------------------------------------
  (prin1 (decode-time (encode-time 0 0 0 1 1 2000 nil nil nil)))
  (prin1 (float-time '(0 0 0 0)))
  (prin1 (current-time-zone 0))
  ;; ---- function arity ----------------------------------------------------------------------------
  (prin1 (func-arity 'car))
  (prin1 (func-arity 'format))
  (prin1 (func-arity '(lambda (a &optional b) a)))
  ;; ---- map-keymap ----------------------------------------------------------------------------------
  (let ((km (make-sparse-keymap)))
    (define-key km "x" 'ignore)
    (map-keymap (lambda (ev def) (prin1 (list ev def))) km))
  ;; ---- make-temp-file-internal dir + text ----------------------------------------------------------
  (let* ((base (expand-file-name "rtk-mtf" temporary-file-directory))
         (f (make-temp-file-internal base nil ".s" "TXT"))
         (d (make-temp-file-internal base t nil nil)))
    (prin1 (with-temp-buffer (insert-file-contents f) (buffer-string)))
    (prin1 (file-directory-p d))
    (delete-file f) (delete-directory d))
  ;; ---- help-function-arglist -----------------------------------------------------------------------
  (prin1 (help-function-arglist 'car))
  (prin1 (help-function-arglist 'car t))
  (prin1 (help-function-arglist '(lambda (a &optional b) a)))
  ;; ---- event fns ------------------------------------------------------------------------------------
  (prin1 (event-modifiers 'control))
  (prin1 (event-basic-type 'control-a))
  (prin1 (event-convert-list '(control ?a)))
  (prin1 (event-modifiers ?\C-a))
  ;; ---- keymap-describe / substitute-command-keys ------------------------------------------------------
  (let ((km (make-sparse-keymap)))
    (define-key km "g" 'goto-line)
    (prin1 (substitute-command-keys "\\{km}")))
  ;; ---- describe-key ---------------------------------------------------------------------------------
  (ignore-errors (describe-key [6]))
  ;; ---- x-parse-geometry, locale-info -----------------------------------------------------------------
  (prin1 (x-parse-geometry "80x24+5+6"))
  (prin1 (consp (locale-info 'days)))
  ;; ---- system-users / groups --------------------------------------------------------------------------
  (prin1 (consp (system-users)))
  (prin1 (listp (system-groups))))
(progn (prin1 'eval7-done))
