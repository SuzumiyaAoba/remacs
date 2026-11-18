;;; qa_ox_html_class_attributes.el --- escape Org HTML class attributes -*- lexical-binding: t; -*-

(require 'cl-lib)

;; Load only the affected formatters and their escaping helper from ox-html.el.
(defconst qa--ox-html-source-file
  (expand-file-name "../lisp/ox-html.el"
                    (file-name-directory load-file-name)))

(defun qa--ox-html-load-definition (name)
  (with-temp-buffer
    (insert-file-contents qa--ox-html-source-file)
    (unless (re-search-forward
             (concat "^[(]\\(?:defvar\\|defun\\) "
                     (regexp-quote (symbol-name name)) "\\_>")
             nil t)
      (error "Definition missing from %s: %s" qa--ox-html-source-file name))
    (goto-char (match-beginning 0))
    (eval (read (current-buffer)))))

(mapc #'qa--ox-html-load-definition
      '(org-html-protect-char-alist
        org-html-encode-plain-text
        org-html--make-attribute-string
        org-html-headline
        org-html-template))

(defconst qa--ox-html-evil-class
  (concat "evil" (string 34) " onmouseover=" (string 34) "alert(1)"))

(defun qa--ox-html-headline-class-output (container-class headline-class)
  (let ((info (list :html-toplevel-hlevel 2
                    :html-format-headline-function
                    (lambda (&rest _) "Headline"))))
    (cl-letf (((symbol-function 'org-element-property)
               (lambda (property _headline)
                 (pcase property
                   (:title "Headline")
                   (:HTML_CONTAINER_CLASS container-class)
                   (:HTML_HEADLINE_CLASS headline-class))))
              ((symbol-function 'org-export-data) (lambda (value &rest _) value))
              ((symbol-function 'org-export-numbered-headline-p)
               (lambda (&rest _) nil))
              ((symbol-function 'org-export-get-headline-number)
               (lambda (&rest _) '(1)))
              ((symbol-function 'org-export-get-relative-level)
               (lambda (&rest _) 1))
              ((symbol-function 'org-export-low-level-p)
               (lambda (&rest _) nil))
              ((symbol-function 'org-html--reference)
               (lambda (&rest _) "headline-id"))
              ((symbol-function 'org-html--container) (lambda (&rest _) "div"))
              ((symbol-function 'org-element-contents) (lambda (&rest _) '(section)))
              ((symbol-function 'org-element-type-p)
               (lambda (_element type) (eq type 'section)))
              ((symbol-function 'org-html-section) (lambda (&rest _) "")))
      (org-html-headline 'headline "body" info))))

(defun qa--ox-html-content-class-output (class)
  (let ((info (list :html-divs '((content "div" "content" "text"))
                    :html-content-class class
                    :html-link-up ""
                    :html-link-home ""
                    :html-doctype "html5"
                    :html-extension "html"
                    :language "en")))
    (cl-letf (((symbol-function 'org-html-html5-p) (lambda (&rest _) t))
              ((symbol-function 'org-html-xhtml-p) (lambda (&rest _) nil))
              ((symbol-function 'org-html-doctype)
               (lambda (&rest _) "<!DOCTYPE html>"))
              ((symbol-function 'org-html--build-meta-info)
               (lambda (&rest _) ""))
              ((symbol-function 'org-html--build-head)
               (lambda (&rest _) ""))
              ((symbol-function 'org-html--build-mathjax-config)
               (lambda (&rest _) ""))
              ((symbol-function 'org-html--build-pre/postamble)
               (lambda (&rest _) ""))
              ((symbol-function 'org-trim) (lambda (value) value)))
      (org-html-template "body" info))))

(qa (let ((html (qa--ox-html-headline-class-output qa--ox-html-evil-class nil)))
      (and (string-match-p
            (regexp-quote
             "class=\"outline-2 evil&quot; onmouseover=&quot;alert(1)\"")
            html)
           (not (string-match-p " onmouseover=\"alert(1)\"" html))))
    t)

(qa (let ((html (qa--ox-html-headline-class-output nil qa--ox-html-evil-class)))
      (and (string-match-p
            (regexp-quote
             "class=\"evil&quot; onmouseover=&quot;alert(1)\"")
            html)
           (not (string-match-p " onmouseover=\"alert(1)\"" html))))
    t)

(qa (let ((html (qa--ox-html-content-class-output qa--ox-html-evil-class)))
      (and (string-match-p
            (regexp-quote
             "class=\"evil&quot; onmouseover=&quot;alert(1)\"")
            html)
           (not (string-match-p " onmouseover=\"alert(1)\"" html))))
    t)

(princ (qa--done))
