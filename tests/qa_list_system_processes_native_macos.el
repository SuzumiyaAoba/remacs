;;; -*- lexical-binding: nil; -*-
;; macOS process enumeration returns nil on restricted KERN_PROC_ALL and
;; otherwise uses the native process table (#609).
(when (eq system-type 'darwin)
  (qa
   (let ((pids (list-system-processes)))
     (or (null pids)
         (and (memq (emacs-pid) pids)
              (memq 0 pids))))
   t))
(princ (qa--done))
