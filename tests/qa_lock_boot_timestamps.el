;;; -*- lexical-binding: nil; -*-
;; Extreme lock timestamps are stale without integer overflow (#600/#601/#602).
(let* ((directory (make-temp-file "qa601-" t))
       (filename (expand-file-name "file" directory))
       (lock (expand-file-name ".#file" directory))
       (prefix (concat (user-login-name) "@" (system-name) ".1:")))
  (unwind-protect
      (progn
        (write-region "data" nil filename nil 'quiet)
        (dolist (stamp '("-9223372036854775808" "-9223372036854775807"
                         "9223372036854775807" "-1" "1"))
          (make-symbolic-link (concat prefix stamp) lock)
          (qa (file-locked-p filename) nil)
          ;; GNU removes stale lock entries when observing them.
          (qa (file-symlink-p lock) nil))
        ;; Missing/zero recorded boot times do not invalidate a live PID.
        (dolist (suffix '(".1" ".1:0"))
          (make-symbolic-link (concat (user-login-name) "@" (system-name) suffix) lock)
          (qa (file-locked-p filename) (user-login-name))
          (qa (stringp (file-symlink-p lock)) t)
          (delete-file lock))
        ;; PID syntax can be valid while the value cannot identify a process.
        (dolist (suffix '(".0" ".-1" ".2147483648" ".9999999999999999999999999999"))
          (make-symbolic-link (concat "other@" (system-name) suffix) lock)
          (qa (file-locked-p filename) nil)
          (qa (file-symlink-p lock) nil))
        ;; The current process keeps ownership even with a mismatched boot.
        (make-symbolic-link (concat (user-login-name) "@" (system-name) "."
                                    (number-to-string (emacs-pid)) ":1") lock)
        (qa (file-locked-p filename) t))
    (delete-directory directory t)))
(princ (qa--done))
