;;; qa_pcvs_revision_coding.el --- PCL-CVS revision coding regression -*- lexical-binding:t -*-

(defvar cvs-program "cvs")
(defvar qa-cvs-revision-file nil)
(defvar qa-cvs-revision-text nil)
(defvar qa-cvs-expected-file nil)
(defvar set-auto-coding-function nil)

(defun cvs-fileinfo->full-name (_fileinfo)
  qa-cvs-revision-file)

(defun qa-cvs-process-file-stub (_program _input _destination _display &rest _args)
  (insert (encode-coding-string qa-cvs-revision-text 'iso-latin-1-unix))
  0)

(defun qa-cvs-coding-for-file (filename _size)
  (if (equal filename qa-cvs-expected-file)
      'iso-latin-1-unix
    'utf-8-unix))

(defun qa-pcvs-load-retrieve-revision ()
  "Evaluate `cvs-retrieve-revision' from pcvs.el source."
  (let ((file (locate-library "pcvs"))
        found)
    (unless file
      (error "Can't locate pcvs source"))
    (with-temp-buffer
      (insert-file-contents file)
      (goto-char (point-min))
      (while (and (not found) (< (point) (point-max)))
        (let ((form (read (current-buffer))))
          (when (and (eq (car-safe form) 'defun)
                     (eq (cadr form) 'cvs-retrieve-revision))
            (eval form t)
            (setq found t)))))
    (unless found
      (error "Missing cvs-retrieve-revision definition"))
    'loaded))

(qa (qa-pcvs-load-retrieve-revision) 'loaded)

(qa
 (let* ((root (make-temp-file "remacs-pcvs-revision-" t))
        (default-directory (file-name-as-directory root))
        (qa-cvs-revision-file "source.el")
        (qa-cvs-revision-text "name café\n")
        (source (expand-file-name qa-cvs-revision-file root))
        (qa-cvs-expected-file source)
        (set-auto-coding-function #'qa-cvs-coding-for-file)
        (retrieved-name (concat qa-cvs-revision-file ".1.1"))
        (original-process-file (symbol-function 'process-file)))
   (unwind-protect
       (progn
         (with-temp-file source
           (insert ";; -*- coding: iso-latin-1-unix -*-\noriginal\n"))
         (fset 'process-file #'qa-cvs-process-file-stub)
         (with-current-buffer (cvs-retrieve-revision nil "1.1")
           (equal (buffer-string) qa-cvs-revision-text)))
     (fset 'process-file original-process-file)
     (when (get-buffer retrieved-name)
       (kill-buffer retrieved-name))
     (delete-directory root t)))
 t)

(princ (qa--done))
