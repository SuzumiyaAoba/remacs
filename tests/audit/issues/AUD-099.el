;;; -*- lexical-binding: t; -*-

;; 'char-equal:nonunicode'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((case-fold-search t)) (list (char-equal 1114112 1114113) (char-equal 55296 55297) (char-equal 4194176 4194303))) t))
           (error (list 'error err))))
  (terpri))

;; 'char-equal:nonunicode-no-fold-control'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(let ((case-fold-search nil)) (list (char-equal 1114112 1114113) (char-equal 55296 55297) (char-equal 4194176 4194303))) t))
           (error (list 'error err))))
  (terpri))
