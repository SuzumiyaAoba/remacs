;;; Shared signal objects and cached condition matching. -*- lexical-binding: t; -*-
(defvar qa-so-input)
(defvar qa-so-seen)
(defvar qa-so-events)
(defvar qa-so-compiled)

(defun qa-so-run (form compiled)
  (let ((qa-so-input (list 'error "original")) qa-so-seen qa-so-events)
    (if compiled (funcall (byte-compile (list 'lambda nil form))) (eval form t))))

(dolist (qa-so-compiled '(nil t))
  (qa (qa-so-run '(condition-case caught (signal qa-so-input)
                    (error (eq caught qa-so-input))) qa-so-compiled) t)
  (qa (qa-so-run
       '(condition-case caught
            (handler-bind ((error (lambda (c) (setq qa-so-seen c)))) (signal qa-so-input))
          (error (list (eq caught qa-so-input) (eq caught qa-so-seen)))) qa-so-compiled)
      '(t t))
  (qa (qa-so-run
       '(condition-case caught
            (handler-bind ((error (lambda (c) (setq qa-so-seen c))))
              (signal (car qa-so-input) (cdr qa-so-input)))
          (error (list (eq caught qa-so-seen) (eq caught qa-so-input)
                       (eq (cdr caught) (cdr qa-so-input))))) qa-so-compiled)
      '(t nil t))
  (qa (qa-so-run
       '(condition-case caught
            (handler-bind ((error (lambda (c) (setq qa-so-seen c)))) (car 5))
          (error (list (eq caught qa-so-seen) caught))) qa-so-compiled)
      '(t (wrong-type-argument listp 5)))
  (qa (qa-so-run
       '(condition-case caught
            (handler-bind ((error (lambda (c) (setq qa-so-seen c)
                                   (setcar c 'file-error) (setcdr c '(changed)))))
              (signal qa-so-input))
          (file-error 'wrong-clause)
          (error (list (eq caught qa-so-seen) (eq caught qa-so-input) caught))) qa-so-compiled)
      '(t t (file-error changed)))
  (qa (qa-so-run
       '(condition-case caught
            (handler-bind ((error (lambda (c) (setcdr c 'changed)))) (signal qa-so-input))
          (error (list (eq caught qa-so-input) (cdr caught)))) qa-so-compiled)
      '(t changed))
  (qa (qa-so-run
       '(condition-case caught
            (handler-bind
             ((error (lambda (c) (setq qa-so-seen c) (setcar c 'file-error)
                       (push 'first qa-so-events)))
              (error (lambda (c) (push (list 'second (eq c qa-so-seen) (car c)) qa-so-events))))
             (signal qa-so-input))
          (file-error 'wrong-clause)
          (error (list (nreverse qa-so-events) (eq caught qa-so-seen)))) qa-so-compiled)
      '((first (second t file-error)) t))
  ;; Replacing a symbol property does not replace the already selected list.
  (qa (qa-so-run
       '(progn
          (put 'qa-so-custom 'error-conditions (list 'qa-so-custom 'error))
          (condition-case caught
              (handler-bind ((error (lambda (_)
                                     (put 'qa-so-custom 'error-conditions
                                          '(qa-so-custom file-error)))))
                (signal 'qa-so-custom nil))
            (file-error 'wrong-clause) (qa-so-custom 'original-clause))) qa-so-compiled)
      'original-clause)
  ;; Destructive changes to that list remain visible to the signal walk.
  (qa (qa-so-run
       '(progn
          (put 'qa-so-custom 'error-conditions (list 'qa-so-custom 'error))
          (condition-case caught
              (handler-bind ((error (lambda (_)
                                     (setcar (cdr (get 'qa-so-custom 'error-conditions))
                                             'file-error))))
                (signal 'qa-so-custom nil))
            (file-error 'changed-clause) (error 'wrong-clause))) qa-so-compiled)
      'changed-clause)
  (qa (qa-so-run
       '(progn
          (put 'qa-so-custom 'error-conditions '(error))
          (condition-case caught (signal 'qa-so-custom nil)
            (qa-so-custom 'wrong-clause) (error 'parent-clause))) qa-so-compiled)
      'parent-clause)
  (qa (qa-so-run
       '(condition-case caught
            (unwind-protect (signal qa-so-input) (setcar qa-so-input 'file-error))
          (file-error 'wrong-clause)
          (error (list (eq caught qa-so-input) (car caught)))) qa-so-compiled)
      '(t file-error))
  (qa (qa-so-run
       '(condition-case caught
            (unwind-protect (signal qa-so-input)
              (condition-case nil (signal 'file-error '(cleanup)) (file-error nil)))
          (file-error 'wrong-clause)
          (error (eq caught qa-so-input))) qa-so-compiled) t)
  ;; An unoffered VM primitive error must retain its original matching list
  ;; while an inner function's unwind-protect cleanup changes that property.
  (qa (qa-so-run
       '(let ((old (get 'wrong-type-argument 'error-conditions)))
          (unwind-protect
              (condition-case caught
                  (funcall (lambda ()
                             (unwind-protect (car 5)
                               (put 'wrong-type-argument 'error-conditions '(file-error)))))
                (file-error 'wrong-clause)
                (error (car caught)))
            (put 'wrong-type-argument 'error-conditions old))) qa-so-compiled)
      'wrong-type-argument)
  (qa (qa-so-run
       '(condition-case nil
            (handler-bind ((error (lambda (_) (push 'observer qa-so-events))))
              (funcall (lambda ()
                         (unwind-protect (car 5) (push 'cleanup qa-so-events)))))
          (error (nreverse qa-so-events))) qa-so-compiled)
      '(observer cleanup))
  (qa (qa-so-run
       '(condition-case first
            (handler-bind ((error (lambda (c) (setcar c 'file-error)))) (signal qa-so-input))
          (file-error 'wrong-clause)
          (error (condition-case second (signal first)
                   (file-error (list (eq first second) (eq second qa-so-input)))))) qa-so-compiled)
      '(t t))
  (qa (qa-so-run
       '(condition-case caught
            (handler-bind ((error (lambda (c) (setq qa-so-seen c) (setcdr c c)
                                   (garbage-collect))))
              (signal qa-so-input))
          (error (list (eq caught qa-so-seen) (eq caught (cdr caught))))) qa-so-compiled)
      '(t t))
  (qa (qa-so-run
       '(let ((weak (make-hash-table :test 'eq :weakness 'key)))
          (condition-case caught
              (unwind-protect
                  (handler-bind ((error (lambda (c) (puthash c 'alive weak))))
                    (error "ephemeral"))
                (garbage-collect))
            (error (gethash caught weak)))) qa-so-compiled)
      'alive))

(defun qa-so-process (kind)
  (let* ((input (list 'error "process")) seen reported finished p
         (debug-on-error nil) (process-error-pause-time 0)
         (command-error-function (lambda (condition &rest _)
                                   (setq reported condition finished t))))
    (unwind-protect
        (let ((callback (lambda (_process _text)
                          (handler-bind ((error (lambda (c) (setq seen c)
                                                 (setcar c 'file-error)
                                                 (setcdr c '(changed)))))
                            (signal input)))))
          (setq p (if (eq kind 'sentinel)
                      (make-pipe-process :name "qa-so-process" :noquery t :sentinel callback)
                    (make-process :name "qa-so-process" :noquery t :connection-type 'pipe
                                  :filter callback :sentinel #'ignore
                                  :command '("/bin/sh" "-c" "printf payload"))))
          (if (eq kind 'sentinel) (delete-process p)
            (let ((deadline (+ (float-time) 3)))
              (while (and (not finished) (< (float-time) deadline))
                (accept-process-output p 0.01))))
          (list finished (eq input seen) (eq seen reported) reported))
      (when p (set-process-sentinel p #'ignore) (delete-process p)))))

(qa (qa-so-process 'filter) '(t t t (file-error changed)))
(qa (qa-so-process 'sentinel) '(t t t (file-error changed)))
(qa-lex
 (let* ((input (list 'error "thread")) observed
        (worker (make-thread
                 (lambda ()
                   (handler-bind ((error (lambda (c) (setq observed c)))) (signal input))))))
   (thread-join worker)
   (let ((last (thread-last-error))) (list (eq last input) (eq last observed))))
 '(t t))

(princ (qa--done))
