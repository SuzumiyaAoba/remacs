;;; -*- lexical-binding: t; -*-
;; 'xml:<a x="1" x="2"/>'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "<a x=\"1\" x=\"2\"/>") (libxml-parse-xml-region)) t))
           (error (list 'error err))))
  (terpri))

;; 'xml:<1bad/>'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "<1bad/>") (libxml-parse-xml-region)) t))
           (error (list 'error err))))
  (terpri))

;; 'xml:<a>&#0;</a>'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "<a>&#0;</a>") (libxml-parse-xml-region)) t))
           (error (list 'error err))))
  (terpri))

;; 'xml:<a>&#1;</a>'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "<a>&#1;</a>") (libxml-parse-xml-region)) t))
           (error (list 'error err))))
  (terpri))

;; 'xml:<a>\x01</a>'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "<a>\1</a>") (libxml-parse-xml-region)) t))
           (error (list 'error err))))
  (terpri))

;; 'xml:<a><![CDATA[\x01]]></a>'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "<a><![CDATA[\1]]></a>") (libxml-parse-xml-region)) t))
           (error (list 'error err))))
  (terpri))

;; 'xml:<a><!--bad--comment--></a>'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "<a><!--bad--comment--></a>") (libxml-parse-xml-region)) t))
           (error (list 'error err))))
  (terpri))

;; 'xml:<a x="<"/>'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "<a x=\"<\"/>") (libxml-parse-xml-region)) t))
           (error (list 'error err))))
  (terpri))

;; 'xml:<a>]]></a>'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "<a>]]></a>") (libxml-parse-xml-region)) t))
           (error (list 'error err))))
  (terpri))
