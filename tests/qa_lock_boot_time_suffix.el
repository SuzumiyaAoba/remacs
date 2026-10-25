;;; -*- lexical-binding: nil; -*-
;; macOS lock targets retain the boot timestamp when sysctl is restricted (#600).
(when (eq system-type 'darwin)
  (qa
   (let* ((dir (make-temp-file "qa-lock-boot-" t))
          (file (expand-file-name "boot.txt" dir))
          (lock (expand-file-name ".#boot.txt" dir))
          (buf nil))
     (unwind-protect
         (progn
           (write-region "" nil file nil 'quiet)
           (setq buf (find-file-noselect file))
           (with-current-buffer buf
             (insert "x")
             (and (stringp (file-symlink-p lock))
                  (not (null (string-match-p ":[0-9]+\\'"
                                               (file-symlink-p lock)))))))
       (when (buffer-live-p buf) (kill-buffer buf))
       (delete-directory dir t)))
   t))
(princ (qa--done))
