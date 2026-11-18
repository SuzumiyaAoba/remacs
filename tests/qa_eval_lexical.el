;;; -*- lexical-binding: t; -*-
;; Issue #266: eval-buffer chooses lexical mode from source/local/default.
(defvar qa266-result nil)
(defun qa266-probe (header local-value caller-value &optional other)
 (let ((qa266-result nil) (target (generate-new-buffer " *qa266-probe*")))
  (unwind-protect
   (progn
    (with-current-buffer target
     (insert header "(setq qa266-result (list lexical-binding (let ((qa266-x 7)) (lambda () qa266-x))))")
     (when (not (eq local-value 'absent)) (setq-local lexical-binding local-value)))
    (let ((lexical-binding caller-value))
     (if other (eval-buffer target)
      (with-current-buffer target (eval-buffer))))
    (list (car qa266-result)
     (condition-case e (funcall (cadr qa266-result)) (error (car e)))))
   (kill-buffer target))))
(qa
(qa266-probe ";;; -*- lexical-binding: t -*-\n" 'absent nil nil)
'(t 7)
)

(qa
(qa266-probe ";;; -*- lexical-binding: t -*-\n" 'absent nil t)
'(t 7)
)

(qa
(qa266-probe ";;; -*- lexical-binding: t -*-\n" 'absent t nil)
'(t 7)
)

(qa
(qa266-probe ";;; -*- lexical-binding: t -*-\n" 'absent t t)
'(t 7)
)

(qa
(qa266-probe ";;; -*- lexical-binding: t -*-\n" 'nil nil nil)
'(nil void-variable)
)

(qa
(qa266-probe ";;; -*- lexical-binding: t -*-\n" 'nil nil t)
'(nil void-variable)
)

(qa
(qa266-probe ";;; -*- lexical-binding: t -*-\n" 'nil t nil)
'(nil void-variable)
)

(qa
(qa266-probe ";;; -*- lexical-binding: t -*-\n" 'nil t t)
'(t 7)
)

(qa
(qa266-probe ";;; -*- lexical-binding: t -*-\n" 't nil nil)
'(t 7)
)

(qa
(qa266-probe ";;; -*- lexical-binding: t -*-\n" 't nil t)
'(nil void-variable)
)

(qa
(qa266-probe ";;; -*- lexical-binding: t -*-\n" 't t nil)
'(t 7)
)

(qa
(qa266-probe ";;; -*- lexical-binding: t -*-\n" 't t t)
'(t 7)
)

(qa
(qa266-probe ";;; -*- lexical-binding: nil -*-\n" 'absent nil nil)
'(nil void-variable)
)

(qa
(qa266-probe ";;; -*- lexical-binding: nil -*-\n" 'absent nil t)
'(nil void-variable)
)

(qa
(qa266-probe ";;; -*- lexical-binding: nil -*-\n" 'absent t nil)
'(nil void-variable)
)

(qa
(qa266-probe ";;; -*- lexical-binding: nil -*-\n" 'absent t t)
'(nil void-variable)
)

(qa
(qa266-probe ";;; -*- lexical-binding: nil -*-\n" 'nil nil nil)
'(nil void-variable)
)

(qa
(qa266-probe ";;; -*- lexical-binding: nil -*-\n" 'nil nil t)
'(nil void-variable)
)

(qa
(qa266-probe ";;; -*- lexical-binding: nil -*-\n" 'nil t nil)
'(nil void-variable)
)

(qa
(qa266-probe ";;; -*- lexical-binding: nil -*-\n" 'nil t t)
'(t 7)
)

(qa
(qa266-probe ";;; -*- lexical-binding: nil -*-\n" 't nil nil)
'(t 7)
)

(qa
(qa266-probe ";;; -*- lexical-binding: nil -*-\n" 't nil t)
'(nil void-variable)
)

(qa
(qa266-probe ";;; -*- lexical-binding: nil -*-\n" 't t nil)
'(t 7)
)

(qa
(qa266-probe ";;; -*- lexical-binding: nil -*-\n" 't t t)
'(t 7)
)

(qa
(qa266-probe "" 'absent nil nil)
'(nil void-variable)
)

(qa
(qa266-probe "" 'absent nil t)
'(nil void-variable)
)

(qa
(qa266-probe "" 'absent t nil)
'(nil void-variable)
)

(qa
(qa266-probe "" 'absent t t)
'(nil void-variable)
)

(qa
(qa266-probe "" 'nil nil nil)
'(nil void-variable)
)

(qa
(qa266-probe "" 'nil nil t)
'(nil void-variable)
)

(qa
(qa266-probe "" 'nil t nil)
'(nil void-variable)
)

(qa
(qa266-probe "" 'nil t t)
'(t 7)
)

(qa
(qa266-probe "" 't nil nil)
'(t 7)
)

(qa
(qa266-probe "" 't nil t)
'(nil void-variable)
)

(qa
(qa266-probe "" 't t nil)
'(t 7)
)

(qa
(qa266-probe "" 't t t)
'(t 7)
)

(qa
(qa266-probe ";;; -*- lexical-binding:t -*-\n" 'absent nil)
'(t 7)
)

(qa
(qa266-probe ";;; -*- mode: emacs-lisp; lexical-binding: t; -*-\n"
	     'absent nil)
'(t 7)
)

(qa
(qa266-probe "; -*- lexical-binding: nil ; lexical-binding: t -*-\n"
	     'absent nil)
'(nil void-variable)
)

(qa
(qa266-probe "; -*- lexical-binding: t ; lexical-binding: nil -*-\n"
	     'absent nil)
'(t 7)
)

(qa
(qa266-probe "; -*- lexical-binding :	t	 -*-\n" 'absent nil)
'(t 7)
)

(qa
(qa266-probe "; -*- lexical-binding: false -*-\n" 'absent nil)
'(t 7)
)

(qa
(qa266-probe "; -*- lexical-binding: () -*-\n" 'absent nil)
'(t 7)
)

(qa
(qa266-probe "; -*- lexical-binding: NIL -*-\n" 'absent nil)
'(t 7)
)

(qa
(qa266-probe "; -*- lexical-binding: -*-\n" 'absent nil)
'(t 7)
)

(qa
(qa266-probe "; -*- lexical-binding: nilpotent -*-\n" 'absent nil)
'(t 7)
)

(qa
(qa266-probe "; -*- lexical-binding: nil" 'absent nil)
'(nil void-function)
)

(qa
(qa266-probe "; lexical-binding: t\n" 'absent nil)
'(nil void-variable)
)

(qa
(qa266-probe "; -*- lexical-binding: nil -*- lexical-binding: t\n"
	     'absent nil)
'(nil void-variable)
)

(qa
(qa266-probe "\n; -*- lexical-binding: t -*-\n" 'absent nil)
'(nil void-variable)
)

(qa
(qa266-probe " ; -*- lexical-binding: t -*-\n" 'absent nil)
'(nil void-variable)
)

(qa
(qa266-probe "; plain\n; -*- lexical-binding: t -*-\n" 'absent nil)
'(nil void-variable)
)

(qa
(qa266-probe
 "#! /bin/emacs --script\n;;; -*- lexical-binding: t -*-\n" 'absent
 nil)
'(t 7)
)

(qa
(qa266-probe
 "#! /bin/emacs --script\n;;; -*- lexical-binding: nil -*-\n" 'absent
 nil)
'(nil void-variable)
)

(qa
(qa266-probe "; -*- mode;other: ignored; lexical-binding: t -*-\n"
	     'absent nil)
'(t 7)
)

(qa
(let ((qa266-result nil))
  (with-temp-buffer
    (insert "; -*- lexical-binding: t -*-\n")
    (let ((beg (point)))
      (insert
       "; -*- lexical-binding: nil -*-\n(setq qa266-result (let ((x 7)) (lambda () x)))")
      (narrow-to-region beg (point-max))
      (let ((lexical-binding t)) (eval-buffer))
      (condition-case e (funcall qa266-result) (error (car e))))))
'void-variable
)

;; `eval-region' uses its READ-FUNCTION for forms that survive the layout
;; scan, and skips the reader entirely for whitespace/comment-only input.
(defvar qa267-read-result nil)
(qa
 (let ((read-count 0))
   (with-temp-buffer
     (insert "(setq qa267-read-result 'first)\n"
             "(setq qa267-read-result 'second)")
     (eval-region
      (point-min) (point-max) nil
      (lambda (stream)
        (setq read-count (1+ read-count))
        (read stream))))
   (list read-count qa267-read-result))
 '(2 second))
(qa
 (let ((read-count 0))
   (with-temp-buffer
     (insert " ; comment\n ")
     (eval-region
      (point-min) (point-max) nil
      (lambda (stream)
        (setq read-count (1+ read-count))
        (read stream))))
   read-count)
 0)
(qa (with-temp-buffer (insert "42") (eval-buffer)) nil)
(qa (with-temp-buffer
      (insert "42")
      (eval-region (point-min) (point-max)))
    nil)

;; eval-buffer consults the live buffer multibyte mode for each subsequent
;; read; changing it in one form makes the next literal unibyte.
(defvar qa267-multibyte-result nil)
(qa
 (let ((qa267-multibyte-result nil))
   (with-temp-buffer
     (insert "(set-buffer-multibyte nil)\n"
             "(setq qa267-multibyte-result "
             "(list (multibyte-string-p \"é\") "
             "(string-to-list \"é\") (string-bytes \"é\")))")
     (eval-buffer))
   qa267-multibyte-result)
 '(nil (195 169) 2))

(qa
(let ((qa266-result nil))
  (with-temp-buffer
    (insert "hidden\n")
    (let ((beg (point)))
      (insert
       "; -*- lexical-binding: t -*-\n(setq qa266-result (let ((x 7)) (lambda () x)))")
      (narrow-to-region beg (point-max))
      (let ((lexical-binding nil)) (eval-buffer))
      (funcall qa266-result))))
'7
)

(qa
(let ((qa266-result nil))
  (with-temp-buffer
    (insert ";;; -*- lexical-binding: t -*-\n"
	    "(setq qa266-result (let ((x 7)) (lambda () x)))")
    (let ((lexical-binding nil))
      (eval-region (point-min) (point-max)))
    (condition-case e (funcall qa266-result) (error (car e)))))
'void-variable
)

(qa
(let ((qa266-result nil))
  (with-temp-buffer
    (insert ";;; -*- lexical-binding: t -*-\n"
	    "(setq qa266-result (let ((x 7)) (lambda () x)))")
    (let ((lexical-binding t)) (eval-region (point-min) (point-max)))
    (condition-case e (funcall qa266-result) (error (car e)))))
'7
)

(qa
(let ((qa266-result nil))
  (with-temp-buffer
    (insert ";;; -*- lexical-binding: nil -*-\n"
	    "(setq qa266-result (let ((x 7)) (lambda () x)))")
    (let ((lexical-binding nil))
      (eval-region (point-min) (point-max)))
    (condition-case e (funcall qa266-result) (error (car e)))))
'void-variable
)

(qa
(let ((qa266-result nil))
  (with-temp-buffer
    (insert ";;; -*- lexical-binding: nil -*-\n"
	    "(setq qa266-result (let ((x 7)) (lambda () x)))")
    (let ((lexical-binding t)) (eval-region (point-min) (point-max)))
    (condition-case e (funcall qa266-result) (error (car e)))))
'7
)

(qa
(let (events (qa266-result nil))
  (with-temp-buffer
    (insert "" "(setq qa266-result (let ((x 7)) (lambda () x)))")
    (let*
	((source (current-buffer))
	 (internal--get-default-lexical-binding-function
	  (lambda (buffer)
	    (push
	     (list (eq buffer source) (point)
		   (eq standard-output 'symbolp)
		   (eq (car eval-buffer-list) source))
	     events)
	    nil)))
      (eval-buffer))
    (list (nreverse events)
	  (condition-case e (funcall qa266-result) (error (car e))))))
'(((t 1 t t)) void-variable)
)

(qa
(let (events (qa266-result nil))
  (with-temp-buffer
    (insert "" "(setq qa266-result (let ((x 7)) (lambda () x)))")
    (let*
	((source (current-buffer))
	 (internal--get-default-lexical-binding-function
	  (lambda (buffer)
	    (push
	     (list (eq buffer source) (point)
		   (eq standard-output 'symbolp)
		   (eq (car eval-buffer-list) source))
	     events)
	    t)))
      (eval-buffer))
    (list (nreverse events)
	  (condition-case e (funcall qa266-result) (error (car e))))))
'(((t 1 t t)) 7)
)

(qa
(let (events (qa266-result nil))
  (with-temp-buffer
    (insert "; comment\n"
	    "(setq qa266-result (let ((x 7)) (lambda () x)))")
    (let*
	((source (current-buffer))
	 (internal--get-default-lexical-binding-function
	  (lambda (buffer)
	    (push
	     (list (eq buffer source) (point)
		   (eq standard-output 'symbolp)
		   (eq (car eval-buffer-list) source))
	     events)
	    nil)))
      (eval-buffer))
    (list (nreverse events)
	  (condition-case e (funcall qa266-result) (error (car e))))))
'(((t 11 t t)) void-variable)
)

(qa
(let (events (qa266-result nil))
  (with-temp-buffer
    (insert "; comment\n"
	    "(setq qa266-result (let ((x 7)) (lambda () x)))")
    (let*
	((source (current-buffer))
	 (internal--get-default-lexical-binding-function
	  (lambda (buffer)
	    (push
	     (list (eq buffer source) (point)
		   (eq standard-output 'symbolp)
		   (eq (car eval-buffer-list) source))
	     events)
	    t)))
      (eval-buffer))
    (list (nreverse events)
	  (condition-case e (funcall qa266-result) (error (car e))))))
'(((t 11 t t)) 7)
)

(qa
(let (events (qa266-result nil))
  (with-temp-buffer
    (insert "#! script\n; comment\n"
	    "(setq qa266-result (let ((x 7)) (lambda () x)))")
    (let*
	((source (current-buffer))
	 (internal--get-default-lexical-binding-function
	  (lambda (buffer)
	    (push
	     (list (eq buffer source) (point)
		   (eq standard-output 'symbolp)
		   (eq (car eval-buffer-list) source))
	     events)
	    nil)))
      (eval-buffer))
    (list (nreverse events)
	  (condition-case e (funcall qa266-result) (error (car e))))))
'(((t 21 t t)) void-variable)
)

(qa
(let (events (qa266-result nil))
  (with-temp-buffer
    (insert "#! script\n; comment\n"
	    "(setq qa266-result (let ((x 7)) (lambda () x)))")
    (let*
	((source (current-buffer))
	 (internal--get-default-lexical-binding-function
	  (lambda (buffer)
	    (push
	     (list (eq buffer source) (point)
		   (eq standard-output 'symbolp)
		   (eq (car eval-buffer-list) source))
	     events)
	    t)))
      (eval-buffer))
    (list (nreverse events)
	  (condition-case e (funcall qa266-result) (error (car e))))))
'(((t 21 t t)) 7)
)

(qa
(let ((calls 0))
  (with-temp-buffer
    (insert "; -*- lexical-binding: t -*-\n42")
    (let
	((internal--get-default-lexical-binding-function
	  (lambda (_) (setq calls (1+ calls)) nil)))
      (eval-buffer)))
  calls)
'0
)

(qa
(let ((calls 0))
  (with-temp-buffer
    (insert "42") (setq-local lexical-binding t)
    (let
	((internal--get-default-lexical-binding-function
	  (lambda (_) (setq calls (1+ calls)) nil)))
      (eval-buffer)))
  calls)
'0
)

(qa
(let
    ((lexical-binding nil) (standard-output 'ignore)
     (eval-buffer-list '(outer)))
  (with-temp-buffer
    (insert "; comment\n42") (goto-char 4)
    (let
	((internal--get-default-lexical-binding-function
	  (lambda (_) (error "cookie hook"))))
      (list (condition-case e (eval-buffer) (error e)) (point)
	    lexical-binding standard-output eval-buffer-list))))
'((error "cookie hook") 4 nil ignore (outer))
)

(qa
(let
    ((lexical-binding nil) (standard-output 'ignore)
     (eval-buffer-list '(outer)))
  (with-temp-buffer
    (insert "; -*- lexical-binding: t -*-\n(error \"inside\")")
    (goto-char 4)
    (list (condition-case e (eval-buffer) (error e)) (point)
	  lexical-binding standard-output eval-buffer-list)))
'((error "inside") 4 nil ignore (outer))
)

(qa
(let
    ((internal--get-default-lexical-binding-function nil)
     (qa266-result nil))
  (with-temp-buffer
    (insert "(setq qa266-result (let ((x 7)) (lambda () x)))")
    (let ((lexical-binding t)) (eval-buffer))
    (condition-case e (funcall qa266-result) (error (car e)))))
'void-variable
)

(qa-err
 (let* ((declaration (make-symbol ":documentation"))
        (function (eval (list 'lambda nil (list declaration 'payload) 42) t)))
   (funcall function))
 'void-function)
(qa-err
 (let* ((declaration (make-symbol ":documentation"))
        (function
         (eval (list 'function
                     (list 'lambda nil (list declaration 'payload) 42))
               t)))
   (funcall function))
 'void-function)


(princ (qa--done)) (terpri)
