;;; qa_bookmark_load_overwrite_current_name.el --- clear removed current bookmark names -*- lexical-binding: t; -*-

(require 'bookmark)

(qa
 (let* ((directory (make-temp-file "bookmark-current-" t))
        (file (expand-file-name "kept.eld" directory))
        (old-buffer (get-buffer-create " *bookmark-current-old*"))
        (kept-buffer (get-buffer-create " *bookmark-current-kept*"))
        (old-file (expand-file-name "old.txt" directory))
        (kept-file (expand-file-name "kept.txt" directory))
        (bookmark-alist nil)
        (bookmark-version-control 'never)
        (bookmark-file-coding-system nil))
   (unwind-protect
       (progn
         (let ((bookmark-alist
                (list (cons "kept"
                            (list (cons 'filename kept-file)
                                  (cons 'position 1))))))
           (bookmark-write-file file))
         (with-current-buffer old-buffer
           (setq-local bookmark-current-bookmark "old"))
         (with-current-buffer kept-buffer
           (setq-local bookmark-current-bookmark "kept"))
         (setq bookmark-alist
               (list (cons "old" (list (cons 'filename old-file)
                                       (cons 'position 1)))
                     (cons "kept" (list (cons 'filename kept-file)
                                        (cons 'position 1)))))
         (bookmark-load file t t)
         (list (mapcar #'car bookmark-alist)
               (with-current-buffer old-buffer bookmark-current-bookmark)
               (with-current-buffer kept-buffer bookmark-current-bookmark)))
     (kill-buffer old-buffer)
     (kill-buffer kept-buffer)
     (delete-directory directory t)))
 '(("kept") nil "kept"))

(princ (qa--done))
