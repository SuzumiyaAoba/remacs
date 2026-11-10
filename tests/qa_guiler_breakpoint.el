;;; qa_guiler_breakpoint.el --- Guile GUD breakpoint path regression -*- lexical-binding:t -*-

(require 'cl-lib)

(load (expand-file-name
       "../lisp/gud.el"
       (file-name-directory load-file-name)))

(defun qa-guiler-breakpoint-commands (file line source-buffer)
  "Collect breakpoint commands for FILE and LINE from SOURCE-BUFFER."
  (with-temp-buffer
    (when source-buffer
      (setq buffer-file-name file)
      (insert "source\n")
      (goto-char (point-min)))
    (let ((gud-comint-buffer (unless source-buffer (current-buffer)))
          (gud-last-frame (unless source-buffer (cons file line)))
          (gud-last-last-frame nil)
          commands)
      (cl-letf (((symbol-function 'gud-basic-call)
                 (lambda (command) (push command commands))))
        (gud-guiler-set-breakpoint))
      (nreverse commands))))

(qa
 (qa-guiler-breakpoint-commands "/tmp/guile source/program.scm" nil t)
 '("(unless (member \"/tmp/guile source/\" %load-path) (add-to-load-path \"/tmp/guile source/\"))"
   ",break-at-source program.scm 1"))

(qa
 (qa-guiler-breakpoint-commands "/opt/scheme/lib/module.scm" 12 nil)
 '("(unless (member \"/opt/scheme/lib/\" %load-path) (add-to-load-path \"/opt/scheme/lib/\"))"
   ",break-at-source module.scm 12"))

(defun qa-guiler-break-command ()
  "Return the argument passed by `gud-break' to the Guile handler."
  (with-temp-buffer
    (use-local-map (make-sparse-keymap))
    (let ((gud-running nil)
          breakpoint-argument)
      (cl-letf (((symbol-function 'gud-common-init)
                 (lambda (&rest _arguments) nil))
                ((symbol-function 'gud-set-repeat-map-property)
                 (lambda (_keymap) nil))
                ((symbol-function 'gud-guiler-set-breakpoint)
                 (lambda (&optional argument)
                   (setq breakpoint-argument argument))))
        (guiler "guile")
        (gud-break 7))
      breakpoint-argument)))

(qa (qa-guiler-break-command) 7)

(princ (qa--done))
