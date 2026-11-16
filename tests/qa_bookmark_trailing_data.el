;;; qa_bookmark_trailing_data.el --- bookmark trailing data regression -*- lexical-binding: t -*-

(require 'bookmark)

(defun qa-bookmark-load-corrupt-tail ()
  (let* ((target (make-temp-file "bookmark-target-"))
         (file (make-temp-file "bookmark-history-"))
         (bookmark-alist nil)
         (bookmark-save-flag nil)
         (bookmark-default-file file)
         (bookmark-bookmarks-timestamp nil)
         (bookmark-alist-modification-count 0))
    (unwind-protect
        (progn
          (with-temp-file file
            (bookmark-insert-file-format-version-stamp 'utf-8-unix)
            (prin1 (list (list "kept" (cons 'filename target)))
                   (current-buffer))
            (insert "\n("))
          (let ((original
                 (with-temp-buffer
                   (insert-file-contents file)
                   (buffer-string))))
            (let ((load-error
                   (condition-case nil
                       (progn (bookmark-load file t t t) nil)
                     (error t))))
              (unless load-error
                (bookmark-save nil file))
              (list load-error
                    (equal original
                           (with-temp-buffer
                             (insert-file-contents file)
                             (buffer-string)))))))
      (delete-file target)
      (delete-file file))))

(if qa--remacs
    (qa (qa-bookmark-load-corrupt-tail) '(t t))
  ;; GNU 31.1 currently loads the valid prefix and saves away the broken tail.
  (qa-xf (qa-bookmark-load-corrupt-tail) '(t t)
         "Remacs rejects bookmark files with unread trailing data"))

(qa (with-temp-buffer
      (bookmark-insert-file-format-version-stamp 'utf-8-unix)
      (prin1 '(("kept" (filename . "/tmp/kept"))) (current-buffer))
      (insert "\n; trailing comment\n")
      (length (bookmark-alist-from-buffer)))
    1)

(princ (qa--done))
