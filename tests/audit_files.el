;;; audit_files.el --- temporary files, symlinks and filename handlers -*- lexical-binding: t; -*-
;; All real mutations stay in an owned temporary directory.
(load (expand-file-name "audit_helpers.el" (file-name-directory load-file-name)) nil t)
(defvar audit-file-handler-log nil)

(defun audit-file-read (path)
  (with-temp-buffer (insert-file-contents-literally path) (buffer-string)))

(defun audit-file-with-directory (fn)
  (let ((dir (make-temp-file "remacs-audit-files-" t)))
    (unwind-protect (funcall fn dir) (delete-directory dir t))))

(dolist (kind '(same-name hardlink))
  (audit-check (format "files:copy-self-%s" kind)
   `(audit-file-with-directory
     (lambda (dir)
       (let* ((source (expand-file-name "source" dir))
              (dest (if (eq ',kind 'same-name) source (expand-file-name "alias" dir))))
         (with-temp-file source (insert "original"))
         (unless (eq ',kind 'same-name) (add-name-to-file source dest))
         (list (condition-case e (copy-file source dest t) (error (car e)))
               (audit-file-read source)))))))

(audit-check "files:rename-broken-symlink"
 '(audit-file-with-directory
   (lambda (dir)
     (let ((source (expand-file-name "source" dir)) (dest (expand-file-name "dest" dir)))
       (make-symbolic-link "missing-target" source)
       (list (condition-case e (rename-file source dest) (error (car e)))
             (stringp (file-symlink-p source)) (stringp (file-symlink-p dest)))))))

(audit-check "files:hardlink-broken-symlink"
 '(audit-file-with-directory
   (lambda (dir)
     (let ((source (expand-file-name "source" dir)) (dest (expand-file-name "dest" dir)))
       (make-symbolic-link "missing-target" source)
       (list (condition-case e (add-name-to-file source dest) (error (car e)))
             (stringp (file-symlink-p source)) (stringp (file-symlink-p dest)))))))

(audit-check "files:rename-over-broken-destination"
 '(audit-file-with-directory
   (lambda (dir)
     (let ((source (expand-file-name "source" dir)) (dest (expand-file-name "dest" dir)))
       (with-temp-file source (insert "original"))
       (make-symbolic-link "missing-target" dest)
       (list (condition-case e (rename-file source dest nil) (error (car e)))
             (file-exists-p source) (stringp (file-symlink-p dest)))))))

(audit-check "files:copy-over-broken-destination"
 '(audit-file-with-directory
   (lambda (dir)
     (let ((source (expand-file-name "source" dir)) (dest (expand-file-name "dest" dir))
           (missing (expand-file-name "missing-target" dir)))
       (with-temp-file source (insert "original"))
       (make-symbolic-link missing dest)
       (list (condition-case e (copy-file source dest nil) (error (car e)))
             (file-exists-p missing) (stringp (file-symlink-p dest)))))))

(audit-check "files:rename-to-directory"
 '(audit-file-with-directory
   (lambda (dir)
     (let ((source (expand-file-name "source" dir)) (dest (expand-file-name "sub" dir)))
       (with-temp-file source (insert "original")) (make-directory dest)
       (list (condition-case e (rename-file source dest) (error (car e)))
             (file-exists-p source) (file-exists-p (expand-file-name "source" dest)))))))

(audit-check "files:copy-new-control"
 '(audit-file-with-directory
   (lambda (dir)
     (let ((source (expand-file-name "source" dir)) (dest (expand-file-name "dest" dir)))
       (with-temp-file source (insert "original"))
       (copy-file source dest)
       (list (audit-file-read source) (audit-file-read dest))))))

(dolist (flag '(bad t 1 "nofollow"))
  (audit-check (format "files:symlink-flag-%S" flag)
   `(audit-file-with-directory
     (lambda (dir)
       (let ((source (expand-file-name "source" dir)))
         (with-temp-file source (insert "original"))
         (list (condition-case e (progn (file-modes source ',flag) 'accepted) (error (car e)))
               (condition-case e (progn (set-file-modes source #o600 ',flag) 'accepted) (error (car e)))
               (condition-case e (progn (set-file-times source 0 ',flag) 'accepted) (error (car e)))))))))

(audit-check "files:chmod-nofollow-control"
 '(audit-file-with-directory
   (lambda (dir)
     (let ((source (expand-file-name "source" dir)) (link (expand-file-name "link" dir)))
       (with-temp-file source (insert "original")) (set-file-modes source #o640)
       (make-symbolic-link source link)
       (set-file-modes link #o600 'nofollow)
       (file-modes source)))))

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

(dolist (entry '((file-exists-p "/audit-magic:/entry")
                 (file-readable-p "/audit-magic:/entry")
                 (file-writable-p "/audit-magic:/entry")
                 (file-modes "/audit-magic:/entry")
                 (delete-file "/audit-magic:/entry")
                 (rename-file "/audit-magic:/source" "/audit-magic:/dest")
                 (copy-file "/audit-magic:/source" "/audit-magic:/dest")
                 (set-file-modes "/audit-magic:/entry" 384)
                 (set-file-times "/audit-magic:/entry" 0)
                 (insert-file-contents "/audit-magic:/entry")
                 (write-region "virtual" nil "/audit-magic:/entry" nil 'quiet)))
  (audit-check (format "files:handler-%s" (car entry))
   `(let ((file-name-handler-alist '(("\\`/audit-magic:" . audit-magic-file-handler)))
          (audit-file-handler-log nil))
      (with-temp-buffer
        (list (condition-case e ,entry (error (car e)))
              (nreverse audit-file-handler-log))))))

(audit-check "files:handler-lookup-control"
 '(let ((file-name-handler-alist '(("\\`/audit-magic:" . audit-magic-file-handler))))
    (eq (find-file-name-handler "/audit-magic:/entry" 'file-exists-p) 'audit-magic-file-handler)))

(princ "AUDIT-DONE\n")
