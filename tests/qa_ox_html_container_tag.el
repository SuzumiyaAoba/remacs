;;; qa_ox_html_container_tag.el --- validate Org HTML container tags -*- lexical-binding: t; -*-

(require 'cl-lib)

(defconst qa--ox-html-source-file
  (expand-file-name "../lisp/ox-html.el"
                    (file-name-directory load-file-name)))

(defun qa--ox-html-load-container-definition ()
  (with-temp-buffer
    (insert-file-contents qa--ox-html-source-file)
    (unless (re-search-forward "^(defun org-html--container\\_>" nil t)
      (error "Definition missing from %s" qa--ox-html-source-file))
    (goto-char (match-beginning 0))
    (eval (read (current-buffer)))))

(if qa--remacs
    (progn
      (unless (fboundp 'org-element-property)
        (defalias 'org-element-property (lambda (&rest _) nil)))
      (unless (fboundp 'org-export-get-relative-level)
        (defalias 'org-export-get-relative-level (lambda (&rest _) 1)))
      (qa--ox-html-load-container-definition))
  (require 'ox-html))

(defun qa--ox-html-container-for (property-tag configured-tag level)
  (if qa--remacs
      (cl-letf (((symbol-function 'org-element-property)
                 (lambda (property _headline)
                   (and (eq property :HTML_CONTAINER) property-tag)))
                ((symbol-function 'org-export-get-relative-level)
                 (lambda (&rest _) level)))
        (org-html--container 'headline (list :html-container configured-tag)))
    (let ((headline (org-element-create
                     'headline (list :HTML_CONTAINER property-tag :level level))))
      (org-html--container headline (list :html-container configured-tag)))))

(defconst qa--ox-html-unsafe-tag "div><script>alert(1)</script><div")

(qa (qa--ox-html-container-for "article" "div" 1) "article")
(qa (qa--ox-html-container-for nil "x-widget" 1) "x-widget")
(qa (qa--ox-html-container-for nil "div" 2) "div")

(if qa--remacs
    (progn
      (qa-err (qa--ox-html-container-for qa--ox-html-unsafe-tag "div" 1) 'error)
      (qa-err (qa--ox-html-container-for nil qa--ox-html-unsafe-tag 1) 'error))
  (qa-xf (qa--ox-html-container-for qa--ox-html-unsafe-tag "div" 1)
         qa--ox-html-unsafe-tag
         "GNU 31.1 inserts invalid HTML_CONTAINER tag names")
  (qa-xf (qa--ox-html-container-for nil qa--ox-html-unsafe-tag 1)
         qa--ox-html-unsafe-tag
         "GNU 31.1 inserts invalid org-html-container values"))

(princ (qa--done))
