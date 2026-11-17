;;; qa_bookmark_load_overwrite_fringe_marks.el --- clear old fringe marks on bookmark overwrite -*- lexical-binding: t; -*-

(require 'bookmark)

(qa
 (let* ((directory (make-temp-file "bookmark-overwrite-" t))
        (file (expand-file-name "empty-bookmarks.eld" directory))
        (old-file (expand-file-name "old.txt" directory))
        (buffer (get-buffer-create " *bookmark-overlay-probe*"))
        (bookmark-alist nil)
        (bookmark-version-control 'never)
        (bookmark-fringe-mark t))
   (unwind-protect
       (progn
         (bookmark-write-file file)
         (with-current-buffer buffer
           (insert "old file\n")
           (setq buffer-file-name old-file)
           (goto-char (point-min))
           (bookmark--set-fringe-mark))
         (setq bookmark-alist
               (list (cons "old" (list (cons 'filename old-file)
                                       (cons 'position 1)))))
         (bookmark-load file t t)
         (list bookmark-alist
               (with-current-buffer buffer
                 (length (overlays-in (point-min) (1+ (point-min)))))))
     (kill-buffer buffer)
     (delete-directory directory t)))
 '(nil 0))

(princ (qa--done))
