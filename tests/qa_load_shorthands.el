;;; -*- lexical-binding: t; -*-
(defvar qa-short-load-value nil)
(defvar qa-short-load-buffer nil)
(defun qa-short-load-text (text &optional suffix)
  (let ((file (make-temp-file "qa-short-load-" nil (or suffix ".el"))))
    (unwind-protect
        (progn
          (let ((coding-system-for-write 'binary)) (write-region text nil file nil 'quiet))
          (load file nil t t))
      (delete-file file))))
(defconst qa-short-load-source
  ";;; -*- lexical-binding: t; -*-\n(setq qa-short-load-value (list 's-x read-symbol-shorthands))\n")

(qa (let* ((outer '(("s-" . "outer-"))) (read-symbol-shorthands outer)
           (hack-read-symbol-shorthands-function nil))
      (qa-short-load-text qa-short-load-source)
      (list qa-short-load-value (eq read-symbol-shorthands outer))) '((s-x nil) t))
(qa (let* ((outer '(("s-" . "outer-"))) (rules '(("s-" . "local-")))
           (read-symbol-shorthands outer)
           (hack-read-symbol-shorthands-function (lambda () rules)))
      (qa-short-load-text qa-short-load-source)
      (list (car qa-short-load-value) (eq (cadr qa-short-load-value) rules)
            (eq read-symbol-shorthands outer))) '(local-x t t))
(qa (let ((hack-read-symbol-shorthands-function #'hack-read-symbol-shorthands))
      (qa-short-load-text
       (concat qa-short-load-source
               ";; Local Variables:\n;; read-symbol-shorthands: ((\"s-\" . \"file-local-\"))\n;; End:\n"))
      (car qa-short-load-value)) 'file-local-x)
(qa (let* ((input 'qa-short-input) (output (lambda (_) nil))
           (scope (list 'qa-short-scope)) (outer '(("s-" . "outer-")))
           (standard-input input) (standard-output output)
           (eval-buffer-list scope) (read-symbol-shorthands outer) seen
           (hack-read-symbol-shorthands-function
            (lambda ()
              (setq seen
                    (list (string-prefix-p " *load*" (buffer-name)) (= (point) 1)
                          load-in-progress (eq load-file-name load-true-file-name)
                          (eq standard-input input) (eq standard-output output)
                          (eq eval-buffer-list scope) (eq read-symbol-shorthands outer)
                          set-auto-coding-for-load (null inhibit-file-name-operation)
                          (buffer-modified-p)))
              nil)))
      (qa-short-load-text qa-short-load-source) seen)
    '(t t t t t t t t t t t))
(qa (let ((read-symbol-shorthands '(("s-" . "outer-")))
          (hack-read-symbol-shorthands-function (lambda () (error "source hook called"))))
      (qa-short-load-text (concat ";ELC\037\0\0\0\n;;; in Emacs version 31.1\n"
                                   "(setq qa-short-load-value (list 's-x read-symbol-shorthands))\n") ".elc")
      qa-short-load-value) '(s-x nil))
(qa (let* ((outer '(("s-" . "outer-"))) (read-symbol-shorthands outer)
           (hack-read-symbol-shorthands-function (lambda () (error "source hook called"))))
      (qa-short-load-text (concat ";ELC\037\0\0\0\n;;; in Emacs version 31.1\n"
                                   "(setq qa-short-load-value (list 's-x read-symbol-shorthands))\n"))
      (list (car qa-short-load-value) (eq (cadr qa-short-load-value) outer))) '(outer-x t))
(qa (let ((read-symbol-shorthands '(("s-" . "outer-"))))
      (with-temp-buffer
        (insert qa-short-load-source)
        (eval-buffer (current-buffer) nil "qa-short.elc"))
      qa-short-load-value) '(s-x nil))
(qa (let* ((outer '(("s-" . "outer-"))) (read-symbol-shorthands outer))
      (with-temp-buffer
        (setq buffer-file-name "qa-short.elc")
        (insert qa-short-load-source)
        (eval-region (point-min) (point-max)))
      (list (car qa-short-load-value) (eq (cadr qa-short-load-value) outer))) '(s-x nil))
(qa (let* ((caller (current-buffer)) (outer '(("s-" . "outer-")))
           (read-symbol-shorthands outer)
           (hack-read-symbol-shorthands-function
            (lambda () (setq qa-short-load-buffer (current-buffer)) (error "short hook"))))
      (list (condition-case error (qa-short-load-text qa-short-load-source) (error error))
            (eq (current-buffer) caller) (eq read-symbol-shorthands outer)
            (buffer-live-p qa-short-load-buffer))) '((error "short hook") t t nil))
(qa (let* ((rules '(("s-" . "edited-")))
           (hack-read-symbol-shorthands-function
            (lambda ()
              (erase-buffer)
              (insert ";;; -*- lexical-binding: t; -*-\n(setq qa-short-load-value 's-edited)\n")
              rules)))
      (qa-short-load-text qa-short-load-source)
      qa-short-load-value) 'edited-edited)
(princ (qa--done)) (terpri)
