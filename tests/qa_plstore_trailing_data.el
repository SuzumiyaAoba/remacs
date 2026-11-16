;;; qa_plstore_trailing_data.el --- plstore trailing data regression -*- lexical-binding: t -*-

(require 'plstore)

(defun qa-plstore-write-corrupt-tail-preserves-file ()
  (let* ((file (make-temp-file "plstore-history-"))
         (original "((\"entry\" :host \"example\"))\n("))
    (unwind-protect
        (progn
          (with-temp-file file
            (insert original))
          (with-temp-buffer
            (setq buffer-file-name file)
            (setq-local plstore-encoded t)
            (insert original)
            (set-buffer-modified-p t)
            (let ((save-failed
                   (condition-case nil
                       (progn (plstore--write-contents-functions) nil)
                     (error t))))
              (list save-failed
                    (buffer-modified-p)
                    (equal original
                           (with-temp-buffer
                             (insert-file-contents file)
                             (buffer-string)))))))
      (delete-file file))))

(if qa--remacs
    (qa (qa-plstore-write-corrupt-tail-preserves-file) '(t t t))
  ;; GNU 31.1 currently accepts the prefix and writes away the broken tail.
  (qa-xf (qa-plstore-write-corrupt-tail-preserves-file) '(t t t)
         "Remacs rejects plstore contents with unread trailing data"))

(qa (plstore--get-alist
     (plstore--decode
      (concat "((\"entry\" :host \"example\"))"
              "\n; trailing comment\n")))
    '(("entry" :host "example")))

(princ (qa--done))
