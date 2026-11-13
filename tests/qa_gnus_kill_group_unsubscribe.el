;;; qa_gnus_kill_group_unsubscribe.el --- Preserve unread state before killing a group -*- lexical-binding:t; -*-

(defconst qa-gnus-group-source-file
  (expand-file-name "../lisp/gnus-group.el" (file-name-directory load-file-name)))

(defun qa-gnus-group-eval-source-defun (function)
  "Evaluate FUNCTION from the checkout gnus-group.el source."
  (with-temp-buffer
    (insert-file-contents qa-gnus-group-source-file)
    (goto-char (point-min))
    (unless (re-search-forward
             (concat "^[(]defun " (regexp-quote (symbol-name function)) "\\_>")
             nil t)
      (error "No source definition for %s" function))
    (goto-char (match-beginning 0))
    (eval (read (current-buffer)))))

(defvar qa-gnus-kill-groups nil)
(defvar qa-gnus-unread nil)
(defvar qa-gnus-status-updates nil)
(defvar gnus-list-of-killed-groups nil)
(defvar gnus-level-killed 9)

(defun gnus-group-process-prefix (&rest _args)
  qa-gnus-kill-groups)

(defun gnus-group-remove-mark (&rest _args)
  nil)

(defun gnus-group-group-level ()
  3)

(defun gnus-delete-line ()
  nil)

(defun gnus-group-entry (group)
  (list group 'entry))

(defun gnus-undo-register (&rest _args)
  nil)

(defun gnus-group-group-name ()
  (car qa-gnus-kill-groups))

(defun gnus-group-change-level (&rest _args)
  (setq qa-gnus-unread nil))

(defun gnus-group-unread (&rest _args)
  qa-gnus-unread)

(defun gnus-request-update-group-status (group status)
  (push (list group status) qa-gnus-status-updates))

(defun gnus-group-position-point ()
  nil)

(qa-gnus-group-eval-source-defun 'gnus-group-kill-group)

(qa
 (with-temp-buffer
   (let ((qa-gnus-kill-groups '("group-one"))
         (qa-gnus-unread 12)
         (qa-gnus-status-updates nil)
         (gnus-list-of-killed-groups nil))
     (gnus-group-kill-group)
     qa-gnus-status-updates))
 '(("group-one" unsubscribe)))

(princ (qa--done))
