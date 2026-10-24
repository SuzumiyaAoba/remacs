;;; Deferred quits, input throws and VM polling. -*- lexical-binding: t; -*-
(require 'bytecomp)
(defvar qa-qf-mode nil)
(defvar qa-qf-seen nil)
(defvar qa-qf-tag nil)
(defvar qa-qf-count 0)
(defvar qa-qf-watched 0)
(defvar qa-qf-arm '(setq quit-flag t max-lisp-eval-depth 100 inhibit-quit nil))
(defun qa-qf-checkpoint () 'checked)
(defun qa-qf-state (form compiled)
  (let ((inhibit-quit t) (quit-flag nil) (byte-optimize t)
        (max-lisp-eval-depth 1600))
    (let* ((definition (list 'lambda nil form))
           (function (if compiled (byte-compile definition) (eval definition t)))
           (result (funcall function)))
      (prog1 (list result quit-flag) (setq quit-flag nil)))))
(defun qa-qf-lower-limit (count)
  (if (> count 0)
      (qa-qf-lower-limit (1- count))
    (eval qa-qf-arm)
    (qa-qf-checkpoint)))

(dolist (qa-qf-mode '(nil t))
  (qa (qa-qf-state
       '(let (qa-qf-seen
              (signal-hook-function
               (lambda (symbol data)
                 (push (list symbol data quit-flag inhibit-quit) qa-qf-seen))))
          (condition-case err
              (let ((inhibit-quit nil)) (setq quit-flag t) (qa-qf-checkpoint))
            (quit (list err quit-flag qa-qf-seen)))) qa-qf-mode)
      '(((quit) nil ((quit nil nil nil))) nil))

  (qa (qa-qf-state
       '(let (qa-qf-seen
              (watcher (lambda (_symbol value operation _where)
                         (push (list operation value) qa-qf-seen))))
          (unwind-protect
              (progn
                (add-variable-watcher 'quit-flag watcher)
                (condition-case nil
                    (let ((inhibit-quit nil)) (setq quit-flag t) (qa-qf-checkpoint))
                  (quit nil))
                (nreverse qa-qf-seen))
            (remove-variable-watcher 'quit-flag watcher))) qa-qf-mode)
      '(((set t)) nil))

  (qa (qa-qf-state
       '(progn
          (setq quit-flag 'pending)
          (let ((result (let ((quit-flag nil) (inhibit-quit nil)) (qa-qf-checkpoint))))
            (list result quit-flag))) qa-qf-mode)
      '((checked pending) pending))

  (qa (qa-qf-state
       '(progn
          (let ((quit-flag nil)) (setq quit-flag 'pending))
          quit-flag) qa-qf-mode) '(pending pending))

  (qa (qa-qf-state
       '(progn
          (unwind-protect (setq quit-flag 'pending) (setq quit-flag nil))
          quit-flag) qa-qf-mode) '(pending pending))
  (qa (qa-qf-state
       '(progn
          (unwind-protect (setq quit-flag 'older) (setq quit-flag 'newer))
          quit-flag) qa-qf-mode) '(newer newer))
  (qa (qa-qf-state
       '(let (qa-qf-seen
              (watcher (lambda (_symbol _value operation _where)
                         (when (eq operation 'unlet) (push quit-flag qa-qf-seen)))))
          (unwind-protect
              (progn
                (add-variable-watcher 'qa-qf-watched watcher)
                (let ((qa-qf-watched 1)) (setq quit-flag 'pending))
                qa-qf-seen)
            (remove-variable-watcher 'qa-qf-watched watcher))) qa-qf-mode)
      '((nil) pending))
  (qa (qa-qf-state
       '(let (qa-qf-seen)
          (condition-case nil
              (unwind-protect (progn (setq quit-flag 'pending) (error "body"))
                (setq qa-qf-seen quit-flag))
            (error nil))
          (list qa-qf-seen quit-flag)) qa-qf-mode) '((nil pending) pending))
  (qa (qa-qf-state
       '(let (qa-qf-seen
              (signal-hook-function
               (lambda (symbol _data) (push (list symbol quit-flag) qa-qf-seen))))
          (setq quit-flag 'pending)
          (condition-case nil (error "body") (error nil))
          qa-qf-seen) qa-qf-mode) '(((error pending)) pending))
  (qa (qa-qf-state
       '(let (qa-qf-seen
              (watcher (lambda (_symbol _value operation _where)
                         (when (eq operation 'unlet) (push quit-flag qa-qf-seen)))))
          (unwind-protect
              (progn
                (add-variable-watcher 'qa-qf-watched watcher)
                (condition-case nil
                    (let ((qa-qf-watched 1))
                      (condition-case nil
                          (unwind-protect (error "body") (setq quit-flag 'newer))
                        (file-error nil)))
                  (error nil)))
            (remove-variable-watcher 'qa-qf-watched watcher))
          (list qa-qf-seen quit-flag)) qa-qf-mode) '(((nil) newer) newer))
  (qa (qa-qf-state
       '(progn
          (condition-case nil
              (unwind-protect (progn (setq quit-flag (list 'tag)) (error "body"))
                (garbage-collect))
            (error nil))
          (car quit-flag)) qa-qf-mode) '(tag (tag)))

  (qa (qa-qf-state
       '(let* ((qa-qf-tag (list 'tag)) (throw-on-input qa-qf-tag)
               qa-qf-seen
               (signal-hook-function (lambda (symbol _data) (push symbol qa-qf-seen))))
          (list (catch qa-qf-tag
                  (let ((inhibit-quit nil)) (setq quit-flag qa-qf-tag) (qa-qf-checkpoint)))
                qa-qf-seen)) qa-qf-mode)
      '((t nil) nil))
  (qa (qa-qf-state
       '(let ((throw-on-input 'absent))
          (condition-case err
              (let ((inhibit-quit nil)) (setq quit-flag 'absent) (qa-qf-checkpoint))
            (no-catch err))) qa-qf-mode) '((no-catch absent t) nil))

  (qa (qa-qf-state
       '(let ((throw-on-input (list 'tag)))
          (condition-case nil
              (let ((inhibit-quit nil)) (setq quit-flag (list 'tag)) (qa-qf-checkpoint))
            (quit 'ordinary-quit))) qa-qf-mode)
      '(ordinary-quit nil))

  (qa (qa-qf-state
       '(prog1
            (let ((inhibit-quit t)) (list (setq quit-flag t inhibit-quit nil) 42))
          (setq quit-flag nil)) qa-qf-mode)
      '((nil 42) nil))
  (qa (qa-qf-state
       '(let ((inhibit-quit t) (items '(1 2 3)))
          (memq (progn (setq quit-flag t inhibit-quit nil) 1) items)) qa-qf-mode)
      '((1 2 3) t))
  (qa (qa-qf-state
       '(condition-case nil
            (let ((inhibit-quit t) (items '(1 2 3)))
              (memq (progn (setq quit-flag t inhibit-quit nil) 9) items))
          (quit 'caught)) qa-qf-mode) '(caught nil))

  (qa (qa-qf-state
       '(condition-case nil
            (let ((inhibit-quit nil)) (setq quit-flag t) 'safe)
          (quit 'caught)) qa-qf-mode)
      (if qa-qf-mode '(safe t) '(caught nil)))

  (qa (qa-qf-state
       '(condition-case nil
            (let ((inhibit-quit t)) (qa-qf-lower-limit 150))
          (quit 'quit-first) (excessive-lisp-nesting 'depth-first)) qa-qf-mode)
      '(quit-first nil))

  (qa (qa-qf-state
       '(let* ((process (make-process :name "qa-quit-write" :connection-type 'pipe
                                     :noquery t :sentinel #'ignore
                                     :command '("/bin/sh" "-c" "sleep 0.2")))
               (timer (run-at-time 0.02 nil (lambda () (setq quit-flag t)))))
          (unwind-protect
              (condition-case nil
                  (let ((inhibit-quit nil))
                    (process-send-string process (make-string (* 2 1024 1024) ?x))
                    'returned)
                (quit 'caught) (error 'send-error))
            (cancel-timer timer)
            (delete-process process))) qa-qf-mode) '(caught nil)))

;; With no calls in the body, GNU checks every 255 backward branches.
(fset 'qa-qf-child
      (byte-compile
       '(lambda ()
          (condition-case nil
              (let ((inhibit-quit nil))
                (setq quit-flag t)
                (while t (setq qa-qf-count (1+ qa-qf-count))))
            (quit qa-qf-count)))))
(defalias 'qa-qf-child-alias 'qa-qf-child)
(qa (let ((qa-qf-count 0)) (qa-qf-state '(qa-qf-child) t)) '(255 nil))
(qa (let ((qa-qf-count 0))
      (qa-qf-state '(let ((index 5))
                      (while (> index 0) (setq index (1- index)))
                      (qa-qf-child)) t)) '(250 nil))
(qa (let ((qa-qf-count 0))
      (qa-qf-state '(let ((index 5))
                      (while (> index 0) (setq index (1- index)))
                      (qa-qf-child-alias)) t)) '(255 nil))
(qa (let ((qa-qf-count 0))
      (qa-qf-state '(let ((index 5))
                      (while (> index 0) (setq index (1- index)))
                      (let ((first (qa-qf-child)))
                        (setq qa-qf-count 0)
                        (list first (qa-qf-child)))) t)) '((250 1) nil))

(qa (let ((saved-inhibit inhibit-quit) (saved-flag quit-flag) result)
      (unwind-protect
          (progn
            (setq inhibit-quit t quit-flag 'probe-tag)
            (setq result (thread-join (make-thread (lambda () (list quit-flag inhibit-quit)))))
            (setq quit-flag nil))
        (setq quit-flag saved-flag inhibit-quit saved-inhibit))
      result) '(probe-tag t))
(qa (qa-qf-state
     '(progn (setq quit-flag 'pending)
             (thread-join (make-thread (lambda () (list quit-flag inhibit-quit))))) nil)
    '((nil nil) pending))

(defvaralias 'qa-qf-flag-alias 'quit-flag)
(dolist (qa-qf-mode '(nil t))
  (qa (qa-qf-state
       '(condition-case nil
            (let ((inhibit-quit nil)) (setq qa-qf-flag-alias t) (qa-qf-checkpoint))
          (quit 'caught)) qa-qf-mode) '(caught nil)))
(princ (qa--done)) (terpri)
