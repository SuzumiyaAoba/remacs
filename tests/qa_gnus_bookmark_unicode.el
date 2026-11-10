;;; qa_gnus_bookmark_unicode.el --- Gnus bookmark Unicode regression -*- lexical-binding:t -*-

(defvar gnus-bookmark-alist nil)
(defvar gnus-bookmark-default-file nil)
(defvar gnus-bookmark-file-coding-system 'utf-8-emacs)
(defvar gnus-bookmark-file-format-version 0)
(defvar gnus-bookmark-end-of-version-stamp-marker
  "-*- End Of Bookmark File Format Version Stamp -*-\n")

(defun gnus-get-buffer-create (name)
  (get-buffer-create name))

(defun qa-gnus-bookmark-load-source-functions ()
  "Evaluate Gnus bookmark writer forms from source."
  (let ((file (locate-library "gnus-bookmark"))
        (needed '(gnus-bookmark-file-coding-system
                  gnus-bookmark-file-format-version
                  gnus-bookmark-end-of-version-stamp-marker
                  gnus-bookmark-insert-file-format-version-stamp
                  gnus-bookmark-write-file)))
    (unless file
      (error "Can't locate gnus-bookmark source"))
    (with-temp-buffer
      (insert-file-contents file)
      (goto-char (point-min))
      (while (and needed (< (point) (point-max)))
        (let* ((form (read (current-buffer)))
               (name (and (memq (car-safe form)
                               '(defgroup defcustom defconst defun))
                          (cadr form))))
          (when (memq name needed)
            (eval form t)
            (setq needed (delq name needed))))))
    (when needed
      (error "Missing gnus-bookmark definitions: %S" needed))
    'loaded))

(qa (qa-gnus-bookmark-load-source-functions) 'loaded)
(qa (default-value 'gnus-bookmark-file-coding-system) 'utf-8-emacs)

(qa
 (let* ((file (make-temp-file "remacs-gnus-bookmark-"))
        (gnus-bookmark-default-file file)
        (gnus-bookmark-alist
         '(("Meeting 😀" (annotation . "Notes 😀")))))
   (unwind-protect
       (progn
         (gnus-bookmark-write-file)
         (with-temp-buffer
           (let ((coding-system-for-read gnus-bookmark-file-coding-system))
             (insert-file-contents file)
             (goto-char (point-min))
             (forward-line 4)
             (let* ((name-found (search-forward "Meeting " nil t))
                    (name-char (and name-found (char-after)))
                    (annotation-found (search-forward "Notes " nil t))
                    (annotation-char
                     (and annotation-found (char-after))))
               (list (and name-char (= name-char 128512))
                     (and annotation-char (= annotation-char 128512)))))))
     (delete-file file)))
 '(t t))

(princ (qa--done))
