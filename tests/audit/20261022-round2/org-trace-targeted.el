;;; -*- lexical-binding: t; -*-
(handler-bind ((error (lambda (e) (when (equal e '(void-function keyword))
 (let ((print-length 8) (print-level 5)) (princ (format "ERROR %S\n" e)) (backtrace))))))
 (princ "BEFORE REQUIRE\n") (require 'org) (princ "AFTER REQUIRE\n")
 (with-temp-buffer (org-mode) (princ "AFTER MODE\n")
 (insert "* Heading\n") (font-lock-mode 1) (font-lock-ensure) (princ "AFTER FONT\n")))
