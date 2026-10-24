;;; -*- lexical-binding: t; -*-
;; 'xml:<a>one\r\ntwo\rthree</a>'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "<a>one\15\ntwo\15three</a>") (libxml-parse-xml-region)) t))
           (error (list 'error err))))
  (terpri))

;; 'xml:<a x="one\ttwo\nthree\rfour"/>'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "<a x=\"one\11two\nthree\15four\"/>") (libxml-parse-xml-region)) t))
           (error (list 'error err))))
  (terpri))
