;;; -*- lexical-binding: nil; -*-
;; Issue #25: stdout/stderr destinations and ordering, GNU 31.1 oracle.
(defvar qa25-command "printf 'O1\\n'; printf 'E1\\n' >&2; printf 'O2\\n'; printf 'E2\\n' >&2; exit 7")

(defun qa25-run (destination region)
  (with-temp-buffer
    (let ((status (if region
                      (progn (insert "input")
                             (call-process-region 1 6 "sh" t destination nil "-c" qa25-command))
                    (call-process "sh" nil destination nil "-c" qa25-command))))
      (list status (buffer-string)))))

(dolist (qa25-region '(nil t))
  (qa (qa25-run t qa25-region) '(7 "O1\nE1\nO2\nE2\n"))
  (qa (qa25-run '(t t) qa25-region) '(7 "O1\nE1\nO2\nE2\n"))
  (qa (qa25-run '(t nil) qa25-region) '(7 "O1\nO2\n"))
  (qa (qa25-run '(t) qa25-region) '(7 "O1\nE1\nO2\nE2\n"))
  (qa (qa25-run '(t . t) qa25-region) '(7 "O1\nE1\nO2\nE2\n"))
  (qa (qa25-run '(t . 123) qa25-region) '(7 "O1\nE1\nO2\nE2\n"))
  (qa (qa25-run '(t . "ignored-tail") qa25-region) '(7 "O1\nE1\nO2\nE2\n"))
  (qa (qa25-run nil qa25-region) '(7 ""))
  (qa (qa25-run '(nil t) qa25-region) '(7 ""))
  (qa (qa25-run '(nil nil) qa25-region) '(7 ""))
  (qa (qa25-run 0 qa25-region) '(nil ""))

  ;; Both streams reach a named/actual buffer and insertion uses its point.
  (qa (let ((target (generate-new-buffer " *qa25-target*")))
        (unwind-protect
            (progn
              (with-current-buffer target (insert "before\nafter") (goto-char 8))
              (qa25-run target qa25-region)
              (with-current-buffer target (buffer-string)))
          (kill-buffer target))) "before\nO1\nE1\nO2\nE2\nafter")
  (qa (let ((name (generate-new-buffer-name " *qa25-name*")))
        (unwind-protect
            (progn (qa25-run name qa25-region)
                   (with-current-buffer name (buffer-string)))
          (when (get-buffer name) (kill-buffer name)))) "O1\nE1\nO2\nE2\n")

  ;; Explicit stderr files overwrite their previous contents; a file
  ;; stdout destination works independently, including with region input.
  (let ((qa25-error-file (make-temp-file "qa25-error-"))
        (qa25-output-file (make-temp-file "qa25-output-")))
    (unwind-protect
        (progn
          (with-temp-file qa25-error-file (insert "old error contents"))
          (qa (qa25-run (list t qa25-error-file) qa25-region) '(7 "O1\nO2\n"))
          (qa (with-temp-buffer (insert-file-contents qa25-error-file) (buffer-string))
              "E1\nE2\n")
          (qa (qa25-run (list (list :file qa25-output-file) qa25-error-file) qa25-region)
              '(7 ""))
          (qa (with-temp-buffer (insert-file-contents qa25-output-file) (buffer-string))
              "O1\nO2\n")
          (qa (with-temp-buffer (insert-file-contents qa25-error-file) (buffer-string))
              "E1\nE2\n")
          (qa (qa25-run (list :file qa25-output-file) qa25-region) '(7 ""))
          (qa (with-temp-buffer (insert-file-contents qa25-output-file) (buffer-string))
              "O1\nE1\nO2\nE2\n"))
      (delete-file qa25-error-file)
      (delete-file qa25-output-file))))

;; Preserve input, delete it when requested, and never append stderr
;; after all stdout: interleaving is the order of child writes.
(qa (with-temp-buffer
      (insert "input\n")
      (call-process-region 1 7 "sh" t t nil "-c"
                           "cat; printf 'err\\n' >&2; printf 'last\\n'")
      (buffer-string)) "input\nerr\nlast\n")
(qa (with-temp-buffer
      (insert "input\n")
      (call-process-region 1 7 "sh" t '(t nil) nil "-c"
                           "cat; printf 'err\\n' >&2; printf 'last\\n'")
      (buffer-string)) "input\nlast\n")
(qa (with-temp-buffer
      (process-file "sh" nil t nil "-c" qa25-command) (buffer-string))
    "O1\nE1\nO2\nE2\n")
(qa (with-temp-buffer
      (process-file "sh" nil '(t nil) nil "-c" qa25-command) (buffer-string))
    "O1\nO2\n")
(qa (with-temp-buffer
      (call-process-shell-command qa25-command nil t) (buffer-string))
    "O1\nE1\nO2\nE2\n")
(qa (with-temp-buffer
      (shell-command qa25-command t) (buffer-string)) "O1\nE1\nO2\nE2\n")
(qa (shell-command-to-string qa25-command) "O1\nE1\nO2\nE2\n")

(qa (let ((output (generate-new-buffer " *qa25-shell-output*"))
          (errors (generate-new-buffer " *qa25-shell-errors*")))
      (unwind-protect
          (progn (shell-command qa25-command output errors)
                 (list (with-current-buffer output (buffer-string))
                       (with-current-buffer errors (buffer-string))))
        (kill-buffer output) (kill-buffer errors))) '("O1\nO2\n" "E1\nE2\n"))

;; The async path already merges stderr. Keep this behavior pinned too.
(qa (let* ((buffer (generate-new-buffer " *qa25-async*"))
           (process (start-process "qa25-async" buffer "sh" "-c" qa25-command))
           (deadline (+ (float-time) 5)))
      (unwind-protect
          (progn
            (set-process-query-on-exit-flag process nil)
            (set-process-sentinel process #'ignore)
            (while (and (process-live-p process) (< (float-time) deadline))
              (accept-process-output process 0.05))
            (accept-process-output process 0.05)
            (list (process-exit-status process)
                  (with-current-buffer buffer (buffer-string))))
        (delete-process process) (kill-buffer buffer))) '(7 "O1\nE1\nO2\nE2\n"))

(princ (qa--done)) (terpri)
