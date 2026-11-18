;;; Reserved error-handling depth and its cleanup. -*- lexical-binding: t; -*-
(defvar qa-sr-seen nil)
(defun qa-sr-recurse () (qa-sr-recurse))

(qa (let ((max-lisp-eval-depth 110) (lisp-eval-depth-reserve 200) qa-sr-seen
          (signal-hook-function
           (lambda (symbol data)
             (setq qa-sr-seen (list symbol data max-lisp-eval-depth lisp-eval-depth-reserve)))))
      (condition-case nil (qa-sr-recurse) (excessive-lisp-nesting nil))
      (list qa-sr-seen max-lisp-eval-depth lisp-eval-depth-reserve))
    '((excessive-lisp-nesting (111) 131 179) 110 200))

(qa (let ((max-lisp-eval-depth 110) (lisp-eval-depth-reserve 200) qa-sr-seen)
      (condition-case nil
          (handler-bind ((excessive-lisp-nesting
                          (lambda (condition)
                            (setq qa-sr-seen
                                  (list (car condition) (cdr condition)
                                        max-lisp-eval-depth lisp-eval-depth-reserve)))))
            (qa-sr-recurse))
        (excessive-lisp-nesting nil))
      (list qa-sr-seen max-lisp-eval-depth lisp-eval-depth-reserve))
    '((excessive-lisp-nesting (111) 310 0) 110 200))

(qa (let ((max-lisp-eval-depth 110) (lisp-eval-depth-reserve 200) qa-sr-seen
          (signal-hook-function (lambda (_symbol _data) nil))
          (watcher (lambda (_symbol _value operation _where) (push operation qa-sr-seen))))
      (unwind-protect
          (progn
            (add-variable-watcher 'max-lisp-eval-depth watcher)
            (add-variable-watcher 'lisp-eval-depth-reserve watcher)
            (condition-case nil (qa-sr-recurse) (excessive-lisp-nesting nil))
            qa-sr-seen)
        (remove-variable-watcher 'max-lisp-eval-depth watcher)
        (remove-variable-watcher 'lisp-eval-depth-reserve watcher))) nil)

(qa (let ((max-lisp-eval-depth 110) (lisp-eval-depth-reserve 200)
          (signal-hook-function
           (lambda (_symbol _data)
             (setq max-lisp-eval-depth (+ max-lisp-eval-depth 5)
                   lisp-eval-depth-reserve (+ lisp-eval-depth-reserve 7)))))
      (condition-case nil (qa-sr-recurse) (excessive-lisp-nesting nil))
      (list max-lisp-eval-depth lisp-eval-depth-reserve)) '(110 212))

(qa (let ((max-lisp-eval-depth 110) (lisp-eval-depth-reserve 200)
          (signal-hook-function (lambda (_symbol _data) (throw 'room-exit 'escaped))))
      (list (catch 'room-exit (qa-sr-recurse))
            max-lisp-eval-depth lisp-eval-depth-reserve)) '(escaped 110 200))

;; BigInt values within the forwarded signed-EMACS_INT range participate in
;; the same temporary debugger-headroom calculation and restore exactly.
(qa (let ((max-lisp-eval-depth 110)
          (lisp-eval-depth-reserve 9223372036854775807)
          qa-sr-seen
          (signal-hook-function
           (lambda (symbol data)
             (setq qa-sr-seen
                   (list symbol data max-lisp-eval-depth lisp-eval-depth-reserve)))))
      (list (condition-case nil (qa-sr-recurse) (excessive-lisp-nesting 'caught))
            qa-sr-seen max-lisp-eval-depth lisp-eval-depth-reserve))
    '(caught (excessive-lisp-nesting (111) 131 9223372036854775786)
      110 9223372036854775807))
(princ (qa--done)) (terpri)
