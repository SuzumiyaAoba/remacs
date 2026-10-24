;;; audit_read_stream.el --- reader stream boundaries and errors -*- lexical-binding: t; -*-
(load (expand-file-name "audit_helpers.el" (file-name-directory load-file-name)) nil t)

(audit-check "read:function-stream-one-form"
 '(let ((input (string-to-list "(a) (b)")))
    (let ((stream (lambda (&optional unread)
                    (if unread (push unread input) (pop input)))))
      (let ((first (read stream)))
        (list first input
              (condition-case err (read stream) (error (car err))))))))

(audit-check "read:function-stream-unread"
 '(let ((input (string-to-list "123 456")) (unreads 0))
    (let ((stream (lambda (&optional unread)
                    (if unread (progn (setq unreads (1+ unreads)) (push unread input))
                      (pop input)))))
      (let ((first (read stream)))
        (list first (> unreads 0)
              (condition-case err (read stream) (error (car err))))))))

(audit-check "read:buffer-error-point"
 '(with-temp-buffer
    (insert "(a") (goto-char 1)
    (list (condition-case err (read (current-buffer)) (error (car err))) (point))))

(audit-check "read:marker-error-point"
 '(with-temp-buffer
    (insert "(a")
    (let ((m (copy-marker 1)))
      (list (condition-case err (read m) (error (car err))) (marker-position m)))))

(audit-check "read:buffer-success-control"
 '(with-temp-buffer
    (insert "(a) (b)") (goto-char 1)
    (list (read (current-buffer)) (point) (read (current-buffer)) (point))))

(princ "AUDIT-DONE\n")
