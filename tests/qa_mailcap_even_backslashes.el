;;; An even backslash run does not escape a mailcap field separator.

(require 'mailcap)

(qa (let ((mailcap--computed-mime-data nil)
          (file (make-temp-file "qa-mailcap-")))
      (unwind-protect
          (progn
            (with-temp-file file
              (insert "application/x-qa-double; cmd \\\\; copiousoutput\n"))
            (mailcap-parse-mailcap file)
            (let* ((major (assoc "application" mailcap--computed-mime-data))
                   (entry (assoc "x-qa-double" major))
                   (info (cdr entry)))
              (list (cdr (assq 'viewer info))
                    (assoc "copiousoutput" info))))
        (delete-file file)))
    '("cmd \\\\" ("copiousoutput" . t)))

(princ (qa--done))
