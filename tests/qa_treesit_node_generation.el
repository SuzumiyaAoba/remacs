;;; -*- lexical-binding: t; -*-
(defvar treesit-extra-load-path nil)
(setq treesit-extra-load-path
      (cons (expand-file-name "~/.emacs.d/tree-sitter") treesit-extra-load-path))

(defvar qa211-results nil)

(defun qa211-eight ()
  (with-temp-buffer
    (insert "[1,2]")
    (let* ((parser (treesit-parser-create 'json))
           (old (treesit-parser-root-node parser))
           (child (treesit-node-child old 0)) roots children)
      (unwind-protect
          (progn
            (dotimes (_ 8)
              (goto-char (point-max)) (insert " ")
              (treesit-parser-root-node parser)
              (push (treesit-node-check old 'outdated) roots)
              (push (treesit-node-check child 'outdated) children))
            (list (nreverse roots) (nreverse children)))
        (treesit-parser-delete parser)))))

(defun qa211-error (function node)
  (condition-case err
      (list 'unexpected-success (funcall function node))
    (error (list (car err) (eq (cadr err) node)))))

(defun qa211-details ()
  (with-temp-buffer
    (insert "[1,2]")
    (let* ((parser (treesit-parser-create 'json))
           (old (treesit-parser-root-node parser))
           (child (treesit-node-child old 0))
           (initial (treesit-node-check old 'outdated)))
      (unwind-protect
          (progn
            (goto-char (point-max)) (insert " ")
            (let ((passive (treesit-node-check old 'outdated))
                  (current (treesit-parser-root-node parser)))
              (list initial passive
                    (treesit-node-check old 'outdated)
                    (treesit-node-check current 'outdated)
                    (treesit-node-p old)
                    (treesit-node-eq old old)
                    (equal old old)
                    (treesit-node-eq old current)
                    (equal old current)
                    (treesit-node-eq current (treesit-parser-root-node parser))
                    (equal current (treesit-parser-root-node parser))
                    (eq current (treesit-parser-root-node parser))
                    (equal (treesit-node-child current 0) (treesit-node-child current 0))
                    (qa211-error #'treesit-node-type old)
                    (qa211-error #'treesit-node-start old)
                    (qa211-error #'treesit-node-end child)
                    (qa211-error #'treesit-node-parent child)
                    (qa211-error (lambda (node) (treesit-node-check node 'live)) old)
                    (prin1-to-string old)
                    (prin1-to-string child)
                    ;; Parser/language resolution and capture wrapping
                    ;; must retain same-generation identity as direct nodes.
                    (equal (cdar (treesit-query-capture parser '((number) @n)))
                           (treesit-node-child (treesit-node-child current 0) 1))
                    (equal (cdar (treesit-query-capture current '((number) @n)))
                           (cdar (treesit-query-capture 'json '((number) @n)))))))
        (treesit-parser-delete parser)))))

(defun qa211-history ()
  (with-temp-buffer
    (insert "[1,2]")
    (let ((parser (treesit-parser-create 'json)) history (valid t))
      (unwind-protect
          (progn
            (dotimes (_ 64)
              (let* ((root (treesit-parser-root-node parser))
                     (array (treesit-node-child root 0)))
                (push (list root array (treesit-node-child array 1)) history))
              (goto-char (point-max)) (insert " ")
              (treesit-parser-root-node parser)
              (dolist (generation history)
                (dolist (node generation)
                  (setq valid
                        (and valid (treesit-node-check node 'outdated)
                             (equal (prin1-to-string node) "#<treesit-node-outdated>"))))))
            valid)
        (treesit-parser-delete parser)))))

(if (and (fboundp 'treesit-language-available-p) (treesit-language-available-p 'json))
    (progn
      (qa (qa211-eight) '((t t t t t t t t) (t t t t t t t t)))
      (let ((qa211-results (qa211-details)))
        (qa (nth 0 qa211-results) nil)
        (qa (nth 1 qa211-results) nil)
        (qa (nth 2 qa211-results) t)
        (qa (nth 3 qa211-results) nil)
        (qa (nth 4 qa211-results) t)
        (qa (nth 5 qa211-results) t)
        (qa (nth 6 qa211-results) t)
        (qa (nth 7 qa211-results) nil)
        (qa (nth 8 qa211-results) nil)
        (qa (nth 9 qa211-results) t)
        (qa (nth 10 qa211-results) t)
        (qa (nth 11 qa211-results) nil)
        (qa (nth 12 qa211-results) t)
        (qa (nth 13 qa211-results) '(treesit-node-outdated t))
        (qa (nth 14 qa211-results) '(treesit-node-outdated t))
        (qa (nth 15 qa211-results) '(treesit-node-outdated t))
        (qa (nth 16 qa211-results) '(treesit-node-outdated t))
        (qa (nth 17 qa211-results) '(treesit-node-outdated t))
        (qa (nth 18 qa211-results) "#<treesit-node-outdated>")
        (qa (nth 19 qa211-results) "#<treesit-node-outdated>")
        (qa (nth 20 qa211-results) t)
        (qa (nth 21 qa211-results) t))
      (qa (qa211-history) t))
  (princ "SKIP qa_treesit_node_generation: JSON grammar unavailable\n"))
(princ (qa--done)) (terpri)
