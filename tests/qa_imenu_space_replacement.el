;;; -*- lexical-binding: t -*-
(require 'imenu)

(defun qa-imenu--space-labels (replacement)
  (let ((original (symbol-function 'completing-read))
        (index (list (cons "some name" 1) (cons "two  gaps" 2)
                     (cons " leading " 3) (cons "   " 4)))
        seen result)
    (unwind-protect
        (progn
          (fset 'completing-read
                (lambda (_prompt collection &rest _args)
                  (setq seen (all-completions "" collection))
                  (car seen)))
          (with-temp-buffer
            (let ((imenu-space-replacement replacement))
              (setq result (imenu--completion-buffer index "Pick"))))
          (list (mapcar #'substring-no-properties seen) (cdr result)
                (mapcar #'car index)))
      (fset 'completing-read original))))

(qa (qa-imenu--space-labels nil)
    '(("some name" "two  gaps" " leading " "   ") 1
      ("some name" "two  gaps" " leading " "   ")))
(qa (qa-imenu--space-labels ".")
    '(("some.name" "two..gaps" ".leading." "...") 1
      ("some name" "two  gaps" " leading " "   ")))
(qa (qa-imenu--space-labels "_x")
    '(("some_xname" "two_x_xgaps" "_xleading_x" "_x_x_x") 1
      ("some name" "two  gaps" " leading " "   ")))
(qa (qa-imenu--space-labels "")
    '(("somename" "twogaps" "leading" "") 1
      ("some name" "two  gaps" " leading " "   ")))
(qa (qa-imenu--space-labels "\\&")
    '(("some\\&name" "two\\&\\&gaps" "\\&leading\\&" "\\&\\&\\&") 1
      ("some name" "two  gaps" " leading " "   ")))
(qa (qa-imenu--space-labels "間")
    '(("some間name" "two間間gaps" "間leading間" "間間間") 1
      ("some name" "two  gaps" " leading " "   ")))

(princ (qa--done))
