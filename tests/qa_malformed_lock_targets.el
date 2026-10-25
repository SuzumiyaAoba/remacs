;;; -*- lexical-binding: nil; -*-
;; Malformed lock targets signal on query and remain untouched by lock operations (#604).
(let* ((dir (make-temp-file "qa-malformed-lock-" t))
       (file (expand-file-name "lock.txt" dir))
       (lock (expand-file-name ".#lock.txt" dir)))
  (unwind-protect
      (progn
        (write-region "" nil file nil 'quiet)
        (dolist (target '("garbage" "other@remote.42:bad"))
          (make-symbolic-link target lock)
          (qa-err (file-locked-p file) 'file-error)
          (qa (file-symlink-p lock) target)
          (qa (lock-file file) nil)
          (qa (unlock-file file) nil)
          (qa (file-symlink-p lock) target)
          (delete-file lock)))
    (when (file-exists-p lock) (delete-file lock))
    (delete-directory dir t)))
(princ (qa--done))
