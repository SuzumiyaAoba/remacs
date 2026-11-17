;;; qa_bookmark_delete_current_names.el --- clear removed current names -*- lexical-binding: t; -*-

(require 'bookmark)

(defun qa--bookmark-delete-current-values (records)
  (let* ((bookmark-alist records)
         (bookmark-save-flag nil)
         (bookmark-bookmarks-timestamp nil)
         (bookmark-watch-bookmark-file nil)
         (bookmark-alist-modification-count 0)
         (bookmark-fringe-mark nil)
         (buffer1 (get-buffer-create " *bookmark-delete-1*"))
         (buffer2 (get-buffer-create " *bookmark-delete-2*"))
         (buffer3 (get-buffer-create " *bookmark-delete-3*"))
         (buffers (list buffer1 buffer2 buffer3)))
    (unwind-protect
        (progn
          (with-current-buffer buffer1
            (setq-local bookmark-current-bookmark "old"))
          (with-current-buffer buffer2
            (setq-local bookmark-current-bookmark "old"))
          (with-current-buffer buffer3
            (setq-local bookmark-current-bookmark "unrelated"))
          (with-current-buffer buffer1
            (bookmark-delete "old" t))
          (list (mapcar (lambda (buffer)
                          (with-current-buffer buffer bookmark-current-bookmark))
                        buffers)
                (mapcar #'car bookmark-alist)))
      (mapc #'kill-buffer buffers))))

;; Deleting the final occurrence clears all buffers that refer to it (#2581).
(qa (qa--bookmark-delete-current-values
     '(("old" (filename . "/tmp/old.txt") (position . 1))))
    '((nil nil "unrelated") nil))

;; Removing one duplicate keeps the remaining name usable in every buffer.
(qa (qa--bookmark-delete-current-values
     '(("old" (filename . "/tmp/old.txt") (position . 1))
       ("old" (filename . "/tmp/another.txt") (position . 1))))
    '(("old" "old" "unrelated") ("old")))

(princ (qa--done))
