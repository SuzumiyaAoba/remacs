;;; qa_gnus_encoded_header_search.el --- Decode Gnus headers before matching -*- lexical-binding:t; -*-

(defconst qa-gnus-sum-source-file
  (expand-file-name "../lisp/gnus-sum.el" (file-name-directory load-file-name)))

(defun qa-gnus-sum-eval-source-defun (function)
  "Evaluate FUNCTION from the checkout gnus-sum.el source."
  (with-temp-buffer
    (insert-file-contents qa-gnus-sum-source-file)
    (goto-char (point-min))
    (unless (re-search-forward
             (concat "^[(]defun " (regexp-quote (symbol-name function)) "\\_>")
             nil t)
      (error "No source definition for %s" function))
    (goto-char (match-beginning 0))
    (eval (read (current-buffer)))))

(defconst qa-gnus-encoded-subject "=?UTF-8?Q?caf=C3=A9?=")
(defvar gnus-newsgroup-headers nil)
(defvar gnus-decode-encoded-word-function nil)

(defun mail-header-subject (header)
  (car header))

(defun mail-header-number (header)
  (cdr header))

(qa-gnus-sum-eval-source-defun 'gnus-find-matching-articles)

(qa
 (let ((gnus-newsgroup-headers
        (list (cons qa-gnus-encoded-subject 1)
              (cons "tea" 2)))
       (gnus-decode-encoded-word-function
        (lambda (header)
          (if (equal header qa-gnus-encoded-subject)
              "café"
            header))))
   (gnus-find-matching-articles "subject" "café"))
 '(1))

(princ (qa--done))
