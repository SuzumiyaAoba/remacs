;;; qa_ox_html_mathjax_options.el --- validate and escape MathJax options -*- lexical-binding: t; -*-

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
            org-html--encode-attribute-value
            org-html--encode-js-string-content
            org-html--mathjax-validate-option
            org-html--mathjax-template-value
            org-html--build-mathjax-config))
  (require 'ox-html))

(dolist (function '(org-element-map org-element-normalize-string
                    org-display-warning))
  (unless (fboundp function)
    (fset function (lambda (&rest _) nil))))

(defun qa--ox-html-build-mathjax-config (template options in-buffer)
  (cl-letf (((symbol-function 'org-element-map) (lambda (&rest _) t))
            ((symbol-function 'org-element-normalize-string)
             (lambda (string) string))
            ((symbol-function 'org-display-warning) (lambda (&rest _) nil)))
    (org-html--build-mathjax-config
     (list :with-latex 'mathjax
           :parse-tree 'math
           :html-mathjax-template template
           :html-mathjax-options options
           :html-mathjax in-buffer))))

(qa (qa--ox-html-build-mathjax-config
     "<script>tags: '%TAGS';</script>"
     '((tags "ams"))
     "")
    "<script>tags: 'ams';</script>")

(if qa--remacs
    (qa-err
     (qa--ox-html-build-mathjax-config
      "<script>tags: '%TAGS';</script>"
      '((tags "ams"))
      (concat "tags: x" (string 39) ";alert(1);//"))
     'error)
  (qa (qa--ox-html-build-mathjax-config
       "<script>tags: '%TAGS';</script>"
       '((tags "ams"))
       (concat "tags: x" (string 39) ";alert(1);//"))
      "<script>tags: 'x';alert(1);//';</script>"))

(if qa--remacs
    (qa (let ((output
               (qa--ox-html-build-mathjax-config
                "<script>indent: '%INDENT';</script>"
                '((indent "0em"))
                (concat "indent: x" (string 39) ";"
                        "</script><script>alert(1)//"))))
          (and (string-match-p
                (regexp-quote
                 (concat "x" (string 92) "';" (string 92) "x3C/script>"
                         (string 92) "x3Cscript>alert(1)//"))
                output)
               (not (string-match-p "</script><script>alert(1)" output))))
        t)
  (qa (qa--ox-html-build-mathjax-config
       "<script>indent: '%INDENT';</script>"
       '((indent "0em"))
       (concat "indent: x" (string 39) ";"
               "</script><script>alert(1)//"))
      (concat "<script>indent: 'x" (string 39) ";"
              "</script><script>alert(1)//';</script>")))

(if qa--remacs
    (qa (qa--ox-html-build-mathjax-config
         "<script src=\"%PATH\"></script>"
         '((path "https://example.test/math.js"))
         "")
        "<script src=\"https://example.test/math.js\"></script>")
  (qa (qa--ox-html-build-mathjax-config
       "<script src=\"%PATH\"></script>"
       '((path "https://example.test/math.js"))
       "")
      "<script src=\"https://example.test/math.js\"></script>"))

(defconst qa--ox-html-evil-path
  (concat "https://example.test/math.js" (string 34)
          " onerror=" (string 34) "alert(1)"))

(if qa--remacs
    (qa (qa--ox-html-build-mathjax-config
         "<script src=\"%PATH\"></script>"
         (list (list 'path qa--ox-html-evil-path))
         "")
        (concat "<script src=\"https://example.test/math.js&quot;"
                " onerror=&quot;alert(1)\"></script>"))
  (qa (qa--ox-html-build-mathjax-config
       "<script src=\"%PATH\"></script>"
       (list (list 'path qa--ox-html-evil-path))
       "")
      (concat "<script src=\"https://example.test/math.js\""
              " onerror=\"alert(1)\"></script>")))

(when qa--remacs
  (qa-err (org-html--mathjax-template-value 'align "bad';alert(1)//") 'error)
  (qa-err (org-html--mathjax-template-value 'font "bad';alert(1)//") 'error)
  (qa-err (org-html--mathjax-template-value 'overflow "bad';alert(1)//") 'error)
  (qa-err (org-html--mathjax-template-value 'tags "bad';alert(1)//") 'error)
  (qa-err (org-html--mathjax-template-value 'tagside "bad';alert(1)//") 'error)
  (qa-err (org-html--mathjax-template-value 'scale "not-a-number") 'error))

(princ (qa--done))
