;;; qa_dired_chmod_directory_x.el --- Dired directory X regression -*- lexical-binding: t -*-

(require 'cl-lib)
(require 'dired-aux)

(defun qa-dired-chmod-directory-x (directory)
  (cl-letf (((symbol-function 'dired-get-marked-files)
             (lambda (&rest _) (list directory)))
            ((symbol-function 'dired-get-filename)
             (lambda (&rest _) directory))
            ((symbol-function 'dired-mark-read-string)
             (lambda (&rest _) "u+X"))
            ((symbol-function 'dired-do-redisplay) #'ignore)
            ((symbol-function 'dired-post-do-command) #'ignore))
    (with-temp-buffer
      (dired-do-chmod))))

(if qa--remacs
    (let ((directory (make-temp-file "dired-chmod-X-" t)))
      (unwind-protect
          (progn
            (set-file-modes directory #o600)
            (qa (file-modes-symbolic-to-number "u+X" #o600) #o600)
            (qa (file-modes-symbolic-to-number "u+X" #o600 t) #o700)
            (qa (let ((read-string-original (symbol-function 'read-string)))
                  (unwind-protect
                      (progn
                        (fset 'read-string (lambda (&rest _) "u+X"))
                        (read-file-modes nil directory))
                    (fset 'read-string read-string-original)))
                #o700)
            (qa (progn
                  (qa-dired-chmod-directory-x directory)
                  (file-modes directory))
                #o700))
        (set-file-modes directory #o700)
        (delete-directory directory t)))
  ;; GNU 31.1 leaves a directory at #o600 after Dired applies u+X.
  (qa-xf (let ((directory (make-temp-file "dired-chmod-X-" t)))
           (unwind-protect
               (progn
                 (set-file-modes directory #o600)
                 (qa-dired-chmod-directory-x directory)
                 (file-modes directory))
             (set-file-modes directory #o700)
             (delete-directory directory t)))
         #o700
         "Dired applies X to directories without existing execute bits"))

(princ (qa--done))
