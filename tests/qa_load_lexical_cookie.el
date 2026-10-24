;;; File cookies and the default lexical-binding callback. -*- lexical-binding: t; -*-
(defvar qa-lc-result nil)
(defvar qa-lc-file nil)
(defvar qa-lc-warnings nil)
(defvar qa-lc-events nil)
(defvar qa-lc-source nil)
(defvar qa-lc-case nil)
(defconst qa-lc-body
  "(setq qa-lc-result (list lexical-binding (let ((qa-lc-value 7)) (lambda () qa-lc-value))))\n")
(defun qa-lc-load (text &optional suffix)
  (let* ((file (make-temp-file "qa-load-cookie-" nil (or suffix ".el")))
         (qa-lc-file file)
         (qa-lc-result nil))
    (unwind-protect
        (progn
          (with-temp-file file (insert text))
          (load file nil t)
          qa-lc-result)
      (delete-file file))))
(defun qa-lc-probe (header)
  (let* ((qa-lc-warnings nil)
         (result (qa-lc-load (concat header qa-lc-body))))
    (list (car result)
          (condition-case err (funcall (cadr result)) (error (car err)))
          (length qa-lc-warnings))))

;; The default Lisp function reports only absent cookies, including nil
;; cookies and arbitrary values other than the exact spelling "nil".
(let ((saved (symbol-function 'display-warning)))
  (unwind-protect
      (progn
        (fset 'display-warning
              (lambda (&rest arguments) (push (car arguments) qa-lc-warnings)))
        (dolist (qa-lc-case
                 '(("; -*- lexical-binding: nil -*-\n" (nil void-variable 0))
                   ("; -*- lexical-binding: t -*-\n" (t 7 0))
                   ("; -*- lexical-binding: false -*-\n" (t 7 0))
                   ("; -*- lexical-binding: () -*-\n" (t 7 0))
                   ("; -*- lexical-binding: NIL -*-\n" (t 7 0))
                   ("; -*- lexical-binding: -*-\n" (t 7 0))
                   ("; -*- lexical-binding: nilpotent -*-\n" (t 7 0))
                   ("; -*- lexical-binding: nil; lexical-binding: t -*-\n" (nil void-variable 0))
                   ("; -*- lexical-binding: t; lexical-binding: nil -*-\n" (t 7 0))
                   ("; lexical-binding: t\n" (nil void-variable 1))
                   (" ; -*- lexical-binding: t -*-\n" (nil void-variable 1))
                   ("\n; -*- lexical-binding: t -*-\n" (nil void-variable 1))
                   ("; plain\n; -*- lexical-binding: t -*-\n" (nil void-variable 1))
                   ("#! script\n; -*- lexical-binding: nil -*-\n" (nil void-variable 0))
                   ("#! script\n; -*- lexical-binding :\tt\t -*-\n" (t 7 0))
                   ("; -*- mode: emacs-lisp; lexical-binding:t; -*-\n" (t 7 0))
                   ("; -*- mode;other: ignored; lexical-binding:t -*-\n" (t 7 0))
                   ("; -*- lexical-binding: nil\n" (nil void-variable 0))
                   ("; -*- lexical-binding: nil -*- lexical-binding: t\n" (nil void-variable 0))
                   ("; --*- lexical-binding: t -*-\n" (nil void-variable 1))
                   ("; -*- lexical-binding\0ignored: nil -*-\n" (nil void-variable 0))
                   ("; -*- lexical-binding: nil\0ignored -*-\n" (nil void-variable 0))))
          (qa (qa-lc-probe (car qa-lc-case)) (cadr qa-lc-case))))
    (fset 'display-warning saved)))

(qa (let ((internal--get-default-lexical-binding-function (lambda (_) (error "unexpected"))))
      (car (qa-lc-load (concat "; -*- lexical-binding: nil -*-\n" qa-lc-body)))) nil)
(qa (let ((internal--get-default-lexical-binding-function (lambda (_) (error "unexpected"))))
      (car (qa-lc-load (concat "; -*- lexical-binding: t -*-\n" qa-lc-body)))) t)
(qa (let* ((value (list 'selected))
           (internal--get-default-lexical-binding-function (lambda (_) value)))
      (eq (car (qa-lc-load qa-lc-body)) value)) t)

(qa (let ((lexical-binding t) (internal--get-default-lexical-binding-function nil))
      (car (qa-lc-load qa-lc-body))) nil)
(qa (let ((saved (default-toplevel-value 'lexical-binding)))
      (unwind-protect
          (progn
            (set-default-toplevel-value 'lexical-binding t)
            (let* ((lexical-binding nil)
                   (internal--get-default-lexical-binding-function nil)
                   (result (qa-lc-load qa-lc-body)))
              (list (car result) (funcall (cadr result)))))
        (set-default-toplevel-value 'lexical-binding saved))) '(t 7))
(qa (let ((saved (default-toplevel-value 'lexical-binding)))
      (unwind-protect
          (progn
            (set-default-toplevel-value 'lexical-binding t)
            (let ((internal--get-default-lexical-binding-function (lambda (_) nil)))
              (car (qa-lc-load qa-lc-body))))
        (set-default-toplevel-value 'lexical-binding saved))) nil)

;; The fallback runs before standard-input and lexical-binding acquire
;; their evaluation values; source conversion already bound output to nil.
(qa (let (qa-lc-events qa-lc-source result)
      (with-temp-buffer
        (insert "caller") (goto-char 4)
        (let* ((caller (current-buffer))
               (standard-input 'outside-input)
               (standard-output 'outside-output)
               (lexical-binding 'outside-binding)
               (internal--get-default-lexical-binding-function
                (lambda (source)
                  (setq qa-lc-source source)
                  (push (list (bufferp source) (eq source (car eval-buffer-list))
                              standard-input standard-output lexical-binding
                              (eq (current-buffer) caller)
                              (with-current-buffer source (point))
                              (equal load-file-name qa-lc-file)) qa-lc-events)
                  'selected)))
          (setq result
                (qa-lc-load
                 "; comment\n(setq qa-lc-result (list lexical-binding (bufferp standard-input) (eq standard-input (car eval-buffer-list)) standard-output))\n"))))
      (list qa-lc-events result (buffer-live-p qa-lc-source)))
    '(((t t outside-input nil nil t 11 t)) (selected t t nil) nil))

(qa (let ((internal--get-default-lexical-binding-function
           (lambda (source)
             (with-current-buffer source
               (erase-buffer) (insert "(setq qa-lc-result 'changed)"))
             t)))
      (qa-lc-load "; comment\n(setq qa-lc-result 'stale)")) 'changed)
(qa (let (qa-lc-events
           (internal--get-default-lexical-binding-function
            (lambda (source) (push (read source) qa-lc-events) t)))
      (qa-lc-load "; comment\n(push :body qa-lc-events)\n(setq qa-lc-result qa-lc-events)"))
    '(:body (push :body qa-lc-events)))

(qa (let ((standard-input 'outside-input) (standard-output 'outside-output)
          (eval-buffer-list '(outside-list)) (lexical-binding 'outside-binding)
          (internal--get-default-lexical-binding-function (lambda (_) (error "cookie hook"))))
      (list (condition-case err (qa-lc-load "; comment\n42") (error err))
            standard-input standard-output eval-buffer-list lexical-binding))
    '((error "cookie hook") outside-input outside-output (outside-list) outside-binding))
(qa (let ((internal--get-default-lexical-binding-function (lambda (_) (throw 'cookie-abort 'aborted))))
      (catch 'cookie-abort (qa-lc-load "; comment\n42"))) 'aborted)

(qa (let (qa-lc-events
           (signal-hook-function
            (lambda (symbol _data)
              (when (eq symbol 'invalid-read-syntax)
                (push (list (equal load-file-name qa-lc-file)
                            (equal load-true-file-name qa-lc-file)
                            load-in-progress lexical-binding
                            (eq standard-input (car eval-buffer-list))
                            (and (bufferp standard-input) (buffer-live-p standard-input)))
                      qa-lc-events)))))
      (condition-case nil
          (qa-lc-load "; -*- lexical-binding: t -*-\n)\n")
        (invalid-read-syntax nil))
      qa-lc-events) '((t t t t t t)))

;; Compiled streams ignore the fallback, but still honor their first cookie.
(qa (let ((internal--get-default-lexical-binding-function (lambda (_) (error "unexpected"))))
      (car (qa-lc-load (concat ";ELC" (string 31 0 0 0)
                              "\n;;; Compiled\n;;; in Emacs version 31.1\n" qa-lc-body) ".elc"))) nil)
(qa (let ((internal--get-default-lexical-binding-function (lambda (_) (error "unexpected"))))
      (car (qa-lc-load (concat "; -*- lexical-binding: false -*-\n"
                              ";;; Compiled\n;;; in Emacs version 31.1\n" qa-lc-body) ".elc"))) t)
(qa (let ((byte-optimize t))
      (qa-lc-load "; -*- lexical-binding: t -*-\n(setq qa-lc-result byte-optimize)")) t)
(qa (let ((byte-optimize nil))
      (qa-lc-load "; -*- lexical-binding: t -*-\n(setq qa-lc-result byte-optimize)")) nil)
(princ (qa--done)) (terpri)
