;;; -*- lexical-binding: t; -*-

;; 'excursion:initially-unset-mark'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "abc") (goto-char 1) (set-marker (mark-marker) nil) (save-mark-and-excursion (set-mark 2) (setq mark-active t) (goto-char 3)) (list (point) (mark t) mark-active)) t))
           (error (list 'error err))))
  (terpri))

;; 'excursion:mark-activation-hooks'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(with-temp-buffer (insert "abc") (set-mark 2) (setq mark-active t) (let ((hits 0)) (let ((activate-mark-hook (list (lambda nil (setq hits (1+ hits)))))) (save-mark-and-excursion (set-mark 3) (setq mark-active nil))) hits)) t))
           (error (list 'error err))))
  (terpri))
