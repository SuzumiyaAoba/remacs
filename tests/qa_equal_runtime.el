;;; -*- lexical-binding: nil; -*-
;; Equality callers must propagate GNU's depth error without losing state.
;; Load after qa_framework.el.  Verified against GNU Emacs 31.1.

(defun qa-equal-callers--deep ()
  (let ((value 0))
    (dotimes (_ 220) (setq value (vector value)))
    value))
(defvar qa-equal-callers-a nil)
(defvar qa-equal-callers-b nil)
(defvar qa-equal-callers-hook nil)
(defvar qa-equal-callers-buffer-value nil)
(defvar qa-equal-callers-base nil)
(defvar qa-equal-callers-failures 0)
(defun qa-equal-callers--fail-once ()
  (setq qa-equal-callers-failures (1+ qa-equal-callers-failures))
  (when (= qa-equal-callers-failures 1) (error "one failure")))
(defun qa-equal-callers--advised () 'base)
(defun qa-equal-callers--ordered-advice () nil)

(let ((qa-equal-callers-a (qa-equal-callers--deep))
      (qa-equal-callers-b (qa-equal-callers--deep)))
  ;; `featurep' uses member only after the main feature is present.
  (let ((features (cons 'qa-equal-callers-feature features)))
    (put 'qa-equal-callers-feature 'subfeatures (list qa-equal-callers-a))
    (qa-err-data (featurep 'qa-equal-callers-feature qa-equal-callers-b)
      'error '("Stack overflow in equal"))
    (put 'qa-equal-callers-feature 'subfeatures
         (list qa-equal-callers-b qa-equal-callers-a))
    (qa (featurep 'qa-equal-callers-feature qa-equal-callers-b) t))

  ;; Hook membership errors precede publication of a replacement list.
  (qa (let ((qa-equal-callers-hook (list qa-equal-callers-a)))
        (condition-case e (add-hook 'qa-equal-callers-hook qa-equal-callers-b)
          (error (list e (eq (car qa-equal-callers-hook) qa-equal-callers-a)
                       (length qa-equal-callers-hook)))))
    '((error "Stack overflow in equal") t 1))
  (qa (let ((qa-equal-callers-hook (list qa-equal-callers-a)))
        (condition-case e (remove-hook 'qa-equal-callers-hook qa-equal-callers-b)
          (error (list e (eq (car qa-equal-callers-hook) qa-equal-callers-a)
                       (length qa-equal-callers-hook)))))
    '((error "Stack overflow in equal") t 1))
  (qa (let ((qa-equal-callers-hook
             (list qa-equal-callers-b qa-equal-callers-a)))
        (add-hook 'qa-equal-callers-hook qa-equal-callers-b)
        (length qa-equal-callers-hook))
    2)

  (qa-err-data
    (let ((qa-equal-callers-buffer-value qa-equal-callers-a))
      (find-buffer 'qa-equal-callers-buffer-value qa-equal-callers-b))
    'error '("Stack overflow in equal"))

  ;; Optimizer predicates, including the implicit `equal', are fallible.
  (qa-err-data
    (let ((table (make-char-table nil qa-equal-callers-a)))
      (set-char-table-range table '(0 . 127) qa-equal-callers-a)
      (set-char-table-range table 1 qa-equal-callers-b)
      (optimize-char-table table))
    'error '("Stack overflow in equal"))
  (qa-err-data
    (let ((table (make-char-table nil 1)))
      (set-char-table-range table 1 2)
      (optimize-char-table table (lambda (a b) (error "comparison test failed"))))
    'error '("comparison test failed"))
  (qa (let ((table (make-char-table nil qa-equal-callers-a)))
        (set-char-table-range table 1 qa-equal-callers-b)
        (optimize-char-table table #'eq))
    nil)

  ;; Advice comparisons must not drop stored entries on an error.
  (advice-add 'qa-equal-callers--advised :before #'ignore
              (list (cons 'name qa-equal-callers-a)))
  (unwind-protect
      (progn
        (qa-err-data (advice-member-p qa-equal-callers-b 'qa-equal-callers--advised)
          'error '("Stack overflow in equal"))
        (qa (let ((old (symbol-function 'qa-equal-callers--advised)))
              (condition-case e
                  (advice-remove 'qa-equal-callers--advised qa-equal-callers-b)
                (error (list e (eq old (symbol-function 'qa-equal-callers--advised))))))
          '((error "Stack overflow in equal") t))
        (qa (let ((old (symbol-function 'qa-equal-callers--advised)))
              (condition-case e
                  (advice-add 'qa-equal-callers--advised :before #'ignore
                              (list (cons 'name qa-equal-callers-b)))
                (error (list e (eq old (symbol-function 'qa-equal-callers--advised))))))
          '((error "Stack overflow in equal") t)))
    (advice-remove 'qa-equal-callers--advised #'ignore))

  ;; Member checks names first; removal checks functions first.
  (let ((fun (list 'lambda nil (list 'quote qa-equal-callers-a)))
        (name (list 'lambda nil (list 'quote qa-equal-callers-b))))
    (advice-add 'qa-equal-callers--ordered-advice :before fun
                (list (cons 'name name)))
    (unwind-protect
        (progn
          (qa (not (null (advice-member-p name 'qa-equal-callers--ordered-advice))) t)
          (qa-err-data (advice-remove 'qa-equal-callers--ordered-advice name)
            'error '("Stack overflow in equal")))
      (advice-remove 'qa-equal-callers--ordered-advice fun)))

  ;; GNU uses identity for these callers, so deep values must not signal.
  (qa (let ((register-alist (list (cons qa-equal-callers-a 7))))
        (let ((missing (get-register qa-equal-callers-b)))
          (set-register qa-equal-callers-b 8)
          (list missing (get-register qa-equal-callers-a)
                (get-register qa-equal-callers-b) (length register-alist))))
    '(nil 7 8 2))
  (qa (let ((qa-equal-callers-base
             (list 'lambda nil (list 'quote qa-equal-callers-b)
                   '(mapbacktrace #'ignore qa-equal-callers-base))))
        (funcall (list 'lambda nil (list 'quote qa-equal-callers-a)
                       '(mapbacktrace #'ignore qa-equal-callers-base))))
    nil)
  (qa (with-temp-buffer
        (insert "abc")
        (internal--labeled-narrow-to-region 1 2 qa-equal-callers-a)
        (internal--labeled-widen qa-equal-callers-b))
    nil))

;; The safe hook error handler removes all occurrences by identity.
(qa (let ((qa-equal-callers-failures 0)
          (post-command-hook (list #'ignore #'qa-equal-callers--fail-once
                                   #'qa-equal-callers--fail-once)))
      (with-temp-buffer (execute-kbd-macro "a"))
      (list (length post-command-hook) (eq (car post-command-hook) #'ignore)))
  '(1 t))

(princ (qa--done)) (terpri)
