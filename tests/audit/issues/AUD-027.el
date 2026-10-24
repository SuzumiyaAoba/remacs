;;; -*- lexical-binding: t; -*-
;; "xml:<!DOCTYPE a [<!ENTITY word 'hello'>]><a x=\"&word;\"/>"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "<!DOCTYPE a [<!ENTITY word 'hello'>]><a x=\"&word;\"/>") (libxml-parse-xml-region)) t))
           (error (list 'error err))))
  (terpri))

;; "xml:<!DOCTYPE a [<!ENTITY word 'hello'>]><a>&word;</a>"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "<!DOCTYPE a [<!ENTITY word 'hello'>]><a>&word;</a>") (libxml-parse-xml-region)) t))
           (error (list 'error err))))
  (terpri))
