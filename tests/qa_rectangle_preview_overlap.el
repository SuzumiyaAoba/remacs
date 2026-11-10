;;; qa_rectangle_preview_overlap.el --- Rectangle preview spacing regression -*- lexical-binding:t -*-

(defvar rectangle-mark-mode nil)
(defvar rectangle--inhibit-region-highlight nil)
(defvar rectangle--string-preview-state nil)
(defvar redisplay-unhighlight-region-function nil)
(defvar qa-rectangle-unhighlighted nil)

(defun qa-rectangle-unhighlight-rol (rol)
  (push rol qa-rectangle-unhighlighted))

(defun qa-rectangle-original-highlight (&rest _args)
  'original-highlight)

(defun qa-rectangle-load-highlight-function ()
  "Evaluate `rectangle--highlight-for-redisplay' from rect.el source."
  (let ((file (locate-library "rect"))
        found)
    (unless file
      (error "Can't locate rect source"))
    (with-temp-buffer
      (insert-file-contents file)
      (goto-char (point-min))
      (while (and (not found) (< (point) (point-max)))
        (let ((form (read (current-buffer))))
          (when (and (eq (car-safe form) 'defun)
                     (eq (cadr form) 'rectangle--highlight-for-redisplay))
            (eval form t)
            (setq found t)))))
    (unless found
      (error "Missing rectangle--highlight-for-redisplay definition"))
    'loaded))

(qa (qa-rectangle-load-highlight-function) 'loaded)

(qa
 (let ((rectangle-mark-mode t)
       (rectangle--inhibit-region-highlight nil)
       (rectangle--string-preview-state '(nil 1 2))
       (redisplay-unhighlight-region-function
        #'qa-rectangle-unhighlight-rol)
       (qa-rectangle-unhighlighted nil))
   (list (rectangle--highlight-for-redisplay
          #'qa-rectangle-original-highlight
          1 2 (selected-window) '(old-region))
         qa-rectangle-unhighlighted))
 '(nil ((old-region))))

(princ (qa--done))
