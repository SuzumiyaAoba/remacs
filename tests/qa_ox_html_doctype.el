;;; qa_ox_html_doctype.el --- validate custom Org HTML doctypes -*- lexical-binding: t; -*-

(defconst qa--ox-html-source-file
  (expand-file-name "../lisp/ox-html.el"
                    (file-name-directory load-file-name)))

(defun qa--ox-html-load-definition (name)
  (with-temp-buffer
    (insert-file-contents qa--ox-html-source-file)
    (unless (re-search-forward
             (concat "^[(]\\(?:defconst\\|defun\\) "
                     (regexp-quote (symbol-name name)) "\\_>")
             nil t)
      (error "Definition missing from %s: %s" qa--ox-html-source-file name))
    (goto-char (match-beginning 0))
    (eval (read (current-buffer)))))

(if qa--remacs
    (mapc #'qa--ox-html-load-definition
          '(org-html-doctype-alist org-html--valid-doctype-p org-html-doctype))
  (require 'ox-html))

(qa (org-html-doctype (list :html-doctype "html5")) "<!DOCTYPE html>")
(qa (org-html-doctype (list :html-doctype "<!DOCTYPE html>"))
    "<!DOCTYPE html>")

(qa (org-html-doctype
     (list :html-doctype
           "<!DOCTYPE html PUBLIC \"-//Example//DTD HTML 1.0//EN\" \"https://example.test/doc.dtd\">"))
    "<!DOCTYPE html PUBLIC \"-//Example//DTD HTML 1.0//EN\" \"https://example.test/doc.dtd\">")

(if qa--remacs
    (progn
      (qa-err (org-html-doctype
               (list :html-doctype "<script>alert(1)</script>"))
             'error)
      (qa-err (org-html-doctype
               (list :html-doctype "<!DOCTYPE html><script>alert(1)</script>"))
             'error))
  (qa (org-html-doctype (list :html-doctype "<script>alert(1)</script>"))
      "<script>alert(1)</script>")
  (qa (org-html-doctype
       (list :html-doctype "<!DOCTYPE html><script>alert(1)</script>"))
      "<!DOCTYPE html><script>alert(1)</script>"))

(princ (qa--done))
