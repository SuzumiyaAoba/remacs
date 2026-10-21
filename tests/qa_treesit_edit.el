;;; -*- lexical-binding: t; -*-
(defvar treesit-extra-load-path nil)
(setq treesit-extra-load-path
      (cons (expand-file-name "~/.emacs.d/tree-sitter") treesit-extra-load-path))

(defun qa210-shape (node)
  (let (children)
    (dotimes (index (treesit-node-child-count node))
      (push (qa210-shape (treesit-node-child node index)) children))
    (list (treesit-node-type node) (treesit-node-start node) (treesit-node-end node)
          (not (null (treesit-node-check node 'has-error)))
          (buffer-substring-no-properties (treesit-node-start node) (treesit-node-end node))
          (nreverse children))))

(defun qa210-fresh-equal (parser)
  (let ((incremental (qa210-shape (treesit-parser-root-node parser)))
        (fresh (treesit-parser-create 'json nil t)))
    (unwind-protect
        (equal incremental (qa210-shape (treesit-parser-root-node fresh)))
      (treesit-parser-delete fresh))))

(defun qa210-case (initial edit)
  (with-temp-buffer
    (insert initial)
    (let ((parser (treesit-parser-create 'json)))
      (unwind-protect
          (progn
            (treesit-parser-root-node parser)
            (funcall edit)
            (qa210-fresh-equal parser))
        (treesit-parser-delete parser)))))

(defun qa210-original (replacement)
  (with-temp-buffer
    (insert "[1,2]")
    (let ((parser (treesit-parser-create 'json)))
      (unwind-protect
          (progn
            (treesit-parser-root-node parser)
            (goto-char 2) (delete-char 1) (insert replacement)
            (let ((root (treesit-parser-root-node parser)))
              (list (treesit-node-check root 'has-error) (treesit-node-string root))))
        (treesit-parser-delete parser)))))

(defvar qa210-notified nil)
(defun qa210-notify (ranges _parser) (push ranges qa210-notified))
(defun qa210-notification ()
  (with-temp-buffer
    (insert "[1,2,3,4,5]")
    (let ((parser (treesit-parser-create 'json)) (qa210-notified nil))
      (unwind-protect
          (progn
            (treesit-parser-root-node parser)
            (treesit-parser-add-notifier parser 'qa210-notify)
            (goto-char 2) (delete-char 1) (insert "null")
            (goto-char 13) (delete-char 1) (insert "false")
            (list (treesit-node-string (treesit-parser-root-node parser)) qa210-notified))
        (treesit-parser-delete parser)))))

(if (and (fboundp 'treesit-language-available-p) (treesit-language-available-p 'json))
    (progn
      ;; The two original audit cases must change the grammar node types.
      (qa (qa210-original "x")
          '(t "(document (array (ERROR (UNEXPECTED 'x')) (number)))"))
      (qa (qa210-original "null") '(nil "(document (array (null) (number)))"))
      ;; Compare every node's type, bounds, error state and text against
      ;; a separate parser created from scratch after each edit.
      (qa (qa210-case "[1,2]" (lambda () (goto-char 2) (delete-char 1) (insert "true"))) t)
      (qa (qa210-case "[null,2]" (lambda () (delete-region 2 6) (goto-char 2) (insert "0"))) t)
      (qa (qa210-case "[1,2]" (lambda () (goto-char 2) (insert "[3],"))) t)
      (qa (qa210-case "[1,2]" (lambda () (delete-region 2 4))) t)
      (qa (qa210-case "[1,2]" (lambda () (erase-buffer) (insert "{\"a\":false}"))) t)
      (qa (qa210-case "[1,2]" (lambda () (erase-buffer))) t)
      (qa (qa210-case "" (lambda () (insert "[null]"))) t)
      ;; The equal prefix can end inside an encoded character if it is
      ;; measured byte-by-byte. These replacements must stay char-aligned.
      (qa (qa210-case "[\"あ\",2]" (lambda () (goto-char 3) (delete-char 1) (insert "い"))) t)
      (qa (qa210-case "[\"é\",2]" (lambda () (goto-char 3) (delete-char 1) (insert "あβ"))) t)
      (qa (qa210-case "[\"😀\",2]" (lambda () (goto-char 3) (delete-char 1) (insert "a"))) t)
      (qa (qa210-case "[\"あβ\",2]" (lambda () (delete-region 2 6) (goto-char 2) (insert "false"))) t)
      ;; Points use byte columns and must reset at newlines.
      (qa (qa210-case "{\n  \"あ\":1\n}" (lambda () (goto-char 9) (delete-char 1) (insert "null"))) t)
      (qa (qa210-case "[1,2]" (lambda () (goto-char 1) (insert "\n  "))) t)
      (qa (qa210-case "[\n  1,\n  2\n]" (lambda () (goto-char 3) (insert "\n\n"))) t)
      (qa (qa210-case "[\n1\n]" (lambda () (delete-region 2 3) (delete-region 3 4))) t)
      ;; Several buffer edits can happen before any parser is requested.
      (qa (qa210-case "[1,2,3]" (lambda ()
                                (goto-char 2) (delete-char 1) (insert "null")
                                (goto-char 7) (delete-char 1) (insert "false"))) t)
      (qa (qa210-case "[1,2]" (lambda () (goto-char 2) (delete-char 1) (insert "1"))) t)
      (qa (qa210-case "[1,2]" (lambda () (subst-char-in-region 2 3 ?1 ?x))) t)
      (qa (qa210-case "[\"é\",2]" (lambda () (subst-char-in-region 3 4 ?é ?β))) t)
      ;; Transposition is one overwrite even when the swapped pieces have
      ;; different character counts and UTF-8 byte widths.
      (qa (qa210-case "[1,\"あ\"]" (lambda () (transpose-regions 2 3 4 7))) t)
      (qa-lex
       (with-temp-buffer
         (insert "xxx [1,\"あ\"] yyy")
         (narrow-to-region 5 12)
         (let ((parser (treesit-parser-create 'json)))
           (unwind-protect
               (progn
                 (treesit-parser-root-node parser)
                 (transpose-regions 6 7 8 11 t)
                 (qa210-fresh-equal parser))
             (treesit-parser-delete parser))))
       t)
      (qa-lex
       (with-temp-buffer
         (insert "[\"あβ\"]--[1,2]")
         (let* ((base (current-buffer))
                (view (make-indirect-buffer base " *qa240-parser-view*")))
           (unwind-protect
               (with-current-buffer view
                 (narrow-to-region 2 6)
                 (let ((parser (treesit-parser-create 'json)))
                   (unwind-protect
                       (progn
                         (treesit-parser-root-node parser)
                         (with-current-buffer base (transpose-regions 1 7 9 14))
                         (qa210-fresh-equal parser))
                     (treesit-parser-delete parser))))
             (kill-buffer view))))
       t)
      (qa (qa210-notification)
          '("(document (array (null) (number) (number) (number) (false)))" (((2 . 6) (13 . 18)))))
      ;; Narrowing changes the parser's visible input and global offsets.
      (qa (qa210-case "xxx [1,2] yyy" (lambda () (narrow-to-region 5 10))) t)
      (qa (qa210-case "xxx [1,2] yyy" (lambda ()
                                      (narrow-to-region 5 10)
                                      (goto-char 6) (delete-char 1) (insert "null"))) t)
      (qa-lex
       (with-temp-buffer
         (insert "[1,2]")
         (let ((parser (treesit-parser-create 'json)) (matches t))
           (unwind-protect
               (progn
                 (treesit-parser-root-node parser)
                 (dolist (replacement '("null" "true" "{\"a\":1}" "\"あ\"" "4"))
                   (goto-char 2)
                   (delete-region 2 (1- (point-max)))
                   (insert replacement)
                   (setq matches (and matches (qa210-fresh-equal parser))))
                 matches)
             (treesit-parser-delete parser))))
       t))
  (princ "SKIP qa_treesit_edit: JSON grammar unavailable\n"))
(princ (qa--done)) (terpri)
