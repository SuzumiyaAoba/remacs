;;; qa_gnus_undo_count.el --- Gnus undo repeat-count regression -*- lexical-binding:t -*-

(defvar gnus-undo-mode nil)
(defvar gnus-undo-actions nil)
(defvar gnus-undo-boundary nil)
(defvar gnus-undo-last nil)
(defvar qa-gnus-undo-log nil)

(defun qa-gnus-undo-one () (push 'one qa-gnus-undo-log))
(defun qa-gnus-undo-two () (push 'two qa-gnus-undo-log))
(defun qa-gnus-undo-three () (push 'three qa-gnus-undo-log))

(defun qa-gnus-undo-load-source ()
  "Evaluate `gnus-undo' from the source file."
  (let ((file (locate-library "gnus-undo"))
        found)
    (unless file
      (error "Can't locate gnus-undo source"))
    (with-temp-buffer
      (insert-file-contents file)
      (goto-char (point-min))
      (while (and (not found) (< (point) (point-max)))
        (let ((form (read (current-buffer))))
          (when (and (eq (car-safe form) 'defun)
                     (eq (cadr form) 'gnus-undo))
            (eval form t)
            (setq found t)))))
    (unless found
      (error "Missing gnus-undo definition"))
    'loaded))

(qa (qa-gnus-undo-load-source) 'loaded)

(qa
 (let ((gnus-undo-mode t)
       (gnus-undo-actions '((qa-gnus-undo-one)
                            (qa-gnus-undo-two)
                            (qa-gnus-undo-three)))
       (gnus-undo-boundary nil)
       (gnus-undo-last nil)
       (last-command 'some-other-command)
       (qa-gnus-undo-log nil))
   (gnus-undo 3)
   (list (reverse qa-gnus-undo-log)
         gnus-undo-actions
         gnus-undo-last))
 '((one two three) nil nil))

(princ (qa--done))
