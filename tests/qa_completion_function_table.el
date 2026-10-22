;;; qa_completion_function_table.el --- Preserve completion protocol objects -*- lexical-binding: t; -*-

(defun qa320-protocol (operation kind unibyte)
  (let* ((input (propertize (if unibyte (unibyte-string 102 111 111) "日foo") 'audit 42))
         (predicate (lambda (&rest _) t))
         (table (lambda (string pred action)
                  (list (eq string input) (get-text-property 0 'audit string)
                        (multibyte-string-p string) (eq pred predicate) action))))
    (unwind-protect
        (progn
          (fset 'qa320-named-table table)
          (fset 'qa320-alias 'qa320-named-table)
          (funcall operation input (cond ((eq kind 'named) 'qa320-named-table)
                                        ((eq kind 'alias) 'qa320-alias)
                                        (t table)) predicate))
      (fmakunbound 'qa320-named-table)
      (fmakunbound 'qa320-alias))))

(defun qa320-mutation (operation)
  (let* ((input (propertize "foo" 'audit 42))
         (result (funcall operation input
                          (lambda (string _pred _action)
                            (put-text-property 0 1 'audit 7 string)
                            string))))
    (list (eq result input) (get-text-property 0 'audit input))))

(defun qa320-results (operation)
  (mapcar (lambda (result)
            (eq result (funcall operation "foo" (lambda (&rest _) result))))
          (list nil t 0 (list 'payload) (vector 'payload) (propertize "result" 'audit 9))))

(defun qa320-validation (operation)
  (let ((called nil))
    (list (condition-case error
              (funcall operation 42 (lambda (&rest _) (setq called t)))
            (error error)) called)))

;; try-completion input identity and protocol closure byte=nil
(qa (qa320-protocol #'try-completion 'closure nil)
    '(t 42 t t nil))

;; try-completion input identity and protocol closure byte=t
(qa (qa320-protocol #'try-completion 'closure t)
    '(t 42 nil t nil))

;; try-completion input identity and protocol named byte=nil
(qa (qa320-protocol #'try-completion 'named nil)
    '(t 42 t t nil))

;; try-completion input identity and protocol named byte=t
(qa (qa320-protocol #'try-completion 'named t)
    '(t 42 nil t nil))

;; try-completion input identity and protocol alias byte=nil
(qa (qa320-protocol #'try-completion 'alias nil)
    '(t 42 t t nil))

;; try-completion input identity and protocol alias byte=t
(qa (qa320-protocol #'try-completion 'alias t)
    '(t 42 nil t nil))

;; try-completion mutation reaches original string
(qa (qa320-mutation #'try-completion)
    '(t 7))

;; try-completion arbitrary return values retain identity
(qa (qa320-results #'try-completion)
    '(t t t t t t))

;; try-completion validates STRING before calling table
(qa (qa320-validation #'try-completion)
    '((wrong-type-argument stringp 42) nil))

;; all-completions input identity and protocol closure byte=nil
(qa (qa320-protocol #'all-completions 'closure nil)
    '(t 42 t t t))

;; all-completions input identity and protocol closure byte=t
(qa (qa320-protocol #'all-completions 'closure t)
    '(t 42 nil t t))

;; all-completions input identity and protocol named byte=nil
(qa (qa320-protocol #'all-completions 'named nil)
    '(t 42 t t t))

;; all-completions input identity and protocol named byte=t
(qa (qa320-protocol #'all-completions 'named t)
    '(t 42 nil t t))

;; all-completions input identity and protocol alias byte=nil
(qa (qa320-protocol #'all-completions 'alias nil)
    '(t 42 t t t))

;; all-completions input identity and protocol alias byte=t
(qa (qa320-protocol #'all-completions 'alias t)
    '(t 42 nil t t))

;; all-completions mutation reaches original string
(qa (qa320-mutation #'all-completions)
    '(t 7))

;; all-completions arbitrary return values retain identity
(qa (qa320-results #'all-completions)
    '(t t t t t t))

;; all-completions validates STRING before calling table
(qa (qa320-validation #'all-completions)
    '((wrong-type-argument stringp 42) nil))

;; test-completion input identity and protocol closure byte=nil
(qa (qa320-protocol #'test-completion 'closure nil)
    '(t 42 t t lambda))

;; test-completion input identity and protocol closure byte=t
(qa (qa320-protocol #'test-completion 'closure t)
    '(t 42 nil t lambda))

;; test-completion input identity and protocol named byte=nil
(qa (qa320-protocol #'test-completion 'named nil)
    '(t 42 t t lambda))

;; test-completion input identity and protocol named byte=t
(qa (qa320-protocol #'test-completion 'named t)
    '(t 42 nil t lambda))

;; test-completion input identity and protocol alias byte=nil
(qa (qa320-protocol #'test-completion 'alias nil)
    '(t 42 t t lambda))

;; test-completion input identity and protocol alias byte=t
(qa (qa320-protocol #'test-completion 'alias t)
    '(t 42 nil t lambda))

;; test-completion mutation reaches original string
(qa (qa320-mutation #'test-completion)
    '(t 7))

;; test-completion arbitrary return values retain identity
(qa (qa320-results #'test-completion)
    '(t t t t t t))

;; test-completion validates STRING before calling table
(qa (qa320-validation #'test-completion)
    '((wrong-type-argument stringp 42) nil))

(princ (qa--done))
