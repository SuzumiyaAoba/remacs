;;; -*- lexical-binding: nil; -*-
;; lock-file reports the caller's path while locking through its truename (#606).
(when (eq system-type 'darwin)
  (let* ((dir (make-temp-file "qa-lock-original-path-" t))
         (file (expand-file-name "lock.txt" dir))
         (canonical (file-truename file))
         (lock (make-lock-file-name canonical)))
    (unwind-protect
        (progn
          (write-region "" nil file nil 'quiet)
          (make-symbolic-link "other@remote.42:1" lock)
          (qa (not (equal file canonical)) t)
          (qa (condition-case err
                  (progn (lock-file file) nil)
                (file-locked (cadr err)))
              file))
      (when (file-exists-p lock) (delete-file lock))
      (delete-directory dir t))))
(princ (qa--done))
