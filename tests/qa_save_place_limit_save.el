;;; qa_save_place_limit_save.el --- save-place write limit regression -*- lexical-binding: t -*-

(require 'saveplace)

(if qa--remacs
    (qa (let* ((file (make-temp-file "save-place-limit-"))
               (save-place-file file)
               (save-place-limit 1)
               (save-place-forget-unreadable-files nil)
               (save-place-alist
                (list (cons "/tmp/a" 1)
                      (cons "/tmp/b" 2)
                      (cons "/tmp/c" 3)))
               (save-place--load-failed nil)
               count)
          (unwind-protect
              (progn
                (save-place-alist-to-file)
                (with-temp-buffer
                  (insert-file-contents save-place-file)
                  (forward-line 1)
                  (setq count (length (read (current-buffer)))))
                (list (length save-place-alist) count))
            (delete-file file)))
        '(1 1))
  ;; GNU 31.1 does not reapply the limit while serializing existing entries.
  (qa-xf (let* ((file (make-temp-file "save-place-limit-"))
                (save-place-file file)
                (save-place-limit 1)
                (save-place-forget-unreadable-files nil)
                (save-place-alist
                 (list (cons "/tmp/a" 1)
                       (cons "/tmp/b" 2)
                       (cons "/tmp/c" 3)))
                (save-place--load-failed nil)
                count)
           (unwind-protect
               (progn
                 (save-place-alist-to-file)
                 (with-temp-buffer
                   (insert-file-contents save-place-file)
                   (forward-line 1)
                   (setq count (length (read (current-buffer)))))
                 (list (length save-place-alist) count))
             (delete-file file)))
         '(1 1)
         "save-place applies the configured limit when saving existing entries"))

(princ (qa--done))
