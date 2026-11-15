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

(qa
 (let ((html (with-temp-buffer
               (insert "xy")
               (put-text-property 1 2 'hfy-anchor "first-tag.1")
               (hfy-fontify-buffer "src" "file.el"))))
   (with-current-buffer html
     (goto-char (point-min))
     (not (null (search-forward "<a name=\"first-tag.1\"></a>" nil t)))))
 t)

(princ (qa--done))
