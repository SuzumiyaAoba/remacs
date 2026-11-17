;;; qa_mm_body_encoding_line_limit.el --- enforce the RFC maximum body line length -*- lexical-binding: t; -*-

(require 'mm-bodies)

(qa
 (let ((mm-use-ultra-safe-encoding nil))
   (list
    (with-temp-buffer
      (insert (make-string 998 ?a))
      (list (mm-long-lines-p 998)
            (eq (mm-body-encoding 'us-ascii) '7bit)))
    (with-temp-buffer
      (insert (make-string 999 ?a))
      (list (mm-long-lines-p 998)
            (not (eq (mm-body-encoding 'us-ascii) '7bit))))))
 '((nil t) (999 t)))

(princ (qa--done))
