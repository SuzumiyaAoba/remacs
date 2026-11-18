;;; qa_ox_html_language_attributes.el --- escape Org HTML language attributes -*- lexical-binding: t; -*-

(require 'cl-lib)

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
        org-html-template))

(dolist (function '(org-html-html5-p org-html-xhtml-p org-html-doctype
                    org-html--build-meta-info org-html--build-head
                    org-html--build-mathjax-config org-html--build-pre/postamble
                    org-trim))
  (unless (fboundp function)
    (fset function (lambda (&rest _) nil))))

(defun qa--ox-html-template-language (language xhtml)
  (let ((info (list :html-divs '((content "div" "content" "text"))
                    :html-content-class "content"
                    :html-link-up ""
                    :html-link-home ""
                    :html-doctype (if xhtml "xhtml" "html5")
                    :html-extension "html"
                    :html-home/up-format "%s%s"
                    :language language)))
    (cl-letf (((symbol-function 'org-html-html5-p)
               (lambda (&rest _) (not xhtml)))
              ((symbol-function 'org-html-xhtml-p)
               (lambda (&rest _) xhtml))
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

(defconst qa--ox-html-evil-language
  (concat "en" (string 34) " onload=" (string 34) "alert(1)"))

(qa (let ((html (qa--ox-html-template-language qa--ox-html-evil-language nil)))
      (and (string-match-p
            (regexp-quote "lang=\"en&quot; onload=&quot;alert(1)\"") html)
           (not (string-match-p " onload=\"alert(1)\"" html))))
    t)

(qa (let ((html (qa--ox-html-template-language qa--ox-html-evil-language t)))
      (and (string-match-p
            (regexp-quote "lang=\"en&quot; onload=&quot;alert(1)\"") html)
           (string-match-p
            (regexp-quote "xml:lang=\"en&quot; onload=&quot;alert(1)\"") html)
           (not (string-match-p " onload=\"alert(1)\"" html))))
    t)

(princ (qa--done))
