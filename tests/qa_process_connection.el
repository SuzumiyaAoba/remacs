;;; -*- lexical-binding: t; -*-
(defun qa200-connection (global mode split)
  (let ((process-connection-type global)
        (output (generate-new-buffer " *qa200-out*"))
        (errors (and split (generate-new-buffer " *qa200-err*"))) p)
    (unwind-protect
        (condition-case err
            (progn
              (setq p (apply #'make-process
                             :name "qa200" :buffer output :noquery t
                             :sentinel #'ignore :stderr errors
                             :command '("/bin/sh" "-c" "for fd in 0 1 2; do if test -t \"$fd\"; then printf T; else printf P; fi; done")
                             (unless (eq mode 'default) (list :connection-type mode))))
              (let ((tty (not (null (process-tty-name p))))
                    (deadline (+ (float-time) 3)))
                (while (and (process-live-p p) (< (float-time) deadline))
                  (accept-process-output p 0.01))
                (accept-process-output p 0.01)
                (list tty (with-current-buffer output (buffer-string)))))
          (error (list 'error err)))
      (when (and p (process-live-p p)) (delete-process p))
      (kill-buffer output)
      (when errors (kill-buffer errors)))))

;; GNU terminal names and kernel isatty(0/1/2), including mixed transports.
(qa (qa200-connection nil 'default nil) '(nil "PPP"))
(qa (qa200-connection nil 'default t) '(nil "PPP"))
(qa (qa200-connection nil nil nil) '(nil "PPP"))
(qa (qa200-connection nil nil t) '(nil "PPP"))
(qa (qa200-connection nil 'pipe nil) '(nil "PPP"))
(qa (qa200-connection nil 'pipe t) '(nil "PPP"))
(qa (qa200-connection nil 'pty nil) '(t "TTT"))
(qa (qa200-connection nil 'pty t) '(t "TTP"))
(qa (qa200-connection nil '(pipe . pty) nil) '(t "PTT"))
(qa (qa200-connection nil '(pipe . pty) t) '(t "PTP"))
(qa (qa200-connection nil '(pty . pipe) nil) '(t "TPP"))
(qa (qa200-connection nil '(pty . pipe) t) '(t "TPP"))
(qa (qa200-connection nil '(pipe . pipe) nil) '(nil "PPP"))
(qa (qa200-connection nil '(pipe . pipe) t) '(nil "PPP"))
(qa (qa200-connection nil '(pty . pty) nil) '(t "TTT"))
(qa (qa200-connection nil '(pty . pty) t) '(t "TTP"))
(qa (qa200-connection t 'default nil) '(t "TTT"))
(qa (qa200-connection t 'default t) '(t "TTP"))
(qa (qa200-connection t nil nil) '(t "TTT"))
(qa (qa200-connection t nil t) '(t "TTP"))
(qa (qa200-connection t 'pipe nil) '(nil "PPP"))
(qa (qa200-connection t 'pipe t) '(nil "PPP"))
(qa (qa200-connection t 'pty nil) '(t "TTT"))
(qa (qa200-connection t 'pty t) '(t "TTP"))
(qa (qa200-connection t '(pipe . pty) nil) '(t "PTT"))
(qa (qa200-connection t '(pipe . pty) t) '(t "PTP"))
(qa (qa200-connection t '(pty . pipe) nil) '(t "TPP"))
(qa (qa200-connection t '(pty . pipe) t) '(t "TPP"))
(qa (qa200-connection t '(pipe . pipe) nil) '(nil "PPP"))
(qa (qa200-connection t '(pipe . pipe) t) '(nil "PPP"))
(qa (qa200-connection t '(pty . pty) nil) '(t "TTT"))
(qa (qa200-connection t '(pty . pty) t) '(t "TTP"))

(defun qa200-streams (mode split)
  (let ((process-connection-type t)
        (errors (and split (generate-new-buffer " *qa200-stream-errors*"))) p)
    (unwind-protect
        (progn
          (setq p (make-process :name "qa200-streams" :noquery t :sentinel #'ignore
                               :command '("/bin/cat") :connection-type mode :stderr errors))
          (mapcar (lambda (stream) (not (null (process-tty-name p stream))))
                  '(nil stdin stdout stderr)))
      (when p (delete-process p))
      (when errors (kill-buffer errors)))))
(qa (qa200-streams 'pipe nil) '(nil nil nil nil))
(qa (qa200-streams 'pty nil) '(t t t t))
(qa (qa200-streams 'pty t) '(t t t nil))
(qa (qa200-streams '(pipe . pty) nil) '(t nil t t))
(qa (qa200-streams '(pipe . pty) t) '(t nil t nil))
(qa (qa200-streams '(pty . pipe) nil) '(t t nil nil))
(qa (qa200-streams '(pipe . pipe) nil) '(nil nil nil nil))
(qa (qa200-streams '(pty . pty) nil) '(t t t t))
(defun qa200-stream-error (stream)
  (let ((p (make-process :name "qa200-stream-error" :noquery t
                         :connection-type 'pipe :command '("/bin/cat") :sentinel #'ignore)))
    (unwind-protect (process-tty-name p stream) (delete-process p))))
(qa-err-data (qa200-stream-error 'unknown) 'error '("Unknown stream" unknown))
(qa-err-data (qa200-stream-error 1) 'error '("Unknown stream" 1))
(qa-err-data (qa200-stream-error "stdout") 'error '("Unknown stream" "stdout"))
;; Control-D is data in a pipe. EOF closes stdin; stdout remains readable.
(qa-lex
 (let ((buffer (generate-new-buffer " *qa200-eof*")) p)
   (unwind-protect
       (progn
         (setq p (make-process :name "qa200-eof" :buffer buffer :noquery t
                              :connection-type 'pipe :sentinel #'ignore
                              :command '("/bin/sh" "-c" "cat; printf DONE; sleep 0.2")))
         (process-send-string p "A\4B\r\0\n")
         (process-send-eof p)
         (let ((later (process-send-string p "discarded"))
               (deadline (+ (float-time) 3)))
           (while (and (process-live-p p) (< (float-time) deadline))
             (accept-process-output p 0.01))
           (accept-process-output p 0.01)
           (list later (with-current-buffer buffer (string-to-list (buffer-string))))))
     (when (and p (process-live-p p)) (delete-process p))
     (kill-buffer buffer)))
 '(nil (65 4 66 13 0 10 68 79 78 69)))
;; Selectors use canonical symbols, rather than a coincident symbol name.
(qa-lex
 (let ((bad (make-symbol "pipe")))
   (condition-case err
       (make-process :name "badmode200" :command '("/bin/cat") :connection-type bad)
     (file-error (eq (nth 3 err) bad)))) t)
(qa-lex
 (let ((bad (make-symbol "stdin")))
   (condition-case err (qa200-stream-error bad)
     (error (eq (nth 2 err) bad)))) t)
(princ (qa--done)) (terpri)
