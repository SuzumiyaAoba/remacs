;;; qa_erc_log_unicode.el --- ERC log Unicode persistence regression -*- lexical-binding:t -*-

(defvar erc-log-file-coding-system 'utf-8-unix)
(defvar erc-log-filter-function nil)
(defvar erc-truncate-buffer-on-save nil)
(defvar erc-last-saved-position nil)
(defvar erc-insert-marker nil)
(defvar erc--insert-marker nil)
(defvar qa-erc-log-file nil)

(defun erc-logging-enabled (&optional _buffer) t)
(defun erc-current-logfile (&optional _buffer) qa-erc-log-file)

(defun qa-erc-load-save-source ()
  "Evaluate `erc-save-buffer-in-logs' from erc-log.el source."
  (let ((file (locate-library "erc-log"))
        found)
    (unless file
      (error "Can't locate erc-log source"))
    (with-temp-buffer
      (insert-file-contents file)
      (goto-char (point-min))
      (while (and (not found) (< (point) (point-max)))
        (let ((form (read (current-buffer))))
          (when (and (eq (car-safe form) 'defun)
                     (eq (cadr form) 'erc-save-buffer-in-logs))
            (eval form t)
            (setq found t)))))
    (unless found
      (error "Missing erc-save-buffer-in-logs definition"))
    'loaded))

(qa (default-value 'erc-log-file-coding-system) 'utf-8-unix)
(qa (qa-erc-load-save-source) 'loaded)

(qa
 (let* ((file (make-temp-file "remacs-erc-log-"))
        (qa-erc-log-file file))
   (unwind-protect
       (with-temp-buffer
         (insert "xname 😀 <x@example.test>\n")
         (setq erc-last-saved-position (copy-marker 1)
               erc-insert-marker (copy-marker (point-max))
               erc--insert-marker nil)
         (erc-save-buffer-in-logs)
         (with-temp-buffer
           (insert-file-contents file)
           (buffer-string)))
     (delete-file file)))
 "name 😀 <x@example.test>\n")

(princ (qa--done))
