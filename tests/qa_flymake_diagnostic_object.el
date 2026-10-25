;;; -*- lexical-binding: nil; -*-
;; Flymake diagnostics need the record type and public field accessors (#577).
(require 'flymake)
(qa
 (with-temp-buffer
   (let ((diag (flymake-make-diagnostic (current-buffer) 1 2 :error "oops")))
     (list (type-of diag)
           (fboundp 'flymake-diagnostic-text)
           (flymake-diagnostic-text diag))))
 '(flymake--diag t "  oops"))
(qa
 (with-temp-buffer
   (let* ((diag (flymake-make-diagnostic
                 (current-buffer) 3 7 :warning '("backend" 42 "detail")
                 'payload '((priority . 1)))))
     (list (eq (flymake-diagnostic-buffer diag) (current-buffer))
           (flymake-diagnostic-beg diag)
           (flymake-diagnostic-end diag)
           (flymake-diagnostic-type diag)
           (flymake-diagnostic-origin diag)
           (flymake-diagnostic-code diag)
           (flymake-diagnostic-message diag)
           (flymake-diagnostic-data diag))))
 '(t 3 7 :warning "backend" "42" "detail" payload))
(princ (qa--done))
