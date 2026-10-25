;;; -*- lexical-binding: nil; -*-
;; Invalid trim regexps signal invalid-regexp with diagnostic data (#379).
(dolist (trim '(string-trim string-trim-left string-trim-right))
  (qa (condition-case err (funcall trim "abc" "[")
        (error (list (car err) (stringp (cadr err)))))
      '(invalid-regexp t)))
(princ (qa--done))
