;;; -*- lexical-binding: t; -*-
(defconst qa203-script
  "printf '<%s>|<%s>' \"${REMACS_AUDIT_CHILD+x}\" \"${REMACS_AUDIT_CHILD-}\"")

(defun qa203-output (environment method &optional program script)
  (let ((process-environment environment)
        (buffer (generate-new-buffer " *qa203-output*")) p)
    (setq program (or program "/bin/sh") script (or script qa203-script))
    (unwind-protect
        (with-current-buffer buffer
          (cond
           ((memq method '(pipe pty start))
            (setq p
                  (if (eq method 'start)
                      (let ((process-connection-type nil))
                        (start-process "qa203" buffer program "-c" script))
                    (make-process :name "qa203" :buffer buffer :noquery t
                                  :sentinel #'ignore :connection-type method
                                  :coding 'utf-8-unix
                                  :command (list program "-c" script))))
            (set-process-sentinel p #'ignore)
            (let ((deadline (+ (float-time) 3)))
              (while (and (process-live-p p) (< (float-time) deadline))
                (accept-process-output p 0.01)))
            (accept-process-output p 0.01))
           ((eq method 'call)
            (call-process program nil t nil "-c" script))
           ((eq method 'region)
            (call-process-region (point-min) (point-max) program nil t nil "-c" script))
           ((eq method 'lines)
            (insert (mapconcat #'identity (process-lines program "-c" script) "\n")))
           ((eq method 'ignore-status)
            (insert (mapconcat #'identity (process-lines-ignore-status program "-c" script) "\n")))
           ((eq method 'shell-string)
            (insert (shell-command-to-string script)))
           ((eq method 'shell)
            (shell-command script buffer)))
          (buffer-string))
      (when (and p (process-live-p p)) (delete-process p))
      (kill-buffer buffer))))

(defvar qa203-method)

;; Both pipe and pty children use the dynamically bound Lisp list. The
;; first entry wins; a bare name suppresses later values. GNU stops at
;; the first non-string element and uses C-string NUL termination.
(dolist (qa203-method '(pipe pty))
  (qa (qa203-output nil qa203-method) "<>|<>")
  (qa (qa203-output '("REMACS_AUDIT_CHILD=from-lisp") qa203-method) "<x>|<from-lisp>")
  (qa (qa203-output '("REMACS_AUDIT_CHILD=first" "REMACS_AUDIT_CHILD=second") qa203-method)
      "<x>|<first>")
  (qa (qa203-output '("REMACS_AUDIT_CHILD" "REMACS_AUDIT_CHILD=second") qa203-method) "<>|<>")
  (qa (qa203-output '("REMACS_AUDIT_CHILD=" "REMACS_AUDIT_CHILD=second") qa203-method) "<x>|<>")
  (qa (qa203-output '("REMACS_AUDIT_CHILD=a=b=c") qa203-method) "<x>|<a=b=c>")
  (qa (qa203-output '(42 "REMACS_AUDIT_CHILD=ignored") qa203-method) "<>|<>")
  (qa (qa203-output '("REMACS_AUDIT_CHILD=first" nil "REMACS_AUDIT_CHILD=ignored") qa203-method)
      "<x>|<first>")
  (qa (qa203-output '("REMACS_AUDIT_CHILD=dotted" . ignored) qa203-method) "<x>|<dotted>")
  (qa (qa203-output (list (concat "REMACS_AUDIT_CHILD=before" (string 0) "after")) qa203-method)
      "<x>|<before>")
  (qa (qa203-output '("REMACS_AUDIT_CHILD=é") qa203-method) "<x>|<é>")
  ;; Child PATH must not replace Lisp exec-path when finding PROGRAM.
  (qa (qa203-output '("PATH=/no-such-directory" "REMACS_AUDIT_CHILD=exec-path") qa203-method "sh")
      "<x>|<exec-path>")
  ;; A nil list really clears the OS environment (sh would add PATH).
  (qa (qa203-output nil qa203-method nil "printf '%s' \"${HOME-unset}\"") "unset"))

;; The same environment applies to the synchronous and wrapper APIs.
(dolist (qa203-method '(start call region lines ignore-status shell-string shell))
  (qa (qa203-output '("REMACS_AUDIT_CHILD=shared") qa203-method) "<x>|<shared>")
  (qa (qa203-output '("REMACS_AUDIT_CHILD" "REMACS_AUDIT_CHILD=hidden") qa203-method) "<>|<>"))

;; Byte strings stay bytes, while a multibyte value is passed as UTF-8.
(dolist (qa203-method '(pipe call))
  (qa (mapcar #'string-to-number
              (split-string
               (qa203-output '("REMACS_AUDIT_CHILD=é") qa203-method nil
                             "printf '%s' \"$REMACS_AUDIT_CHILD\" | /usr/bin/od -An -tu1")))
      '(195 169))
  (qa (mapcar #'string-to-number
              (split-string
               (qa203-output (list (concat "REMACS_AUDIT_CHILD=" (unibyte-string 233))) qa203-method nil
                             "printf '%s' \"$REMACS_AUDIT_CHILD\" | /usr/bin/od -An -tu1")))
      '(233)))

;; Lookup honors a private exec-path, and a cwd hit is executed directly
;; rather than searched again using the child's unrelated PATH.
(defun qa203-exec-search (method cwd)
  (let* ((file (make-temp-file (expand-file-name ".qa203-exec-"
                                                (if cwd default-directory temporary-file-directory))))
         (exec-path (unless cwd (list (file-name-directory file)))))
    (unwind-protect
        (progn
          (write-region "#!/bin/sh\nprintf '%s' \"$REMACS_AUDIT_CHILD\"\n" nil file nil 'silent)
          (set-file-modes file #o700)
          (qa203-output '("PATH=/no-such-directory" "REMACS_AUDIT_CHILD=found")
                        method (file-name-nondirectory file)))
      (delete-file file))))
(dolist (qa203-method '(pipe call))
  (qa (qa203-exec-search qa203-method nil) "found")
  (qa (qa203-exec-search qa203-method t) "found"))

;; The child's environment is captured at spawn, before later mutation.
(qa-lex
 (let* ((entry (copy-sequence "REMACS_AUDIT_CHILD=before"))
        (process-environment (list entry))
        (buffer (generate-new-buffer " *qa203-snapshot*")) p)
   (unwind-protect
       (progn
         (setq p (make-process :name "qa203-snapshot" :buffer buffer
                              :noquery t :sentinel #'ignore :connection-type 'pipe
                              :command (list "/bin/sh" "-c"
                                             "read line; printf '%s' \"$REMACS_AUDIT_CHILD\"")))
         (aset entry (1- (length entry)) ?X)
         (setq process-environment '("REMACS_AUDIT_CHILD=after"))
         (process-send-string p "go\n")
         (process-send-eof p)
         (let ((deadline (+ (float-time) 3)))
           (while (and (process-live-p p) (< (float-time) deadline))
             (accept-process-output p 0.01)))
         (accept-process-output p 0.01)
         (with-current-buffer buffer (buffer-string)))
     (when (and p (process-live-p p)) (delete-process p))
     (kill-buffer buffer)))
 "before")

;; Dynamic bindings restore the outer environment and do not alter it.
(qa-lex
 (let ((process-environment '("REMACS_AUDIT_CHILD=outer")))
   (list (qa203-output '("REMACS_AUDIT_CHILD=inner") 'pipe)
         (qa203-output process-environment 'pipe)
         process-environment))
 '("<x>|<inner>" "<x>|<outer>" ("REMACS_AUDIT_CHILD=outer")))
(princ (qa--done)) (terpri)
