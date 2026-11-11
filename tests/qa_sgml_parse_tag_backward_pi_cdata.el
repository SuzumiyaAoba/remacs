;;; -*- lexical-binding: nil; -*-
;; Backward tag parsing must return an enclosing CDATA or PI node, not
;; markup-looking text inside it.
(load-file (expand-file-name "../lisp/sgml-mode.el"
                             (file-name-directory load-file-name)))
(qa
 (with-temp-buffer
   (sgml-mode)
   (insert "<root><![CDATA[<fake>text]]><child/></root>")
   (goto-char (point-min))
   (search-forward "text")
   (let ((tag (sgml-parse-tag-backward)))
     (list (sgml-tag-type tag)
           (buffer-substring-no-properties
            (sgml-tag-start tag) (sgml-tag-end tag)))))
 '(cdata "<![CDATA[<fake>text]]>"))
(qa
 (with-temp-buffer
   (sgml-mode)
   (insert "<root><?pi <fake>text?></root>")
   (goto-char (point-min))
   (search-forward "text")
   (let ((tag (sgml-parse-tag-backward)))
     (list (sgml-tag-type tag)
           (buffer-substring-no-properties
            (sgml-tag-start tag) (sgml-tag-end tag)))))
 '(pi "<?pi <fake>text?>"))

;; Delimiter-looking text inside CDATA/PI data is not a nested opener.
(qa
 (with-temp-buffer
   (sgml-mode)
   (insert "<![CDATA[outer <![CDATA[inner]]>")
   (goto-char (point-max))
   (let ((tag (sgml-parse-tag-backward)))
     (list (sgml-tag-type tag)
           (sgml-tag-start tag)
           (buffer-substring-no-properties
            (sgml-tag-start tag) (point-max)))))
 '(cdata 1 "<![CDATA[outer <![CDATA[inner]]>"))

(qa
 (with-temp-buffer
   (sgml-mode)
   (insert "<?target data <?inside ?>")
   (goto-char (point-max))
   (let ((tag (sgml-parse-tag-backward)))
     (list (sgml-tag-type tag)
           (sgml-tag-start tag)
           (buffer-substring-no-properties
            (sgml-tag-start tag) (point-max)))))
 '(pi 1 "<?target data <?inside ?>"))

(princ (qa--done))
