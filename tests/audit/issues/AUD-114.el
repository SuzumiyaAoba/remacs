;;; -*- lexical-binding: t; -*-

;; 'match-data:reuse-integers'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(progn (string-match ".." "abc") (let ((reuse (match-data t))) (string-match "b" "abc") (let ((out (match-data t reuse))) (list (eq out reuse) out reuse)))) t))
           (error (list 'error err))))
  (terpri))

;; 'match-data:reuse-long-list'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((reuse (make-list 6 'old))) (string-match "a" "a") (let ((out (match-data t reuse))) (list (eq out reuse) out reuse))) t))
           (error (list 'error err))))
  (terpri))

;; 'match-data:reseat-old-markers'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "ab") (goto-char 1) (re-search-forward "a") (let* ((reuse (match-data)) (start (car reuse)) (end (cadr reuse))) (string-match "b" "b") (let ((out (match-data t reuse t))) (list (eq out reuse) (null (marker-buffer start)) (null (marker-buffer end)) (mapcar (lambda (x) (if (integerp x) x 'marker)) reuse) out)))) t))
           (error (list 'error err))))
  (terpri))
