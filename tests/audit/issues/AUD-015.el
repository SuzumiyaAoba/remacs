;;; -*- lexical-binding: t; -*-
;; "hash:equal-markers"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (let ((h (make-hash-table :test 'equal))) (puthash (point-marker) 42 h) (gethash (point-marker) h))) t))
           (error (list 'error err))))
  (terpri))
