;;; -*- lexical-binding: nil; -*-
;; file-locked-p removes recognized stale locks but preserves malformed ones (#603).
(when (memq system-type '(darwin gnu/linux))
  (let* ((dir (make-temp-file "qa-file-locked-stale-" t))
         (file (expand-file-name "lock.txt" dir))
         (lock (expand-file-name ".#lock.txt" dir)))
    (unwind-protect
        (progn
          (write-region "" nil file nil 'quiet)
          (make-symbolic-link
           (format "stale@%s.1:1" (system-name)) lock)
          (qa (list (file-locked-p file) (file-symlink-p lock)) '(nil nil))
          (make-symbolic-link "malformed lock contents" lock)
          (qa (list (file-locked-p file) (file-symlink-p lock))
              '(nil "malformed lock contents")))
      (when (file-exists-p lock) (delete-file lock))
      (delete-directory dir t))))
(princ (qa--done))
