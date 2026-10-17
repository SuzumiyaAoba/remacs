;;; audit_process.el --- bounded local subprocess and I/O probes -*- lexical-binding: t; -*-
;; Children are local, named audit-*, and deleted in unwind-protect.
(load (expand-file-name "audit_helpers.el" (file-name-directory load-file-name)) nil t)
(require 'timer)

(defun audit-process-drain (p)
  (let ((n 0))
    (while (and (process-live-p p) (< n 200))
      (setq n (1+ n)) (accept-process-output p 0.01))
    (accept-process-output p 0.01)))

(defun audit-process-with (command options fn)
  (let ((b (generate-new-buffer " *audit-child-output*")) p)
    (unwind-protect
        (progn
          (setq p (apply #'make-process :name "audit-child" :buffer b
                         :command command :noquery t :sentinel #'ignore options))
          (funcall fn p b))
      (when (and p (process-live-p p)) (delete-process p))
      (kill-buffer b))))

(audit-check "process:pipe-connection"
 '(audit-process-with '("/bin/cat") '(:connection-type pipe)
    (lambda (p _b) (null (process-tty-name p)))))

(audit-check "process:decode-latin1"
 '(audit-process-with '("/bin/sh" "-c" "printf '\\351'")
    '(:connection-type pipe :coding iso-latin-1)
    (lambda (p b) (audit-process-drain p) (with-current-buffer b (string-to-list (buffer-string))))))

(audit-check "process:encode-latin1"
 '(audit-process-with '("/usr/bin/od" "-An" "-tx1")
    '(:connection-type pipe :coding iso-latin-1)
    (lambda (p b)
      (process-send-string p "é\n") (process-send-eof p)
      (audit-process-drain p) (with-current-buffer b (string-trim (buffer-string))))))

(audit-check "process:split-utf8"
 '(audit-process-with '("/bin/sh" "-c" "printf '\\343'; sleep 0.1; printf '\\201\\202'")
    '(:connection-type pipe :coding utf-8)
    (lambda (p b) (audit-process-drain p) (with-current-buffer b (buffer-string)))))

(audit-check "process:filter-t"
 '(audit-process-with '("/bin/sh" "-c" "printf ok") '(:connection-type pipe :filter t)
    (lambda (p b)
      (let ((first (condition-case e (progn (accept-process-output p 0.1) 'ok)
                     (error (car e)))))
        (set-process-filter p nil) (audit-process-drain p)
        (list first (with-current-buffer b (buffer-string)))))))

(audit-check "process:wait-without-timeout"
 '(audit-process-with '("/bin/cat") '(:connection-type pipe)
    (lambda (p b)
      (let* ((timer-list nil)
             (timer (run-at-time 0.02 nil (lambda () (process-send-string p "ok\n")))))
        (unwind-protect
            (list (accept-process-output p) (with-current-buffer b (buffer-string)))
          (cancel-timer timer))))))

(audit-check "process:just-this-one"
 '(audit-process-with '("/bin/sh" "-c" "sleep 0.02; printf other") '(:connection-type pipe)
    (lambda (_other b)
      (let ((p (make-pipe-process :name "audit-quiet" :noquery t :sentinel #'ignore)))
        (unwind-protect
            (progn (accept-process-output p 0.1 nil t)
                   (with-current-buffer b (buffer-string)))
          (delete-process p))))))

(audit-check "process:send-region-marker"
 '(with-temp-buffer
    (insert "abc")
    (let ((p (make-pipe-process :name "audit-region" :noquery t)))
      (unwind-protect (process-send-region p (copy-marker 1) (copy-marker 3))
        (delete-process p)))))

(audit-check "process:send-region-low-bound"
 '(with-temp-buffer
    (insert "abc")
    (let ((p (make-pipe-process :name "audit-region" :noquery t)))
      (unwind-protect
          (condition-case e (progn (process-send-region p 0 2) 'sent) (error (car e)))
        (delete-process p)))))

(audit-check "process:send-region-narrowed"
 '(with-temp-buffer
    (insert "abc") (narrow-to-region 2 3)
    (let ((p (make-pipe-process :name "audit-region" :noquery t)))
      (unwind-protect
          (condition-case e (progn (process-send-region p 1 4) 'sent) (error (car e)))
        (delete-process p)))))

(audit-check "process:invalid-coding"
 '(let ((p (make-pipe-process :name "audit-coding" :noquery t)))
    (unwind-protect
        (progn (set-process-coding-system p 'audit-no-such-coding 'audit-no-such-coding) 'accepted)
      (delete-process p))))

(audit-check "process:environment"
 '(let ((process-environment (cons "REMACS_AUDIT_CHILD=from-lisp" process-environment)))
    (audit-process-with '("/bin/sh" "-c" "printf '%s' \"$REMACS_AUDIT_CHILD\"")
     '(:connection-type pipe)
     (lambda (p b) (audit-process-drain p) (with-current-buffer b (buffer-string))))))

(audit-check "process:working-directory"
 '(let* ((dir (make-temp-file "remacs-audit-cwd-" t)) (default-directory dir))
    (unwind-protect
        (audit-process-with '("/bin/pwd") '(:connection-type pipe)
         (lambda (p b)
           (audit-process-drain p)
           (equal (file-truename (with-current-buffer b (string-trim (buffer-string))))
                  (directory-file-name (file-truename dir)))))
      (delete-directory dir))))

(audit-check "process:exec-path"
 '(let* ((dir (make-temp-file "remacs-audit-path-" t))
         (program (expand-file-name "audit-child-command" dir)) (exec-path (list dir)))
    (unwind-protect
        (progn
          (with-temp-file program (insert "#!/bin/sh\nprintf found"))
          (set-file-modes program #o700)
          (audit-process-with '("audit-child-command") '(:connection-type pipe)
           (lambda (p b) (audit-process-drain p) (with-current-buffer b (buffer-string)))))
      (delete-directory dir t))))

;; The runner isolates this case: remacs currently panics before AUDIT-DONE.
(audit-check "process:timeout-overflow"
 '(accept-process-output nil 1e30))

(princ "AUDIT-DONE\n")
