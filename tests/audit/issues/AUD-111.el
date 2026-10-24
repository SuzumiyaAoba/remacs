;;; -*- lexical-binding: t; -*-

;; 'search:backward-count-all-or-nothing'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "a") (goto-char (point-max)) (list (search-backward "a" nil t 2) (point))) t))
           (error (list 'error err))))
  (terpri))

;; 'search:count-re-search-backward'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "a") (goto-char (point-max)) (list (re-search-backward "a" nil t 2) (point))) t))
           (error (list 'error err))))
  (terpri))

;; 'search:count-posix-search-backward'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "a") (goto-char (point-max)) (list (posix-search-backward "a" nil t 2) (point))) t))
           (error (list 'error err))))
  (terpri))

;; 'search:count-search-forward'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "a") (goto-char (point-max)) (list (search-forward "a" nil t -2) (point))) t))
           (error (list 'error err))))
  (terpri))

;; 'search:backward-count-error'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "a") (goto-char (point-max)) (list (condition-case err (search-backward "a" nil nil 2) (error (car err))) (point))) t))
           (error (list 'error err))))
  (terpri))

;; 'search:backward-one-control'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "a") (goto-char (point-max)) (list (search-backward "a" nil t 1) (point))) t))
           (error (list 'error err))))
  (terpri))
