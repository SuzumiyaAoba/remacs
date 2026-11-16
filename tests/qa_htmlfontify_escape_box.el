;;; -*- lexical-binding: t; -*-
;;; Prevent htmlfontify :box values from escaping CSS (Issues #2304, #2388).

(require 'htmlfontify)

(qa (and (hfy-box "red") t) t)
(qa (hfy-box "red;}</style><script id=\"probe\">") nil)
(qa (assoc "border-width" (hfy-box-to-style '(:line-width 4 :color "red")))
    '("border-width" . "4px"))
(qa (assoc "border-width" (hfy-box-to-style '(:width 3 :color "red")))
    '("border-width" . "3px"))

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
