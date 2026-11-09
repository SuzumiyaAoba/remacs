;;; -*- lexical-binding: nil; -*-
;; A remote ARG_MAX configures and reapplies the connection-local limit.
(require 'cl-lib)
(load-file (expand-file-name "../lisp/tramp-sh.el"
                             (file-name-directory load-file-name)))
(defun qa-tramp-command-line-max--run (arg-max)
  (let (profile-setting criteria-setting events)
    (cl-letf (((symbol-function 'tramp-get-remote-arg-max)
               (lambda (_vec) arg-max))
              ((symbol-function 'tramp-get-connection-local-criteria)
               (lambda (_vec)
                 '(:application tramp :protocol "ssh" :machine "example")))
              ((symbol-function 'connection-local-profile-name-for-criteria)
               (lambda (_criteria) 'qa-tramp-command-line-profile))
              ((symbol-function 'connection-local-update-profile-variables)
               (lambda (profile variables)
                 (setq profile-setting (list profile variables))
                 (push 'profile-variables events)))
              ((symbol-function 'connection-local-set-profiles)
               (lambda (criteria profile)
                 (setq criteria-setting (list criteria profile))
                 (push 'profiles events)))
              ((symbol-function 'tramp-set-connection-local-variables)
               (lambda (_vec) (push 'applied events))))
      (list (tramp--set-command-line-max-length 'qa-tramp-vec)
            profile-setting criteria-setting (nreverse events)))))
(qa
 (qa-tramp-command-line-max--run 65536)
 '(16384
   (qa-tramp-command-line-profile
    ((command-line-max-length . 16384)))
   ((:application tramp :protocol "ssh" :machine "example")
    qa-tramp-command-line-profile)
   (profile-variables profiles applied)))
(qa
 (car (qa-tramp-command-line-max--run nil))
 4094)
(princ (qa--done))
