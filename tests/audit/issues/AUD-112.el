;;; -*- lexical-binding: t; -*-

;; 'replace-match:backrefs-after-widen'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "prefix ab suffix") (narrow-to-region 8 10) (goto-char (point-min)) (re-search-forward "\\(a\\)\\(b\\)") (widen) (replace-match "\\2\\1" t) (buffer-string)) t))
           (error (list 'error err))))
  (terpri))

;; 'replace-match:backrefs-narrowed-control'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "prefix ab suffix") (narrow-to-region 8 10) (goto-char (point-min)) (re-search-forward "\\(a\\)\\(b\\)") (replace-match "\\2\\1" t) (widen) (buffer-string)) t))
           (error (list 'error err))))
  (terpri))
