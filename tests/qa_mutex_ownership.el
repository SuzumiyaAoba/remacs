;;; -*- lexical-binding: t; -*-
;; Issue #198 / AUD-034: mutex ownership, contention and recursive depth.
(require 'timer)
(qa-lex
 (let ((m (make-mutex)) (entered nil) worker)
   (mutex-lock m)
   (unwind-protect
       (progn
         (setq worker
               (make-thread
                (lambda ()
                  (mutex-lock m)
                  (unwind-protect (setq entered t) (mutex-unlock m)))))
         (sleep-for 0.03)
         (let ((before-unlock entered))
           (ignore-errors (mutex-unlock m))
           (thread-join worker)
           (list before-unlock entered)))
     (ignore-errors (mutex-unlock m))))
 '(nil t))
(qa-lex
 (let ((m (make-mutex)))
   (mutex-lock m)
   (unwind-protect
       (thread-join
        (make-thread
         (lambda ()
           (condition-case e
               (progn (mutex-unlock m) 'unlocked)
             (error (car e))))))
     (ignore-errors (mutex-unlock m))))
 'error)
(qa-lex
 (let ((m (make-mutex)))
   (mutex-lock m)
   (mutex-lock m)
   (unwind-protect
       (list (mutex-unlock m) (mutex-unlock m))
     (ignore-errors (mutex-unlock m))))
 '(nil nil))
(princ (qa--done)) (terpri)
