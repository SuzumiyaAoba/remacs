;;; qa_ox_html_equation_reference.el --- escape MathJax equation references -*- lexical-binding: t; -*-

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
        org-html--format-mathjax-reference))

(qa (org-html--format-mathjax-reference "\\eqref{%s}" "eq:test")
    "\\eqref{eq:test}")

(qa (org-html--format-mathjax-reference
     "<script>alert(1)</script>%s" "eq:test")
    "&lt;script&gt;alert(1)&lt;/script&gt;eq:test")

(qa (org-html--format-mathjax-reference "x&y>%s" "z")
    "x&amp;y&gt;z")

(princ (qa--done))
