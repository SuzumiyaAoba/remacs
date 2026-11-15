;;; -*- lexical-binding: t; -*-
;;; CSS string escaping for htmlfontify face families (Issue #2303).

(require 'htmlfontify)

(qa
 (let ((css (cdr (car (hfy-family "x</style><script id=\"probe\">")))))
   (list (not (string-match-p "</style" css))
         (not (string-match-p "<script" css))
         (and (string-match-p (regexp-quote "\\3c ") css) t)
         (and (string-match-p (regexp-quote "\\3e ") css) t)))
 '(t t t t))

(qa (cdr (car (hfy-family "x\"; body { background: red } /*")))
    "\"x\\\"; body { background: red } /*\"")

(qa (cdr (car (hfy-family "a\\b"))) "\"a\\\\b\"")
(qa (cdr (car (hfy-family "a\nb"))) "\"a\\a b\"")

(qa
 (let ((css
        (cdr (hfy-face-to-css-default
              '(:family "x</style><script id=\"probe\">")))))
   (list (not (string-match-p "</style" css))
         (not (string-match-p "<script" css))))
 '(t t))

(princ (qa--done))
