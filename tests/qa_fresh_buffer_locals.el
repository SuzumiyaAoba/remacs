;;; -*- lexical-binding: nil -*-
;; Issue #39: C buffer slots have fixed initialization and slot order.
(defvar qa39-slots '(buffer-undo-list buffer-display-time buffer-display-count
 buffer-invisibility-spec buffer-file-truename point-before-scroll
 buffer-auto-save-file-format buffer-file-format enable-multibyte-characters
 mark-active mode-name local-minor-modes major-mode buffer-read-only
 buffer-auto-save-file-name buffer-saved-size buffer-backed-up default-directory buffer-file-name))
(defun qa39-keys () (mapcar (lambda (x) (if (consp x) (car x) x)) (buffer-local-variables)))
(defun qa39-new (fn &optional name)
 (let ((b (generate-new-buffer (or name "qa39-new"))))
  (unwind-protect (with-current-buffer b (funcall fn)) (kill-buffer b))))
(qa (qa39-new (lambda () (qa39-keys))) qa39-slots)
(qa (qa39-new (lambda () (mapcar (lambda (s) (buffer-local-value s (current-buffer)))
 '(buffer-undo-list buffer-display-time buffer-display-count buffer-invisibility-spec
 buffer-file-truename point-before-scroll buffer-auto-save-file-format buffer-file-format
 enable-multibyte-characters mark-active mode-name local-minor-modes major-mode buffer-read-only
 buffer-auto-save-file-name buffer-saved-size buffer-backed-up buffer-file-name))))
 '(nil nil 0 t nil nil t nil t nil "Fundamental" nil fundamental-mode nil nil 0 nil nil))
(qa (with-temp-buffer (assq 'buffer-undo-list (buffer-local-variables))) '(buffer-undo-list . t))
(qa (qa39-new (lambda () (assq 'buffer-undo-list (buffer-local-variables)))) '(buffer-undo-list))
(qa (default-value 'mode-name) nil)
(dolist (slot '(mode-name major-mode local-minor-modes buffer-auto-save-file-format buffer-display-time point-before-scroll))
 (qa (qa39-new (lambda () (list (local-variable-p slot) (local-variable-if-set-p slot)))) '(t t)))
(qa (qa39-new (lambda () (list (local-variable-p 'require-final-newline)
                             (local-variable-if-set-p 'require-final-newline)
                             (assq 'require-final-newline (buffer-local-variables))))) '(nil nil nil))
(let ((old (default-value 'require-final-newline)))
 (unwind-protect
  (qa (qa39-new (lambda () (setq require-final-newline 'qa39-global)
       (list (local-variable-p 'require-final-newline) (default-value 'require-final-newline)))) '(nil qa39-global))
  (set-default 'require-final-newline old)))
;; Fixed slots ignore changed global defaults; mode-name's default remains nil.
(dolist (entry '((mode-name "Fundamental") (major-mode fundamental-mode)
 (local-minor-modes nil) (buffer-auto-save-file-format t) (buffer-display-time nil)
 (point-before-scroll nil) (buffer-display-count 0) (buffer-saved-size 0)
 (buffer-file-format nil) (buffer-read-only nil) (buffer-undo-list nil)))
 (let* ((slot (car entry)) (expected (cadr entry)) (old (default-value slot)))
  (unwind-protect
   (progn (set-default slot 'qa39-changed-default)
    (qa (qa39-new (lambda () (buffer-local-value slot (current-buffer)))) expected))
   (set-default slot old))))
(qa (let ((default-directory "/tmp/")) (qa39-new (lambda () default-directory))) "/tmp/")
;; Reset preserves C slots, except the explicitly reset mode-dependent slots.
(dolist (kill-permanent '(nil t))
 (qa (qa39-new (lambda ()
  (setq mode-name "Other" major-mode 'other-mode buffer-invisibility-spec nil
        local-minor-modes '(qa39-mode) buffer-auto-save-file-format 'qa39-format
        buffer-display-time 'qa39-time point-before-scroll 42 buffer-display-count 8
        buffer-undo-list '(qa39-undo))
  (setq-local qa39-custom 9)
  (kill-all-local-variables kill-permanent)
  (list mode-name major-mode buffer-invisibility-spec local-minor-modes
        buffer-auto-save-file-format buffer-display-time point-before-scroll
        buffer-display-count buffer-undo-list (local-variable-p 'qa39-custom))))
 '("Fundamental" fundamental-mode t (qa39-mode) qa39-format qa39-time 42 8 (qa39-undo) nil)))
(dolist (slot '(mode-name major-mode local-minor-modes buffer-auto-save-file-format buffer-display-time point-before-scroll))
 (qa (qa39-new (lambda () (set slot 'qa39-preserved) (kill-local-variable slot)
       (list (local-variable-p slot) (symbol-value slot)))) '(t qa39-preserved)))
;; Lisp locals keep insertion order, including remove/re-add and void entries.
(qa (qa39-new (lambda ()
 (setq-local qa39-a 1) (setq-local qa39-b 2) (setq-local qa39-c 3)
 (setq qa39-b 4) (kill-local-variable 'qa39-b) (setq-local qa39-b 5)
 (make-local-variable 'qa39-unbound) (makunbound 'qa39-unbound)
 (nthcdr 19 (buffer-local-variables)))) '((qa39-a . 1) (qa39-c . 3) (qa39-b . 5) qa39-unbound))
(qa (qa39-new (lambda ()
 (setq-local qa39-a 1) (setq-local tab-width 3) (setq-local cursor-type 'bar)
 (setq-local fill-column 55) (setq-local qa39-b 2)
 (seq-filter (lambda (x) (memq x '(qa39-a qa39-b tab-width fill-column cursor-type))) (qa39-keys))))
 '(cursor-type fill-column tab-width qa39-a qa39-b))
;; Plain versus cloned indirect buffers share text/undo but not all local state.
(dolist (clone '(nil t))
 (qa (qa39-new (lambda ()
  (setq mode-name "Other" buffer-display-time 'qa39-time buffer-display-count 25
        local-minor-modes '(qa39-mode) point-before-scroll 42 buffer-auto-save-file-format 'qa39-format)
  (let ((b (make-indirect-buffer (current-buffer) (generate-new-buffer-name "qa39-indirect") clone)))
   (unwind-protect (with-current-buffer b
    (list mode-name buffer-display-time buffer-display-count local-minor-modes point-before-scroll buffer-auto-save-file-format))
    (kill-buffer b)))))
  (if clone '("Other" qa39-time 0 nil 42 qa39-format) '("Fundamental" nil 0 nil nil t))))
;; Display metadata changes only on display, including repeated same-buffer display.
(qa (let* ((w (selected-window)) (old (window-buffer w)) (b (generate-new-buffer "qa39-display")))
 (unwind-protect
  (progn
   (with-current-buffer b (qa (list buffer-display-count buffer-display-time) '(0 nil)))
   (set-window-buffer w b)
   (let ((first (buffer-local-value 'buffer-display-time b)))
    (set-window-buffer w b)
    (list (buffer-local-value 'buffer-display-count b)
          (consp first) (consp (buffer-local-value 'buffer-display-time b)))))
  (set-window-buffer w old) (kill-buffer b))) '(2 t t))
(qa (let ((b (get-buffer "*scratch*")))
      (and (> (buffer-local-value 'buffer-display-count b) 0)
           (consp (buffer-local-value 'buffer-display-time b)))) t)
(qa (mapcar (lambda (s) (default-value s)) '(mode-name local-minor-modes buffer-auto-save-file-format buffer-display-time point-before-scroll)) '(nil nil t nil nil))
(qa (qa39-new (lambda ()
 (setq mode-name "Other" buffer-display-time 'qa39-time buffer-display-count 25
       local-minor-modes '(qa39-mode) point-before-scroll 42 buffer-file-name "/tmp/qa39-name")
 (let ((b (clone-indirect-buffer "qa39-clone" nil)))
  (unwind-protect (with-current-buffer b
   (list mode-name buffer-display-time buffer-display-count local-minor-modes point-before-scroll buffer-file-name
         (cdr (assq 'buffer-file-name (buffer-local-variables)))))
   (kill-buffer b))))) '("Other" qa39-time 0 nil 42 nil nil))
(dolist (clone '(nil t))
 (qa (qa39-new (lambda ()
  (set-buffer-multibyte nil)
  (let ((b (make-indirect-buffer (current-buffer) (generate-new-buffer-name "qa39-byte") clone)))
   (unwind-protect (buffer-local-value 'enable-multibyte-characters b) (kill-buffer b))))) nil))
(qa (let* ((w (selected-window)) (old (window-buffer w)) (b (generate-new-buffer "qa39-pair"))
           (current-time-list nil))
 (unwind-protect (progn (set-window-buffer w b)
   (let ((stamp (buffer-local-value 'buffer-display-time b)))
    (and (consp stamp) (integerp (car stamp)) (equal (cdr stamp) 1000000000))))
  (set-window-buffer w old) (kill-buffer b))) t)
(qa (save-window-excursion
 (let ((b (generate-new-buffer "qa39-display-buffer")))
  (unwind-protect (progn (display-buffer b '(display-buffer-same-window))
    (and (> (buffer-local-value 'buffer-display-count b) 0)
         (consp (buffer-local-value 'buffer-display-time b))))
   (kill-buffer b)))) t)
(qa (save-window-excursion
 (let ((b (generate-new-buffer "qa39-split")))
  (unwind-protect (progn (switch-to-buffer b)
    (setq buffer-display-count 0 buffer-display-time nil)
    (split-window-right)
    (list buffer-display-count (consp buffer-display-time)))
   (kill-buffer b)))) '(1 t))
(qa (save-window-excursion
 (let* ((file (make-temp-file "remacs-qa39-file-")) (b nil))
  (unwind-protect (progn (find-file file) (setq b (current-buffer))
    (and (> buffer-display-count 0) (consp buffer-display-time)))
   (when (buffer-live-p b) (kill-buffer b)) (delete-file file)))) t)
(qa (save-window-excursion
 (let ((b (generate-new-buffer "qa39-state")))
  (unwind-protect (progn (switch-to-buffer b)
    (let ((state (window-state-get nil t)))
     (setq buffer-display-time nil) (window-state-put state nil 'safe)
     (consp (buffer-local-value 'buffer-display-time b))))
   (kill-buffer b)))) t)
;; Reusing an already displayed buffer does not call set-window-buffer.
(qa (save-window-excursion
 (let ((b (generate-new-buffer "qa39-reuse")))
  (unwind-protect (progn
   (switch-to-buffer b)
   (with-current-buffer b (setq buffer-display-count 0 buffer-display-time nil))
   (window--display-buffer b (selected-window) 'reuse nil)
   (list (buffer-local-value 'buffer-display-count b) (buffer-local-value 'buffer-display-time b)))
   (kill-buffer b)))) '(0 nil))
(qa (save-window-excursion
 (let ((b (generate-new-buffer "qa39-repeat-display")))
  (unwind-protect (progn
   (display-buffer b '(display-buffer-same-window))
   (let ((first (buffer-local-value 'buffer-display-count b)))
    (display-buffer b '(display-buffer-same-window))
    (equal first (buffer-local-value 'buffer-display-count b))))
   (kill-buffer b)))) t)
;; C-backed slots cannot be made void.
(dolist (slot '(buffer-auto-save-file-format buffer-display-time local-minor-modes point-before-scroll))
 (qa (with-temp-buffer
       (let ((before (symbol-value slot)))
        (list (condition-case e (makunbound slot) (error e))
              (boundp slot) (equal before (symbol-value slot)))))
     (list (list 'error (concat "Built-in variable may not be unbound : " (symbol-name slot))) t t)))
(princ (qa--done)) (terpri)
