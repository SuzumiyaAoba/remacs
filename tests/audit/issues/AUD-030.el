;;; -*- lexical-binding: t; -*-
;; "html:<table><tr><td>a</td></tr></table><table><tbody><tr><td>b</td></tr></tbody></table>"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "<table><tr><td>a</td></tr></table><table><tbody><tr><td>b</td></tr></tbody></table>") (libxml-parse-html-region)) t))
           (error (list 'error err))))
  (terpri))

;; "html-discard:<table><tr><td>a</td></tr></table><table><tbody><tr><td>b</td></tr></tbody></table>"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "<table><tr><td>a</td></tr></table><table><tbody><tr><td>b</td></tr></tbody></table>") (libxml-parse-html-region nil nil nil t)) t))
           (error (list 'error err))))
  (terpri))
