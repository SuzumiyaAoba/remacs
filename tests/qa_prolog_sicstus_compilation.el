;;; qa_prolog_sicstus_compilation.el --- SICStus compilation navigation regression -*- lexical-binding:t -*-

(require 'compile)

(defvar prolog-system nil)
(defvar prolog-consult-compile-output "")
(defvar prolog-consult-compile-first-line 0)
(defvar prolog-consult-compile-file nil)
(defvar prolog-consult-compile-real-file nil)
(defvar prolog-consult-compile-error-file nil)
(defvar prolog-process-flag nil)

(defun prolog-prompt-regexp () "$^")

(defun qa-prolog-load-sicstus-error-functions ()
  "Evaluate the SICStus compilation parser helpers from prolog.el."
  (let ((file (expand-file-name
               "../lisp/prolog.el"
               (file-name-directory load-file-name)))
        (needed '(prolog-sicstus-compilation-error-regexp
                  prolog-sicstus-setup-compilation-errors
                  prolog-consult-compile-filter)))
    (unless file
      (error "Can't locate prolog source"))
    (with-temp-buffer
      (insert-file-contents file)
      (goto-char (point-min))
      (while (and needed (< (point) (point-max)))
        (let* ((form (read (current-buffer)))
               (name (and (memq (car-safe form) '(defun defconst))
                          (cadr form))))
          (when (memq name needed)
            (eval form t)
            (setq needed (delq name needed))))))
    (when needed
      (error "Missing Prolog compilation helpers: %S" needed))
    'loaded))

(qa (qa-prolog-load-sicstus-error-functions) 'loaded)

(qa
 (let* ((directory (make-temp-file "remacs-prolog-sicstus-" t))
        (source-file (expand-file-name "source.pl" directory))
        (compilation-buffer (get-buffer-create " *qa-prolog-compilation*"))
        (prolog-system 'sicstus)
        (prolog-consult-compile-output "")
        (prolog-consult-compile-first-line 0)
        (prolog-consult-compile-file source-file)
        (prolog-consult-compile-real-file nil)
        (prolog-consult-compile-error-file source-file)
        (prolog-process-flag t))
   (unwind-protect
       (progn
         (with-temp-file source-file
           (insert "one\ntwo\nthree\nfour\n"))
         (with-current-buffer compilation-buffer
           (setq default-directory directory)
           (compilation-mode)
           (setq buffer-read-only nil)
           (prolog-sicstus-setup-compilation-errors)
           (insert "Compiler output:\n")
           (prolog-consult-compile-filter
            nil "{ERROR: simulated syntax error in line 3}\n")
           (goto-char (point-min))
           (compilation-next-error-function 1)
           (list (file-name-nondirectory (buffer-file-name))
                 (line-number-at-pos))))
     (when (buffer-live-p compilation-buffer)
       (kill-buffer compilation-buffer))
     (when (get-file-buffer source-file)
       (kill-buffer (get-file-buffer source-file)))
     (delete-directory directory t)))
 '("source.pl" 3))

(princ (qa--done))
