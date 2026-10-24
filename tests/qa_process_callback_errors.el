;;; Process callback errors use the command error reporter. -*- lexical-binding: t; -*-

(defun qa-pce-case (kind action debug reporter)
  (let* ((owner (current-buffer))
         (other (generate-new-buffer " *qa-pce-other*"))
         (inhibit-quit nil) (last-nonmenu-event 'outside)
         (quit-flag nil) (debug-on-error debug) (process-error-pause-time 0)
         (debugger (lambda (&rest _) nil))
         seen reports p
         (command-error-function
          (unless (eq reporter 'nil)
            (lambda (data context origin)
              (push (list data context (eq origin 'eval)
                          inhibit-quit last-nonmenu-event) reports)
              (cond ((eq reporter 'error) (error "reporter-error"))
                    ((eq reporter 'throw) (throw 'qa-pce 'reporter-escape)))
              ;; The process error handler re-inhibits quit after reporting.
              (setq inhibit-quit nil)))))
    (unwind-protect
        (let ((callback (lambda (_process _text)
                          (setq seen t)
                          (eval action t))))
          (setq p (if (eq kind 'sentinel)
                      (make-pipe-process :name "qa-pce" :noquery t :sentinel callback)
                    (make-process :name "qa-pce" :noquery t :connection-type 'pipe
                                  :sentinel #'ignore :filter callback
                                  :command '("/bin/sh" "-c" "printf payload"))))
          (string-match "\\(caller\\)" "caller")
          (let ((outcome
                 (catch 'qa-pce
                   (condition-case err
                       (progn
                         (if (eq kind 'sentinel) (delete-process p)
                           (let ((deadline (+ (float-time) 3)))
                             (while (and (not seen) (< (float-time) deadline))
                               (accept-process-output p 0.01))))
                         'returned)
                     (t (list 'caught err))))))
            (list seen outcome (reverse reports) (eq (current-buffer) owner)
                  inhibit-quit last-nonmenu-event quit-flag)))
      (when p (set-process-sentinel p #'ignore) (delete-process p))
      (kill-buffer other))))

(defvar qa-pce-kind)
(dolist (qa-pce-kind '(filter sentinel))
  (qa (qa-pce-case qa-pce-kind '(error "callback-error") nil 'capture)
      `(t returned (((error "callback-error")
                     ,(format "error in process %s: " qa-pce-kind) t t t))
          t nil outside nil))
  (qa (qa-pce-case qa-pce-kind '(car 5) nil 'capture)
      `(t returned (((wrong-type-argument listp 5)
                     ,(format "error in process %s: " qa-pce-kind) t t t))
          t nil outside nil))
  (qa (qa-pce-case qa-pce-kind '(user-error "callback-error") nil 'capture)
      `(t returned (((user-error "callback-error")
                     ,(format "error in process %s: " qa-pce-kind) t t t))
          t nil outside nil))
  (qa (qa-pce-case qa-pce-kind '(progn (setq quit-flag t) (error "callback-error")) nil 'capture)
      `(t returned (((error "callback-error")
                     ,(format "error in process %s: " qa-pce-kind) nil t t))
          t nil outside nil))
  (qa (qa-pce-case qa-pce-kind '(error "callback-error") nil 'nil)
      '(t returned nil t nil outside nil))
  ;; The catch policy is chosen on entry, before the callback changes it.
  (qa (qa-pce-case qa-pce-kind '(progn (setq debug-on-error t) (error "callback-error")) nil 'capture)
      `(t returned (((error "callback-error")
                     ,(format "error in process %s: " qa-pce-kind) nil t t))
          t nil outside nil)))

;; The process catch boundary must suppress observers outside the callback,
;; while handler-bind observers installed inside it still see the signal.
(defun qa-pce-observers (kind)
  (let ((debug-on-error nil) (process-error-pause-time 0)
        (command-error-function (lambda (&rest _) nil))
        seen events p)
    (unwind-protect
        (let ((callback
               (lambda (_process _text)
                 (setq seen t)
                 (handler-bind
                  ((error (lambda (condition) (push (list 'inner (car condition)) events))))
                  (error "callback-error")))))
          (setq p (if (eq kind 'sentinel)
                      (make-pipe-process :name "qa-pce-observers" :noquery t :sentinel callback)
                    (make-process :name "qa-pce-observers" :noquery t :connection-type 'pipe
                                  :sentinel #'ignore :filter callback
                                  :command '("/bin/sh" "-c" "printf payload"))))
          (handler-bind
           ((error (lambda (condition) (push (list 'outer (car condition)) events))))
           (if (eq kind 'sentinel) (delete-process p)
             (let ((deadline (+ (float-time) 3)))
               (while (and (not seen) (< (float-time) deadline))
                 (accept-process-output p 0.01)))))
          (list seen (reverse events)))
      (when p (set-process-sentinel p #'ignore) (delete-process p)))))

(qa (qa-pce-observers 'filter) '(t ((inner error))))
(qa (qa-pce-observers 'sentinel) '(t ((inner error))))

(defun qa-pce-recovery (kind pause)
  (let* ((owner (current-buffer))
         (other (generate-new-buffer " *qa-pce-report*"))
         (debug-on-error nil) (process-error-pause-time (if pause 1 0))
         (inhibit-quit nil) (last-nonmenu-event 'outside)
         seen reported timed timer p
         (command-error-function
          (lambda (&rest _)
            (setq reported t)
            (string-match "reported" "reported")
            (set-buffer other)
            (when pause (setq timer (run-at-time 0.01 nil (lambda () (setq timed t)))))
            (setq inhibit-quit nil))))
    (unwind-protect
        (let ((callback (lambda (_process _text)
                          (setq seen t)
                          (string-match "callback" "callback")
                          (error "callback-error"))))
          (setq p (if (eq kind 'sentinel)
                      (make-pipe-process :name "qa-pce-recovery" :noquery t :sentinel callback)
                    (make-process :name "qa-pce-recovery" :noquery t :connection-type 'pipe
                                  :sentinel #'ignore :filter callback
                                  :command '("/bin/sh" "-c" "printf payload"))))
          (string-match "\\(caller\\)" "caller")
          (if (eq kind 'sentinel) (delete-process p)
            (let ((deadline (+ (float-time) 3)))
              (while (and (not seen) (< (float-time) deadline))
                (accept-process-output p 0.01))))
          (list reported (eq (current-buffer) owner) (match-string 1 "caller")
                (or (not pause) timed) inhibit-quit last-nonmenu-event))
      (when timer (cancel-timer timer))
      (when p (set-process-sentinel p #'ignore) (delete-process p))
      (kill-buffer other))))

(dolist (qa-pce-kind '(filter sentinel))
  (qa (qa-pce-recovery qa-pce-kind nil) '(t t "caller" t nil outside))
  (qa (qa-pce-recovery qa-pce-kind t) '(t t "caller" t nil outside)))

(qa-lex
 (let* ((debug-on-error nil) (process-error-pause-time 0)
        (reports 0) (finished 0) chunks p
        (command-error-function
         (lambda (&rest _)
           (setq reports (1+ reports))
           (process-send-string p "go\n"))))
   (unwind-protect
       (progn
         (setq p (make-process
                  :name "qa-pce-continuation" :noquery t :connection-type 'pipe
                  :command '("/bin/sh" "-c" "printf FIRST; read go; printf SECOND")
                  :filter (lambda (_process text)
                            (push text chunks)
                            (when (= (length chunks) 1) (error "first-chunk-error")))
                  :sentinel (lambda (_process _text) (setq finished (1+ finished)))))
         (let ((deadline (+ (float-time) 3)))
           (while (and (= finished 0) (< (float-time) deadline))
             (accept-process-output p 0.01)))
         (list reports finished (reverse chunks)))
     (when p (set-process-sentinel p #'ignore) (delete-process p))))
 '(1 1 ("FIRST" "SECOND")))

;; Error reporters execute outside the callback's internal error catch.
;; Like GNU, a nonlocal exit bypasses the manual asynchronous search cleanup,
;; so leave these cases after the ordinary return cases.
(put 'qa-pce-nonerror 'error-conditions '(qa-pce-nonerror))
(dolist (qa-pce-kind '(filter sentinel))
  (qa (qa-pce-case qa-pce-kind '(signal 'qa-pce-nonerror '(detail)) nil 'capture)
      `(t returned (((qa-pce-nonerror detail)
                     ,(format "error in process %s: " qa-pce-kind) t t t))
          t nil outside nil))
  (qa (qa-pce-case qa-pce-kind '(signal 'quit nil) nil 'capture)
      `(t returned (((quit)
                     ,(format "error in process %s: " qa-pce-kind) nil t t))
          t nil outside nil))
  (qa (qa-pce-case qa-pce-kind '(error "callback-error") t 'capture)
      '(t (caught (error "callback-error")) nil t nil outside nil))
  (qa (qa-pce-case qa-pce-kind '(progn (setq debug-on-error nil) (error "callback-error")) t 'capture)
      '(t (caught (error "callback-error")) nil t nil outside nil))
  (qa (qa-pce-case qa-pce-kind '(error "callback-error") nil 'error)
      `(t (caught (error "reporter-error"))
          (((error "callback-error") ,(format "error in process %s: " qa-pce-kind) t t t))
          t nil outside nil))
  (qa (qa-pce-case qa-pce-kind '(error "callback-error") nil 'throw)
      `(t reporter-escape
          (((error "callback-error") ,(format "error in process %s: " qa-pce-kind) t t t))
          t nil outside nil))
  (qa (qa-pce-case qa-pce-kind '(throw 'qa-pce 'callback-escape) nil 'capture)
      '(t callback-escape nil t nil outside nil)))

(princ (qa--done))
