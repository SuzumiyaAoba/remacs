;; Probe: uncovered branch clusters — lexical bind path, undo entries,
;; read-from-string offsets, fields, bool-vectors, replacements, misc.
(progn
  ;; ---- lexical closures with &optional/&rest (bind_lambda_args) -----
  (let ((f (lambda (x &optional y &rest r) (list x y r))))
    (prin1 (funcall f 1))
    (prin1 (funcall f 1 2))
    (prin1 (funcall f 1 2 3 4)))
  (let ((g (lambda (&optional a b) (list a b))))
    (prin1 (funcall g)) (prin1 (funcall g 9)))
  (let ((h (lambda (&rest rs) rs)))
    (prin1 (funcall h)) (prin1 (funcall h 1 2)))
  ;; supplied-p / init pairs are invalid in plain lambdas (GNU too)
  (condition-case e
      (let ((f (lambda (&optional (y 7 s?)) y))) (funcall f))
    (error (prin1 (car e))))
  ;; ---- primitive-undo entry types -----------------------------------
  (with-temp-buffer
    (insert "hello world")
    (let ((l (list 5                     ; point motion
                   '(1 . 4)              ; delete insertion range
                   '("XY" . 1)           ; re-insert deleted text
                   nil)))                ; boundary marker
      (prin1 (primitive-undo 4 l)))
    (prin1 (buffer-string)))
  ;; ---- read-from-string with START/END ------------------------------
  (prin1 (read-from-string "(a b) tail"))
  (prin1 (read-from-string "xx (a b) yy" 3 8))
  (prin1 (read-from-string "abc" 1))
  (condition-case e (read-from-string "") (error (prin1 (car e))))
  ;; ---- syntax-after -------------------------------------------------
  (with-temp-buffer
    (insert "a (b")
    (prin1 (syntax-after 1))
    (prin1 (syntax-after 3)))
  ;; ---- fields --------------------------------------------------------
  (with-temp-buffer
    (insert "abc")
    (put-text-property 1 2 'field 'x)
    (prin1 (field-beginning)) (prin1 (field-end))
    (prin1 (field-string 1))
    (prin1 (constrain-to-field 3 1))
    (prin1 (field-string-no-properties 1)))
  ;; ---- bool-vector ops -----------------------------------------------
  (let ((a (bool-vector t nil t nil))
        (b (bool-vector t t nil nil)))
    (prin1 (bool-vector-not a))
    (prin1 (bool-vector-subsetp a b))
    (prin1 (bool-vector-count-population a))
    (prin1 (bool-vector-intersection a b))
    (prin1 (bool-vector-union a b))
    (prin1 (bool-vector-set-difference a b))
    (prin1 (bool-vector-exclusive-or a b))
    (let ((c (make-bool-vector 4 nil)))
      (prin1 (bool-vector-intersection a b c))
      (prin1 c)))
  ;; ---- make-temp-file-internal all args ------------------------------
  (let* ((base (expand-file-name "rtk-tmp" temporary-file-directory))
         (f (make-temp-file-internal base nil ".sfx" "data"))
         (d (make-temp-file-internal base t "" nil)))
    (prin1 (file-exists-p f)) (prin1 (file-directory-p d))
    (delete-file f) (delete-directory d))
  ;; ---- replace-regexp-in-string arg variants -------------------------
  (prin1 (replace-regexp-in-string "a\\(b\\)c" "[\\1]" "abc abc"))
  (prin1 (replace-regexp-in-string "a+" "X" "aaa" t))        ; fixedcase
  (prin1 (replace-regexp-in-string "a+" "x" "aAa" nil t))   ; literal
  (prin1 (replace-regexp-in-string "a+" "X" "zaaaq" nil nil nil 2))
  (condition-case e
      (replace-regexp-in-string "a\\(b\\)?" "[\\2]" "ab" nil nil 2)
    (error (prin1 (car e))))
  (condition-case e
      (replace-regexp-in-string "a" "b" "x" nil nil nil 9)
    (error (prin1 (car e))))
  ;; ---- regexp replacement expansion (\\N \\& \\\\) --------------------
  (with-temp-buffer
    (insert "foo-123")
    (goto-char (point-min))
    (re-search-forward "o-\\([0-9]+\\)")
    (replace-match "<\\1>&\\&\\\\")
    (prin1 (buffer-string)))
  (with-temp-buffer
    (insert "foo-123")
    (goto-char (point-min))
    (re-search-forward "o-")
    (condition-case e (replace-match "x\\'") (error (prin1 (car e))))
    (condition-case e (replace-match "x\\") (error (prin1 (car e))))
    (condition-case e (replace-match "\\9") (error (prin1 (car e))))
    (prin1 (buffer-string)))
  ;; ---- search-backward variants --------------------------------------
  (with-temp-buffer
    (insert "one two one")
    (prin1 (search-backward "one"))
    (prin1 (search-backward "one" nil t))
    (prin1 (search-backward "zzz" nil t))
    (condition-case e (search-backward "zzz") (error (prin1 (car e))))
    (goto-char (point-max))
    (prin1 (search-backward-regexp "o.e" nil t))
    (prin1 (word-search-backward "two" nil t)))
  ;; ---- forward-sexp more syntax cases --------------------------------
  (with-temp-buffer
    (insert "'(a b) #(v) \"s\"")
    (goto-char (point-min))
    (prin1 (forward-sexp 1))
    (prin1 (forward-sexp 1))
    (prin1 (forward-sexp 1))
    (prin1 (forward-sexp 1)))
  (with-temp-buffer
    (insert "foo \"bar\" baz")
    (goto-char (point-min))
    (prin1 (forward-sexp 2)))
  ;; ---- scan-lists depth cases ----------------------------------------
  (with-temp-buffer
    (insert "(a (b c) d)")
    (prin1 (condition-case v (scan-lists 1 1 1) (error (car v))))
    (prin1 (scan-lists 1 -1 0))
    (prin1 (scan-lists 2 1 0)))
  ;; ---- scan-sexps ----------------------------------------------------
  (with-temp-buffer
    (insert "aa bb")
    (prin1 (scan-sexps 1 1))
    (prin1 (scan-sexps 1 -1)))
  ;; ---- keymap parent ---------------------------------------------------
  (let ((p (make-sparse-keymap)) (c (make-sparse-keymap)))
    (define-key p "a" 'ignore)
    (set-keymap-parent c p)
    (prin1 (keymap-parent c))
    (set-keymap-parent c nil)
    (prin1 (keymap-parent c)))
  ;; ---- color-defined-p -------------------------------------------------
  (prin1 (list (color-defined-p "red") (color-defined-p "#ff0000")
               (color-defined-p "nosuchcolor123")))
  ;; ---- buffer-swap-text -------------------------------------------------
  (let ((b1 (get-buffer-create " swp-a")) (b2 (get-buffer-create " swp-b")))
    (with-current-buffer b1 (insert "AAA"))
    (with-current-buffer b2 (insert "BBB"))
    (with-current-buffer b1 (buffer-swap-text b2))
    (prin1 (with-current-buffer b1 (buffer-string)))
    (prin1 (with-current-buffer b2 (buffer-string))))
  ;; ---- file name expansion / save --------------------------------------
  (prin1 (expand-file-name "~/x"))
  (prin1 (expand-file-name "x" "/tmp"))
  (prin1 (expand-file-name "./y" "/tmp"))
  (prin1 (expand-file-name "../z" "/tmp/q"))
  (with-temp-buffer
    (insert "s")
    (let ((f (expand-file-name "rtk-save" temporary-file-directory)))
      (write-region nil nil f nil 'quiet)
      (prin1 (file-exists-p f))
      (delete-file f)))
  ;; ---- substitute-command-keys \\{...} spec -----------------------------
  (prin1 (substitute-command-keys "press \\[forward-char]"))
  (prin1 (substitute-command-keys "key \\<global-map>\\[forward-char]"))
  ;; ---- describe-key -----------------------------------------------------
  (ignore-errors (describe-key "a"))
  ;; ---- emacs-uptime / misc -----------------------------------------------
  (prin1 (stringp (emacs-uptime)))
  (prin1 (stringp (emacs-uptime "%S")))
  ;; ---- call-process variants ----------------------------------------------
  (prin1 (call-process "echo" nil nil nil "hi"))
  (with-temp-buffer
    (prin1 (call-process "echo" nil t nil "hi"))
    (prin1 (buffer-string)))
  (with-temp-buffer
    (insert "xyz")
    (prin1 (call-process-region 1 4 "cat" t t nil))
    (prin1 (buffer-string)))
  ;; ---- thing-at-point -----------------------------------------------------
  (with-temp-buffer
    (insert "word (list) sym")
    (goto-char 2)
    (prin1 (thing-at-point 'word))
    (goto-char 6)
    (prin1 (thing-at-point 'list)))
  ;; ---- forward-comment ---------------------------------------------------
  (with-temp-buffer
    (insert "ab ;c\nxy")
    (goto-char 1)
    (prin1 (forward-comment 1)))
  ;; ---- move-to-column ------------------------------------------------------
  (with-temp-buffer
    (insert "a\tb\nxyz")
    (goto-char (point-min))
    (prin1 (move-to-column 2))
    (goto-char (point-min))
    (prin1 (move-to-column 9 t)))
  ;; ---- insert-before-markers ----------------------------------------------
  (with-temp-buffer
    (let ((m (point-marker)))
      (insert-before-markers "Q")
      (prin1 (marker-position m))))
  ;; ---- window-margins -------------------------------------------------------
  (prin1 (window-margins))
  ;; ---- wrong-number-of-args shapes ------------------------------------------
  (condition-case e (car 1 2) (error (prin1 e)))
  (condition-case e (goto-char) (error (prin1 e)))
  ;; ---- message output -------------------------------------------------------
  (message "hello %s" "world")
  ;; ---- interactive spec remaining codes -------------------------------------
  (let ((lexical-binding nil))
    (defun dyn-f (x &optional y &rest r) (list x y r))
    (prin1 (dyn-f 1)) (prin1 (dyn-f 1 2 3))))
(progn (prin1 'eval5-done))
