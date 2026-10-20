;;; -*- lexical-binding: t; -*-
;; Issue #197 / AUD-033: isolate each thread's buffer and dynamic bindings.
(defvar audit-thread-special 'global)
(qa-lex
 (let ((a (generate-new-buffer " *audit-thread-a*"))
       (b (generate-new-buffer " *audit-thread-b*")))
   (unwind-protect
       (save-current-buffer
         (set-buffer a)
         (let ((worker (make-thread (lambda () (set-buffer b)))))
           (thread-join worker)
           (eq (current-buffer) a)))
     (kill-buffer a)
     (kill-buffer b)))
 t)
(qa-lex
 (let ((audit-thread-special 'parent))
   (thread-join
    (make-thread (lambda () (symbol-value 'audit-thread-special)))))
 'global)
(princ (qa--done)) (terpri)
