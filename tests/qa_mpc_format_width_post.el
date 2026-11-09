;;; -*- lexical-binding: nil; -*-
;; The width includes the optional -POST suffix.
(load-file (expand-file-name "../lisp/mpc.el"
                             (file-name-directory load-file-name)))
(defun qa-mpc-format--content ()
  ;; Strip the two alignment marker spaces surrounding a width-limited field.
  (substring (buffer-string) 1 -1))
(qa
 (with-temp-buffer
   (mpc-format "%5{Title-post}" '((Title . "abcdefgh")))
   (let ((content (qa-mpc-format--content)))
     (and (>= (length content) 4)
          (string= (substring content (- (length content) 4)) "post")
          (<= (string-width content) 5))))
 t)
(qa
 (with-temp-buffer
   (mpc-format "%3{Title-post}" '((Title . "x")))
   (<= (string-width (qa-mpc-format--content)) 3))
 t)
(princ (qa--done))
