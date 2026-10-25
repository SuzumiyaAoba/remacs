;;; -*- lexical-binding: nil; -*-
;; Missing and zero lock boot timestamps do not invalidate a live local PID (#602).
(when (memq system-type '(darwin gnu/linux))
  (let* ((dir (make-temp-file "qa-lock-no-boot-" t))
         (file (expand-file-name "boot.txt" dir))
         (lock (expand-file-name ".#boot.txt" dir)))
    (unwind-protect
        (progn
          (write-region "" nil file nil 'quiet)
          (dolist (suffix '("" ":0"))
            (make-symbolic-link
             (concat "lockuser@" (system-name) ".1" suffix)
             lock)
            (qa (file-locked-p file) "lockuser")
            (qa (stringp (file-symlink-p lock)) t)
            (delete-file lock)))
      (delete-directory dir t))))
(princ (qa--done))
