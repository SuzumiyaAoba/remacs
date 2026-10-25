;;; -*- lexical-binding: nil; -*-
;; file-attributes keeps valid timestamps before the Unix epoch (#494).
(let ((file (make-temp-file "qa-pre-epoch-")))
  (unwind-protect
      (progn
        (set-file-times file (seconds-to-time -315619200))
        (qa (list (nth 4 (file-attributes file))
                  (nth 5 (file-attributes file)))
            '((-4816 2176 0 0) (-4816 2176 0 0))))
    (delete-file file)))
(princ (qa--done))
