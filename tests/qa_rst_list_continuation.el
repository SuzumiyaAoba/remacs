;;; -*- lexical-binding: t; -*-
;; rst-insert-list continues multiline list items (#2005).

(require 'cl-lib)

;; Evaluate only the source defuns under test because the full rst.el load pulls
;; in optional editor integrations that fail in the headless QA interpreter.
(defconst qa-rst-source-file
  (expand-file-name "../lisp/rst.el" (file-name-directory load-file-name)))

(defvar rst-re-alist)
(setq rst-re-alist
      '((lin-end "^[ \t]*$")
        (itmany-beg-1 "^[ \t]*\\([*+-]\\|[0-9]+\\.\\)[ \t]+")
        (cntexp-tag "[0-9]+\\|[A-Za-z]+\\|[IVXLCDMivxlcdm]+")
        (num-tag "[0-9]+")
        (rom-tag "[IVXLCDMivxlcdm]+")
        (ltr-tag "[A-Za-z]")))

(defun qa-rst-eval-source-defun (function)
  "Evaluate FUNCTION from the checkout rst.el source."
  (with-temp-buffer
    (insert-file-contents qa-rst-source-file)
    (goto-char (point-min))
    (unless (re-search-forward
             (concat "^[(]defun " (regexp-quote (symbol-name function)) "\\_>")
             nil t)
      (error "No source definition for %s" function))
    (goto-char (match-beginning 0))
    (eval (read (current-buffer)))))

(defmacro 1value (form) form)
(defvar qa-rst-prompted nil)
(defun rst-insert-list-new-item ()
  (setq qa-rst-prompted t))

(dolist (function '(rst-re rst-forward-line-strict rst-insert-list-continue
                    rst--list-item-context-for-continuation rst-insert-list))
  (qa-rst-eval-source-defun function))

(defun qa-rst-insert-list-on-continuation (text)
  (with-temp-buffer
    (insert text)
    (goto-char (point-max))
    (forward-line -1)
    (end-of-line)
    (let ((qa-rst-prompted nil))
      (rst-insert-list)
      (list qa-rst-prompted (buffer-string)))))

(qa
 (list
  (qa-rst-insert-list-on-continuation
   "    * first item\n      wrapped text\n")
  (qa-rst-insert-list-on-continuation
   "  1. first item\n     wrapped text\n")
  (qa-rst-insert-list-on-continuation
   "  * parent\n    * child\n      wrapped text\n"))
 '((nil "    * first item\n      wrapped text\n    * \n")
   (nil "  1. first item\n     wrapped text\n  2. \n")
   (nil "  * parent\n    * child\n      wrapped text\n    * \n")))

(princ (qa--done))
