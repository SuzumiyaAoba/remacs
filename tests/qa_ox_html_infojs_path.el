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
            org-html--encode-js-string-content
            org-html--validate-infojs-option-value
            org-html-infojs-opts-table
            org-html-infojs-install-script))
  (require 'ox-html))

(defconst qa--ox-html-evil-infojs-path
  (concat "x" (string 34) "</script><script>alert(1)</script><script"))

(defun qa--ox-html-infojs-head-extra (options)
  (let* ((org-html-infojs-opts-table
          '((path PATH "https://orgmode.org/org-info.js")
            (view VIEW "info")
            (mouse MOUSE_HINT "underline")
            (up LINK_UP :html-link-up)
            (home LINK_HOME :html-link-home)))
         (exp-plist
          (list :export-options nil
                :html-use-infojs 'when-configured
                :infojs-opt options
                :html-infojs-template
                "<script src=\"%SCRIPT_PATH\"></script>\n<script>%MANAGER_OPTIONS</script>"
                :html-infojs-options
                '((path . "safe.js") (view . "info") (mouse . "underline")
                  (up . :html-link-up) (home . :html-link-home))
                :with-toc 1
                :headline-levels 3
                :html-link-up "https://example.test/up"
                :html-link-home "https://example.test/home"
                :html-head-extra "")))
    (plist-get (org-html-infojs-install-script exp-plist 'html)
               :html-head-extra)))

(if qa--remacs
    (qa (let ((extra (qa--ox-html-infojs-head-extra
                      (concat "path:" qa--ox-html-evil-infojs-path))))
          (and (string-match-p
                (regexp-quote
                 "src=\"x&quot;&lt;/script&gt;&lt;script&gt;alert(1)&lt;/script&gt;&lt;script\"")
                extra)
               (not (string-match-p
                     "</script><script>alert(1)</script><script"
                     extra))))
        t)
  (qa (let ((extra (qa--ox-html-infojs-head-extra
                    (concat "path:" qa--ox-html-evil-infojs-path))))
        (and (string-match-p
              (regexp-quote
               "src=\"x\"</script><script>alert(1)</script><script\"")
              extra)
             t))
      t))

(defconst qa--ox-html-evil-infojs-option
  (concat "x" (string 34) ";alert(1);//</script><script>alert(2)</script>"))

(if qa--remacs
    (qa-err (qa--ox-html-infojs-head-extra
             (concat "view:" qa--ox-html-evil-infojs-option))
            'error)
  (qa (let ((extra (qa--ox-html-infojs-head-extra
                    (concat "view:" qa--ox-html-evil-infojs-option))))
        (and (string-match-p
              (regexp-quote
               (concat "org_html_manager.set(\"VIEW\", \"x"
                       (string 34) ";alert(1);//</script><script>alert(2)</script>\");"))
              extra)
             t))
      t))

(if qa--remacs
    (qa (let ((extra (qa--ox-html-infojs-head-extra
                     (concat "up:" qa--ox-html-evil-infojs-option))))
          (and (string-match-p
                (regexp-quote
                 (concat "org_html_manager.set(\"LINK_UP\", \"x"
                         (string 92) (string 34) ";alert(1);//"
                         (string 92) "x3C/script>"
                         (string 92) "x3Cscript>alert(2)"
                         (string 92) "x3C/script>\");"))
                extra)
               (not (string-match-p
                     "</script><script>alert(2)</script>" extra))))
        t)
  (qa (let ((extra (qa--ox-html-infojs-head-extra
                    (concat "up:" qa--ox-html-evil-infojs-option))))
        (and (string-match-p
              (regexp-quote
               (concat "org_html_manager.set(\"LINK_UP\", \"x"
                       (string 34) ";alert(1);//</script><script>alert(2)</script>\");"))
              extra)
             t))
      t))

(princ (qa--done))
