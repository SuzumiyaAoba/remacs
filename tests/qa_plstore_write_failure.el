;;; qa_plstore_write_failure.el --- plstore write failure regression -*- lexical-binding: t -*-

(require 'plstore)

(defun qa-plstore-write-state (fail-write)
  (let* ((file (make-temp-file "plstore-write-failure-"))
         (write-region-original (symbol-function 'write-region))
         result)
    (unwind-protect
        (with-temp-buffer
          (setq buffer-file-name file)
          (setq-local plstore-encoded t)
          (insert "((\"entry\" :host \"example\"))")
          (set-buffer-modified-p t)
          (when fail-write
            (fset 'write-region
                  (lambda (&rest _)
                    (signal 'file-error '("simulated write failure")))))
          (setq result
                (condition-case error
                    (progn
                      (plstore--write-contents-functions)
                      (list 'ok (buffer-file-name) (buffer-modified-p)))
                  (error
                   (list (car error)
                         (buffer-file-name)
                         (buffer-modified-p)))))
          (list (car result)
                (equal (cadr result) file)
                (nth 2 result)))
      (fset 'write-region write-region-original)
      (delete-file file))))

(if qa--remacs
    (progn
      (qa (qa-plstore-write-state t) '(file-error t t))
      (qa (qa-plstore-write-state nil) '(ok t nil)))
  ;; GNU 31.1 also clears the modified flag after a failed write.
  (progn
    (qa-xf (qa-plstore-write-state t) '(file-error t t)
           "Failed plstore writes must preserve buffer state")
    (qa (qa-plstore-write-state nil) '(ok t nil))))

(princ (qa--done))
