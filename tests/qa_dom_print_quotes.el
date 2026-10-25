;;; -*- lexical-binding: nil; -*-
;; DOM printing entity-escapes double quotes in text and attributes (#544).
(require 'dom)
(let ((node (list 'root (list (cons 'a (string 34))) (string 34))))
  (dolist (xml '(nil t))
    (with-temp-buffer
      (dom-print node nil xml)
      (qa (buffer-string) "<root a=\"&quot;\">&quot;</root>"))))
(qa (dom--insert-entities (concat "&<>" (string 34)))
    "&amp;&lt;&gt;&quot;")
(princ (qa--done))
