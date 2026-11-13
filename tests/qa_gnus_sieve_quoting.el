;;; qa_gnus_sieve_quoting.el --- Sieve string quoting regression -*- lexical-binding:t; -*-

(defconst qa-gnus-sieve-source-file
  (expand-file-name "../lisp/gnus-sieve.el" (file-name-directory load-file-name)))

(defun qa-gnus-sieve-eval-source-defun (function)
  "Evaluate FUNCTION from the checkout gnus-sieve.el source."
  (with-temp-buffer
    (insert-file-contents qa-gnus-sieve-source-file)
    (goto-char (point-min))
    (unless (re-search-forward
             (concat "^[(]defun " (regexp-quote (symbol-name function)) "\\_>")
             nil t)
      (error "No source definition for %s" function))
    (goto-char (match-beginning 0))
    (eval (read (current-buffer)))))

(dolist (function '(gnus-sieve-quote-string gnus-sieve-string-list
                    gnus-sieve-test-token))
  (qa-gnus-sieve-eval-source-defun function))

(qa
 (list
  (gnus-sieve-test-token "from\" \"to")
  (gnus-sieve-test-token "a\\b\"c")
  (gnus-sieve-string-list '("a\"b" "c\\d")))
 '("\"from\\\" \\\"to\""
   "\"a\\\\b\\\"c\""
   "[\"a\\\"b\", \"c\\\\d\"]"))

(princ (qa--done))
