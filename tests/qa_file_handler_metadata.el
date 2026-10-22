;;; qa_file_handler_metadata.el --- Dispatch metadata file handlers -*- lexical-binding: t; -*-

(defvar qa328-log nil)
(defvar qa328-result '(handled))
(defvar qa328-directory-name nil)
(defun qa328-handler (operation &rest arguments)
  (cond ((eq operation 'expand-file-name)
         (let ((file-name-handler-alist nil)) (apply #'expand-file-name arguments)))
        ((eq operation 'directory-file-name)
         (if qa328-directory-name
             (progn (push (list 'primary operation arguments) qa328-log)
                    qa328-directory-name)
           (let ((file-name-handler-alist nil)) (apply #'directory-file-name arguments))))
        (t (push (list 'primary operation arguments) qa328-log)
           qa328-result)))
(defun qa328-secondary (operation &rest arguments)
  (if (eq operation 'expand-file-name)
      (let ((file-name-handler-alist nil)) (apply #'expand-file-name arguments))
    (push (list 'secondary operation arguments) qa328-log)
    'secondary-result))
(defun qa328-run (operation arguments &optional operations)
  (let ((qa328-log nil)
        (old-operations (get 'qa328-handler 'operations))
        (default-directory "/qa328-virtual/")
        (file-name-handler-alist
         '(("\\`/qa328-virtual/" . qa328-handler)
           ("\\`qa328:" . qa328-handler)
           ("\\`/qa328-secondary/" . qa328-secondary))))
    (unwind-protect
        (progn
          (put 'qa328-handler 'operations operations)
          (list (condition-case error (apply operation arguments) (error (car error)))
                (nreverse qa328-log)))
      (put 'qa328-handler 'operations old-operations))))

;; access-file absolute handler
(qa (qa328-run #'access-file '("/qa328-virtual/file" "check access"))
    '((handled) ((primary access-file ("/qa328-virtual/file" "check access")))))

;; file-equal-p absolute handler
(qa (qa328-run #'file-equal-p '("/qa328-virtual/file" "/qa328-virtual/file"))
    '((handled) ((primary file-equal-p ("/qa328-virtual/file" "/qa328-virtual/file")))))

;; file-newer-than-file-p absolute handler
(qa (qa328-run #'file-newer-than-file-p '("/qa328-virtual/file" "/qa328-virtual/file"))
    '((handled) ((primary file-newer-than-file-p ("/qa328-virtual/file" "/qa328-virtual/file")))))

;; file-directory-p absolute handler
(qa (qa328-run #'file-directory-p '("/qa328-virtual/file"))
    '((handled) ((primary file-directory-p ("/qa328-virtual/file")))))

;; file-regular-p absolute handler
(qa (qa328-run #'file-regular-p '("/qa328-virtual/file"))
    '((handled) ((primary file-regular-p ("/qa328-virtual/file")))))

;; file-executable-p absolute handler
(qa (qa328-run #'file-executable-p '("/qa328-virtual/file"))
    '((handled) ((primary file-executable-p ("/qa328-virtual/file")))))

;; file-attributes absolute handler
(qa (qa328-run #'file-attributes '("/qa328-virtual/file"))
    '((handled) ((primary file-attributes ("/qa328-virtual/file")))))

;; file-symlink-p absolute handler
(qa (qa328-run #'file-symlink-p '("/qa328-virtual/file"))
    '((handled) ((primary file-symlink-p ("/qa328-virtual/file")))))

;; file-local-copy absolute handler
(qa (qa328-run #'file-local-copy '("/qa328-virtual/file"))
    '((handled) ((primary file-local-copy ("/qa328-virtual/file")))))

;; file-exists-p absolute handler
(qa (qa328-run #'file-exists-p '("/qa328-virtual/file"))
    '((handled) ((primary file-exists-p ("/qa328-virtual/file")))))

;; file-modes absolute handler
(qa (qa328-run #'file-modes '("/qa328-virtual/file"))
    '((handled) ((primary file-modes ("/qa328-virtual/file" nil)))))

;; access-file relative handler
(qa (qa328-run #'access-file '("qa328:file" "check access"))
    '((handled) ((primary access-file ("/qa328-virtual/qa328:file" "check access")))))

;; file-equal-p relative handler
(qa (qa328-run #'file-equal-p '("qa328:file" "qa328:other"))
    '((handled) ((primary file-equal-p ("qa328:file" "qa328:other")))))

;; file-newer-than-file-p relative handler
(qa (qa328-run #'file-newer-than-file-p '("qa328:file" "qa328:other"))
    '((handled) ((primary file-newer-than-file-p ("/qa328-virtual/qa328:file" "/qa328-virtual/qa328:other")))))

;; file-directory-p relative handler
(qa (qa328-run #'file-directory-p '("qa328:file"))
    '((handled) ((primary file-directory-p ("/qa328-virtual/qa328:file")))))

;; file-regular-p relative handler
(qa (qa328-run #'file-regular-p '("qa328:file"))
    '((handled) ((primary file-regular-p ("/qa328-virtual/qa328:file")))))

;; file-executable-p relative handler
(qa (qa328-run #'file-executable-p '("qa328:file"))
    '((handled) ((primary file-executable-p ("/qa328-virtual/qa328:file")))))

;; file-attributes relative handler
(qa (qa328-run #'file-attributes '("qa328:file"))
    '((handled) ((primary file-attributes ("/qa328-virtual/qa328:file")))))

;; file-symlink-p relative handler
(qa (qa328-run #'file-symlink-p '("qa328:file"))
    '((handled) ((primary file-symlink-p ("/qa328-virtual/qa328:file")))))

;; file-local-copy relative handler
(qa (qa328-run #'file-local-copy '("qa328:file"))
    '((handled) ((primary file-local-copy ("qa328:file")))))

;; file-directory-p strips directory slash
(qa (qa328-run #'file-directory-p '("/qa328-virtual/dir/"))
    '((handled) ((primary file-directory-p ("/qa328-virtual/dir")))))

;; file-regular-p strips directory slash
(qa (qa328-run #'file-regular-p '("/qa328-virtual/dir/"))
    '((handled) ((primary file-regular-p ("/qa328-virtual/dir")))))

;; file-newer-than-file-p strips directory slash
(qa (qa328-run #'file-newer-than-file-p '("/qa328-virtual/dir/" "/qa328-virtual/dir/"))
    '((handled) ((primary file-newer-than-file-p ("/qa328-virtual/dir" "/qa328-virtual/dir")))))

;; file-attributes optional format nil
(qa (qa328-run #'file-attributes '("/qa328-virtual/file" nil))
    '((handled) ((primary file-attributes ("/qa328-virtual/file")))))

;; file-attributes optional format string
(qa (qa328-run #'file-attributes '("/qa328-virtual/file" string))
    '((handled) ((primary file-attributes ("/qa328-virtual/file" string)))))

;; file-attributes optional format integer
(qa (qa328-run #'file-attributes '("/qa328-virtual/file" integer))
    '((handled) ((primary file-attributes ("/qa328-virtual/file" integer)))))

;; file-equal-p operand handler precedence
(qa (qa328-run #'file-equal-p '("/qa328-virtual/file" "/qa328-secondary/file"))
    '((handled) ((primary file-equal-p ("/qa328-virtual/file" "/qa328-secondary/file")))))

;; file-equal-p operand handler precedence
(qa (qa328-run #'file-equal-p '("/qa328-secondary/file" "/qa328-virtual/file"))
    '(secondary-result ((secondary file-equal-p ("/qa328-secondary/file" "/qa328-virtual/file")))))

;; file-equal-p operand handler precedence
(qa (qa328-run #'file-equal-p '("/qa328-physical-file" "/qa328-secondary/file"))
    '(secondary-result ((secondary file-equal-p ("/qa328-physical-file" "/qa328-secondary/file")))))

;; file-newer-than-file-p operand handler precedence
(qa (qa328-run #'file-newer-than-file-p '("/qa328-virtual/file" "/qa328-secondary/file"))
    '((handled) ((primary file-newer-than-file-p ("/qa328-virtual/file" "/qa328-secondary/file")))))

;; file-newer-than-file-p operand handler precedence
(qa (qa328-run #'file-newer-than-file-p '("/qa328-secondary/file" "/qa328-virtual/file"))
    '(secondary-result ((secondary file-newer-than-file-p ("/qa328-secondary/file" "/qa328-virtual/file")))))

;; file-newer-than-file-p operand handler precedence
(qa (qa328-run #'file-newer-than-file-p '("/qa328-physical-file" "/qa328-secondary/file"))
    '(secondary-result ((secondary file-newer-than-file-p ("/qa328-physical-file" "/qa328-secondary/file")))))

;; access-file validation order
(qa (qa328-run #'access-file '(42 "check"))
    '(wrong-type-argument nil))

;; access-file validation order
(qa (qa328-run #'access-file '("/qa328-virtual/file" 42))
    '(wrong-type-argument nil))

;; file-attributes validation order
(qa (qa328-run #'file-attributes '(42))
    '(nil nil))

;; file-equal-p validation order
(qa (qa328-run #'file-equal-p '("/qa328-virtual/file" 42))
    '((handled) ((primary file-equal-p ("/qa328-virtual/file" 42)))))

;; file-newer-than-file-p validation order
(qa (qa328-run #'file-newer-than-file-p '("/qa328-virtual/file" 42))
    '(wrong-type-argument nil))

;; handler operations property file-directory-p
(qa (qa328-run #'file-directory-p '("/qa328-virtual/file") '(file-directory-p))
    '((handled) ((primary file-directory-p ("/qa328-virtual/file")))))

;; handler operations property file-attributes
(qa (qa328-run #'file-directory-p '("/qa328-virtual/file") '(file-attributes))
    '(nil nil))

;; arbitrary handler return identity
(qa (let ((qa328-result (vector 'answer))) (mapcar (lambda (entry) (eq (car (apply #'qa328-run entry)) qa328-result)) '((access-file ("/qa328-virtual/file" "check access")) (file-equal-p ("/qa328-virtual/file" "/qa328-virtual/file")) (file-newer-than-file-p ("/qa328-virtual/file" "/qa328-virtual/file")) (file-directory-p ("/qa328-virtual/file")) (file-regular-p ("/qa328-virtual/file")) (file-executable-p ("/qa328-virtual/file")) (file-attributes ("/qa328-virtual/file")) (file-symlink-p ("/qa328-virtual/file")) (file-local-copy ("/qa328-virtual/file")))))
    '(t t t t t t t t t))

;; file-directory-p redirected directory handler
(qa (let ((qa328-directory-name "/qa328-secondary/redirected")) (qa328-run #'file-directory-p '("/qa328-virtual/dir/")))
    '(secondary-result ((primary directory-file-name ("/qa328-virtual/dir/")) (secondary file-directory-p ("/qa328-secondary/redirected")))))

;; file-directory-p invalid directory handler result
(qa (let ((qa328-directory-name 'invalid)) (qa328-run #'file-directory-p '("/qa328-virtual/dir/")))
    '(error ((primary directory-file-name ("/qa328-virtual/dir/")))))

;; file-regular-p redirected directory handler
(qa (let ((qa328-directory-name "/qa328-secondary/redirected")) (qa328-run #'file-regular-p '("/qa328-virtual/dir/")))
    '(secondary-result ((primary directory-file-name ("/qa328-virtual/dir/")) (secondary file-regular-p ("/qa328-secondary/redirected")))))

;; file-regular-p invalid directory handler result
(qa (let ((qa328-directory-name 'invalid)) (qa328-run #'file-regular-p '("/qa328-virtual/dir/")))
    '(error ((primary directory-file-name ("/qa328-virtual/dir/")))))

;; file-newer-than-file-p redirected directory handler
(qa (let ((qa328-directory-name "/qa328-secondary/redirected")) (qa328-run #'file-newer-than-file-p '("/qa328-virtual/dir/" "/qa328-virtual/dir/")))
    '(secondary-result ((primary directory-file-name ("/qa328-virtual/dir/")) (primary directory-file-name ("/qa328-virtual/dir/")) (secondary file-newer-than-file-p ("/qa328-secondary/redirected" "/qa328-secondary/redirected")))))

;; file-newer-than-file-p invalid directory handler result
(qa (let ((qa328-directory-name 'invalid)) (qa328-run #'file-newer-than-file-p '("/qa328-virtual/dir/" "/qa328-virtual/dir/")))
    '(error ((primary directory-file-name ("/qa328-virtual/dir/")))))

(princ (qa--done))
