;;; -*- lexical-binding: nil; -*-
;; Syntax descriptions retain valid high-Mule match characters (#423).
(qa (let ((chars (with-temp-buffer
                   (internal-describe-syntax-value '(1 . #x110000))
                   (string-to-list (buffer-string)))))
      (list (and (memq #x110000 chars) t)
            (let ((count 0))
              (dolist (ch chars) (when (= ch #x110000) (setq count (1+ count))))
              count)))
    '(t 2))
(princ (qa--done))
