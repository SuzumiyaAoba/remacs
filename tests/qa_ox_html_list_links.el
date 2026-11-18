;;; qa_ox_html_list_links.el --- escape Org HTML listing and table links -*- lexical-binding: t; -*-

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

(if qa--remacs
    (mapc #'qa--ox-html-load-definition
          '(org-html-protect-char-alist
            org-html-encode-plain-text
            org-html--make-attribute-string
            org-html-list-of-listings
            org-html-list-of-tables))
  (require 'ox-html))

(dolist (function '(org-export-collect-listings org-export-collect-tables
                    org-export-get-caption org-export-data org-trim
                    org-html--translate org-html--reference))
  (unless (fboundp function)
    (fset function (lambda (&rest _) nil))))

(defconst qa--ox-html-evil-list-label
  (concat "bad" (string 34) " onmouseover=" (string 34) "alert(1)"))

(defun qa--ox-html-listing-output (label)
  (cl-letf (((symbol-function 'org-export-collect-listings)
             (lambda (&rest _) '(listing)))
            ((symbol-function 'org-export-get-caption)
             (lambda (&rest _) "Code"))
            ((symbol-function 'org-export-data) (lambda (value &rest _) value))
            ((symbol-function 'org-trim) (lambda (value) value))
            ((symbol-function 'org-html--translate)
             (lambda (value &rest _) value))
            ((symbol-function 'org-html--reference) (lambda (&rest _) label)))
    (org-html-list-of-listings '(:html-toplevel-hlevel 2))))

(defun qa--ox-html-table-output (label)
  (cl-letf (((symbol-function 'org-export-collect-tables)
             (lambda (&rest _) '(table)))
            ((symbol-function 'org-export-get-caption)
             (lambda (&rest _) "Table"))
            ((symbol-function 'org-export-data) (lambda (value &rest _) value))
            ((symbol-function 'org-trim) (lambda (value) value))
            ((symbol-function 'org-html--translate)
             (lambda (value &rest _) value))
            ((symbol-function 'org-html--reference) (lambda (&rest _) label)))
    (org-html-list-of-tables '(:html-toplevel-hlevel 2))))

(defun qa--ox-html-list-link-safe-p (html)
  (if qa--remacs
      (and (string-match-p
            (regexp-quote "href=\"#bad&quot; onmouseover=&quot;alert(1)\"") html)
           (not (string-match-p " onmouseover=\"alert(1)\"" html)))
    (and (string-match-p
          (regexp-quote "href=\"#bad\" onmouseover=\"alert(1)\"") html)
         t)))

(qa (qa--ox-html-list-link-safe-p
     (qa--ox-html-listing-output qa--ox-html-evil-list-label))
    t)

(qa (qa--ox-html-list-link-safe-p
     (qa--ox-html-table-output qa--ox-html-evil-list-label))
    t)

(princ (qa--done))
