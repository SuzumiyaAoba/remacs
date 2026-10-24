;;; -*- lexical-binding: t; -*-
;; "xml:<a x=\"&missing;\"/>"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "<a x=\"&missing;\"/>") (libxml-parse-xml-region)) t))
           (error (list 'error err))))
  (terpri))
