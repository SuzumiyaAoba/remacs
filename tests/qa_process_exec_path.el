;;; -*- lexical-binding: t; -*-
(defvar qa205-directory)
(defvar qa205-other)
(defvar qa205-method)

(defun qa205-lookup (path method &optional command)
  (let* ((buffer (generate-new-buffer " *qa205-output*"))
         (default-directory qa205-directory)
         (exec-path path)
         (process-environment '("PATH=/no-such-directory")) p)
    (setq command (or command '("qa205-tool")))
    (unwind-protect
        (progn
          (cond
           ((memq method '(pipe pty start))
            (setq p
                  (if (eq method 'start)
                      (let ((process-connection-type nil))
                        (apply #'start-process "qa205" buffer (car command) (cdr command)))
                    (make-process :name "qa205" :buffer buffer :noquery t
                                  :sentinel #'ignore :connection-type method
                                  :command command)))
            (set-process-sentinel p #'ignore)
            (let ((deadline (+ (float-time) 3)))
              (while (and (process-live-p p) (< (float-time) deadline))
                (accept-process-output p 0.01)))
            (accept-process-output p 0.01))
           ((eq method 'call)
            (apply #'call-process (car command) nil buffer nil (cdr command)))
           ((eq method 'region)
            (apply #'call-process-region "" nil (car command) nil buffer nil (cdr command)))
           ((eq method 'lines)
            (let ((output (apply #'process-lines command)))
              (with-current-buffer buffer (insert (mapconcat #'identity output "\n"))))))
          (with-current-buffer buffer (string-trim (buffer-string))))
      (when (and p (process-live-p p)) (delete-process p))
      (kill-buffer buffer))))

(defun qa205-error (path method &optional command)
  (condition-case err
      (qa205-lookup path method command)
    (file-error (list (car err) (nth 1 err) (nth 2 err)
                      (equal (nth 3 err) (car (or command '("qa205-tool"))))))))

(defun qa205-write-tool (file output)
  (write-region (concat "#!/bin/sh\nprintf " output "\n") nil file nil 'silent)
  (set-file-modes file #o700))

(let* ((qa205-directory (make-temp-file "qa205 cwd 日本語 " t))
       (qa205-other (make-temp-file "qa205-other-" t))
       (a (expand-file-name "qa205-tool" qa205-directory))
       (b (expand-file-name "qa205-tool" qa205-other)))
  (unwind-protect
      (progn
        (qa205-write-tool a "CWD")
        (qa205-write-tool b "OTHER")
        (make-directory (expand-file-name "sub" qa205-directory))
        (make-directory (expand-file-name "sub" qa205-other))
        (qa205-write-tool (expand-file-name "sub/qa205-tool" qa205-directory) "CWD-SUB")
        (qa205-write-tool (expand-file-name "sub/qa205-tool" qa205-other) "OTHER-SUB")
        (dolist (qa205-method '(pipe pty start call region lines))
          ;; Original report: executable exists only in Lisp exec-path;
          ;; the child PATH deliberately cannot find it.
          (qa (qa205-lookup (list qa205-other) qa205-method) "OTHER")
          (qa (qa205-lookup (list nil qa205-other) qa205-method) "CWD")
          (qa (qa205-lookup (list qa205-other nil) qa205-method) "OTHER")
          (qa (qa205-lookup nil qa205-method) "CWD")
          (qa (qa205-lookup (list "" qa205-other) qa205-method) "CWD")
          (qa (qa205-lookup (list "." qa205-other) qa205-method) "CWD")
          (qa (qa205-lookup (list 42 qa205-other) qa205-method) "OTHER")
          (qa (qa205-lookup (cons qa205-other 42) qa205-method) "OTHER")
          (qa (qa205-lookup '("sub") qa205-method) "CWD-SUB")
          ;; A slash in a relative program does not bypass exec-path.
          (qa (qa205-lookup (list qa205-other) qa205-method '("./qa205-tool")) "OTHER")
          (qa (qa205-lookup (list qa205-other) qa205-method '("sub/qa205-tool")) "OTHER-SUB")
          (qa (qa205-lookup (list qa205-other) qa205-method
                            (list (expand-file-name "qa205-tool" qa205-directory))) "CWD")
          ;; A non-string base in a list means /, like expand-file-name.
          (qa (qa205-lookup '(42) qa205-method '("bin/echo" "ROOT")) "ROOT")
          ;; No implicit cwd fallback when a nonempty path has no match.
          (qa (qa205-error 42 qa205-method)
              '(file-missing "Searching for program" "No such file or directory" t))
          (qa (qa205-error '(42) qa205-method)
              '(file-missing "Searching for program" "No such file or directory" t))
          (qa (qa205-error '(42 . 43) qa205-method)
              '(file-missing "Searching for program" "No such file or directory" t))
          (qa (qa205-error '("/no-such-remacs-exec-path") qa205-method)
              '(file-missing "Searching for program" "No such file or directory" t)))
        ;; make-process directly accepts an absolute name; synchronous
        ;; openp still requires a path list, even for an absolute name.
        (dolist (qa205-method '(pipe pty start))
          (qa (qa205-lookup 42 qa205-method
                            (list (expand-file-name "qa205-tool" qa205-directory))) "CWD"))
        (dolist (qa205-method '(call region lines))
          (qa (qa205-error 42 qa205-method
                           (list (expand-file-name "qa205-tool" qa205-directory)))
              '(file-missing "Searching for program" "No such file or directory" t)))
        ;; A circular tail must not prevent an earlier successful hit.
        (qa (let ((path (list qa205-other)))
              (setcdr path path) (qa205-lookup path 'pipe)) "OTHER")
        ;; Check effective execution permission, rather than any x bit.
        (set-file-modes b #o600)
        (dolist (qa205-method '(pipe call))
          (qa (qa205-lookup (list qa205-other qa205-directory) qa205-method) "CWD")
          (qa (qa205-error (list qa205-other "/no-such-remacs-exec-path") qa205-method)
              '(permission-denied "Searching for program" "Permission denied" t)))
        (unless (= (user-uid) 0)
          (set-file-modes b #o001)
          (dolist (qa205-method '(pipe call))
            (qa (qa205-lookup (list qa205-other qa205-directory) qa205-method) "CWD")
            (qa (qa205-error (list qa205-other) qa205-method)
                '(permission-denied "Searching for program" "Permission denied" t))))
        ;; Preserve the last specific OS error across missing candidates.
        (delete-file b)
        (make-directory b)
        (dolist (qa205-method '(pipe call))
          (qa (qa205-lookup (list qa205-other qa205-directory) qa205-method) "CWD")
          (qa (qa205-error (list qa205-other "/no-such-remacs-exec-path") qa205-method)
              '(file-error "Searching for program" "Is a directory" t)))
        (delete-directory b)
        (make-symbolic-link "qa205-tool" b)
        (dolist (qa205-method '(pipe call))
          (qa (qa205-lookup (list qa205-other qa205-directory) qa205-method) "CWD")
          (qa (qa205-error (list qa205-other "/no-such-remacs-exec-path") qa205-method)
              '(file-error "Searching for program" "Too many levels of symbolic links" t))))
    (delete-directory qa205-directory t)
    (delete-directory qa205-other t)))
(princ (qa--done)) (terpri)
