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
;; "files:handler-file-exists-p"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((file-name-handler-alist '(("\\`/audit-magic:" . audit-magic-file-handler))) (audit-file-handler-log nil)) (with-temp-buffer (list (condition-case e (file-exists-p "/audit-magic:/entry") (error (car e))) (nreverse audit-file-handler-log)))) t))
           (error (list 'error err))))
  (terpri))

;; "files:handler-file-readable-p"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((file-name-handler-alist '(("\\`/audit-magic:" . audit-magic-file-handler))) (audit-file-handler-log nil)) (with-temp-buffer (list (condition-case e (file-readable-p "/audit-magic:/entry") (error (car e))) (nreverse audit-file-handler-log)))) t))
           (error (list 'error err))))
  (terpri))

;; "files:handler-file-writable-p"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((file-name-handler-alist '(("\\`/audit-magic:" . audit-magic-file-handler))) (audit-file-handler-log nil)) (with-temp-buffer (list (condition-case e (file-writable-p "/audit-magic:/entry") (error (car e))) (nreverse audit-file-handler-log)))) t))
           (error (list 'error err))))
  (terpri))

;; "files:handler-file-modes"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((file-name-handler-alist '(("\\`/audit-magic:" . audit-magic-file-handler))) (audit-file-handler-log nil)) (with-temp-buffer (list (condition-case e (file-modes "/audit-magic:/entry") (error (car e))) (nreverse audit-file-handler-log)))) t))
           (error (list 'error err))))
  (terpri))

;; "files:handler-copy-file"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((file-name-handler-alist '(("\\`/audit-magic:" . audit-magic-file-handler))) (audit-file-handler-log nil)) (with-temp-buffer (list (condition-case e (copy-file "/audit-magic:/source" "/audit-magic:/dest") (error (car e))) (nreverse audit-file-handler-log)))) t))
           (error (list 'error err))))
  (terpri))

;; "files:handler-rename-file"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((file-name-handler-alist '(("\\`/audit-magic:" . audit-magic-file-handler))) (audit-file-handler-log nil)) (with-temp-buffer (list (condition-case e (rename-file "/audit-magic:/source" "/audit-magic:/dest") (error (car e))) (nreverse audit-file-handler-log)))) t))
           (error (list 'error err))))
  (terpri))

;; "files:handler-delete-file"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((file-name-handler-alist '(("\\`/audit-magic:" . audit-magic-file-handler))) (audit-file-handler-log nil)) (with-temp-buffer (list (condition-case e (delete-file "/audit-magic:/entry") (error (car e))) (nreverse audit-file-handler-log)))) t))
           (error (list 'error err))))
  (terpri))

;; "files:handler-set-file-modes"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((file-name-handler-alist '(("\\`/audit-magic:" . audit-magic-file-handler))) (audit-file-handler-log nil)) (with-temp-buffer (list (condition-case e (set-file-modes "/audit-magic:/entry" 384) (error (car e))) (nreverse audit-file-handler-log)))) t))
           (error (list 'error err))))
  (terpri))

;; "files:handler-set-file-times"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((file-name-handler-alist '(("\\`/audit-magic:" . audit-magic-file-handler))) (audit-file-handler-log nil)) (with-temp-buffer (list (condition-case e (set-file-times "/audit-magic:/entry" 0) (error (car e))) (nreverse audit-file-handler-log)))) t))
           (error (list 'error err))))
  (terpri))

;; "files:handler-insert-file-contents"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((file-name-handler-alist '(("\\`/audit-magic:" . audit-magic-file-handler))) (audit-file-handler-log nil)) (with-temp-buffer (list (condition-case e (insert-file-contents "/audit-magic:/entry") (error (car e))) (nreverse audit-file-handler-log)))) t))
           (error (list 'error err))))
  (terpri))

;; "files:handler-write-region"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((file-name-handler-alist '(("\\`/audit-magic:" . audit-magic-file-handler))) (audit-file-handler-log nil)) (with-temp-buffer (list (condition-case e (write-region "virtual" nil "/audit-magic:/entry" nil 'quiet) (error (car e))) (nreverse audit-file-handler-log)))) t))
           (error (list 'error err))))
  (terpri))
