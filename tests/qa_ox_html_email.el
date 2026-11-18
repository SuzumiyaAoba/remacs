;;; qa_ox_html_email.el --- escape HTML postamble email links -*- lexical-binding: t; -*-

(require 'cl-lib)

;; Load only the formatter and its two escaping helpers from ox-html.el.
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
        org-html--encode-mailto-address
        org-html-format-spec))

(defun qa--ox-html-email-format (email)
  (cl-letf (((symbol-function 'org-export-data)
             (lambda (value &rest _) value))
            ((symbol-function 'org-export-get-date)
             (lambda (&rest _) nil)))
    (cdr (assq ?e
               (org-html-format-spec
                (list :email email
                      :html-metadata-timestamp-format "%Y"))))))

(qa (qa--ox-html-email-format
     (concat "x@example.com" (string 34)
             " onmouseover=" (string 34) "alert(1)"))
    "<a href=\"mailto:x@example.com%22%20onmouseover%3D%22alert(1)\">x@example.com\" onmouseover=\"alert(1)</a>")

(qa (qa--ox-html-email-format "a&b@example.com<")
    "<a href=\"mailto:a%26b@example.com%3C\">a&amp;b@example.com&lt;</a>")

(qa (qa--ox-html-email-format "person@example.com")
    "<a href=\"mailto:person@example.com\">person@example.com</a>")

(qa (qa--ox-html-email-format (concat (string 233) "@example.com"))
    (concat "<a href=\"mailto:%C3%A9@example.com\">"
            (string 233) "@example.com</a>"))

(princ (qa--done))
