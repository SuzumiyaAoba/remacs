;;; qa_mm_charset_after_bounds.el --- return nil at end-of-buffer positions -*- lexical-binding: t; -*-

(require 'mm-util)

(qa
 (with-temp-buffer
   (insert "abc")
   (list (mm-charset-after (point-min))
         (mm-charset-after (point-max))
         (mm-charset-after (1+ (point-max)))))
 '(ascii nil nil))

(princ (qa--done))
