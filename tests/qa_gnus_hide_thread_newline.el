;;; qa_gnus_hide_thread_newline.el --- Hidden newline boundary regression -*- lexical-binding:t -*-

(defvar qa-gnus-next-thread-target nil)

(defun qa-gnus-next-thread-to-target (_n &optional _silent)
  (goto-char qa-gnus-next-thread-target)
  0)

(defun qa-gnus-hide-thread-load-source ()
  "Evaluate `gnus-summary-hide-thread' from the source file."
  (let ((file (locate-library "gnus-sum"))
        found)
    (unless file
      (error "Can't locate gnus-sum source"))
    (with-temp-buffer
      (insert-file-contents file)
      (goto-char (point-min))
      (while (and (not found) (< (point) (point-max)))
        (let ((form (read (current-buffer))))
          (when (and (eq (car-safe form) 'defun)
                     (eq (cadr form) 'gnus-summary-hide-thread))
            (eval form t)
            (setq found t)))))
    (unless found
      (error "Missing gnus-summary-hide-thread definition"))
    'loaded))

(defun gnus-summary-article-intangible-p () nil)
(defun gnus-summary-article-number () 1)
(defun gnus-summary-goto-subject (_article) nil)
(defun gnus-summary-position-point () nil)

(qa (qa-gnus-hide-thread-load-source) 'loaded)

(qa
 (with-temp-buffer
   (let* ((buffer-invisibility-spec '(gnus-sum))
          (text "parent\nchild\nleaf\nsibling\n")
          (parent-eol nil)
          (hidden-newline nil)
          (sibling-start nil)
          hidden-overlay)
     (insert text)
     (goto-char (point-min))
     (setq parent-eol (line-end-position))
     (forward-line 2)
     (setq hidden-newline (line-end-position))
     (forward-line 1)
     (setq sibling-start (point))
     (setq hidden-overlay (make-overlay hidden-newline (1+ hidden-newline)))
     (overlay-put hidden-overlay 'invisible 'gnus-sum)
     (setq qa-gnus-next-thread-target sibling-start)
     (goto-char (point-min))
     (let ((original-next-thread
            (symbol-function 'gnus-summary-next-thread)))
       (unwind-protect
           (progn
             (fset 'gnus-summary-next-thread
                   #'qa-gnus-next-thread-to-target)
             (gnus-summary-hide-thread))
         (fset 'gnus-summary-next-thread original-next-thread)))
     (let ((new-overlay (car (overlays-at parent-eol))))
       (list (invisible-p hidden-newline)
             (overlay-start new-overlay)
             (overlay-end new-overlay)
             hidden-newline))))
 '(t 7 13 18))

(princ (qa--done))
