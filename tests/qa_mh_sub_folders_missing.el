;;; -*- lexical-binding: nil; -*-
;; A removed cached MH folder must be evicted without invoking the command.
(require 'cl-lib)
(load-file (expand-file-name "../lisp/mh-utils.el"
                             (file-name-directory load-file-name)))
(qa
 (let ((mh-user-path "/nonexistent/Mail")
       (mh-sub-folders-cache (make-hash-table :test #'equal))
       call-count)
   (puthash "+gone" '(("old-child" . t)) mh-sub-folders-cache)
   (cl-letf (((symbol-function 'file-directory-p)
              (lambda (_path) nil))
             ((symbol-function 'call-process)
              (lambda (&rest _args)
                (setq call-count (1+ (or call-count 0))))))
     (let ((result (mh-sub-folders "+gone")))
       (list result (gethash "+gone" mh-sub-folders-cache) call-count))))
 '(nil nil nil))
(qa
 (let ((mh-user-path "/nonexistent/Mail")
       called)
   (cl-letf (((symbol-function 'file-directory-p)
              (lambda (_path) nil))
             ((symbol-function 'call-process)
              (lambda (&rest _args) (setq called t))))
     (list (condition-case nil
               (mh-sub-folders-actual "+gone")
             (error 'error))
           called)))
 '(nil nil))
(princ (qa--done))
