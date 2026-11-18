;;; qa_bookmark_load_buffer_lifetime.el --- preserve open bookmark buffers -*- lexical-binding: t; -*-

(require 'bookmark)

(defun qa--bookmark-load-buffer-lifetime ()
  (let* ((directory (make-temp-file "bookmark-load-buffer-" t))
         (file (expand-file-name "bookmarks" directory))
         (buffer nil)
         (bookmark-alist nil)
         (bookmark-save-flag nil)
         (bookmark-default-file file)
         (bookmark-bookmarks-timestamp (list file (current-time)))
         (bookmark-watch-bookmark-file nil)
         (bookmark-after-load-file-hook nil)
         (bookmark-fringe-mark nil)
         saved-point saved-text)
    (unwind-protect
        (progn
          (bookmark-write-file file)
          (setq buffer (find-file-noselect file))
          (with-current-buffer buffer
            (goto-char (point-max))
            (insert "\n; unsaved edit\n")
            (goto-char (+ (point-min) 2))
            (setq saved-point (point)
                  saved-text (buffer-string)))
          (bookmark-load file t t)
          (let ((existing-buffer-result
                 (list (buffer-live-p buffer)
                       (and (buffer-live-p buffer)
                            (with-current-buffer buffer (buffer-modified-p)))
                       (and (buffer-live-p buffer)
                            (with-current-buffer buffer
                              (equal saved-text (buffer-string))))
                       (and (buffer-live-p buffer)
                            (with-current-buffer buffer
                              (= saved-point (point)))))))
            (when (buffer-live-p buffer)
              (kill-buffer buffer))
            (setq buffer nil)
            ;; A buffer created solely to load the file should still be killed.
            (bookmark-load file t t)
            (append existing-buffer-result
                    (list (not (get-file-buffer file))))))
      (when (buffer-live-p buffer)
        (kill-buffer buffer))
      (delete-directory directory t))))

(if qa--remacs
    (qa (qa--bookmark-load-buffer-lifetime) '(t t t t t))
  (qa-xf (qa--bookmark-load-buffer-lifetime) '(nil nil nil nil t)
         "GNU Emacs 31.1 kills a pre-existing bookmark buffer"))

(princ (qa--done))
