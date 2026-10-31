;;; -*- lexical-binding: t -*-
;; #3377: load the complete source and process sequences without face stubs.
(require 'ansi-color)

(qa (featurep 'ansi-color) t)
(qa (and (string-match-p
          "Parameter  Description"
          (documentation-property 'ansi-color-basic-faces-vector
                                  'variable-documentation))
         t)
    t)
(qa (face-attribute 'ansi-color-double-underline :underline) '(:style double-line))
(qa (let ((ansi-color-context nil)) (ansi-color-apply "plain")) "plain")
(qa (let ((ansi-color-context nil)) (ansi-color-apply "")) "")
(qa (let ((ansi-color-context nil)) (ansi-color-apply "one\e[0mtwo")) "onetwo")
(qa (let ((ansi-color-context nil)) (ansi-color-apply "\e[2Khello")) "hello")

;; A color applies only until reset, and text after the last escape is kept.
(qa (let* ((ansi-color-context nil)
           (text (ansi-color-apply "\e[38;2;255;0;0mred\e[0mplain")))
      (list (substring-no-properties text)
            (and (get-text-property 0 'font-lock-face text) t)
            (get-text-property 3 'font-lock-face text)))
    '("redplain" t nil))

;; Incomplete escapes are saved and completed by the next call.
(qa (let ((ansi-color-context nil))
      (list (ansi-color-apply "\e[38;2;255;")
            (substring-no-properties (ansi-color-apply "0;0mred"))))
    '("" "red"))

;; The obsolete parser must also remain a separate callable top-level form.
(qa (ansi-color-apply-sequence "\e[31m" nil) '(31))

(princ (qa--done))
