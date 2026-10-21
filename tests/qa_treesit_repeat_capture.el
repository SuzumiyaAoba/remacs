;;; -*- lexical-binding: t; -*-
(defvar treesit-extra-load-path nil)
(setq treesit-extra-load-path
      (cons (expand-file-name "~/.emacs.d/tree-sitter") treesit-extra-load-path))

(defun qa215-describe (captures node-only grouped)
  (mapcar (lambda (capture)
            (if grouped
                (qa215-describe capture node-only nil)
              (if node-only
                  (treesit-node-text capture t)
                (list (car capture) (treesit-node-text (cdr capture) t)))))
          captures))

(defun qa215-run (text query &optional node-only grouped)
  (with-temp-buffer
    (insert text)
    (let ((parser (treesit-parser-create 'json)))
      (unwind-protect
          (qa215-describe (treesit-query-capture parser query nil nil node-only grouped)
                         node-only grouped)
        (treesit-parser-delete parser)))))

(defun qa215-eq (text value &optional reversed)
  (qa215-run text
             (format "((array (number) @n (number) @n) (#eq? %s))"
                     (if reversed (format "%S @n" value) (format "@n %S" value)))))

(defun qa215-last-is-two (node) (equal (treesit-node-text node t) "2"))
(defun qa215-last-is-one (node) (equal (treesit-node-text node t) "1"))

(if (and (fboundp 'treesit-language-available-p) (treesit-language-available-p 'json))
    (progn
      (require 'treesit)
      ;; Original audit and its reverse distinguish GNU's last-capture
      ;; lookup from both first-capture lookup and an all-captures test.
      (qa (qa215-eq "[1,2]" "1") nil)
      (qa (qa215-eq "[2,1]" "1") '((n "2") (n "1")))
      (qa (qa215-eq "[1,2]" "2") '((n "1") (n "2")))
      (qa (qa215-eq "[2,1]" "2") nil)
      (qa (qa215-eq "[1,1]" "1") '((n "1") (n "1")))
      (qa (qa215-eq "[2,2]" "1") nil)
      (qa (qa215-eq "[2,1]" "1" t) '((n "2") (n "1")))
      (qa (qa215-eq "[1,2]" "1" t) nil)
      (qa (qa215-run "[1,2]" "((array (number) @n (number) @n) (#eq? @n @n))")
          '((n "1") (n "2")))
      (qa (qa215-run "[[1,2],[1,2]]"
                    "((array (array (number) @n (number) @n) (array (number) @m (number) @m)) (#eq? @n @m))")
          '((n "1") (n "2") (m "1") (m "2")))
      (qa (qa215-run "[[1,2],[2,1]]"
                    "((array (array (number) @n (number) @n) (array (number) @m (number) @m)) (#eq? @n @m))") nil)
      (qa (qa215-run "[[1,1],[1]]"
                    "((array (array (number) @n (number) @n) (array (number) @m)) (#eq? @n @m))")
          '((n "1") (n "1") (m "1")))
      (qa (qa215-run "[[1,2],[1]]"
                    "((array (array (number) @n (number) @n) (array (number) @m)) (#eq? @n @m))") nil)
      ;; Each match has its own capture range; passing groups retain
      ;; their original node order even though lookup runs in reverse.
      (qa (qa215-run "[[2,1],[3,1],[1,2]]"
                    "((array (number) @n (number) @n) (#eq? @n \"1\"))")
          '((n "2") (n "1") (n "3") (n "1")))
      (qa (qa215-run "[2,1]" "((array (number) @n (number) @n) (#eq? @n \"1\"))" nil t)
          '(((n "2") (n "1"))))
      (qa (qa215-run "[2,1]" "((array (number) @n (number) @n) (#eq? @n \"1\"))" t nil)
          '("2" "1"))
      (qa (qa215-run "[2,1]" "((array (number) @n (number) @n) (#eq? @n \"1\"))" t t)
          '(("2" "1")))
      (qa (qa215-run "[2,1]" '(((array (number) @n (number) @n) (:equal @n "1"))))
          '((n "2") (n "1")))
      (qa (qa215-run "[2,1]"
                    (treesit-query-compile 'json "((array (number) @n (number) @n) (#eq? @n \"1\"))"))
          '((n "2") (n "1")))
      (qa (qa215-run "[\"あ\",\"い\"]"
                    "((array (string) @n (string) @n) (#eq? @n \"\\\"い\\\"\"))")
          '((n "\"あ\"") (n "\"い\"")))
      (qa (qa215-run "[\"い\",\"あ\"]"
                    "((array (string) @n (string) @n) (#eq? @n \"\\\"い\\\"\"))") nil)
      ;; All predicate kinds use the same capture-name resolution.
      (qa (qa215-run "[1,2]" "((array (number) @n (number) @n) (#match? @n \"^2$\"))")
          '((n "1") (n "2")))
      (qa (qa215-run "[1,2]" "((array (number) @n (number) @n) (#match? @n \"^1$\"))") nil)
      (qa (qa215-run "[1,2]" "((array (number) @n (number) @n) (#pred? \"qa215-last-is-two\" @n))")
          '((n "1") (n "2")))
      (qa (qa215-run "[1,2]" "((array (number) @n (number) @n) (#pred? \"qa215-last-is-one\" @n))") nil)
      ;; A capture from another pattern cannot satisfy a missing name.
      (qa (condition-case err
              (qa215-run "[1,2]" "((number) @other) ((number) @n (#eq? @other \"1\"))")
            (error (list (car err) (cadr err) (nth 2 err))))
          '(treesit-query-error "Cannot find captured node" other)))
  (princ "SKIP qa_treesit_repeat_capture: JSON grammar unavailable\n"))
(princ (qa--done)) (terpri)
