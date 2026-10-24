;;; -*- lexical-binding: t; -*-
;; "html:<head></head><body>x</body>"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "<head></head><body>x</body>") (libxml-parse-html-region)) t))
           (error (list 'error err))))
  (terpri))

;; "html-discard:<head></head><body>x</body>"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "<head></head><body>x</body>") (libxml-parse-html-region nil nil nil t)) t))
           (error (list 'error err))))
  (terpri))
