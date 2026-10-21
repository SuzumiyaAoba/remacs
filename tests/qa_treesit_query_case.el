;;; -*- lexical-binding: t; -*-
(defvar treesit-extra-load-path nil)
(setq treesit-extra-load-path
      (cons (expand-file-name "~/.emacs.d/tree-sitter") treesit-extra-load-path))

(defun qa214-query (regexp kind)
  (let ((source (format "((string) @s (#match? @s %S))" regexp)))
    (cond ((eq kind 'lisp) `(((string) @s (:match ,regexp @s))))
          ((eq kind 'reversed) (format "((string) @s (#match? %S @s))" regexp))
          ((eq kind 'compiled) (treesit-query-compile 'json source))
          (t source))))

(defun qa214-texts (parser query)
  (mapcar (lambda (capture) (treesit-node-text (cdr capture) t))
          (treesit-query-capture parser query)))

(defun qa214-case (fold regexp kind)
  (with-temp-buffer
    (insert "[\"A\",\"a\",\"É\",\"é\"]")
    (setq-local case-fold-search fold)
    (let ((parser (treesit-parser-create 'json)))
      (unwind-protect
          (list (qa214-texts parser (qa214-query regexp kind)) case-fold-search)
        (treesit-parser-delete parser)))))

(defun qa214-binding (invalid)
  (with-temp-buffer
    (insert "[\"A\",\"a\"]")
    (let ((parser (treesit-parser-create 'json)) (case-fold-search t))
      (unwind-protect
          (progn
            (treesit-parser-root-node parser)
            (let ((result (condition-case nil
                              (qa214-texts parser (qa214-query (if invalid "[" "A") 'string))
                            (error 'error))))
              (list result case-fold-search)))
        (treesit-parser-delete parser)))))

(defun qa214-cross-buffer ()
  (with-temp-buffer
    (insert "[\"A\",\"a\"]")
    (setq-local case-fold-search t)
    (let ((owner (current-buffer)) (parser (treesit-parser-create 'json)))
      (unwind-protect
          (with-temp-buffer
            (setq-local case-fold-search nil)
            (list (qa214-texts parser (qa214-query "A" 'string))
                  case-fold-search
                  (buffer-local-value 'case-fold-search owner)))
        (treesit-parser-delete parser)))))

(if (and (fboundp 'treesit-language-available-p) (treesit-language-available-p 'json))
    (progn
      (require 'treesit)
      ;; The query predicate ignores both ordinary and non-nil search flags.
      (qa (qa214-case t "A" 'string) '(("\"A\"") t))
      (qa (qa214-case nil "A" 'string) '(("\"A\"") nil))
      (qa (qa214-case 42 "A" 'string) '(("\"A\"") 42))
      (qa (qa214-case t "A" 'lisp) '(("\"A\"") t))
      (qa (qa214-case nil "A" 'lisp) '(("\"A\"") nil))
      (qa (qa214-case 42 "A" 'lisp) '(("\"A\"") 42))
      (qa (qa214-case t "A" 'compiled) '(("\"A\"") t))
      (qa (qa214-case nil "A" 'compiled) '(("\"A\"") nil))
      (qa (qa214-case 42 "A" 'compiled) '(("\"A\"") 42))
      (qa (qa214-case t "A" 'reversed) '(("\"A\"") t))
      (qa (qa214-case nil "A" 'reversed) '(("\"A\"") nil))
      (qa (qa214-case 42 "A" 'reversed) '(("\"A\"") 42))
      (qa (qa214-case t "a" 'string) '(("\"a\"") t))
      (qa (qa214-case t "a" 'lisp) '(("\"a\"") t))
      (qa (qa214-case t "a" 'compiled) '(("\"a\"") t))
      (qa (qa214-case t "[Aa]" 'string) '(("\"A\"" "\"a\"") t))
      (qa (qa214-case t "[Aa]" 'compiled) '(("\"A\"" "\"a\"") t))
      (qa (qa214-case t "É" 'string) '(("\"É\"") t))
      (qa (qa214-case t "é" 'string) '(("\"é\"") t))
      (qa (qa214-case t "É" 'lisp) '(("\"É\"") t))
      (qa (qa214-case t "é" 'compiled) '(("\"é\"") t))
      (qa (qa214-case t "Z" 'compiled) '(nil t))
      (qa (qa214-binding nil) '(("\"A\"") t))
      (qa (qa214-binding t) '(error t))
      (qa (qa214-cross-buffer) '(("\"A\"") nil t))
      ;; A compiled query must use the same matching rule after the
      ;; caller changes its dynamic search setting between executions.
      (qa (with-temp-buffer
            (insert "[\"A\",\"a\"]")
            (let ((parser (treesit-parser-create 'json))
                  (query (qa214-query "A" 'compiled)))
              (unwind-protect
                  (list (let ((case-fold-search t)) (qa214-texts parser query))
                        (let ((case-fold-search nil)) (qa214-texts parser query)))
                (treesit-parser-delete parser))))
          '(("\"A\"") ("\"A\""))))
  (princ "SKIP qa_treesit_query_case: JSON grammar unavailable\n"))
(princ (qa--done)) (terpri)
