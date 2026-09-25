;; Probe: residual branches — buffers, scan, arith, seq, fields, events.
(progn
  ;; ---- special forms are not funcallable ------------------------------
  (prin1 (condition-case e (funcall 'or) (error (car e))))
  (prin1 (condition-case e (funcall 'and) (error (car e))))
  (prin1 (condition-case e (apply 'if '(t 1)) (error (car e))))
  (prin1 (condition-case e (funcall 'condition-case) (error (car e))))
  ;; ---- buffer args by name --------------------------------------------
  (with-temp-buffer
    (prin1 (buffer-size (buffer-name (current-buffer))))
    (prin1 (buffer-size)))
  (condition-case e (buffer-size "no-such-buf-xyz") (error (prin1 (car e))))
  ;; ---- move-to-column force ---------------------------------------------
  (with-temp-buffer
    (insert "ab")
    (goto-char 1)
    (prin1 (move-to-column 6 t))
    (prin1 (buffer-string))
    (prin1 (point)))
  ;; ---- scan-lists ascend / scan-error -------------------------------------
  (with-temp-buffer
    (insert "(a (b) c)")
    (prin1 (condition-case v (scan-lists 5 -1 0) (error (car v))))
    (prin1 (condition-case v (scan-lists 6 1 0) (error (car v))))
    (prin1 (scan-lists 5 -1 1))
    (prin1 (scan-lists 5 -1 2))
    (prin1 (condition-case v (scan-lists 5 -1 3) (error (car v))))
    (prin1 (scan-lists 5 1 1))
    (prin1 (scan-lists 5 1 2))
    (prin1 (condition-case v (scan-lists 9 -1 0) (error (car v))))
    (prin1 (scan-lists 9 -1 1))
    (prin1 (condition-case v (scan-lists 9 1 0) (error (car v))))
    (prin1 (scan-lists 1 2 0)))
  (with-temp-buffer
    (insert "(a b")
    (condition-case e (backward-sexp) (error (prin1 (car e)))))
  ;; ---- char-syntax classes -------------------------------------------------
  (prin1 (char-syntax ?\|))
  (prin1 (char-syntax ?\{))
  (prin1 (char-syntax ?\}))
  (prin1 (char-syntax ?\\))
  (prin1 (char-syntax ?\'))
  (with-temp-buffer
    (modify-syntax-entry ?\| "\"")
    (prin1 (char-syntax ?\|)))
  ;; ---- defmacro call (Cons macro value) ---------------------------------------
  (defmacro mac8 (x) `(+ ,x 10))
  (prin1 (mac8 5))
  (prin1 (macroexpand '(mac8 1)))
  ;; ---- standard-output destinations ---------------------------------------------
  (let ((m (with-current-buffer (get-buffer-create " *mk8*") (point-marker))))
    (let ((standard-output m)) (princ "MM"))
    (prin1 (with-current-buffer (marker-buffer m) (buffer-string))))
  ;; Function-valued standard-output: called per character; pp8-seen
  ;; must be special for the global defun to see it under lexical binding.
  (let ((standard-output 'pp8))
    (defvar pp8-seen "")
    (defun pp8 (s) (setq pp8-seen (concat pp8-seen (string s))))
    (princ "vv")
    (setq standard-output t)
    (prin1 pp8-seen))
  ;; ---- bool-vector-not target ------------------------------------------------------
  (let ((dst (make-bool-vector 3 nil)))
    (prin1 (eq (bool-vector-not (bool-vector t nil t) dst) dst))
    (prin1 (aref dst 0)))
  (condition-case e (bool-vector-not (bool-vector t) "x") (error (prin1 (car e))))
  (condition-case e (bool-vector-not (bool-vector t nil) (make-bool-vector 4 nil))
    (error (prin1 (car e))))
  ;; ---- malformed bool-vector record ---------------------------------------------------
  (condition-case e (bool-vector-not (record 'bool-vector)) (error (prin1 (car e))))
  ;; ---- arith single-arg / mixed -------------------------------------------------------------
  (prin1 (/ 4))
  (prin1 (/ 4.0))
  (condition-case e (/ 0) (error (prin1 (car e))))
  (prin1 (< 5))
  (condition-case e (< "x") (error (prin1 (car e))))
  (prin1 (ffloor 7.5))
  (prin1 (fceiling -3.5))
  (prin1 (fround 3.7))
  (prin1 (ftruncate 9.9))
  (prin1 (round 3.5))
  (prin1 (round -3.5))
  (condition-case e (ffloor "x") (error (prin1 (car e))))
  (prin1 (expt 2 0.5))
  (prin1 (expt 2.0 3))
  (prin1 (expt 2 -1))
  (prin1 (abs -3))
  (prin1 (float 7))
  (prin1 (logb 16))
  ;; ---- mapcan nconc + seq-remove + string-to-sequence -----------------------------------------
  (prin1 (mapcan 'list '(1 2 3)))
  (prin1 (seq-remove #'evenp '(1 2 3 4)))
  (prin1 (seq-remove (lambda (x) (> x 2)) [1 2 3 4]))
  ;; GNU 31.1 removed `string-to-sequence' entirely (even after
  ;; (require 'seq)); exercise the hidden dump-time definition.
  (let ((s2s (get 'string-to-sequence 'remacs--dump-fn)))
    (prin1 (funcall s2s "ab" 'list))
    (prin1 (funcall s2s "ab" 'vector))
    (prin1 (funcall s2s "ab" 'string)))
  ;; ---- field functions -------------------------------------------------------------------------
  (with-temp-buffer
    (insert "aabbcc")
    (put-text-property 1 3 'field 'f1)
    (put-text-property 3 7 'field 'f2)
    (prin1 (constrain-to-field 2 4))
    (prin1 (constrain-to-field 5 1))
    (prin1 (field-beginning 4))
    (prin1 (field-end 4))
    (prin1 (field-string 4))
    (prin1 (field-string-no-properties 4)))
  ;; ---- buffer-swap-text by name ------------------------------------------------------------------
  (let ((b2 (get-buffer-create " *swap2*")))
    (with-current-buffer b2 (insert "X"))
    (with-temp-buffer
      (insert "Y")
      (buffer-swap-text " *swap2*")
      (prin1 (buffer-string))
      (prin1 (with-current-buffer b2 (buffer-string)))))
  ;; ---- window-frame misc -------------------------------------------------------------------------
  (prin1 (framep (window-frame)))
  (prin1 (framep (window-frame (minibuffer-window))))
  ;; ---- regexp class escapes ------------------------------------------------------------------------
  (prin1 (string-match "[\n\t\r\f\v]" "x\tx"))
  (prin1 (string-match "[\s\-]" "-"))
  (prin1 (string-match "[]a]" "]"))
  (prin1 (string-match "[^a]b" "zb"))
  (prin1 (string-match "[[:digit:]]" "7"))
  (prin1 (string-match "[[:upper:][:space:]]" " Z"))
  ;; ---- re-search-backward ----------------------------------------------------------------------------
  (with-temp-buffer
    (insert "x1 y2 x3")
    (goto-char (point-max))
    (prin1 (re-search-backward "x[0-9]"))
    (prin1 (match-string 0))
    (goto-char 1)
    (condition-case e (re-search-backward "q") (error (prin1 (car e)))))
  ;; ---- replace-regexp-in-string SUBEXP ----------------------------------------------------------------
  (prin1 (replace-regexp-in-string "\\(a\\)\\(b\\)" "Z\\2" "xabx" nil nil 2))
  (prin1 (replace-regexp-in-string "a" "\\?q" "a"))
  ;; ---- let* shadowing + lexical let restore ------------------------------------------------------------
  (prin1 (let ((x 1)) (let ((x 2) (y x)) (list x y))))
  (condition-case e (let ((x (error "boom"))) x) (error (prin1 (car e))))
  ;; ---- condition-case list handler ---------------------------------------------------------------------
  (prin1 (condition-case v (signal 'arith-error nil)
           ((arith-error file-error) 'matched)
           (error 'other)))
  ;; ---- copy-tree / copy-sequence --------------------------------------------------------------------------
  (prin1 (copy-tree '(1 (2 (3)) 4)))
  (let ((v (copy-sequence [1 2]))) (aset v 0 9) (prin1 v))
  ;; ---- frames --------------------------------------------------------------------------------------------
  (prin1 (frame-list))
  (prin1 (framep (car (frame-list))))
  (prin1 (frame-parameters (selected-frame)))
  (prin1 (selected-frame))
  ;; ---- string-compare (extension) ---------------------------------------------------------------------------
  ;; GNU 31.1 leaves `string-compare' void at -Q (even after
  ;; (require 'subr-x)); exercise the hidden dump-time definition.
  (let ((sc (get 'string-compare 'remacs--dump-fn)))
    (prin1 (funcall sc "a" "b" 1))
    (prin1 (funcall sc "b" "a" 1))
    (prin1 (funcall sc "a" "a" 1)))
  ;; ---- format widths ----------------------------------------------------------------------------------------
  (prin1 (format "%08.3f" 1.5))
  (prin1 (format "%-10s|" "x"))
  (prin1 (format "%+d %+d % d" 3 -4 5))
  (prin1 (format "%x %X %o %#x %#o" 255 255 8 255 8))
  (prin1 (format "%c" ?A))
  (prin1 (format "%%"))
  (condition-case e (format "%*d" "x" 1) (error (prin1 (car e))))
  (prin1 (format "%1$s %1$s" "z"))
  ;; ---- emacs-uptime fmt arg ------------------------------------------------------------------------------------
  (prin1 (stringp (emacs-uptime "%d days")))
  (prin1 (stringp (emacs-uptime)))
  ;; ---- values / multiple values ---------------------------------------------------------------------------------
  ;; GNU 31.1 leaves `values' void at -Q; use the dump-time definition.
  (let ((vf (get 'values 'remacs--dump-fn)))
    (prin1 (funcall vf 1 2 3))
    (prin1 (funcall vf)))
  ;; ---- nthcdr/cdr errors --------------------------------------------------------------------------------------
  (condition-case e (cdr 5) (error (prin1 (car e))))
  (condition-case e (nthcdr 1 5) (error (prin1 (car e))))
  ;; ---- message to *Messages* -----------------------------------------------------------------------------------
  (ignore-errors (get-buffer-create " *Messages*"))
  (message "logged %d" 9)
  ;; ---- events ----------------------------------------------------------------------------------------
  (prin1 (eventp ?a))
  (prin1 (eventp '(mouse-1)))
  (prin1 (key-description [?\C-x ?\M-a]))
  (prin1 (key-description "ab"))
  ;; ---- intern-soft nil / unintern --------------------------------------------------------------------
  (prin1 (intern-soft "nosuch-sym-xyz"))
  (prin1 (unintern "sym-to-unintern" nil))
  (prin1 (unintern "nosuch-sym-xyz2" nil))
  ;; ---- window funcs -------------------------------------------------------------------------------------
  (prin1 (windowp (selected-window)))
  (prin1 (window-live-p (selected-window)))
  (prin1 (window-buffer (selected-window)))
  (prin1 (window-point))
  (prin1 (window-start))
  (prin1 (window-end))
  (prin1 (pos-visible-in-window-p))
  (prin1 (window-body-height))
  (prin1 (window-body-width))
  (prin1 (window-total-height))
  (prin1 (window-total-width))
  (prin1 (window-pixel-height))
  (prin1 (window-pixel-width))
  (prin1 (window-fringes))
  (prin1 (window-scroll-bars))
  (prin1 (window-margins))
  (prin1 (window-edges))
  (prin1 (window-pixel-edges))
  (prin1 (window-absolute-pixel-edges))
  (prin1 (window-inside-edges))
  (prin1 (window-inside-pixel-edges))
  (prin1 (window-inside-absolute-pixel-edges))
  (prin1 (window-text-pixel-size))
  (prin1 (window-line-height))
  (prin1 (window-mode-line-height))
  (prin1 (window-header-line-height))
  (prin1 (window-tab-line-height))
  (prin1 (window-dedicated-p))
  (prin1 (window-parameter (selected-window) 'no-other-window))
  (prin1 (window-parameters))
  (prin1 (window-list))
  (prin1 (window-list-1))
  (prin1 (frame-first-window))
  (prin1 (frame-root-window))
  (prin1 (frame-selected-window))
  (prin1 (next-window))
  (prin1 (previous-window))
  (prin1 (get-largest-window))
  (prin1 (get-lru-window))
  (prin1 (walk-windows #'windowp nil t))
  (prin1 (minibuffer-window-active-p (active-minibuffer-window))))
(progn (prin1 'eval8-done))
