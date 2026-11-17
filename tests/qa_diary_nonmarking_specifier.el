;;; qa_diary_nonmarking_specifier.el --- diary specifier regression -*- lexical-binding: t -*-

(require 'diary-lib)

(qa
 (let ((diary-file (make-temp-file "diary-specifier-"))
       (diary-list-entries-hook nil)
       (diary-nongregorian-listing-hook nil)
       (diary-hook nil)
       (diary-included-files nil))
   (unwind-protect
       (progn
         (with-temp-file diary-file
           (insert "&1/1/2026 nonmarking\n"
                   "1/1/2026 ordinary\n"))
         (mapcar (lambda (entry) (list (cadr entry) (caddr entry)))
                 (diary-list-entries '(1 1 2026) 1 t)))
     (delete-file diary-file)))
 '(("nonmarking" "1/1/2026")
   ("ordinary" "1/1/2026")))

(princ (qa--done))
