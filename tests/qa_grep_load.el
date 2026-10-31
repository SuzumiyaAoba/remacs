;;; -*- lexical-binding: t -*-
;; #3378: reach grep's provide and keep global setting updates outside the
;; optional host-cache branch.
(require 'grep)

(qa (featurep 'grep) t)
(qa (let ((grep-command "before")
          (grep-host-defaults-alist nil))
      (grep-apply-setting 'grep-command "grep -n")
      (default-value 'grep-command))
    "grep -n")

(qa (let ((default-directory temporary-file-directory)
          (grep-command "before")
          (grep-host-defaults-alist
           (copy-tree '((localhost (grep-command "local-old"))
                        (nil (grep-command "default-old"))
                        (other (grep-command "other-old"))))))
      (grep-apply-setting 'grep-command "grep -n")
      (cons (default-value 'grep-command)
            (mapcar (lambda (host)
                      (cadr (assq 'grep-command
                                  (assq host grep-host-defaults-alist))))
                    '(localhost nil other))))
    '("grep -n" "grep -n" "grep -n" "other-old"))

(qa (let ((default-directory temporary-file-directory)
          (grep-command "before")
          (grep-host-defaults-alist
           (copy-tree '((other (grep-command "other-old"))))))
      (grep-apply-setting 'grep-command "grep -n")
      (list (default-value 'grep-command) grep-host-defaults-alist))
    '("grep -n" ((other (grep-command "other-old")))))

(princ (qa--done))
