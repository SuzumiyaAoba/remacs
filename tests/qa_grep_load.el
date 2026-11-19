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

(defun qa--grep-file-line-captures (line)
  (when (string-match (caar grep-regexp-alist) line)
    (list (match-string 1 line) (match-string 2 line))))

(if qa--remacs
    (progn
      (qa (qa--grep-file-line-captures "a:1:match") '("a" "1"))
      (qa (qa--grep-file-line-captures "ab:1:match") '("ab" "1"))
      (qa (qa--grep-file-line-captures "/tmp/a:b:1:match") '("/tmp/a:b" "1"))
      (qa (qa--grep-file-line-captures "/tmp/a:034:1:match")
          '("/tmp/a:034" "1")))
  (progn
    (qa-xf (qa--grep-file-line-captures "a:1:match") '("a" "1")
           "Issue #2657: parse one-character filenames")
    (qa-xf (qa--grep-file-line-captures "ab:1:match") '("ab" "1")
           "Issue #2657: parse non-NUL file:line output")
    (qa-xf (qa--grep-file-line-captures "/tmp/a:b:1:match")
           '("/tmp/a:b" "1")
           "Issue #2657: parse colon-bearing filenames")
    (qa-xf (qa--grep-file-line-captures "/tmp/a:034:1:match")
           '("/tmp/a:034" "1")
           "Issue #2657: keep colon-number text in the filename")))

(princ (qa--done))
