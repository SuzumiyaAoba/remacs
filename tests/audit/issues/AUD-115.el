;;; -*- lexical-binding: t; -*-

;; 'regexp:syntax-property-word'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "a.a.a") (put-text-property 2 3 'syntax-table (string-to-syntax "w")) (let ((parse-sexp-lookup-properties t)) (goto-char 1) (re-search-forward "\\w+") (list (match-beginning 0) (match-end 0)))) t))
           (error (list 'error err))))
  (terpri))

;; 'regexp:syntax-property-boundary'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "a.a") (put-text-property 2 3 'syntax-table (string-to-syntax "w")) (let ((parse-sexp-lookup-properties t)) (goto-char 2) (looking-at "\\b"))) t))
           (error (list 'error err))))
  (terpri))

;; 'regexp:syntax-property-string'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((source (propertize "a.a" 'syntax-table (string-to-syntax "w"))) (parse-sexp-lookup-properties t)) (string-match "\\w+" source) (list (match-beginning 0) (match-end 0))) t))
           (error (list 'error err))))
  (terpri))

;; 'regexp:syntax-property-disabled-control'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "a.a.a") (put-text-property 2 3 'syntax-table (string-to-syntax "w")) (let ((parse-sexp-lookup-properties nil)) (goto-char 1) (re-search-forward "\\w+") (list (match-beginning 0) (match-end 0)))) t))
           (error (list 'error err))))
  (terpri))

;; 'regexp:syntax-table-control'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "a.a") (let ((table (copy-syntax-table (syntax-table)))) (modify-syntax-entry 46 "w" table) (set-syntax-table table) (goto-char 1) (re-search-forward "\\w+") (list (match-beginning 0) (match-end 0)))) t))
           (error (list 'error err))))
  (terpri))
