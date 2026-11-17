;;; qa_svg_print_xml_attribute_escape.el --- escape SVG XML attributes -*- lexical-binding: t; -*-

(require 'svg)

(defun qa-svg--signals-error-p (thunk)
  (condition-case nil
      (progn (funcall thunk) nil)
    (error t)))

(qa
 (list
  (with-temp-buffer
    (svg-print
     (list 'svg
           (list (cons 'width
                       (concat "1" (string 34) " onload=" (string 34) "evil"))
                 (cons 'data-note "&<>"))))
    (let ((quote (string 34)))
      (equal (buffer-string)
             (concat "<svg width=" quote
                     "1&quot; onload=&quot;evil"
                     quote " data-note=" quote
                     "&amp;&lt;&gt;" quote "></svg>"))))
  (qa-svg--signals-error-p
   (lambda () (svg--encode-text (concat "A" (string 1) "B"))))
  (equal (svg--encode-text "&<>") "&amp;&lt;&gt;"))
 '(t t t))

(princ (qa--done))
