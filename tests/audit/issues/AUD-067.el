;;; -*- lexical-binding: t; -*-
(defvar audit-file-handler-log nil)

(defun audit-file-read (path)
  (with-temp-buffer (insert-file-contents-literally path) (buffer-string)))

(defun audit-file-with-directory (fn)
  (let ((dir (make-temp-file "remacs-audit-files-" t)))
    (unwind-protect (funcall fn dir) (delete-directory dir t))))

(defun audit-magic-file-handler (operation &rest args)
  (if (memq operation '(file-exists-p file-readable-p file-writable-p file-modes
                       delete-file rename-file copy-file set-file-modes set-file-times
                       insert-file-contents write-region))
      (progn
        (push operation audit-file-handler-log)
        (cond ((memq operation '(file-exists-p file-readable-p file-writable-p set-file-times)) t)
              ((eq operation 'file-modes) #o640)
              ((eq operation 'insert-file-contents) (insert "virtual") (list (car args) 7))
              (t nil)))
    (let ((inhibit-file-name-handlers (cons 'audit-magic-file-handler inhibit-file-name-handlers))
          (inhibit-file-name-operation operation))
      (apply operation args))))
;; "files:rename-broken-symlink"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(audit-file-with-directory (lambda (dir) (let ((source (expand-file-name "source" dir)) (dest (expand-file-name "dest" dir))) (make-symbolic-link "missing-target" source) (list (condition-case e (rename-file source dest) (error (car e))) (stringp (file-symlink-p source)) (stringp (file-symlink-p dest)))))) t))
           (error (list 'error err))))
  (terpri))

;; "files:rename-over-broken-destination"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(audit-file-with-directory (lambda (dir) (let ((source (expand-file-name "source" dir)) (dest (expand-file-name "dest" dir))) (with-temp-file source (insert "original")) (make-symbolic-link "missing-target" dest) (list (condition-case e (rename-file source dest nil) (error (car e))) (file-exists-p source) (stringp (file-symlink-p dest)))))) t))
           (error (list 'error err))))
  (terpri))

;; "files:copy-over-broken-destination"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(audit-file-with-directory (lambda (dir) (let ((source (expand-file-name "source" dir)) (dest (expand-file-name "dest" dir)) (missing (expand-file-name "missing-target" dir))) (with-temp-file source (insert "original")) (make-symbolic-link missing dest) (list (condition-case e (copy-file source dest nil) (error (car e))) (file-exists-p missing) (stringp (file-symlink-p dest)))))) t))
           (error (list 'error err))))
  (terpri))
