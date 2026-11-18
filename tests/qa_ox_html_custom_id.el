;;; qa_ox_html_custom_id.el --- escape headline ids in HTML attributes -*- lexical-binding: t; -*-

(require 'cl-lib)

;; Load only the headline formatters and their escaping helper from ox-html.el.
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
        org-html--anchor
        org-html--format-toc-headline
        org-html-headline
        org-html-section))

(defconst qa--ox-html-evil-id
  (concat "evil" (string 34) " onmouseover=" (string 34) "alert(1)"))

(defun qa--ox-html-headline-output (id &optional self-link)
  (let ((info (list :html-toplevel-hlevel 2
                    :html-self-link-headlines self-link
                    :html-format-headline-function
                    (lambda (&rest _) "Headline"))))
    (cl-letf (((symbol-function 'org-element-property)
               (lambda (property _headline)
                 (and (eq property :title) "Headline")))
              ((symbol-function 'org-export-data) (lambda (value &rest _) value))
              ((symbol-function 'org-export-numbered-headline-p)
               (lambda (&rest _) nil))
              ((symbol-function 'org-export-get-headline-number)
               (lambda (&rest _) '(1)))
              ((symbol-function 'org-export-get-relative-level)
               (lambda (&rest _) 1))
              ((symbol-function 'org-export-low-level-p)
               (lambda (&rest _) nil))
              ((symbol-function 'org-html--reference) (lambda (&rest _) id))
              ((symbol-function 'org-html--container) (lambda (&rest _) "div"))
              ((symbol-function 'org-element-contents) (lambda (&rest _) '(section)))
              ((symbol-function 'org-element-type-p)
               (lambda (_element type) (eq type 'section)))
              ((symbol-function 'org-html-section) (lambda (&rest _) "")))
      (org-html-headline 'headline "body" info))))

(defun qa--ox-html-section-output (id)
  (cl-letf (((symbol-function 'org-element-lineage) (lambda (&rest _) 'headline))
            ((symbol-function 'org-element-property)
             (lambda (property _headline)
               (and (eq property :CUSTOM_ID) id)))
            ((symbol-function 'org-export-get-relative-level)
             (lambda (&rest _) 1))
            ((symbol-function 'org-export-numbered-headline-p)
             (lambda (&rest _) nil))
            ((symbol-function 'org-export-get-reference)
             (lambda (&rest _) "generated")))
    (org-html-section 'section "body" '(:html-toplevel-hlevel 2))))

(defun qa--ox-html-toc-output (id)
  (let ((info (list :html-format-headline-function
                    (lambda (&rest _) "Headline"))))
    (cl-letf (((symbol-function 'org-export-get-headline-number)
               (lambda (&rest _) '(1)))
              ((symbol-function 'org-export-get-alt-title)
               (lambda (&rest _) "Headline"))
              ((symbol-function 'org-export-toc-entry-backend)
               (lambda (&rest _) 'html))
              ((symbol-function 'org-export-data-with-backend)
               (lambda (value &rest _) value))
              ((symbol-function 'org-element-property)
               (lambda (&rest _) nil))
              ((symbol-function 'org-export-numbered-headline-p)
               (lambda (&rest _) nil))
              ((symbol-function 'org-export-low-level-p)
               (lambda (&rest _) nil))
              ((symbol-function 'org-html--reference) (lambda (&rest _) id)))
      (org-html--format-toc-headline 'headline info))))

(qa (let ((html (qa--ox-html-headline-output qa--ox-html-evil-id)))
      (and (string-match-p
            (regexp-quote "id=\"outline-container-evil&quot; onmouseover=&quot;alert(1)\"")
            html)
           (string-match-p
            (regexp-quote "<h2 id=\"evil&quot; onmouseover=&quot;alert(1)\">")
            html)
           (not (string-match-p " onmouseover=\"alert(1)\"" html))))
    t)

(qa (let ((html (qa--ox-html-headline-output qa--ox-html-evil-id t)))
      (and (string-match-p
            (regexp-quote "href=\"#evil&quot; onmouseover=&quot;alert(1)\"")
            html)
           (not (string-match-p " onmouseover=\"alert(1)\"" html))))
    t)

(qa (let ((html (qa--ox-html-section-output qa--ox-html-evil-id)))
      (and (string-match-p
            (regexp-quote "id=\"text-evil&quot; onmouseover=&quot;alert(1)\"")
            html)
           (not (string-match-p " onmouseover=\"alert(1)\"" html))))
    t)

(qa (let ((html (org-html--anchor qa--ox-html-evil-id "Headline" nil
                                 '(:html-allow-name-attribute-in-anchors t))))
      (and (string-match-p
            (regexp-quote "id=\"evil&quot; onmouseover=&quot;alert(1)\"")
            html)
           (string-match-p
            (regexp-quote "name=\"evil&quot; onmouseover=&quot;alert(1)\"")
            html)
           (not (string-match-p " onmouseover=\"alert(1)\"" html))))
    t)

(qa (let ((html (qa--ox-html-toc-output qa--ox-html-evil-id)))
      (and (string-match-p
            (regexp-quote "href=\"#evil&quot; onmouseover=&quot;alert(1)\"")
            html)
           (not (string-match-p " onmouseover=\"alert(1)\"" html))))
    t)

(princ (qa--done))
