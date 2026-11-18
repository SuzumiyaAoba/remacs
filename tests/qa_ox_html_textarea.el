;;; qa_ox_html_textarea.el --- escape Org HTML textarea content and attributes -*- lexical-binding: t; -*-

(require 'cl-lib)

(dolist (function '(org-export-unravel-code org-export-read-attribute
                    org-count-lines))
  (unless (fboundp function)
    (fset function (lambda (&rest _) nil))))

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
        org-html--textarea-block))

(defun qa--ox-html-textarea-output (code width height)
  (cl-letf (((symbol-function 'org-export-unravel-code)
             (lambda (_element) (list code)))
            ((symbol-function 'org-export-read-attribute)
             (lambda (&rest _) (list :width width :height height)))
            ((symbol-function 'org-count-lines) (lambda (&rest _) 1)))
    (org-html--textarea-block 'element)))

(qa (qa--ox-html-textarea-output "plain text" nil nil)
    "<p>\n<textarea cols=\"80\" rows=\"1\">\nplain text</textarea>\n</p>")

(qa (qa--ox-html-textarea-output
     "</textarea><script>alert(1)</script>" nil nil)
    "<p>\n<textarea cols=\"80\" rows=\"1\">\n&lt;/textarea&gt;&lt;script&gt;alert(1)&lt;/script&gt;</textarea>\n</p>")

(qa (qa--ox-html-textarea-output
     "text" (concat "80" (string 34) " onfocus=" (string 34) "alert(1)") nil)
    "<p>\n<textarea cols=\"80&quot; onfocus=&quot;alert(1)\" rows=\"1\">\ntext</textarea>\n</p>")

(qa (qa--ox-html-textarea-output
     "text" nil (concat "24" (string 34) " onfocus=" (string 34) "alert(1)"))
    "<p>\n<textarea cols=\"80\" rows=\"24&quot; onfocus=&quot;alert(1)\">\ntext</textarea>\n</p>")

(princ (qa--done))
