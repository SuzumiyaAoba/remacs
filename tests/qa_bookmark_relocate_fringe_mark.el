;;; qa_bookmark_relocate_fringe_mark.el --- relocate bookmark fringe marks -*- lexical-binding: t; -*-

(require 'bookmark)
(require 'cl-lib)

(qa
 (let* ((old-file "/tmp/old-bookmark.txt")
        (new-file "/tmp/new-bookmark.txt")
        (bookmark-alist
         (list (list "relocate-me"
                     (cons 'filename old-file)
                     (cons 'position 1))))
        (bookmark-save-flag nil)
        (bookmark-bookmarks-timestamp '("/tmp/no-file" 0))
        (bookmark-watch-bookmark-file nil)
        (bookmark-alist-modification-count 0)
        (bookmark-fringe-mark t)
        (old-buffer (get-buffer-create " *bookmark-relocate-old*"))
        (new-buffer (get-buffer-create " *bookmark-relocate-new*")))
   (unwind-protect
       (progn
         (with-current-buffer old-buffer
           (insert "line\n")
           (setq buffer-file-name old-file)
           (goto-char (point-min))
           (bookmark--set-fringe-mark))
         (with-current-buffer new-buffer
           (insert "line\n")
           (setq buffer-file-name new-file))
         (cl-letf (((symbol-function 'read-file-name)
                    (lambda (&rest _) new-file)))
           (with-current-buffer old-buffer
             (bookmark-relocate "relocate-me")))
         (list (bookmark-get-filename "relocate-me")
               (with-current-buffer old-buffer
                 (length (overlays-in (point-min) (point-max))))
               (with-current-buffer new-buffer
                 (length (overlays-in (point-min) (point-max))))))
     (kill-buffer old-buffer)
     (kill-buffer new-buffer)))
 '("/tmp/new-bookmark.txt" 0 1))

(princ (qa--done))
