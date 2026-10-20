;;; -*- lexical-binding: t; -*-
(defun qa204-output (directory method &optional command)
  (let ((buffer (generate-new-buffer " *qa204-output*")) p)
    (setq command (or command '("/bin/pwd")))
    (unwind-protect
        (let ((default-directory directory))
          (cond
           ((memq method '(pipe pty start))
            (setq p
                  (if (eq method 'start)
                      (let ((process-connection-type nil))
                        (apply #'start-process "qa204" buffer (car command) (cdr command)))
                    (make-process :name "qa204" :buffer buffer :noquery t
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
            (with-current-buffer buffer
              (insert (mapconcat #'identity
                                 (let ((default-directory directory))
                                   (apply #'process-lines command)) "\n"))))
           ((eq method 'ignore-status)
            (with-current-buffer buffer
              (insert (mapconcat #'identity
                                 (let ((default-directory directory))
                                   (apply #'process-lines-ignore-status command)) "\n"))))
           ((eq method 'shell-string)
            (let ((output (shell-command-to-string
                           (mapconcat #'shell-quote-argument command " "))))
              (with-current-buffer buffer (insert output))))
           ((eq method 'shell)
            (shell-command (mapconcat #'shell-quote-argument command " ") buffer)))
          (with-current-buffer buffer (string-trim (buffer-string))))
      (when (and p (process-live-p p)) (delete-process p))
      (kill-buffer buffer))))

(defvar qa204-directory)
(defvar qa204-method)
(defvar qa204-local-directory)

;; Each API launches in the caller's buffer-local/dynamic directory.
;; Compare filesystem identities to allow macOS filename normalization.
(let ((qa204-directory (make-temp-file "qa204 spaces 日本語 " t)))
  (unwind-protect
      (dolist (qa204-method '(pipe pty start call region lines ignore-status shell-string shell))
        (qa (file-equal-p (qa204-output qa204-directory qa204-method) qa204-directory) t)
        (qa (file-equal-p (qa204-output (file-name-as-directory qa204-directory) qa204-method)
                         qa204-directory) t))
    (delete-directory qa204-directory)))

;; PWD is corrected when present and remains absent when unset. Use
;; printenv directly: a shell could repair an incorrect PWD by itself.
(let ((qa204-directory (make-temp-file "qa204-pwd-" t)))
  (unwind-protect
      (dolist (qa204-method '(pipe call))
        (qa (let ((process-environment '("PWD=/wrong")))
              (file-equal-p (qa204-output qa204-directory qa204-method '("/usr/bin/printenv" "PWD"))
                            qa204-directory)) t)
        (qa (let ((process-environment nil))
              (qa204-output qa204-directory qa204-method '("/usr/bin/printenv" "PWD"))) "")
        (qa (let ((process-environment '("PWD" "PWD=/hidden")))
              (qa204-output qa204-directory qa204-method '("/usr/bin/printenv" "PWD"))) ""))
    (delete-directory qa204-directory)))

;; The child changes directory, while PWD preserves the logical symlink.
(defun qa204-symlink (method)
  (let* ((directory (make-temp-file "qa204-target-" t))
         (link (concat directory "-link"))
         (process-environment '("PWD=/wrong")))
    (unwind-protect
        (progn
          (make-symbolic-link directory link)
          (list (file-equal-p (qa204-output link method) directory)
                (equal (qa204-output (file-name-as-directory link) method '("/usr/bin/printenv" "PWD"))
                       link)))
      (when (file-symlink-p link) (delete-file link))
      (delete-directory directory))))
(qa (qa204-symlink 'pipe) '(t t))
(qa (qa204-symlink 'call) '(t t))

;; Relative executable and exec-path entries are relative to the child
;; directory, even when its PATH cannot locate any executable.
(defun qa204-program (method program path)
  (let* ((directory (make-temp-file "qa204-exec-" t))
         (bin (expand-file-name "bin" directory))
         (exec-path path)
         (process-environment '("PATH=/no-such-directory")))
    (unwind-protect
        (progn
          (make-directory bin)
          (dolist (file (list (expand-file-name "tool" directory) (expand-file-name "tool" bin)))
            (write-region "#!/bin/sh\nexec /bin/pwd\n" nil file nil 'silent)
            (set-file-modes file #o700))
          (file-equal-p (qa204-output directory method (list program)) directory))
      (delete-directory directory t))))
(dolist (qa204-method '(pipe call lines))
  (qa (qa204-program qa204-method "./tool" nil) t)
  (qa (qa204-program qa204-method "tool" nil) t)
  (qa (qa204-program qa204-method "tool" '("bin")) t))

;; Relative call-process INFILE is opened in the same directory.
(qa-lex
 (let* ((directory (make-temp-file "qa204-input-" t))
        (file (expand-file-name "input" directory))
        (buffer (generate-new-buffer " *qa204-input*")))
   (unwind-protect
       (progn
         (write-region "relative-input" nil file nil 'silent)
         (let ((default-directory directory))
           (call-process "/bin/cat" "input" buffer))
         (with-current-buffer buffer (buffer-string)))
     (kill-buffer buffer) (delete-directory directory t)))
 "relative-input")

(defun qa204-bad-directory (method kind)
  (let ((path (make-temp-file "qa204-bad-" (not (eq kind 'file)))))
    (unwind-protect
        (progn
          (when (eq kind 'missing) (delete-directory path))
          (condition-case err
              (qa204-output path method)
            (error (list (car err) (nth 1 err) (nth 2 err) (equal (nth 3 err) path)))))
      (if (eq kind 'file) (delete-file path)
        (when (file-exists-p path) (delete-directory path))))))
(dolist (qa204-method '(pipe call))
  (qa (qa204-bad-directory qa204-method 'missing)
      '(file-missing "Setting current directory" "No such file or directory" t))
  (qa (qa204-bad-directory qa204-method 'file)
      '(file-error "Setting current directory" "Not a directory" t))
  (qa-err-data (qa204-output nil qa204-method) 'wrong-type-argument '(stringp nil))
  (qa-err-data (qa204-output 42 qa204-method) 'wrong-type-argument '(stringp 42)))

;; Relative default-directory has GNU's invocation-directory base.
(let ((invocation-directory temporary-file-directory))
  (qa (file-equal-p (qa204-output "" 'pipe) temporary-file-directory) t)
  (qa (file-equal-p (qa204-output "." 'call) temporary-file-directory) t))

;; File-name handlers may supply a local directory or nil for HOME.
(defun qa204-handler (operation &rest arguments)
  (if (eq operation 'unhandled-file-name-directory) qa204-local-directory
    (let ((inhibit-file-name-handlers
           (cons 'qa204-handler (and (eq inhibit-file-name-operation operation)
                                    inhibit-file-name-handlers)))
          (inhibit-file-name-operation operation))
      (apply operation arguments))))
(let ((qa204-directory (make-temp-file "qa204-handler-" t)))
  (unwind-protect
      (let ((file-name-handler-alist '(("\\`/qa204-virtual/" . qa204-handler))))
        (dolist (qa204-method '(pipe call))
          (qa (let ((qa204-local-directory qa204-directory))
                (file-equal-p (qa204-output "/qa204-virtual/" qa204-method) qa204-directory)) t)
          (qa (let ((qa204-local-directory nil)
                    (process-environment (list (concat "HOME=" qa204-directory))))
                (file-equal-p (qa204-output "/qa204-virtual/" qa204-method) qa204-directory)) t)))
    (delete-directory qa204-directory)))

;; Quoted local directory names use the same physical directory.
(let ((qa204-directory (make-temp-file "qa204-quoted-" t)))
  (unwind-protect
      (qa (file-equal-p (qa204-output (concat "/:" qa204-directory) 'pipe) qa204-directory) t)
    (delete-directory qa204-directory)))
(princ (qa--done)) (terpri)
