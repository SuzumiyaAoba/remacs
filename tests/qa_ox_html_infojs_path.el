;;; qa_ox_html_infojs_path.el --- escape the Org InfoJS script path -*- lexical-binding: t; -*-

(require 'cl-lib)
(defvar org-html-infojs-opts-table)

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
            org-html-infojs-opts-table
            org-html-infojs-install-script))
  (require 'ox-html))

(defconst qa--ox-html-evil-infojs-path
  (concat "x" (string 34) "</script><script>alert(1)</script><script"))

(defun qa--ox-html-infojs-head-extra (path)
  (let* ((org-html-infojs-opts-table
          '((path PATH "https://orgmode.org/org-info.js")
            (view VIEW "info")))
         (exp-plist
          (list :export-options nil
                :html-use-infojs 'when-configured
                :infojs-opt (concat "path:" path)
                :html-infojs-template
                "<script src=\"%SCRIPT_PATH\"></script>\n<script>%MANAGER_OPTIONS</script>"
                :html-infojs-options '((path . "safe.js") (view . "info"))
                :with-toc 1
                :headline-levels 3
                :html-head-extra "")))
    (plist-get (org-html-infojs-install-script exp-plist 'html)
               :html-head-extra)))

(if qa--remacs
    (qa (let ((extra (qa--ox-html-infojs-head-extra
                      qa--ox-html-evil-infojs-path)))
          (and (string-match-p
                (regexp-quote
                 "src=\"x&quot;&lt;/script&gt;&lt;script&gt;alert(1)&lt;/script&gt;&lt;script\"")
                extra)
               (not (string-match-p
                     "</script><script>alert(1)</script><script"
                     extra))))
        t)
  (qa (let ((extra (qa--ox-html-infojs-head-extra
                    qa--ox-html-evil-infojs-path)))
        (and (string-match-p
              (regexp-quote
               "src=\"x\"</script><script>alert(1)</script><script\"")
              extra)
             t))
      t))

(princ (qa--done))
