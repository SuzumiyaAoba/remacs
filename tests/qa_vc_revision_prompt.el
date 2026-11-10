;;; qa_vc_revision_prompt.el --- VC revision deletion prompt regression -*- lexical-binding:t -*-

(require 'cl-lib)

(defun qa-vc-load-remove-revisions-function ()
  "Evaluate the current checkout's `vc--remove-revisions-from-end'."
  (let ((file (expand-file-name
               "../lisp/vc.el"
               (file-name-directory load-file-name)))
        found)
    (with-temp-buffer
      (insert-file-contents file)
      (goto-char (point-min))
      (while (and (not found) (< (point) (point-max)))
        (let ((form (read (current-buffer))))
          (when (and (eq (car-safe form) 'defun)
                     (eq (cadr form) 'vc--remove-revisions-from-end))
            (eval form t)
            (setq found t)))))
    (unless found
      (error "Can't find vc--remove-revisions-from-end"))
    'loaded))

(qa (qa-vc-load-remove-revisions-function) 'loaded)

(defun qa-vc-revision-delete-prompt ()
  "Return the confirmation shown for an unpublished working revision."
  (let (prompt)
    (cl-letf (((symbol-function 'vc-call-backend)
               (lambda (_backend function &rest _arguments)
                 (cond
                  ((eq function 'revision-granularity) 'repository)
                  ((eq function 'working-revision-symbol) 'HEAD)
                  ((eq function 'revision-published-p) nil)
                  ((eq function 'uncommit-revisions-from-end) 'done)
                  (t (error "Unexpected backend operation: %S" function)))))
              ((symbol-function 'vc-find-backend-function)
               (lambda (_backend _function) t))
              ((symbol-function 'yes-or-no-p)
               (lambda (question)
                 (setq prompt question)
                 t)))
      (vc--remove-revisions-from-end "v1" nil nil 'qa-backend))
    prompt))

(qa
 (qa-vc-revision-delete-prompt)
 (concat "Delete revisions newer than v1? "
         "Published revisions are only removed locally; "
         "unpublished revisions may be lost permanently."))

(princ (qa--done))
