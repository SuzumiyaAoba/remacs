;;; qa_change_major_mode_hook.el --- Run old-mode cleanup before resetting locals -*- lexical-binding: t; -*-

(defvar qa332-local 1)
(defun qa332-clean (kill-permanent action)
  (with-temp-buffer
    (setq-local qa332-local 7 major-mode 'qa332-mode)
    (let ((seen nil))
      (setq-local change-major-mode-hook
                  (list (lambda ()
                          (push (list qa332-local major-mode) seen)
                          (when (memq action '(write write-error)) (setq qa332-local 9))
                          (when (memq action '(error write-error)) (error "qa332-stop")))))
      (let ((result (condition-case error (kill-all-local-variables kill-permanent)
                      (error error))))
        (list result (nreverse seen) qa332-local
              (local-variable-p 'qa332-local) major-mode)))))

(defun qa332-global (local)
  (let ((seen nil)
        (old-default (default-value 'change-major-mode-hook)))
    (unwind-protect
        (progn
          ;; Explicitly install a global default, regardless of the caller's
          ;; current buffer already having its own local cleanup hook.
          (set-default 'change-major-mode-hook
                       (list (lambda () (push (list 'global qa332-local major-mode) seen))))
          (with-temp-buffer
            (setq-local qa332-local 7 major-mode 'qa332-mode)
            (when local
              (setq-local change-major-mode-hook
                          (list (lambda () (push (list 'local qa332-local major-mode) seen)) t)))
            (kill-all-local-variables)
            (nreverse seen)))
      (set-default 'change-major-mode-hook old-default))))

;; hook normal kill-permanent=nil
(qa (qa332-clean nil 'normal)
    '(nil ((7 qa332-mode)) 1 nil fundamental-mode))

;; hook error kill-permanent=nil
(qa (qa332-clean nil 'error)
    '((error "qa332-stop") ((7 qa332-mode)) 7 t qa332-mode))

;; hook write kill-permanent=nil
(qa (qa332-clean nil 'write)
    '(nil ((7 qa332-mode)) 1 nil fundamental-mode))

;; hook write-error kill-permanent=nil
(qa (qa332-clean nil 'write-error)
    '((error "qa332-stop") ((7 qa332-mode)) 9 t qa332-mode))

;; hook normal kill-permanent=t
(qa (qa332-clean t 'normal)
    '(nil ((7 qa332-mode)) 1 nil fundamental-mode))

;; hook error kill-permanent=t
(qa (qa332-clean t 'error)
    '((error "qa332-stop") ((7 qa332-mode)) 7 t qa332-mode))

;; hook write kill-permanent=t
(qa (qa332-clean t 'write)
    '(nil ((7 qa332-mode)) 1 nil fundamental-mode))

;; hook write-error kill-permanent=t
(qa (qa332-clean t 'write-error)
    '((error "qa332-stop") ((7 qa332-mode)) 9 t qa332-mode))

;; normal hooks include default sentinel
(qa (qa332-global nil)
    '((global 7 qa332-mode)))

;; normal hooks include default sentinel
(qa (qa332-global t)
    '((local 7 qa332-mode) (global 7 qa332-mode)))

;; normal hooks ignore non-nil return values
(qa (with-temp-buffer (let ((seen nil)) (setq-local change-major-mode-hook (list (lambda () (push 'first seen) t) (lambda () (push 'second seen)))) (kill-all-local-variables) (nreverse seen)))
    '(first second))

;; nonlocal hook exit preserves locals
(qa (with-temp-buffer (setq-local qa332-local 7 major-mode 'qa332-mode) (setq-local change-major-mode-hook (list (lambda () (throw 'qa332-exit 'stopped)))) (list (catch 'qa332-exit (kill-all-local-variables)) qa332-local (local-variable-p 'qa332-local) major-mode))
    '(stopped 7 t qa332-mode))

(princ (qa--done))
