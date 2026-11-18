;;; qa_ox_html_figure_label.el --- escape HTML figure label attributes -*- lexical-binding: t; -*-

(require 'cl-lib)

(unless (fboundp 'org-string-nw-p)
  (defalias 'org-string-nw-p
    (lambda (value) (and (stringp value) (> (length value) 0) value))))
(unless (fboundp 'org-html--html5-fancy-p)
  (defalias 'org-html--html5-fancy-p (lambda (&rest _) nil)))

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
        org-html--wrap-image))

(defconst qa--ox-html-evil-label
  (concat "pic" (string 34) " onload=" (string 34) "alert(1)"))

(defun qa--ox-html-wrap-image (label fancy)
  (cl-letf (((symbol-function 'org-html--html5-fancy-p)
             (lambda (&rest _) fancy))
            ((symbol-function 'org-string-nw-p)
             (lambda (value) (and (stringp value) (> (length value) 0) value))))
    (org-html--wrap-image "image" nil nil label)))

(qa (qa--ox-html-wrap-image "picture" t)
    "\n<figure id=\"picture\">\nimage\n</figure>")

(qa (qa--ox-html-wrap-image qa--ox-html-evil-label t)
    "\n<figure id=\"pic&quot; onload=&quot;alert(1)\">\nimage\n</figure>")

(qa (qa--ox-html-wrap-image qa--ox-html-evil-label nil)
    "\n<div id=\"pic&quot; onload=&quot;alert(1)\" class=\"figure\">\n<p>image</p>\n</div>")

(qa (qa--ox-html-wrap-image nil t)
    "\n<figure>\nimage\n</figure>")

(princ (qa--done))
