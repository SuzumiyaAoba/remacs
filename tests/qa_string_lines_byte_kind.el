;;; -*- lexical-binding: nil; -*-
;; string-lines preserves unibyte strings and split lines (#410).
(let* ((whole (unibyte-string 233 255))
       (split (string-lines (unibyte-string 233 10 255))))
  (qa (list (multibyte-string-p (car (string-lines whole)))
            (string-bytes (car (string-lines whole)))
            (mapcar #'multibyte-string-p split)
            (mapcar #'string-bytes split))
      '(nil 2 (nil nil) (1 1))))
(princ (qa--done))
