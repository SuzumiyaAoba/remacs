;;; qa_save_place_trailing_data.el --- save-place read boundary regression -*- lexical-binding: t -*-

(require 'saveplace)

(when qa--remacs
  (qa (save-place--read-tail-valid-p "\n; line comment\n" 0) t)
  (qa (save-place--read-tail-valid-p "#| block #| nested |# comment |#\n" 0) t)
  (qa (save-place--read-tail-valid-p "#; discarded" 0) t))

(if qa--remacs
    (qa (let* ((target (make-temp-file "save-place-target-"))
               (history (make-temp-file "save-place-history-"))
               (save-place-file history)
               (save-place-loaded nil)
               (save-place-alist nil)
               (save-place--load-failed nil))
          (unwind-protect
              (progn
                (with-temp-file history
                  (prin1 (list (cons target 2)) (current-buffer))
                  (insert "\n("))
                (let ((original
                       (with-temp-buffer
                         (insert-file-contents history)
                         (buffer-string))))
                  (save-place-load-alist-from-file)
                  (save-place-kill-emacs-hook)
                  (list save-place--load-failed
                        (equal original
                               (with-temp-buffer
                                 (insert-file-contents history)
                                 (buffer-string))))))
            (delete-file target)
            (delete-file history)))
        '(t t))
  ;; GNU 31.1 currently accepts the valid prefix and overwrites the damaged tail.
  (qa-xf (let* ((target (make-temp-file "save-place-target-"))
                (history (make-temp-file "save-place-history-"))
                (save-place-file history)
                (save-place-loaded nil)
                (save-place-alist nil)
                (save-place--load-failed nil))
           (unwind-protect
               (progn
                 (with-temp-file history
                   (prin1 (list (cons target 2)) (current-buffer))
                   (insert "\n("))
                 (let ((original
                        (with-temp-buffer
                          (insert-file-contents history)
                          (buffer-string))))
                   (save-place-load-alist-from-file)
                   (save-place-kill-emacs-hook)
                   (list save-place--load-failed
                         (equal original
                                (with-temp-buffer
                                  (insert-file-contents history)
                                  (buffer-string))))))
             (delete-file target)
             (delete-file history)))
         '(t t)
         "Remacs protects save-place files with unread trailing data"))

(qa (let* ((target (make-temp-file "save-place-target-"))
           (history (make-temp-file "save-place-history-"))
           (save-place-file history)
           (save-place-loaded nil)
           (save-place-alist nil)
           (save-place--load-failed nil))
      (unwind-protect
          (progn
            (with-temp-file history
              (prin1 (list (cons target 2)) (current-buffer))
              (insert "\n; line comment\n"
                      "#| block #| nested |# comment |#\n"
                      "#; discarded"))
            (save-place-load-alist-from-file)
            (list (not save-place--load-failed)
                  (equal (caar save-place-alist) target)))
        (delete-file target)
        (delete-file history)))
    '(t t))

(princ (qa--done))
