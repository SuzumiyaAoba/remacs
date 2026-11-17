;;; qa_bookmark_rename_current_names.el --- update current bookmark names in all buffers -*- lexical-binding: t; -*-

(require 'bookmark)

(qa
 (let ((bookmark-alist
        '(("old" (filename . "/tmp/old.txt") (position . 1))))
       (bookmark-save-flag nil)
       (bookmark-bookmarks-timestamp nil)
       (bookmark-watch-bookmark-file nil)
       (bookmark-alist-modification-count 0)
       (bookmark-fringe-mark nil)
       (buffer1 (get-buffer-create " *bookmark-rename-1*"))
       (buffer2 (get-buffer-create " *bookmark-rename-2*"))
       (buffer3 (get-buffer-create " *bookmark-rename-3*")))
   (unwind-protect
       (progn
         (with-current-buffer buffer1
           (setq-local bookmark-current-bookmark "old"))
         (with-current-buffer buffer2
           (setq-local bookmark-current-bookmark "old"))
         (with-current-buffer buffer3
           (setq-local bookmark-current-bookmark "unrelated"))
         (with-current-buffer buffer1
           (bookmark-rename "old" "new"))
         (list (caar bookmark-alist)
               (with-current-buffer buffer1 bookmark-current-bookmark)
               (with-current-buffer buffer2 bookmark-current-bookmark)
               (with-current-buffer buffer3 bookmark-current-bookmark)))
     (kill-buffer buffer1)
     (kill-buffer buffer2)
     (kill-buffer buffer3)))
 '("new" "new" "new" "unrelated"))

(princ (qa--done))
