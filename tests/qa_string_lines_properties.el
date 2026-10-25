;;; -*- lexical-binding: nil; -*-
;; string-lines reuses an unchanged string and substrings retain properties (#409).
(let* ((s (propertize "abc" 'face 'bold))
       (line (car (string-lines s))))
  (qa (eq line s) t)
  (qa (get-text-property 0 'face line) 'bold))
(let ((lines (string-lines (propertize "a\nb" 'face 'bold))))
  (qa (mapcar (lambda (line) (get-text-property 0 'face line)) lines)
      '(bold bold)))
(princ (qa--done))
