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

(princ (qa--done))
