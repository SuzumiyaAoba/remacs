;;; qa_local_dynamic_unwind.el --- Do not resurrect killed local bindings -*- lexical-binding: t; -*-

(defvar qa334-var 1)
(defun qa334-kill (all) (if all (kill-all-local-variables) (kill-local-variable 'qa334-var)))
(defun qa334-dynamic (all action)
  (let ((old (default-value 'qa334-var)))
    (unwind-protect
        (progn
          (set-default 'qa334-var 1)
          (with-temp-buffer
            (setq-local qa334-var 10)
            (let ((inside
                   (condition-case error
                       (let ((qa334-var 20) (nested nil))
                         (qa334-kill all)
                         (cond ((eq action 'default) (set-default 'qa334-var 3))
                               ((eq action 'recreate) (setq-local qa334-var 30))
                               ((eq action 'recreate-kill)
                                (setq-local qa334-var 30) (kill-local-variable 'qa334-var))
                               ((eq action 'make-local) (make-local-variable 'qa334-var))
                               ((eq action 'nested)
                                (let ((qa334-var 30))
                                  (setq nested (list qa334-var (local-variable-p 'qa334-var)))))
                               ((eq action 'error) (error "qa334-stop")))
                         (list qa334-var (local-variable-p 'qa334-var) nested))
                     (error (car error)))))
              (list inside qa334-var (local-variable-p 'qa334-var) (default-value 'qa334-var)))))
      (set-default 'qa334-var old))))
(defun qa334-switch (all)
  (with-temp-buffer
    (setq-local qa334-var 10)
    (let ((original (current-buffer)) (other (generate-new-buffer " *qa334-other*")))
      (unwind-protect
          (progn
            (with-current-buffer other (setq-local qa334-var 99))
            (let ((inside (let ((qa334-var 20))
                            (qa334-kill all)
                            (set-buffer other)
                            (list qa334-var (local-variable-p 'qa334-var)))))
              (list inside qa334-var (local-variable-p 'qa334-var)
                    (with-current-buffer original
                      (list qa334-var (local-variable-p 'qa334-var))))))
        (set-buffer original)
        (kill-buffer other)))))
(defun qa334-watch (all recreate)
  (with-temp-buffer
    (setq-local qa334-var 10)
    (let* ((events nil)
           (buffer (current-buffer))
           (watcher (lambda (_symbol value operation where)
                      (when (eq operation 'unlet)
                        (push (list value (eq where buffer)
                                    (local-variable-p 'qa334-var)) events)))))
      (unwind-protect
          (progn
            (add-variable-watcher 'qa334-var watcher)
            (let ((qa334-var 20))
              (qa334-kill all)
              (when recreate (setq-local qa334-var 30)))
            (list qa334-var (local-variable-p 'qa334-var) (nreverse events)))
        (remove-variable-watcher 'qa334-var watcher)))))

;; dynamic deletion normal all=nil
(qa (qa334-dynamic nil 'normal)
    '((1 nil nil) 1 nil 1))

;; dynamic deletion default all=nil
(qa (qa334-dynamic nil 'default)
    '((3 nil nil) 3 nil 3))

;; dynamic deletion recreate all=nil
(qa (qa334-dynamic nil 'recreate)
    '((30 t nil) 10 t 1))

;; dynamic deletion recreate-kill all=nil
(qa (qa334-dynamic nil 'recreate-kill)
    '((1 nil nil) 1 nil 1))

;; dynamic deletion make-local all=nil
(qa (qa334-dynamic nil 'make-local)
    '((1 t nil) 10 t 1))

;; dynamic deletion nested all=nil
(qa (qa334-dynamic nil 'nested)
    '((1 nil (30 nil)) 1 nil 1))

;; dynamic deletion error all=nil
(qa (qa334-dynamic nil 'error)
    '(error 1 nil 1))

;; unwind checks bound buffer rather than current buffer
(qa (qa334-switch nil)
    '((99 t) 99 t (1 nil)))

;; dynamic deletion normal all=t
(qa (qa334-dynamic t 'normal)
    '((1 nil nil) 1 nil 1))

;; dynamic deletion default all=t
(qa (qa334-dynamic t 'default)
    '((3 nil nil) 3 nil 3))

;; dynamic deletion recreate all=t
(qa (qa334-dynamic t 'recreate)
    '((30 t nil) 10 t 1))

;; dynamic deletion recreate-kill all=t
(qa (qa334-dynamic t 'recreate-kill)
    '((1 nil nil) 1 nil 1))

;; dynamic deletion make-local all=t
(qa (qa334-dynamic t 'make-local)
    '((1 t nil) 10 t 1))

;; dynamic deletion nested all=t
(qa (qa334-dynamic t 'nested)
    '((1 nil (30 nil)) 1 nil 1))

;; dynamic deletion error all=t
(qa (qa334-dynamic t 'error)
    '(error 1 nil 1))

;; unwind checks bound buffer rather than current buffer
(qa (qa334-switch t)
    '((99 t) 99 t (1 nil)))

;; unlet notifications only for restored locals
(qa (qa334-watch nil nil)
    '(1 nil nil))

;; unlet notifications only for restored locals
(qa (qa334-watch nil t)
    '(10 t ((10 t t))))

;; unlet notifications only for restored locals
(qa (qa334-watch t nil)
    '(1 nil nil))

;; ordinary local dynamic unwind control
(qa (with-temp-buffer (setq-local qa334-var 10) (list (let ((qa334-var 20)) qa334-var) qa334-var (local-variable-p 'qa334-var)))
    '(20 10 t))

(qa (with-temp-buffer
      (setq-local qa334-var 10)
      (list (let ((qa334-var 20))
              (makunbound 'qa334-var)
              (list (boundp 'qa334-var) (local-variable-p 'qa334-var)))
            qa334-var (local-variable-p 'qa334-var)))
    '((nil t) 10 t))

(princ (qa--done))
