;;; -*- lexical-binding: t; -*-
;; "html-discard:<!--before--><p><!--inside-->text</p><!--after-->"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "<!--before--><p><!--inside-->text</p><!--after-->") (libxml-parse-html-region nil nil nil t)) t))
           (error (list 'error err))))
  (terpri))
