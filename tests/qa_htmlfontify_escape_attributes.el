;;; -*- lexical-binding: t; -*-
;;; HTML attribute escaping in htmlfontify (Issues #2287, #2292, #2306).

(provide 'cus-edit)
(require 'htmlfontify)

(qa
 (let* ((file "evil\" onmouseover=\"alert(1)")
        (href (hfy-href "src.el" (list file) "tag" (list (list file 7))))
        (html (format "<a href=\"%s\">tag</a>" href)))
   (list (not (string-match-p "\" onmouseover=\"" html))
         (if (string-match-p "&quot; onmouseover=&quot;" href) t nil)))
 '(t t))

(qa
 (with-temp-buffer
   (hfy-insert-anchor "quote\" onclick=\"alert(1)")
   (buffer-string))
 "<a name=\"quote&quot; onclick=&quot;alert(1)\"></a>")

(qa
 (with-temp-buffer
   (hfy-begin-span "evil\" onclick=\"alert(1)" nil nil nil)
   (buffer-string))
 "<span class=\"evil&quot; onclick=&quot;alert(1)\">")

(princ (qa--done))
