;;; -*- lexical-binding: nil; -*-
;; unlock-file leaves another host's live lock alone (#605).
(let* ((dir (make-temp-file "qa-unlock-foreign-" t))
       (file (expand-file-name "lock.txt" dir))
       (lock (expand-file-name ".#lock.txt" dir))
       (target "other@remote.42:1"))
  (unwind-protect
      (progn
        (write-region "" nil file nil 'quiet)
        (make-symbolic-link target lock)
        (qa (unlock-file file) nil)
        (qa (file-symlink-p lock) target))
    (when (file-exists-p lock) (delete-file lock))
    (delete-directory dir t)))
(princ (qa--done))
