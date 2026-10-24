;;; Cleanup exits replace pending exits with the right handlers active. -*- lexical-binding: t; -*-
(defvar qa-vmu-events)
(defvar qa-vmu-mode)
(defvar qa-vmu-value 'global)
(defun qa-vmu-body () (push 'body qa-vmu-events) 'body)
(defun qa-vmu-run (form compiled)
  (let (qa-vmu-events)
    (list (catch 'qa-vmu-out
            (condition-case err
                (list 'return (if compiled
                                  (funcall (byte-compile (list 'lambda nil form)))
                                (eval form t)))
              (t (list 'signal err))))
          (reverse qa-vmu-events))))

(dolist (qa-vmu-mode '(nil t))
  (qa (qa-vmu-run '(unwind-protect (qa-vmu-body) (push 'clean qa-vmu-events)) qa-vmu-mode)
      '((return body) (body clean)))
  (qa (qa-vmu-run '(unwind-protect (qa-vmu-body) (push 'clean qa-vmu-events)
                    (error "cleanup")) qa-vmu-mode)
      '((signal (error "cleanup")) (body clean)))
  (qa (qa-vmu-run '(unwind-protect (error "body") (push 'clean qa-vmu-events)) qa-vmu-mode)
      '((signal (error "body")) (clean)))
  (qa (qa-vmu-run '(unwind-protect (error "body") (push 'clean qa-vmu-events)
                    (error "cleanup")) qa-vmu-mode)
      '((signal (error "cleanup")) (clean)))
  (qa (qa-vmu-run '(unwind-protect (throw 'qa-vmu-out 'body)
                    (push 'clean qa-vmu-events)) qa-vmu-mode)
      '(body (clean)))
  (qa (qa-vmu-run '(unwind-protect (throw 'qa-vmu-out 'body)
                    (push 'clean qa-vmu-events) (error "cleanup")) qa-vmu-mode)
      '((signal (error "cleanup")) (clean)))
  (qa (qa-vmu-run '(unwind-protect (error "body")
                    (push 'clean qa-vmu-events) (throw 'qa-vmu-out 'cleanup)) qa-vmu-mode)
      '(cleanup (clean)))
  (qa (qa-vmu-run '(unwind-protect (qa-vmu-body)
                    (push 'clean qa-vmu-events) (throw 'qa-vmu-out 'cleanup)) qa-vmu-mode)
      '(cleanup (body clean)))
  ;; A pending jump does not remove the handler before its cleanup runs.
  (qa (qa-vmu-run '(condition-case err
                      (unwind-protect (error "body") (error "cleanup"))
                    (error (cadr err))) qa-vmu-mode)
      '((return "cleanup") nil))
  (qa (qa-vmu-run '(condition-case nil
                      (unwind-protect (error "body") (signal 'file-error '("cleanup")))
                    (file-error 'file-clause) (error 'error-clause)) qa-vmu-mode)
      '((return file-clause) nil))
  (qa (qa-vmu-run '(condition-case nil
                      (unwind-protect (throw 'qa-vmu-out 'body) (error "cleanup"))
                    (error 'recovered)) qa-vmu-mode)
      '((return recovered) nil))
  (qa (qa-vmu-run '(catch 'local
                    (unwind-protect (error "body") (throw 'local 'cleanup))) qa-vmu-mode)
      '((return cleanup) nil))
  (qa (qa-vmu-run '(catch 'local
                    (unwind-protect (throw 'local 'body) (throw 'local 'cleanup))) qa-vmu-mode)
      '((return cleanup) nil))
  (qa (qa-vmu-run '(catch 'outer (catch 'inner
                    (unwind-protect (throw 'inner 'body) (throw 'outer 'cleanup)))) qa-vmu-mode)
      '((return cleanup) nil))
  (qa (qa-vmu-run '(catch 'outer (catch 'inner
                    (unwind-protect (throw 'outer 'body) (throw 'inner 'cleanup)))) qa-vmu-mode)
      '((return cleanup) nil))
  (qa (qa-vmu-run '(condition-case nil
                      (catch 'local
                        (unwind-protect (error "body") (signal 'file-error '("cleanup"))))
                    (file-error 'file-clause) (error 'error-clause)) qa-vmu-mode)
      '((return file-clause) nil))
  (qa (qa-vmu-run '(unwind-protect
                      (unwind-protect (error "body") (push 'inner qa-vmu-events)
                        (error "inner"))
                    (push 'outer qa-vmu-events) (error "outer")) qa-vmu-mode)
      '((signal (error "outer")) (inner outer)))
  (qa (qa-vmu-run '(unwind-protect
                      (unwind-protect (qa-vmu-body) (push 'inner qa-vmu-events)
                        (throw 'qa-vmu-out 'inner))
                    (push 'outer qa-vmu-events) (throw 'qa-vmu-out 'outer)) qa-vmu-mode)
      '(outer (body inner outer)))
  (qa (qa-vmu-run '(let ((qa-vmu-value 'outer))
                    (condition-case err
                        (unwind-protect
                            (let ((qa-vmu-value 'inner)) (error "body"))
                          (push qa-vmu-value qa-vmu-events) (error "cleanup"))
                      (error (list (cadr err) qa-vmu-value)))) qa-vmu-mode)
      '((return ("cleanup" outer)) (outer)))
  ;; An observer for the cleanup's error executes before outer restoration.
  (qa (qa-vmu-run '(let ((qa-vmu-value 'bound))
                    (condition-case err
                        (handler-bind ((file-error
                                        (lambda (_)
                                          (push qa-vmu-value qa-vmu-events))))
                          (unwind-protect (error "body")
                            (signal 'file-error '("cleanup"))))
                      (file-error (cadr err)))) qa-vmu-mode)
      '((return "cleanup") (bound)))
  ;; Once an exit has selected a handler, ordinary cleanup cannot retarget it
  ;; merely by destructively changing the cached condition list.
  (qa (qa-vmu-run '(progn
                    (put 'qa-vmu-class 'error-conditions (list 'qa-vmu-class 'error))
                    (condition-case nil
                        (condition-case nil
                            (unwind-protect (signal 'qa-vmu-class nil)
                              (setcar (cdr (get 'qa-vmu-class 'error-conditions)) 'file-error))
                          (file-error 'wrong-handler))
                      (error 'selected-handler))) qa-vmu-mode)
      '((return selected-handler) nil))
  (qa (qa-vmu-run '(with-temp-buffer
                    (insert "abcdef") (goto-char 4) (narrow-to-region 2 6)
                    (condition-case nil
                        (save-excursion
                          (save-restriction
                            (widen) (goto-char 1)
                            (unwind-protect (error "body") (error "cleanup"))))
                      (error (list (point) (point-min) (point-max))))) qa-vmu-mode)
      '((return (4 2 6)) nil)))

;; Dynamic argument bindings have their own cleanup at the VM boundary.
(defvar qa-vmu-a 100)
(defvar qa-vmu-b 200)
(defvar qa-vmu-fail)
(defun qa-vmu-watcher (symbol value operation _where)
  (when (memq operation '(let unlet))
    (push (list symbol operation value (symbol-value symbol)) qa-vmu-events))
  (when (and qa-vmu-fail
             (if (eq qa-vmu-fail 'bind)
                 (and (eq symbol 'qa-vmu-a) (eq operation 'let))
               (and (eq symbol qa-vmu-fail) (eq operation 'unlet))))
    (setq qa-vmu-fail nil)
    (error "watcher-error")))
(defun qa-vmu-args (failure)
  (let (qa-vmu-events (qa-vmu-fail failure))
    (setq qa-vmu-a 100 qa-vmu-b 200)
    (add-variable-watcher 'qa-vmu-a #'qa-vmu-watcher)
    (add-variable-watcher 'qa-vmu-b #'qa-vmu-watcher)
    (unwind-protect
        (let ((outcome (condition-case err
                           (funcall (make-byte-code '(qa-vmu-a qa-vmu-b)
                                                    (unibyte-string 192 135) [value] 1) 1 2)
                         (error (cdr err)))))
          (list outcome qa-vmu-a qa-vmu-b (reverse qa-vmu-events)))
      (remove-variable-watcher 'qa-vmu-a #'qa-vmu-watcher)
      (remove-variable-watcher 'qa-vmu-b #'qa-vmu-watcher)
      (setq qa-vmu-a 100 qa-vmu-b 200))))

(qa (qa-vmu-args 'qa-vmu-b)
    '(("watcher-error") 100 2 ((qa-vmu-a let 1 100) (qa-vmu-b let 2 200)
                              (qa-vmu-b unlet 200 2) (qa-vmu-a unlet 100 1))))
(qa (qa-vmu-args 'qa-vmu-a)
    '(("watcher-error") 1 200 ((qa-vmu-a let 1 100) (qa-vmu-b let 2 200)
                              (qa-vmu-b unlet 200 2) (qa-vmu-a unlet 100 1))))
(qa (qa-vmu-args 'bind)
    '(("watcher-error") 100 200 ((qa-vmu-a let 1 100) (qa-vmu-a unlet 100 100))))

(defun qa-vmu-watch-body (fail)
  (push (list 'body qa-vmu-a qa-vmu-b) qa-vmu-events)
  (if fail (error "body-error") 'value))
(defun qa-vmu-lets (failure compiled internal)
  (let (qa-vmu-events (qa-vmu-fail failure))
    (setq qa-vmu-a 100 qa-vmu-b 200)
    (add-variable-watcher 'qa-vmu-a #'qa-vmu-watcher)
    (add-variable-watcher 'qa-vmu-b #'qa-vmu-watcher)
    (unwind-protect
        (let* ((body `(let ((qa-vmu-a 1) (qa-vmu-b 2))
                       (qa-vmu-watch-body ,(not internal))))
               (form (if internal
                         `(condition-case caught ,body
                            (error (list 'caught (cadr caught) qa-vmu-a qa-vmu-b)))
                       body))
               (outcome (condition-case err
                            (if compiled (funcall (byte-compile (list 'lambda nil form)))
                              (eval form t))
                          (error (list 'escaped (cadr err) qa-vmu-a qa-vmu-b))))
               unlets)
          (dolist (event (reverse qa-vmu-events))
            (when (eq (cadr event) 'unlet) (push (car event) unlets)))
          (list outcome (nreverse unlets)
                (and (member '(body 1 2) qa-vmu-events) t)))
      (remove-variable-watcher 'qa-vmu-a #'qa-vmu-watcher)
      (remove-variable-watcher 'qa-vmu-b #'qa-vmu-watcher)
      (setq qa-vmu-a 100 qa-vmu-b 200))))

(dolist (qa-vmu-mode '(nil t))
  (qa (qa-vmu-lets 'qa-vmu-a qa-vmu-mode t)
      `((caught "watcher-error" 1 200)
        ,(if qa-vmu-mode '(qa-vmu-a qa-vmu-b) '(qa-vmu-b qa-vmu-a)) t))
  (qa (qa-vmu-lets 'qa-vmu-b qa-vmu-mode t)
      `((caught "watcher-error" 100 2)
        ,(if qa-vmu-mode '(qa-vmu-a qa-vmu-b) '(qa-vmu-b qa-vmu-a)) t))
  (qa (qa-vmu-lets 'qa-vmu-a qa-vmu-mode nil)
      `((escaped "watcher-error" 1 200)
        ,(if qa-vmu-mode '(qa-vmu-a qa-vmu-b) '(qa-vmu-b qa-vmu-a)) t))
  (qa (qa-vmu-lets 'qa-vmu-b qa-vmu-mode nil)
      `((escaped "watcher-error" 100 2)
        ,(if qa-vmu-mode '(qa-vmu-a qa-vmu-b) '(qa-vmu-b qa-vmu-a)) t))
  (qa (qa-vmu-lets 'bind qa-vmu-mode t)
      `((caught "watcher-error" 100 200)
        ,(if qa-vmu-mode '(qa-vmu-a qa-vmu-b) '(qa-vmu-a)) nil)))

(princ (qa--done))
