;;; -*- lexical-binding: t; -*-

;; 'replace-region:preserve-marker'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "abcdef") (goto-char 4) (let ((m (copy-marker 3))) (replace-region-contents 1 7 "abcdeX") (list (buffer-string) (point) (marker-position m)))) t))
           (error (list 'error err))))
  (terpri))

;; 'replace-region:unchanged'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "abcdef") (goto-char 4) (buffer-enable-undo) (set-buffer-modified-p nil) (let* ((m (copy-marker 3)) (changes nil) (after-change-functions (list (lambda (&rest args) (push args changes))))) (list (replace-region-contents 1 7 "abcdef") (point) (marker-position m) (buffer-modified-p) buffer-undo-list changes))) t))
           (error (list 'error err))))
  (terpri))
