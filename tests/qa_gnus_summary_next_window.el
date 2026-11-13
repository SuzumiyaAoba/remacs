;;; qa_gnus_summary_next_window.el --- Missing summary window regression -*- lexical-binding:t; -*-

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

(defvar gnus-group-buffer)
(defvar gnus-newsgroup-name)
(defvar gnus-auto-select-next)
(defvar gnus-auto-extend-newsgroup)
(defvar gnus-keep-same-level)
(defvar qa-gnus-summary-walked nil)

(defun gnus-summary-search-forward (&rest _args)
  nil)

(defun gnus-ephemeral-group-p (&rest _args)
  t)

(defun gnus-summary-search-group (&rest _args)
  nil)

(defun gnus-summary-walk-group-buffer (&rest _args)
  (setq qa-gnus-summary-walked t))

(qa-gnus-sum-eval-source-defun 'gnus-summary-next-article)

(qa
 (with-temp-buffer
   (setq major-mode 'gnus-summary-mode)
   (let ((gnus-group-buffer (current-buffer))
         (gnus-newsgroup-name "qa-group")
         (gnus-auto-select-next t)
         (gnus-auto-extend-newsgroup nil)
         (gnus-keep-same-level nil)
         (last-command-event ?n)
         (last-input-event ?n)
         (qa-gnus-summary-walked nil))
     ;; The temporary summary buffer has no window in the selected frame.
     (gnus-summary-next-article)
     qa-gnus-summary-walked))
 t)

(princ (qa--done))
