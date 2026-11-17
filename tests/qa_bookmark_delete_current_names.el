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

(defun qa--bookmark-delete-fringe-result (first-position second-position)
  (let ((bookmark-alist
         `(("first" (filename . "/tmp/same.txt")
                    (position . ,first-position))
           ("second" (filename . "/tmp/same.txt")
                     (position . ,second-position))))
        (bookmark-save-flag nil)
        (bookmark-bookmarks-timestamp '("/tmp/no-file" 0))
        (bookmark-watch-bookmark-file nil)
        (bookmark-alist-modification-count 0)
        (bookmark-fringe-mark t))
    (with-temp-buffer
      (insert "line\nnext\n")
      (setq buffer-file-name "/tmp/same.txt")
      (goto-char second-position)
      (bookmark--set-fringe-mark)
      (bookmark-delete "second" t)
      (list (bookmark-all-names)
            (length (overlays-in (point-min) (point-max)))))))

;; Deleting the final occurrence clears all buffers that refer to it (#2581).
(qa (qa--bookmark-delete-current-values
     '(("old" (filename . "/tmp/old.txt") (position . 1))))
    '((nil nil "unrelated") nil))

;; Removing one duplicate keeps the remaining name usable in every buffer.
(qa (qa--bookmark-delete-current-values
     '(("old" (filename . "/tmp/old.txt") (position . 1))
       ("old" (filename . "/tmp/another.txt") (position . 1))))
    '(("old" "old" "unrelated") ("old")))

;; A mark stays while another bookmark points to the same line (#2582).
(qa (qa--bookmark-delete-fringe-result 1 3) '(("first") 1))
(qa (qa--bookmark-delete-fringe-result 1 6) '(("first") 0))

(princ (qa--done))
