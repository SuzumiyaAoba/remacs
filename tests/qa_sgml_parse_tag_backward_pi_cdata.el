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
(princ (qa--done))
