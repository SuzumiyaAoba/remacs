;;; -*- lexical-binding: t; -*-
(require 'timer)
(defvar qa207-args)
(defvar qa207-flag)

(defun qa207-timer (arguments wait &optional global kind)
  (let ((timer-list nil) (fired nil) (expired nil)
        (buffer (generate-new-buffer " *qa207-timer*")) timers p)
    (unwind-protect
        (progn
          (unless (eq kind 'none)
            (setq p (make-process :name "qa207-timer" :buffer buffer :noquery t
                                  :sentinel #'ignore :connection-type 'pipe
                                  :filter (and (eq kind 'paused) t)
                                  :command '("/bin/cat"))))
          (push (run-at-time
                 (if wait 0.02 60) nil
                 (lambda ()
                   (setq fired t)
                   (cond ((eq kind 'paused) (delete-process p))
                         ((eq kind 'none)
                          (setq p (make-process :name "qa207-later" :buffer buffer :noquery t
                                                :sentinel #'ignore :connection-type 'pipe
                                                :command '("/bin/sh" "-c" "printf LATER"))))
                         (t (process-send-string p "DATA\n"))))) timers)
          ;; A regression must fail rather than leave a batch runner
          ;; waiting forever. Global waits need actual watchdog output.
          (push (run-at-time
                 3 nil
                 (lambda ()
                   (setq expired t)
                   (when (and p (process-live-p p)) (delete-process p))
                   (when (or global (eq kind 'none))
                     (setq p (make-process :name "qa207-watchdog" :buffer buffer :noquery t
                                           :sentinel #'ignore :connection-type 'pipe
                                           :command '("/bin/sh" "-c" "printf WATCHDOG")))))) timers)
          (let ((result (apply #'accept-process-output (unless global p) arguments)))
            (list result fired expired (with-current-buffer buffer (buffer-string)))))
      (mapc #'cancel-timer timers)
      (when (and p (process-live-p p)) (delete-process p))
      (kill-buffer buffer))))

;; Both time arguments nil/omitted wait for a specified process.
(dolist (qa207-args '(nil (nil) (nil nil)))
  (qa (qa207-timer qa207-args t) '(t t nil "DATA\n")))
;; A paused process still waits for a timer to resume/delete it.
(qa (qa207-timer nil t nil 'paused) '(nil t nil ""))
(qa (qa207-timer '(nil nil) t nil 'paused) '(nil t nil ""))
;; PROCESS=nil polls immediately, even when live processes exist.
(qa (qa207-timer nil nil t) '(nil nil nil ""))
(qa (qa207-timer '(nil nil) nil t) '(nil nil nil ""))
(qa (qa207-timer nil nil t 'none) '(nil nil nil ""))
;; Explicit zero/negative seconds or a non-nil legacy MILLISEC are finite.
(dolist (qa207-args '((0) (0.0) (-1) (nil 0) (nil 1) (0 1)))
  (qa (qa207-timer qa207-args nil) '(nil nil nil "")))

;; No output plus process exit/deletion returns nil instead of waiting.
(qa-lex
 (let ((p (make-process :name "qa207-exit" :noquery t :connection-type 'pipe
                        :sentinel #'ignore :command '("/bin/sh" "-c" "exit 0"))))
   (unwind-protect
       (list (accept-process-output p) (process-status p)
             (accept-process-output p))
     (when (process-live-p p) (delete-process p))))
 '(nil exit nil))
(qa-lex
 (let ((p (make-pipe-process :name "qa207-deleted" :noquery t :sentinel #'ignore)))
   (delete-process p)
   (accept-process-output p))
 nil)

;; A cooperative Lisp thread can make progress even when JUST-THIS-ONE
;; inhibits timers. The external writer has a finite fallback so broken
;; scheduling gives a failed assertion rather than a permanent hang.
(defun qa207-thread (flag)
  (let* ((timer-list nil) (fired nil) (ran nil)
         (directory (make-temp-file "qa207-thread-" t))
         (marker (expand-file-name "ready" directory))
         (buffer (generate-new-buffer " *qa207-thread*")) p writer timer)
    (unwind-protect
        (progn
          (setq p (make-process
                   :name "qa207-thread" :buffer buffer :noquery t
                   :sentinel #'ignore :connection-type 'pipe
                   :command (list "/bin/sh" "-c"
                                  "n=0; while test ! -e \"$1\" && test $n -lt 300; do sleep 0.01; n=$((n+1)); done; printf THREAD"
                                  "qa207" marker)))
          (setq writer (make-thread
                        (lambda () (thread-yield) (setq ran t)
                          (write-region "" nil marker nil 'silent))))
          (setq timer (run-at-time 0 nil (lambda () (setq fired t))))
          (let* ((result (accept-process-output p nil nil flag))
                 (snapshot (list result fired ran (with-current-buffer buffer (buffer-string)))))
            (thread-join writer)
            snapshot))
      (when timer (cancel-timer timer))
      (when writer (ignore-errors (thread-join writer)))
      (when (and p (process-live-p p)) (delete-process p))
      (kill-buffer buffer) (delete-directory directory t))))
(dolist (qa207-flag '(nil t 0 1 -1 1.0 1267650600228229401496703205376))
  (qa (qa207-thread qa207-flag)
      (list t (not (fixnump qa207-flag)) t "THREAD")))

;; The other process runs first in remacs's process vector. Its filter
;; makes target output ready before the remaining processes are polled.
;; Reading TARGET again there used to consume output but discard its
;; return value, leaving the outer wait waiting after delivery.
(defun qa207-other-filter ()
  (let* ((timer-list nil) (expired nil) (calls 0)
         (directory (make-temp-file "qa207-other-" t))
         (ready (expand-file-name "target-ready" directory))
         (trigger (expand-file-name "trigger-ready" directory))
         (buffer (generate-new-buffer " *qa207-target*"))
         other target writer watchdog)
    (unwind-protect
        (progn
          (setq other
                (make-pipe-process
                 :name "qa207-other" :noquery t :sentinel #'ignore
                 :filter (lambda (_process _text)
                           (setq calls (1+ calls))
                           (process-send-string target "go\n")
                           (let ((deadline (+ (float-time) 3)))
                             ;; thread-yield does not pump process I/O.
                             (while (and (not (file-exists-p ready)) (< (float-time) deadline))
                               (thread-yield))
                             (unless (file-exists-p ready) (error "Target did not write output"))))))
          (setq target
                (make-process :name "qa207-target" :buffer buffer :noquery t
                              :sentinel #'ignore :connection-type 'pipe
                              :command (list "/bin/sh" "-c"
                                             "read input; printf TARGET; : > \"$1\"; read done"
                                             "qa207" ready)))
          (setq writer
                (make-process :name "qa207-trigger" :stderr other :noquery t
                              :sentinel #'ignore :connection-type 'pipe
                              :command (list "/bin/sh" "-c" "printf TRIGGER >&2; : > \"$1\""
                                             "qa207" trigger)))
          (let ((deadline (+ (float-time) 3)))
            (while (and (not (file-exists-p trigger)) (< (float-time) deadline))
              (thread-yield))
            (unless (file-exists-p trigger) (error "Trigger was not queued")))
          (setq watchdog (run-at-time 3 nil (lambda () (setq expired t) (delete-process target))))
          (let ((result (accept-process-output target)))
            (list result expired calls (with-current-buffer buffer (buffer-string)))))
      (when watchdog (cancel-timer watchdog))
      (dolist (process (list writer target other))
        (when (and process (process-live-p process)) (delete-process process)))
      (kill-buffer buffer) (delete-directory directory t))))
(qa (qa207-other-filter) '(t nil 1 "TARGET"))
(princ (qa--done)) (terpri)
