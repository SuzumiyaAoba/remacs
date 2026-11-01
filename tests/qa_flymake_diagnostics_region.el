;;; -*- lexical-binding: nil; -*-
;; Flymake diagnostics are filtered by their diagnostic region (#580).
(require 'flymake)
(qa
 (with-temp-buffer
   (insert "abcdef")
   (let ((first (flymake-make-diagnostic
                 (current-buffer) 1 2 :error "first"))
         (second (flymake-make-diagnostic
                  (current-buffer) 5 6 :warning "second")))
     (overlay-put (make-overlay 1 2) 'flymake-diagnostic first)
     (overlay-put (make-overlay 5 6) 'flymake-diagnostic second)
     (list (length (flymake-diagnostics))
           (length (flymake-diagnostics 1 2))
           (mapcar #'flymake-diagnostic-message (flymake-diagnostics 5))
           (length (flymake-diagnostics 3)))))
 '(2 1 ("second") 0))
(princ (qa--done))
