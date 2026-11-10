;;; qa_ecomplete_unicode_save.el --- Unicode ecomplete database regression -*- lexical-binding:t -*-

(require 'ecomplete)

(defconst qa-ecomplete-unicode-database
  '((mail ("key" 1 0 "name 😀 <x@example.test>"))))

(qa (default-value 'ecomplete-database-file-coding-system)
    'utf-8-emacs-unix)

(qa
 (let ((file (make-temp-file "remacs-ecomplete-")))
   (unwind-protect
       (progn
         (let ((ecomplete-database-file file)
               (ecomplete-database-file-coding-system
                (default-value 'ecomplete-database-file-coding-system))
               (ecomplete-database qa-ecomplete-unicode-database))
           (ecomplete-save))
         (let ((ecomplete-database-file file)
               (ecomplete-database-file-coding-system
                (default-value 'ecomplete-database-file-coding-system))
               (ecomplete-database nil))
           (ecomplete-setup)
           (equal ecomplete-database qa-ecomplete-unicode-database)))
     (delete-file file)))
 t)

(princ (qa--done))
