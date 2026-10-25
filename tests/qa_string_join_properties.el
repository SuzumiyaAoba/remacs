;;; -*- lexical-binding: nil; -*-
;; string-join preserves element and separator text properties (#371).
(let ((s (propertize "a" 'face 'bold))
      (separator (propertize "," 'face 'italic)))
  (qa (list (get-text-property 0 'face (string-join (list s s) separator))
            (get-text-property 1 'face (string-join (list s s) separator)))
      '(bold italic)))

;; Preserve all source properties with the separator only between elements (#555).
(let ((joined (string-join (list (propertize "a" 'tag 'first)
                                 (propertize "b" 'tag 'last))
                           (propertize "-" 'tag 'separator))))
  (qa (mapcar (lambda (n) (get-text-property n 'tag joined)) '(0 1 2))
      '(first separator last)))

(let ((empty-multibyte (make-string 0 ?a t)))
  (qa (list (multibyte-string-p (string-join (list empty-multibyte)))
            (eq (string-join (list empty-multibyte)) empty-multibyte))
      '(t t)))
(princ (qa--done))
