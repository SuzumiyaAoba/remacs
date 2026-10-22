;;; qa_permanent_local_hooks.el --- Filter partially permanent hook lists -*- lexical-binding: t; -*-

(defun qa333-label (value permanent ordinary unbound alias)
  (cond ((eq value permanent) 'permanent)
        ((eq value ordinary) 'ordinary)
        ((eq value unbound) 'unbound-permanent)
        ((eq value alias) 'alias)
        ((eq value t) 'default)
        ((null value) nil)
        ((functionp value) 'closure)
        ((consp value) (cons (qa333-label (car value) permanent ordinary unbound alias)
                             (qa333-label (cdr value) permanent ordinary unbound alias)))
        ((vectorp value) 'vector)
        (t value)))
(defun qa333-filter (property kind &optional kill-permanent)
  (let ((variable (make-symbol "qa333-variable"))
        (permanent (make-symbol "qa333-permanent"))
        (ordinary (make-symbol "qa333-ordinary"))
        (unbound (make-symbol "qa333-unbound"))
        (alias (make-symbol "qa333-alias")))
    (set-default variable nil)
    (fset permanent (lambda () nil))
    (fset ordinary (lambda () nil))
    (fset alias permanent)
    (put permanent 'permanent-local-hook t)
    (put unbound 'permanent-local-hook t)
    (put variable 'permanent-local property)
    (when (eq kind 'alias) (put alias 'permanent-local-hook t))
    (let ((original (cond ((eq kind 'none) (list ordinary (lambda () nil) 42 alias))
                          ((eq kind 'all) (list permanent t unbound))
                          ((eq kind 'dotted) (cons ordinary (cons permanent 42)))
                          ((eq kind 'nil) nil)
                          ((eq kind 'alias) (list alias ordinary))
                          (t (list ordinary permanent t (lambda () nil) unbound alias 42)))))
      (with-temp-buffer
        (make-local-variable variable)
        (set variable original)
        (kill-all-local-variables kill-permanent)
        (let ((result (symbol-value variable)))
          (list (local-variable-p variable)
                (qa333-label result permanent ordinary unbound alias)
                (eq result original)))))))

;; partially permanent mixed
(qa (qa333-filter 'permanent-local-hook 'mixed)
    '(t (permanent default unbound-permanent) nil))

;; partially permanent none
(qa (qa333-filter 'permanent-local-hook 'none)
    '(t nil nil))

;; partially permanent all
(qa (qa333-filter 'permanent-local-hook 'all)
    '(t (permanent default unbound-permanent) nil))

;; partially permanent dotted
(qa (qa333-filter 'permanent-local-hook 'dotted)
    '(t (permanent) nil))

;; partially permanent nil
(qa (qa333-filter 'permanent-local-hook 'nil)
    '(t nil t))

;; partially permanent alias
(qa (qa333-filter 'permanent-local-hook 'alias)
    '(t (alias) nil))

;; other permanent-local value t
(qa (qa333-filter t 'mixed)
    '(t (ordinary permanent default closure unbound-permanent alias 42) t))

;; other permanent-local value 'other
(qa (qa333-filter 'other 'mixed)
    '(t (ordinary permanent default closure unbound-permanent alias 42) t))

;; other permanent-local value nil
(qa (qa333-filter nil 'mixed)
    '(nil nil nil))

;; kill-permanent clears even preserved lists
(qa (qa333-filter 'permanent-local-hook 'mixed t)
    '(nil nil nil))

;; kill-permanent clears even preserved lists
(qa (qa333-filter t 'mixed t)
    '(nil nil nil))

;; kill-permanent clears even preserved lists
(qa (qa333-filter 'other 'mixed t)
    '(nil nil nil))

;; real after-save-hook filters functions
(qa (let ((permanent (make-symbol "qa333-keep")) (ordinary (make-symbol "qa333-drop"))) (fset permanent (lambda () nil)) (fset ordinary (lambda () nil)) (put permanent 'permanent-local-hook t) (with-temp-buffer (add-hook 'after-save-hook permanent nil t) (add-hook 'after-save-hook ordinary nil t) (kill-all-local-variables) (list (and (memq permanent after-save-hook) t) (and (memq ordinary after-save-hook) t))))
    '(t nil))

(defun qa333-watcher ()
  (let ((variable (make-symbol "qa333-watched"))
        (permanent (make-symbol "qa333-keep"))
        (ordinary (make-symbol "qa333-drop"))
        (events nil))
    (set-default variable nil)
    (put variable 'permanent-local 'permanent-local-hook)
    (put permanent 'permanent-local-hook t)
    (with-temp-buffer
      (setq-local major-mode 'qa333-mode)
      (make-local-variable variable)
      (set variable (list ordinary permanent))
      (let* ((buffer (current-buffer))
             (watcher (lambda (symbol value operation where)
                        (push (list (eq symbol variable)
                                    (mapcar (lambda (item) (eq item permanent)) value)
                                    operation (eq where buffer)
                                    (local-variable-p variable) major-mode) events))))
        (unwind-protect
            (progn
              (add-variable-watcher variable watcher)
              (kill-all-local-variables)
              (nreverse events))
          (remove-variable-watcher variable watcher))))))

(qa (qa333-watcher) '((t nil makunbound t t fundamental-mode) (t (t) makunbound t t fundamental-mode)))

(princ (qa--done))
