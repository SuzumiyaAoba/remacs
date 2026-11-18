;;; qa_kbd_macro_repeat.el --- Ending a macro includes its recorded iteration -*- lexical-binding: t; -*-
(defun qa343-iterations (repeat)
  (let ((calls 0))
    (start-kbd-macro nil)
    (let ((value (end-kbd-macro repeat
                   (lambda () (setq calls (1+ calls)) (< calls 3)))))
      (list value calls (and defining-kbd-macro t) last-kbd-macro))))
(defun qa343-invalid (repeat)
  (start-kbd-macro nil)
  (unwind-protect
      (condition-case err
          (list :success (end-kbd-macro repeat (lambda () nil))
                (and defining-kbd-macro t))
        (error (list err (and defining-kbd-macro t))))
    (when defining-kbd-macro (end-kbd-macro))))

(qa (qa343-iterations nil) '(nil 0 nil []))

(qa (qa343-iterations 1) '(nil 0 nil []))

(qa (qa343-iterations 2) '(nil 1 nil []))

(qa (qa343-iterations 3) '(nil 2 nil []))

(qa (qa343-iterations 0) '(nil 3 nil []))

(qa (qa343-invalid t) '((wrong-type-argument fixnump t) t))

(qa (qa343-invalid 1.5) '((wrong-type-argument fixnump 1.5) t))

(qa (qa343-invalid '(2)) '((wrong-type-argument fixnump (2)) t))

(qa (qa343-invalid []) '((wrong-type-argument fixnump []) t))

(qa (qa343-invalid 2305843009213693952) '((wrong-type-argument fixnump 2305843009213693952) t))

(qa (progn (start-kbd-macro nil) (let ((result (end-kbd-macro nil 'qa343-missing-loop-function))) (list result defining-kbd-macro))) '(nil nil))

(qa (progn (start-kbd-macro nil) (let ((result (end-kbd-macro 1 'qa343-missing-loop-function))) (list result defining-kbd-macro))) '(nil nil))

;; C kbd-macro state updates bypass variable watchers, and nested execution
;; restores the outer macro, index, and real command.
(qa
 (let ((executing-kbd-macro "outer")
       (executing-kbd-macro-index 7)
       (real-this-command 'outer-command))
   (execute-kbd-macro "")
   (list executing-kbd-macro executing-kbd-macro-index real-this-command))
 '("outer" 7 outer-command))

(defvar qa343-kbd-state-events nil)
(defun qa343-kbd-state-watcher (symbol _value operation _where)
  (when (eq operation 'set)
    (push symbol qa343-kbd-state-events)))
(qa
 (let ((last-command 'outer-command)
       (this-command 'previous-command)
       (real-this-command 'previous-real-command)
       (last-kbd-macro "")
       (executing-kbd-macro nil)
       (executing-kbd-macro-index 9)
       (qa343-kbd-state-events nil))
   (dolist (symbol '(this-command real-this-command
                     executing-kbd-macro executing-kbd-macro-index))
     (add-variable-watcher symbol #'qa343-kbd-state-watcher))
   (unwind-protect
       (progn
         (call-last-kbd-macro)
         (list (nreverse qa343-kbd-state-events)
               this-command real-this-command
               executing-kbd-macro executing-kbd-macro-index))
     (dolist (symbol '(this-command real-this-command
                       executing-kbd-macro executing-kbd-macro-index))
       (remove-variable-watcher symbol #'qa343-kbd-state-watcher))))
 '(() outer-command "" nil 9))

;; command_loop_1 updates its forwarded command variables as C state, without
;; invoking Lisp variable watchers while replaying a key command.
(qa
 (let ((last-command nil)
       (this-command nil)
       (last-command-event nil)
       (current-prefix-arg nil)
       (prefix-arg '(1))
       (qa343-kbd-state-events nil)
       (symbols '(last-command this-command last-command-event
                  current-prefix-arg prefix-arg)))
   (dolist (symbol symbols)
     (add-variable-watcher symbol #'qa343-kbd-state-watcher))
   (unwind-protect
       (with-temp-buffer
         (execute-kbd-macro "a")
         (list (buffer-string) (nreverse qa343-kbd-state-events)))
     (dolist (symbol symbols)
       (remove-variable-watcher symbol #'qa343-kbd-state-watcher))))
 '("a" nil))

(qa
 (let ((defining-kbd-macro nil)
       (qa343-kbd-state-events nil))
   (add-variable-watcher 'defining-kbd-macro #'qa343-kbd-state-watcher)
   (unwind-protect
       (progn
         (start-kbd-macro nil)
         (let ((started defining-kbd-macro))
           (end-kbd-macro)
           (list started defining-kbd-macro qa343-kbd-state-events)))
     (remove-variable-watcher 'defining-kbd-macro #'qa343-kbd-state-watcher)))
 '(t nil nil))

(defun qa343-digit-argument-error (&rest _)
  (interactive)
  (signal 'file-error '("digit argument failed")))
(qa
 (let ((old (symbol-function 'digit-argument)))
   (unwind-protect
       (progn
         (fset 'digit-argument #'qa343-digit-argument-error)
         (condition-case error
             (progn (execute-kbd-macro (vector ?\C-u ?1)) 'returned)
           (error (car error))))
     (fset 'digit-argument old)))
 'file-error)

;; Is a search command inside a keyboard macro handled by the nested
;; isearch loop, so the following C-g aborts search without asking for a
;; line-oriented search string.
(qa
 (progn
   (require 'isearch)
   (execute-kbd-macro (vector ?\C-s ?\C-g))
   isearch-mode)
 nil)

(princ (qa--done))
