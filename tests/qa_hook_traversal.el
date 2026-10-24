;;; GNU hook traversal observes cons mutations and reads defaults lazily. -*- lexical-binding: nil; -*-
(defvar qa-ht-hook nil)
(defvar qa-ht-seen nil)
(defvar qa-ht-result nil)
(defun qa-ht-one (&rest values)
  (push (cons 'one values) qa-ht-seen)
  qa-ht-result)
(defun qa-ht-two (&rest values)
  (push (cons 'two values) qa-ht-seen)
  qa-ht-result)
(defun qa-ht-three (&rest values)
  (push (cons 'three values) qa-ht-seen)
  qa-ht-result)
(defun qa-ht-replace-tail (&rest values)
  (setcdr qa-ht-hook (list #'qa-ht-three))
  (qa-ht-one))
(defun qa-ht-replace-next (&rest values)
  (setcar (cdr qa-ht-hook) #'qa-ht-three)
  (apply #'qa-ht-one values))

(let ((qa-ht-hook '(lambda (&rest values) (push values qa-ht-seen) 'single))
      (qa-ht-seen nil))
  (qa (run-hook-with-args-until-success 'qa-ht-hook 1 2) 'single)
  (qa qa-ht-seen '((1 2))))

(let ((qa-ht-hook (cons #'qa-ht-one 'ignored)) (qa-ht-seen nil))
  (qa (run-hooks 'qa-ht-hook) nil)
  (qa qa-ht-seen '((one))))

(let ((qa-ht-hook (list #'qa-ht-replace-tail #'qa-ht-two)) (qa-ht-seen nil))
  (run-hooks 'qa-ht-hook)
  (qa (nreverse qa-ht-seen) '((one) (three))))

(let ((qa-ht-hook (list #'qa-ht-replace-next #'qa-ht-two)) (qa-ht-seen nil))
  (run-hook-with-args 'qa-ht-hook 7)
  (qa (nreverse qa-ht-seen) '((one 7) (three 7))))

(let ((qa-ht-hook (list (lambda ()
                          (setcdr qa-ht-hook (list (lambda () 'winner)))
                          nil)
                        (lambda () 'stale))))
  (qa (run-hook-with-args-until-success 'qa-ht-hook) 'winner))

(let ((qa-ht-hook (list (lambda ()
                          (setcdr qa-ht-hook (list (lambda () nil)))
                          t)
                        (lambda () t))))
  (qa (run-hook-with-args-until-failure 'qa-ht-hook) nil))

(let ((qa-ht-hook (list #'qa-ht-one #'qa-ht-two)) (qa-ht-seen nil))
  (qa (run-hook-wrapped 'qa-ht-hook
        (lambda (function)
          (push function qa-ht-seen)
          (when (eq function #'qa-ht-one)
            (setcdr qa-ht-hook (list #'qa-ht-three)))
          nil)) nil)
  (qa (nreverse qa-ht-seen) '(qa-ht-one qa-ht-three)))

;; Replacing the binding does not replace the list already being walked.
(let ((qa-ht-hook (list (lambda () (setq qa-ht-hook nil)) #'qa-ht-two))
      (qa-ht-seen nil))
  (run-hooks 'qa-ht-hook)
  (qa qa-ht-seen '((two))))

;; Even a default list can contain t. Its nested default walk skips t.
(let ((qa-ht-hook (list t #'qa-ht-one)) (qa-ht-seen nil))
  (run-hooks 'qa-ht-hook)
  (qa qa-ht-seen '((one) (one))))

(let ((qa-ht-hook (list #'qa-ht-two)) (qa-ht-seen nil))
  (with-temp-buffer
    (setq-local qa-ht-hook
                (list (lambda () (set-default 'qa-ht-hook (list #'qa-ht-three)))
                      t #'qa-ht-one))
    (run-hooks 'qa-ht-hook)
    (qa (nreverse qa-ht-seen) '((three) (one)))))

(let ((qa-ht-hook '(lambda () (qa-ht-two))) (qa-ht-seen nil))
  (with-temp-buffer
    (setq-local qa-ht-hook (list t #'qa-ht-one))
    (run-hooks 'qa-ht-hook)
    (qa (nreverse qa-ht-seen) '((two) (one)))))

(let ((qa-ht-hook (list t #'qa-ht-two)) (qa-ht-seen nil))
  (with-temp-buffer
    (setq-local qa-ht-hook (list t t))
    (run-hooks 'qa-ht-hook)
    (qa (nreverse qa-ht-seen) '((two) (two)))))

(makunbound 'qa-ht-hook)
(with-temp-buffer
  (setq-local qa-ht-hook (list t))
  (qa-err-data (run-hooks 'qa-ht-hook) 'void-variable '(qa-ht-hook)))
(setq qa-ht-hook nil)
(princ (qa--done))
