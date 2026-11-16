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

(defun qa--mailcap-extra-fields (text)
  (with-temp-buffer
    (set-syntax-table mailcap-parse-args-syntax-table)
    (insert text)
    (mailcap-parse-mailcap-extras (point-min) (point-max))))

(qa (let ((fields (qa--mailcap-extra-fields
                   (concat ";x=foo" (string 92 92) "; copiousoutput"))))
      (list (assoc "x" fields) (assoc "copiousoutput" fields)))
    (list (cons "x" (concat "foo" (string 92)))
          (cons "copiousoutput" t)))

(qa (let ((fields (qa--mailcap-extra-fields
                   (concat ";description=foo" (string 92)
                           ";bar; other=value"))))
      (list (assoc "description" fields) (assoc "other" fields)))
    '(("description" . "foo;bar") ("other" . "value")))

(qa (let ((fields (qa--mailcap-extra-fields ";x==foo")))
      (assoc "x" fields))
    '("x" . "=foo"))

(qa (let ((fields (qa--mailcap-extra-fields ";x='foo'; copiousoutput")))
      (list (assoc "x" fields) (assoc "copiousoutput" fields)))
    '(("x" . "foo") ("copiousoutput" . t)))

(qa (let ((fields (qa--mailcap-extra-fields
                   (concat ";description=\"foo" (string 92 34)
                           "bar\"; other=value"))))
      (list (assoc "description" fields) (assoc "other" fields)))
    (list (cons "description" (concat "foo" (string 34) "bar"))
          '("other" . "value")))

(princ (qa--done))
