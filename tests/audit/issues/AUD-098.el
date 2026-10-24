;;; -*- lexical-binding: t; -*-





;; 'string-compare:custom-case-table'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (let ((ct (copy-case-table (standard-case-table))) (case-fold-search t)) (set-case-syntax-pair 88 121 ct) (set-case-table ct) (list (upcase 121) (char-equal 88 121) (compare-strings "X" nil nil "y" nil nil t)))) t))
           (error (list 'error err))))
  (terpri))

;; 'compare-strings:sharp-s'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(compare-strings "ß" nil nil "S" nil nil t) t))
           (error (list 'error err))))
  (terpri))

;; 'search:custom-case-table'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (let ((ct (copy-case-table (standard-case-table))) (case-fold-search t)) (set-case-syntax-pair 88 121 ct) (set-case-table ct) (insert "y") (goto-char 1) (let ((literal (search-forward "X" nil t))) (goto-char 1) (list (upcase 121) literal (re-search-forward "X" nil t) (string-match "X" "y"))))) t))
           (error (list 'error err))))
  (terpri))

;; 'search:default-case-table-control'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (let ((case-fold-search t)) (insert "x") (goto-char 1) (let ((literal (search-forward "X" nil t))) (goto-char 1) (list literal (re-search-forward "X" nil t) (string-match "X" "x"))))) t))
           (error (list 'error err))))
  (terpri))
