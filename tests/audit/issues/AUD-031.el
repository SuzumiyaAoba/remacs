;;; -*- lexical-binding: t; -*-
;; "html:<table>text<tr><td>a</td></tr></table>"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "<table>text<tr><td>a</td></tr></table>") (libxml-parse-html-region)) t))
           (error (list 'error err))))
  (terpri))

;; "html:<p><div>x</div>y</p>"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "<p><div>x</div>y</p>") (libxml-parse-html-region)) t))
           (error (list 'error err))))
  (terpri))

;; "html:<svg viewBox=\"0 0 1 1\"><circle/></svg>"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "<svg viewBox=\"0 0 1 1\"><circle/></svg>") (libxml-parse-html-region)) t))
           (error (list 'error err))))
  (terpri))

;; "html-discard:<table>text<tr><td>a</td></tr></table>"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "<table>text<tr><td>a</td></tr></table>") (libxml-parse-html-region nil nil nil t)) t))
           (error (list 'error err))))
  (terpri))

;; "html-discard:<p><div>x</div>y</p>"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "<p><div>x</div>y</p>") (libxml-parse-html-region nil nil nil t)) t))
           (error (list 'error err))))
  (terpri))

;; "html-discard:<svg viewBox=\"0 0 1 1\"><circle/></svg>"
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "<svg viewBox=\"0 0 1 1\"><circle/></svg>") (libxml-parse-html-region nil nil nil t)) t))
           (error (list 'error err))))
  (terpri))
