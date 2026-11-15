;;; -*- lexical-binding: t; -*-
;;; Prevent htmlfontify :box values from escaping CSS (Issues #2304, #2388).

(require 'htmlfontify)

(qa (and (hfy-box "red") t) t)
(qa (hfy-box "red;}</style><script id=\"probe\">") nil)

(qa
 (assq "border-color"
       (hfy-box-to-style '(:color "red;}</style><script id=\"probe\">")))
 nil)

(qa
 (let ((css (cdr (hfy-face-to-css-default
                  '(:box (:color "red;}</style><script id=\"probe\">"))))))
   (list (not (string-match-p "</style" css))
         (not (string-match-p "<script" css))))
 '(t t))

(princ (qa--done))
