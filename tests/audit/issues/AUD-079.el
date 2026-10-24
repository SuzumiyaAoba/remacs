;;; -*- lexical-binding: t; -*-

;; 'transpose:markers-nil'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "ab--XYZ") (goto-char 2) (let ((first (copy-marker 2)) (second (copy-marker 6)) (middle (copy-marker 4))) (transpose-regions 1 3 5 8 nil) (list (buffer-string) (point) (mapcar #'marker-position (list first second middle))))) t))
           (error (list 'error err))))
  (terpri))

;; 'transpose:markers-t'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "ab--XYZ") (goto-char 2) (let ((first (copy-marker 2)) (second (copy-marker 6)) (middle (copy-marker 4))) (transpose-regions 1 3 5 8 t) (list (buffer-string) (point) (mapcar #'marker-position (list first second middle))))) t))
           (error (list 'error err))))
  (terpri))
