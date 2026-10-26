;;; Native compiled names preserve GNU's logical and C-name views. -*- lexical-binding: nil; -*-
;; #653: explicit GNU 31.1 byte spellings for surrogate C names.
(let ((directory (make-temp-file "qa-native-wide-names-" t))
      (serial 0) (names nil))
  (unwind-protect
      (progn
        (unless (native-comp-available-p) (error "Actual native compilation is required"))
        (dolist (case '(((983168) (983168) t)
                        ((55295) (55295) t)
                        ((55296) (193 173 192 160 192 128) nil)
                        ((57343) (193 173 192 191 192 191) nil)
                        ((57344) (57344) t)
                        ((1703936) (1703936) t)
                        ((983168 55296) (243 176 130 128 193 173 192 160 192 128) nil)))
          (let* ((text (concat "qa-native-wide-name-" (apply #'string (car case))))
                 (name (intern text)))
            (push name names)
            (fset name (lambda () 42))
            (let ((compiled (native-compile name
                             (expand-file-name (format "name-%d.eln" serial) directory))))
              (setq serial (1+ serial))
              (qa (native-comp-function-p compiled) t)
              (qa (funcall compiled) 42)
              (qa (string-to-list (subr-name compiled))
                  (append (string-to-list "qa-native-wide-name-") (cadr case)))
              (qa (multibyte-string-p (subr-name compiled)) (nth 2 case))))))
    (dolist (name names) (fmakunbound name))
    (delete-directory directory t)))
(princ (qa--done))
