;;; -*- lexical-binding: t; -*-
;; Issue #202 / AUD-038: decoded characters span actual process reads.
;; Use stdin as an acknowledgement, forcing a read before the second fragment.
(defun qa202-split (first second)
  (let ((buffer (generate-new-buffer " *qa202*")) p)
    (unwind-protect
        (progn
          (setq p (make-process :name "qa202" :buffer buffer :noquery t
                               :connection-type 'pipe :coding 'utf-8 :sentinel #'ignore
                               :command (list "/bin/sh" "-c"
                                 (concat "printf '" first "'; IFS= read -r line; printf '" second "'"))))
          (let ((deadline (+ (float-time) 3)) read)
            (while (and (not read) (< (float-time) deadline))
              (setq read (accept-process-output p 0.01)))
            (unless read (error "first fragment was not read")))
          (let ((before (with-current-buffer buffer (string-to-list (buffer-string)))))
            (process-send-string p "go\n")
            (let ((deadline (+ (float-time) 3)))
              (while (and (process-live-p p) (< (float-time) deadline))
                (accept-process-output p 0.01)))
            (when (process-live-p p) (error "child did not finish"))
            (accept-process-output p 0.01)
            (list before (with-current-buffer buffer (string-to-list (buffer-string))))))
      (when (and p (process-live-p p)) (delete-process p))
      (kill-buffer buffer))))
(qa (qa202-split "\\343" "\\201\\202") '(nil (12354)))
(qa (qa202-split "\\343\\201" "\\202") '(nil (12354)))
(qa (qa202-split "\\360\\237" "\\230\\200") '(nil (128512)))
(qa (qa202-split "\\343" "") '(nil (4194275)))
(princ (qa--done)) (terpri)
