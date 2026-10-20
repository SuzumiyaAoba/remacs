;;; -*- lexical-binding: t; -*-
(require 'timer)
(defun qa199-timer-count (target flag)
  (let ((timer-list nil) (hits 0)
        (p (make-pipe-process :name "audit-timer-inhibit" :noquery t)))
    (unwind-protect
        (progn
          (run-at-time 0 nil (lambda () (setq hits (1+ hits))))
          (accept-process-output (and target p) 0.03 nil flag)
          hits)
      (mapc #'cancel-timer timer-list)
      (delete-process p))))
(defun qa199-queued (target flag &optional timer nested escape)
  (let* ((timer-list nil) (hits 0) (reads 0)
         (directory (make-temp-file "qa199-ready-" t))
         (marker (expand-file-name "ready" directory))
         (p (make-pipe-process :name "qa199-empty" :noquery t))
         (other (make-pipe-process
                 :name "qa199-output" :noquery t
                 :filter (lambda (_ text)
                           (setq reads (1+ reads))
                           (when nested (sleep-for 0.001))
                           (when escape (throw 'qa199-out 'escaped)))))
         (child (make-process
                 :name "qa199-writer" :noquery t :connection-type 'pipe
                 :command (list "sh" "-c" "printf queued >&2; : > \"$1\""
                                "qa199" marker)
                 :stderr other :sentinel #'ignore)))
    (unwind-protect
        (progn
          (let ((deadline (+ (float-time) 3)))
            ;; Yielding does not pump process output. The marker proves
            ;; bytes are queued before accept-process-output is called.
            (while (and (not (file-exists-p marker)) (< (float-time) deadline))
              (thread-yield))
            (unless (file-exists-p marker) (error "writer did not become ready")))
          (when timer (run-at-time 0 nil (lambda () (setq hits (1+ hits)))))
          (let* ((result (catch 'qa199-out
                           (accept-process-output
                            (cond ((eq target 'output) other) (target p))
                            0.02 nil flag)))
                 (before (list reads hits)))
            (accept-process-output other 0.02)
            (sleep-for 0.01)
            (list result before (list reads hits))))
      (mapc #'cancel-timer timer-list)
      (mapc #'delete-process (list child other p))
      (delete-directory directory t))))

;; Timer inhibition uses GNU FIXNUMP; PROCESS=nil ignores the flag.
(qa (qa199-timer-count t nil) 1)
(qa (qa199-timer-count t t) 1)
(qa (qa199-timer-count t 0) 0)
(qa (qa199-timer-count t 1) 0)
(qa (qa199-timer-count t -1) 0)
(qa (qa199-timer-count t 1.0) 1)
(qa (qa199-timer-count t 'flag) 1)
(qa (qa199-timer-count t 1267650600228229401496703205376) 1)
(qa (qa199-timer-count nil nil) 1)
(qa (qa199-timer-count nil t) 1)
(qa (qa199-timer-count nil 0) 1)
(qa (qa199-timer-count nil 1) 1)
(qa (qa199-timer-count nil -1) 1)
(qa (qa199-timer-count nil 1.0) 1)
(qa (qa199-timer-count nil 'flag) 1)
(qa (qa199-timer-count nil 1267650600228229401496703205376) 1)

;; Unread output from other channels remains available after a restricted wait.
(qa (qa199-queued t nil) '(nil (1 0) (1 0)))
(qa (qa199-queued t t) '(nil (0 0) (1 0)))
(qa (qa199-queued t 0) '(nil (0 0) (1 0)))
(qa (qa199-queued t 1) '(nil (0 0) (1 0)))
(qa (qa199-queued t 1.0) '(nil (0 0) (1 0)))
(qa (qa199-queued t (expt 2 100)) '(nil (0 0) (1 0)))
;; With PROCESS=nil, check which output was read.
(qa (cdr (qa199-queued nil 1)) '((1 0) (1 0)))
;; Check due timers before buffered output, and preserve reentrant waits.
(qa (qa199-queued 'output nil t) '(t (1 1) (1 1)))
(qa (qa199-queued 'output 1 t) '(t (1 0) (1 1)))
(qa (qa199-queued 'output 1 t t) '(t (1 1) (1 1)))
(qa (qa199-queued 'output 1 t nil t) '(escaped (1 0) (1 1)))
;; Inhibited timers stay armed, including repeat timers, and fire afterward.
(defun qa199-deferred (flag repeat)
  (let* ((timer-list nil) (hits 0)
         (p (make-pipe-process :name "qa199-deferred" :noquery t))
         (timer (run-at-time 0 repeat (lambda () (setq hits (1+ hits))))))
    (unwind-protect
        (progn
          (accept-process-output p 0.01 nil flag)
          (let ((before (list hits (timer--triggered timer)
                              (not (null (memq timer timer-list))))))
            (sleep-for 0.01)
            (list before (list hits (timer--triggered timer)
                               (not (null (memq timer timer-list)))))))
      (mapc #'cancel-timer timer-list)
      (delete-process p))))
(qa (qa199-deferred 1 nil) '((0 nil t) (1 t nil)))
(qa (qa199-deferred 0 nil) '((0 nil t) (1 t nil)))
(qa (qa199-deferred -1 100) '((0 nil t) (1 nil t)))
(princ (qa--done)) (terpri)
