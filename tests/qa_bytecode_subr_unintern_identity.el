;;; -*- lexical-binding: nil; -*-
;; Byte-code primitive dispatch survives uninterning its public name (#596).
(qa
 (let ((fn (byte-compile '(lambda (x y) (+ x y)))))
   (unintern "+" nil)
   (list (funcall fn 1 2) (null (intern-soft "+"))))
 '(3 t))

;; `.elc' byte-code functions keep byte-code metadata in the native-comp
;; build even when their callsites run in a compiled-equivalent context.
(qa
 (let ((function (cdr (symbol-function 'when))))
   (list (functionp function)
         (compiled-function-p function)
         (native-comp-function-p function)
         (subrp function)
         (type-of function)
         (byte-code-function-p function)
         (substring (prin1-to-string function) 0 2)))
 '(t t nil nil byte-code-function t "#["))
(qa-err (subr-arity (cdr (symbol-function 'when))) 'wrong-type-argument)
(qa-err (subr-name (cdr (symbol-function 'when))) 'wrong-type-argument)
(qa-err (subr-type (cdr (symbol-function 'when))) 'wrong-type-argument)
(qa-err (subr-native-lambda-list (cdr (symbol-function 'when)))
        'wrong-type-argument)
(qa-err (subr-native-comp-unit (cdr (symbol-function 'when)))
        'wrong-type-argument)

(princ (qa--done))
