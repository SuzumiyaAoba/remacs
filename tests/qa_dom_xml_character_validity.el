;;; qa_dom_xml_character_validity.el --- validate XML characters and preserve CR -*- lexical-binding: t; -*-

(require 'dom)

(qa
 (list
  (condition-case nil
      (progn
        (with-temp-buffer
          (dom-print (list 'root nil (concat "A" (string 1) "B")) nil t))
        nil)
    (error t))
  (with-temp-buffer
    (dom-print (list 'root nil (concat "A" (string 13) "B")) nil t)
    (buffer-string))
  (with-temp-buffer
    (dom-print (list 'root (list (cons 'title
                                     (concat "a" (string 13) (string 10) "b"))))
               nil t)
    (buffer-string)))
 '(t "<root>A&#13;B</root>" "<root title=\"a&#13;&#10;b\" />"))

(princ (qa--done))
