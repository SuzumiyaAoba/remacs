;;; qa_temp_directory.el --- Honor temporary-file-directory -*- lexical-binding: t; -*-

(require 'subr-x)

(defun qa330-directory (prefix slash directory &optional context)
  (let* ((root (make-temp-file "qa330-root-" t))
         (selected (expand-file-name "selected" root))
         (working (expand-file-name "working" root))
         (file nil))
    (unwind-protect
        (progn
          (make-directory selected)
          (make-directory working)
          (make-directory (expand-file-name "sub" selected))
          (let* ((default-directory (file-name-as-directory working))
                 (temporary-file-directory
                  (cond ((eq context 'relative) "../selected/")
                        ((eq context 'use-default) nil)
                        (slash (file-name-as-directory selected))
                        (t selected)))
                 (absolute (eq prefix 'absolute))
                 (prefix (if absolute (expand-file-name "control-" root) prefix))
                 (expected-directory
                  (cond (absolute root)
                        ((eq context 'use-default) working)
                        ((string-prefix-p "../" prefix) root)
                        ((string-prefix-p "sub/" prefix) (expand-file-name "sub" selected))
                        (t selected))))
            (setq file (make-temp-file prefix directory ".suffix" (unless directory "payload")))
            (list (equal (file-name-directory file) (file-name-as-directory expected-directory))
                  (string-prefix-p (file-name-nondirectory prefix) (file-name-nondirectory file))
                  (string-suffix-p ".suffix" file)
                  (unless directory
                    (with-temp-buffer (insert-file-contents-literally file) (buffer-string))))))
      (when (and file (file-exists-p file))
        (if (file-directory-p file) (delete-directory file) (delete-file file)))
      (delete-directory root t))))

;; relative prefix "child-" slash=nil
(qa (qa330-directory "child-" nil nil)
    '(t t t "payload"))

;; relative prefix "" slash=nil
(qa (qa330-directory "" nil nil)
    '(t t t "payload"))

;; relative prefix "." slash=nil
(qa (qa330-directory "." nil nil)
    '(t t t "payload"))

;; relative prefix ".." slash=nil
(qa (qa330-directory ".." nil nil)
    '(t t t "payload"))

;; relative prefix "sub/child-" slash=nil
(qa (qa330-directory "sub/child-" nil nil)
    '(t t t "payload"))

;; relative prefix "../sibling-" slash=nil
(qa (qa330-directory "../sibling-" nil nil)
    '(t t t "payload"))

;; directory respects selected temp dir
(qa (qa330-directory "child-" nil t)
    '(t t t nil))

;; absolute prefix ignores selected temp dir
(qa (qa330-directory 'absolute nil nil)
    '(t t t "payload"))

;; relative prefix "child-" slash=t
(qa (qa330-directory "child-" t nil)
    '(t t t "payload"))

;; relative prefix "" slash=t
(qa (qa330-directory "" t nil)
    '(t t t "payload"))

;; relative prefix "." slash=t
(qa (qa330-directory "." t nil)
    '(t t t "payload"))

;; relative prefix ".." slash=t
(qa (qa330-directory ".." t nil)
    '(t t t "payload"))

;; relative prefix "sub/child-" slash=t
(qa (qa330-directory "sub/child-" t nil)
    '(t t t "payload"))

;; relative prefix "../sibling-" slash=t
(qa (qa330-directory "../sibling-" t nil)
    '(t t t "payload"))

;; directory respects selected temp dir
(qa (qa330-directory "child-" t t)
    '(t t t nil))

;; absolute prefix ignores selected temp dir
(qa (qa330-directory 'absolute t nil)
    '(t t t "payload"))

;; relative temporary-file-directory
(qa (qa330-directory "child-" t nil 'relative)
    '(t t t "payload"))

;; nil temporary-file-directory falls back to default-directory
(qa (qa330-directory "child-" t nil 'use-default)
    '(t t t "payload"))

(princ (qa--done))
