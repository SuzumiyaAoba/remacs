;; Probe: editor/mod.rs uncovered paths — kill-ring, keymaps,
;; subprocesses, key descriptions, file ops, minibuffer helpers.
;; Self-contained: each section guards expected signals.
(defun write-file-string-fn-test (f)
  (with-temp-file f (insert "w")))

(progn
  ;; ---- kill-ring ------------------------------------------------
  (kill-new "one")
  (kill-new "two")
  (prin1 (current-kill 0))
  (prin1 (current-kill 1 t))
  (prin1 (current-kill -1))
  (let ((interprogram-cut-function nil))
    (kill-new "three" t))
  (prin1 (current-kill 0))
  (kill-append "-more" nil)
  (kill-append "-end" t)
  (prin1 (current-kill 0))
  ;; ---- keymap -----------------------------------------------------
  (let ((m1 (make-sparse-keymap))
        (m2 (make-keymap)))
    (define-key m1 "a" 'ignore)
    (set-keymap-parent m2 m1)
    (prin1 (eq (keymap-parent m2) m1))
    (set-keymap-parent m2 nil)
    (prin1 (keymap-parent m2)))
  (let ((m (make-sparse-keymap)))
    (define-key m [f5] 'ignore)
    (define-key m "x" 'ignore)
    (prin1 (lookup-key m "x"))
    (prin1 (lookup-key m [f5])))
  ;; ---- key descriptions ------------------------------------------
  (mapc (lambda (k) (prin1 (single-key-description k)))
        '(13 9 32 27 127 0 1 28 29 30 31 65 97))
  (prin1 (single-key-description 13 t))
  (prin1 (single-key-description (+ 134217728 97)))
  (prin1 (single-key-description (+ 67108864 65)))
  (prin1 (single-key-description 'f10))
  (prin1 (key-description [?\C-x ?\M-a 13]))
  (prin1 (key-description "ab"))
  ;; ---- subprocesses ----------------------------------------------
  (prin1 (call-process "echo" nil nil nil "hi"))
  (let ((b (get-buffer-create " *cpr*")))
    (call-process "echo" nil b nil "out")
    (prin1 (bufferp b)))
  (with-temp-buffer
    (insert "abc\n")
    (call-process-region (point-min) (point-max) "cat" t t nil)
    (prin1 (buffer-string)))
  (prin1 (car-less-than-car '(1 . a) '(2 . b)))
  (condition-case e (car-less-than-car '(2 . a) '(1 . b)) (error (prin1 e)))
  ;; ---- color / geometry -------------------------------------------
  (prin1 (color-defined-p "red"))
  (prin1 (color-defined-p "#ff0000"))
  (prin1 (color-defined-p "no-such-color-xyz"))
  (condition-case e (color-defined-p 5) (error (prin1 e)))
  ;; ---- windows -----------------------------------------------------
  (prin1 (window-margins))
  (prin1 (window-margins (selected-window)))
  (condition-case e (window-margins 'x) (error (prin1 e)))
  (prin1 (windowp (minibuffer-window)))
  (prin1 (framep (window-frame)))
  (prin1 (framep (window-frame (selected-window))))
  ;; ---- directory/files ----------------------------------------------
  (let ((d (file-name-as-directory (file-truename temporary-file-directory))))
    (prin1 (directory-files d nil "^a^a^a^a" t))
    (make-directory (concat d "rd/sub") t)
    (write-file-string-fn-test (concat d "rd/f.txt"))
    (prin1 (file-exists-p (concat d "rd/f.txt")))
    (let ((tmp (make-temp-file "pfx" nil ".suf")))
      (prin1 (stringp tmp))
      (prin1 (file-exists-p tmp))
      (delete-file tmp))
    (let ((tmpd (make-temp-file "dpx" t)))
      (prin1 (file-directory-p tmpd)))
    (delete-directory (concat d "rd") t))
  ;; ---- expand-file-name -------------------------------------------
  (prin1 (expand-file-name "~/x"))
  (prin1 (expand-file-name "$HOME/x"))
  (prin1 (expand-file-name "rel" "/base/"))
  (prin1 (expand-file-name "/abs" "/base/"))
  ;; ---- substitute-command-keys ------------------------------------
  (prin1 (substitute-command-keys "\\[forward-char] moves \\{global-map}X \\<global-map>\\[next-line] Y"))
  (prin1 (substitute-command-keys "plain"))
  ;; ---- key-binding / documentation --------------------------------
  (prin1 (key-binding "a"))
  (prin1 (key-binding "\C-x\C-f"))
  (prin1 (stringp (documentation 'car)))
  (prin1 (documentation 'car t))
  ;; ---- global-set-key / define-prefix-command ----------------------
  (global-set-key [f9] 'ignore)
  (prin1 (lookup-key global-map [f9]))
  (define-prefix-command 'my-pfx)
  (define-key global-map "\C-cp" 'my-pfx)
  (define-key global-map "\C-cpa" 'ignore)
  (prin1 (lookup-key global-map "\C-cpa"))
  ;; ---- save-buffer on a temp file ----------------------------------
  (let ((f (concat (file-name-as-directory (file-truename temporary-file-directory)) "sb.txt")))
    (with-current-buffer (find-file-noselect f)
      (insert "data")
      (save-buffer)
      (prin1 (not (buffer-modified-p)))
      (set-buffer-modified-p nil)
      (kill-buffer (current-buffer)))
    (delete-file f))
  ;; ---- insert-file-contents ----------------------------------------
  (let ((f (concat (file-name-as-directory (file-truename temporary-file-directory)) "ifc.txt")))
    (with-temp-file f (insert "INSIDE"))
    (with-temp-buffer
      (insert "X")
      (insert-file-contents f)
      (prin1 (buffer-string)))
    ;; VISIT=t on a non-empty buffer errors like GNU; REPLACE is arg 5.
    (with-temp-buffer
      (insert "ABC")
      (condition-case e (insert-file-contents f t) (error (prin1 e)))
      (prin1 (buffer-string)))
    (with-temp-buffer
      (insert "ABC")
      (insert-file-contents f nil nil nil t)
      (prin1 (buffer-string)))
    (delete-file f))
  ;; y-or-n-p reads input — blocks in batch; not probeable
  ;; ---- with-timeout ----------------------------------------------
  (prin1 (with-timeout (1 'timed-out) (sleep-for 0.01) 'done))
  (prin1 'done))
