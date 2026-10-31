;;; -*- lexical-binding: t -*-
(require 'wdired)

(defun qa-wdired--with-prepared-field (function &optional ordinary type)
  (let ((original (symbol-function 'wdired-get-filename)))
    (unwind-protect
        (progn
          (fset 'wdired-get-filename (lambda (&rest _args) "qa-file"))
          (with-temp-buffer
            (insert "  " (or type "-") "rw-r--r-- 1 user group 0 Jan 1 00:00 qa-file\n")
            (let ((inhibit-read-only t)
                  (wdired-allow-to-change-permissions (if ordinary t 'advanced)))
              (put-text-property (point-min) (point-max) 'read-only t)
              (goto-char (point-min))
              (wdired--preprocess-perms))
            (funcall function 3)))
      (fset 'wdired-get-filename original))))

(defun qa-wdired--edit-allowed (function)
  (condition-case nil
      (progn (funcall function) t)
    (text-read-only nil)
    (buffer-read-only nil)))

(qa (qa-wdired--with-prepared-field
     (lambda (begin)
       (list (get-text-property begin 'read-only)
             (get-text-property (1+ begin) 'read-only)
             (get-text-property (+ begin 9) 'read-only)
             (get-text-property (+ begin 10) 'read-only)
             (get-text-property begin 'old-perm))))
    '(t nil nil t "-rw-r--r--"))
(qa (qa-wdired--with-prepared-field
     (lambda (begin)
       (goto-char begin)
       (qa-wdired--edit-allowed (lambda () (delete-char 1)))))
    nil)
(qa (qa-wdired--with-prepared-field
     (lambda (begin)
       (goto-char begin)
       (qa-wdired--edit-allowed (lambda () (insert "d")))))
    nil)
(qa (qa-wdired--with-prepared-field
     (lambda (begin)
       (goto-char (1+ begin))
       (qa-wdired--edit-allowed (lambda () (insert "r")))))
    t)
(qa (qa-wdired--with-prepared-field
     (lambda (begin)
       (goto-char (1+ begin))
       (qa-wdired--edit-allowed (lambda () (delete-char 1)))))
    t)
(qa (qa-wdired--with-prepared-field
     (lambda (begin)
       (goto-char (1+ begin))
       (delete-char 1) (insert "r")
       (buffer-substring-no-properties begin (+ begin 10))))
    "-rw-r--r--")
(qa (qa-wdired--with-prepared-field
     (lambda (begin)
       (goto-char (1+ begin))
       (delete-char 1) (insert "r")
       (list (get-text-property begin 'old-perm)
             (get-text-property (1+ begin) 'old-perm))))
    '("-rw-r--r--" nil))
(qa (qa-wdired--with-prepared-field
     (lambda (begin)
       (list (get-text-property begin 'read-only)
             (get-text-property (1+ begin) 'read-only)
             (not (null (get-text-property (1+ begin) 'keymap))))) t)
    '(t t t))
(qa (qa-wdired--with-prepared-field
     (lambda (begin)
       (list (get-text-property begin 'read-only)
             (get-text-property (1+ begin) 'read-only))) nil "d")
    '(t nil))

(princ (qa--done))
