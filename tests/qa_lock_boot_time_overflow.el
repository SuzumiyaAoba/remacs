;;; -*- lexical-binding: nil; -*-
;; Extreme lock boot timestamps classify as stale without signed overflow (#601).
(when (memq system-type '(darwin gnu/linux))
  (let* ((dir (make-temp-file "qa-lock-time-overflow-" t))
         (file (expand-file-name "boot.txt" dir))
         (lock (expand-file-name ".#boot.txt" dir)))
    (unwind-protect
        (progn
          (write-region "" nil file nil 'quiet)
          (dolist (boot '("-9223372036854775808" "9223372036854775807"))
            (make-symbolic-link
             (format "%s@%s.1:%s" (user-login-name) (system-name) boot)
             lock)
            (qa (file-locked-p file) nil)
            (delete-file lock)))
      (delete-directory dir t))))
(princ (qa--done))
