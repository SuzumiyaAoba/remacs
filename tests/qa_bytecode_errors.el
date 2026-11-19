;;; -*- lexical-binding: t; -*-
;; Defined entry validation and invalid-instruction diagnostics, verified
;; against GNU 31.1. Constant indices in these programs are always in range.
(qa-err-data (byte-code nil [] 0) 'error '("Invalid byte-code"))
(qa-err-data (byte-code [] [] 0) 'error '("Invalid byte-code"))
(qa-err-data (byte-code "" nil 0) 'error '("Invalid byte-code"))
(qa-err-data (byte-code "" '(1) 0) 'error '("Invalid byte-code"))
(qa-err-data (byte-code "" [] -1) 'error '("Invalid byte-code"))
(qa-err-data (byte-code "" [] nil) 'error '("Invalid byte-code"))
(qa-err-data (byte-code "" [] t) 'error '("Invalid byte-code"))
(qa-err-data (byte-code "" [] 0.0) 'error '("Invalid byte-code"))
(qa-err-data (byte-code "" [] "0") 'error '("Invalid byte-code"))
(qa-err-data (byte-code "" [] []) 'error '("Invalid byte-code"))
(qa-err-data (byte-code "" [] 2305843009213693952) 'error '("Invalid byte-code"))
(qa-err-data (byte-code (unibyte-string 135) [] 600000)
             'error '("Bytecode stack overflow"))
(qa-err-data (funcall (make-byte-code 0 (unibyte-string 135) [] 600000))
             'error '("Bytecode stack overflow"))
(when qa--remacs
  ;; The declared stack depth cannot hold the optional argument slots.
  (qa-err-data (funcall (make-byte-code 32768 (unibyte-string 135) [] 1))
               'error '("Invalid byte-code")))

(qa-err-data (byte-code "" [] 0) 'error '("Invalid byte opcode: op=0, ptr=0"))
(qa-err-data (byte-code "" [] 1) 'error '("Invalid byte opcode: op=0, ptr=0"))
(qa-err-data (byte-code (unibyte-string 0) [] 0)
             'error '("Invalid byte opcode: op=0, ptr=0"))
(qa-err-data (byte-code (unibyte-string 51) [] 0)
             'error '("Invalid byte opcode: op=51, ptr=0"))
(qa-err-data (byte-code (unibyte-string 128) [] 0)
             'error '("Invalid byte opcode: op=128, ptr=0"))
(qa-err-data (byte-code (unibyte-string 184) [] 0)
             'error '("Invalid byte opcode: op=184, ptr=0"))
(qa-err-data (byte-code (unibyte-string 191) [] 0)
             'error '("Invalid byte opcode: op=191, ptr=0"))
(qa-err-data (byte-code (unibyte-string 192 0) [ok] 1)
             'error '("Invalid byte opcode: op=0, ptr=1"))
(qa-err-data (byte-code (unibyte-string 192 51) [ok] 1)
             'error '("Invalid byte opcode: op=51, ptr=1"))
(qa-err-data (byte-code (unibyte-string 192) [ok] 1)
             'error '("Invalid byte opcode: op=0, ptr=1"))
(qa-err-data (byte-code (unibyte-string 192 136) [ok] 1)
             'error '("Invalid byte opcode: op=0, ptr=2"))
(qa-err-data (byte-code (unibyte-string 130 3 0 0) [] 0)
             'error '("Invalid byte opcode: op=0, ptr=3"))
(qa-err-data (byte-code (unibyte-string 192 131 5 0 0 51) [nil] 1)
             'error '("Invalid byte opcode: op=51, ptr=5"))
(qa-err-data (byte-code (unibyte-string 192 131 5 0 0 51) [t] 1)
             'error '("Invalid byte opcode: op=0, ptr=4"))
(qa-err-data
 (let ((table (make-hash-table :test 'eq)))
   (puthash 'key -1 table)
   (funcall (make-byte-code '(x)
                            (unibyte-string 8 193 183 130 10 0
                                            194 135 195 135 196 135)
                            (vector 'x table 1 2 42)
                            2)
            'key))
 'error '("Invalid byte opcode: op=0, ptr=-1"))
(qa (let ((table (make-hash-table :test 'eq)))
      (puthash 'key 6 table)
      (funcall (make-byte-code '(x)
                               (unibyte-string 8 193 183 130 10 0
                                               194 135 195 135 196 135)
                               (vector 'x table 1 2 42)
                               2)
               'key))
    1)
(qa (let ((table (make-hash-table :test 'eq)))
      (puthash 'other 6 table)
      (funcall (make-byte-code '(x)
                               (unibyte-string 8 193 183 130 10 0
                                               194 135 195 135 196 135)
                               (vector 'x table 1 2 42)
                               2)
               'key))
    42)
(qa-err-data (byte-code (unibyte-string 8 135) [42] 1)
             'wrong-type-argument '(symbolp 42))

;; Valid constant loads and byte-code objects keep their ordinary behavior.
(qa (byte-code (unibyte-string 192 135) [answer] 1) 'answer)
(qa (byte-code (unibyte-string 192 135) [0] 1) 0)
(qa (byte-code (unibyte-string 197 135) [0 1 2 3 4 five] 1) 'five)
(qa (byte-code (unibyte-string 255 135) (make-vector 64 'last) 1) 'last)
(qa (byte-code (unibyte-string 129 0 1 135) (make-vector 257 'wide) 1) 'wide)
;; GNU's unchecked out-of-range reads are heap-layout-dependent. Remacs
;; rejects malformed constant operands instead of reading outside the vector.
(when qa--remacs
  (qa-err-data (byte-code (unibyte-string 193 135) [only] 1)
               'error '("Invalid byte-code"))
  (qa-err-data (byte-code (unibyte-string 129 1 1 135) [only] 1)
               'error '("Invalid byte-code"))
  (qa-err-data (byte-code (unibyte-string 9 135) [only] 1)
               'error '("Invalid byte-code")))
;; stack-set operand zero discards TOS; positive operands replace the
;; requested element below TOS, including the two-byte stack-set2 form.
(qa (byte-code (unibyte-string 192 193 178 0 135) [left right] 2) 'left)
(qa (byte-code (unibyte-string 192 193 194 178 1 135)
               [left middle right] 3)
    'right)
(qa (byte-code (unibyte-string 192 193 194 179 1 0 135)
               [left middle right] 3)
    'right)
(qa (funcall (make-byte-code 0 (unibyte-string 192 135) [ok] 1)) 'ok)
(qa-err-data (funcall (make-byte-code 0 (unibyte-string 51) [] 0))
             'error '("Invalid byte opcode: op=51, ptr=0"))
(qa-err-data (funcall (make-byte-code 0 (unibyte-string 192 0) [ok] 1))
             'error '("Invalid byte opcode: op=0, ptr=1"))

;; The legacy byte-catch opcode evaluates one form, catches only its tag,
;; propagates unmatched throws, and removes the tag after errors.
(qa
 (let* ((code (unibyte-string 192 193 141 135))
        (normal (make-byte-code 0 code
                                (vector 'qa-bytecatch-tag '(+ 20 22)) 2))
        (matched (make-byte-code 0 code
                                 (vector 'qa-bytecatch-tag
                                         '(throw 'qa-bytecatch-tag 'caught)) 2))
        (unmatched (make-byte-code 0 code
                                   (vector 'qa-bytecatch-inner
                                           '(throw 'qa-bytecatch-outer 'escaped)) 2))
        (nil-tag (make-byte-code 0 code
                                 (vector nil '(throw nil 'uncaught)) 2))
        (failing (make-byte-code 0 code
                                 (vector 'qa-bytecatch-tag '(error "failure")) 2)))
   (list (funcall normal)
         (funcall matched)
         (catch 'qa-bytecatch-outer (funcall unmatched))
         (condition-case err (funcall nil-tag) (no-catch (car err)))
         (condition-case err (funcall failing) (error (car err)))
         (condition-case err
             (throw 'qa-bytecatch-tag 'after-error)
           (no-catch (car err)))))
 '(42 caught escaped no-catch error no-catch))

;; The legacy byte-condition-case opcode shares the normal special-form
;; path, including evaluating its body after validating handler clauses.
(qa
 (let* ((code (unibyte-string 192 193 194 143 135))
        (normal (make-byte-code 0 code
                                (vector 'qa-bytecond-error-var '(+ 20 22)
                                        '((error qa-bytecond-error-var))) 3))
        (caught (make-byte-code 0 code
                                (vector 'qa-bytecond-error-var '(error "legacy")
                                        '((error (car qa-bytecond-error-var)))) 3))
        (bad-handler (make-byte-code
                      0 code
                      (vector 'qa-bytecond-error-var
                              '(setq qa-bytecond-body-ran t)
                              '((1 nil))) 3))
        (qa-bytecond-body-ran nil))
   (list (funcall normal)
         (funcall caught)
         (condition-case err (funcall bad-handler) (error (car err)))
         qa-bytecond-body-ran))
 '(42 error error nil))

;; Legacy temporary-output-buffer opcodes bind standard-output, run the
;; setup/show callbacks, and unwind the binding if the body signals.
(require 'cl-lib)
(defun qa-bytecode-temp-buffer-observed-window-limit (limit &optional setup-hook)
  (let ((window-combination-limit limit)
        (temp-buffer-setup-hook setup-hook)
        (temp-buffer-show-function nil)
        (temp-buffer-show-hook nil)
        (observed nil)
        (function
         (make-byte-code 0
                         (unibyte-string 192 144 194 193 33 145 135)
                         (vector "*qa-bytecode-temp-window-limit*" "x" 'princ)
                         3)))
    (cl-letf (((symbol-function 'display-buffer)
               (lambda (&rest _)
                 (setq observed window-combination-limit)
                 nil)))
      (funcall function))
    observed))
(qa (list (qa-bytecode-temp-buffer-observed-window-limit 'temp-buffer)
          (qa-bytecode-temp-buffer-observed-window-limit
           'temp-buffer-resize
           (list (lambda () (setq-local temp-buffer-resize-mode t))))
          (qa-bytecode-temp-buffer-observed-window-limit 'other))
    '(temp-buffer temp-buffer-resize other))
(defvar qa-bytecode-temp-buffer-display-current nil)
(qa
 (let* ((qa-bytecode-temp-buffer-display-current nil)
        (temp-buffer-setup-hook nil)
        (temp-buffer-show-function nil)
        (temp-buffer-show-hook nil)
        (function
         (make-byte-code 0
                         (unibyte-string 192 144 194 193 33 145 135)
                         (vector "*qa-bytecode-temp-current-buffer*"
                                 "x" 'princ)
                         3)))
   (let ((caller (current-buffer)))
     (cl-letf (((symbol-function 'display-buffer)
                (lambda (&rest _args)
                  (setq qa-bytecode-temp-buffer-display-current
                        (current-buffer))
                  nil)))
       (funcall function))
     (eq caller qa-bytecode-temp-buffer-display-current)))
 t)

(qa
 (let* ((setup-seen nil)
        (shown nil)
        (before standard-output)
        (temp-buffer-setup-hook
         (list (lambda ()
                 (setq setup-seen
                       (list (eq (current-buffer)
                                 (get-buffer "*qa-bytecode-temp-output*"))
                             (buffer-string))))))
        (temp-buffer-show-function
         (lambda (buffer)
           (setq shown (with-current-buffer buffer (buffer-string)))))
        (function
         (make-byte-code 0
                         (unibyte-string 192 144 194 193 33 145 135)
                         (vector "*qa-bytecode-temp-output*" "hello" 'princ)
                         3)))
   (list (funcall function) setup-seen shown (eq standard-output before)))
 '("hello" (t "") "hello" t))
(qa
 (let* ((before standard-output)
        (hook-state nil)
        (temp-buffer-setup-hook nil)
        (temp-buffer-show-function nil)
        (temp-buffer-show-hook
         (list (lambda ()
                 (setq hook-state
                       (list (eq (current-buffer)
                                 (get-buffer "*qa-bytecode-temp-display*"))
                             (window-live-p (selected-window)))))))
        (function
         (make-byte-code 0
                         (unibyte-string 192 144 194 193 33 145 135)
                         (vector "*qa-bytecode-temp-display*" "world" 'princ)
                         3))
        (result (funcall function))
        (buffer (get-buffer "*qa-bytecode-temp-display*")))
   (list result
         (eq standard-output before)
         (with-current-buffer buffer (buffer-string))
         hook-state))
 '("world" t "world" (t t)))
(defvar qa-bytecode-temp-buffer-visible-range nil)
(defun qa-bytecode-temp-buffer-capture-visible-range (buffer)
  (setq qa-bytecode-temp-buffer-visible-range
        (with-current-buffer buffer (list (point-min) (point-max)))))
(qa
 (let* ((qa-bytecode-temp-buffer-visible-range nil)
        (temp-buffer-setup-hook nil)
        (temp-buffer-show-function
         #'qa-bytecode-temp-buffer-capture-visible-range)
        (function
         (make-byte-code
          0
          (unibyte-string 192 144 193 194 33 136 195 196 33 145 135)
          (vector "*qa-bytecode-temp-narrow*" 'princ "hello" 'eval
                  '(with-current-buffer "*qa-bytecode-temp-narrow*"
                     (narrow-to-region 2 3)))
          5)))
   (funcall function)
   qa-bytecode-temp-buffer-visible-range)
 '(1 6))
(qa
 (let ((before standard-output)
       (temp-buffer-setup-hook nil)
       (function
        (make-byte-code 0
                        (unibyte-string 192 144 193 194 33 135)
                        (vector "*qa-bytecode-temp-output-error*" 'error "failure")
                        3)))
   (list (condition-case err (funcall function) (error (car err)))
         (eq standard-output before)))
 '(error t))

;; The obsolete save-window-excursion opcode evaluates its TOS as a progn
;; body and restores the full configuration on both return and error.
(qa
 (let* ((selected (selected-window))
        (before (length (window-list)))
        (normal (make-byte-code 0 (unibyte-string 192 139 135)
                                (vector '((split-window) 'answer)) 1))
        (failing (make-byte-code 0 (unibyte-string 192 139 135)
                                 (vector '((split-window) (error "failure"))) 1))
        (throwing (make-byte-code
                   0 (unibyte-string 192 139 135)
                   (vector '((split-window)
                             (throw 'qa-window-excursion-tag 'escaped))) 1)))
   (list (funcall normal)
         (= before (length (window-list)))
         (eq selected (selected-window))
         (condition-case nil (funcall failing) (error 'caught))
         (= before (length (window-list)))
         (eq selected (selected-window))
         (catch 'qa-window-excursion-tag (funcall throwing))
         (= before (length (window-list)))
         (eq selected (selected-window))))
 '(answer t t caught t t escaped t t))

;; List argdescs keep the function object as the WNA datum, while packed
;; integer argdescs use the (MIN . MAX) descriptor.
(qa (let* ((function (make-byte-code '(x)
                                     (unibyte-string 192 135) [x] 1))
           (name (make-symbol "qa-bytecode-wna")))
      (fset name function)
      (unwind-protect
          (list
           (condition-case error
               (funcall function)
             (wrong-number-of-arguments (eq (cadr error) function)))
           (condition-case error
               (eval (list name))
             (wrong-number-of-arguments (eq (cadr error) function))))
        (fmakunbound name)))
    '(t t))
(qa (let ((function (make-byte-code 0 (unibyte-string 192 135) [ok] 1)))
      (condition-case error
          (funcall function 'extra)
        (wrong-number-of-arguments
         (equal (cadr error) '(0 . 0)))))
    t)
(qa (let ((function (read "#[0 \"\" [] 0]")))
      (condition-case error
          (funcall function 'extra)
        (wrong-number-of-arguments
         (equal (cadr error) '(0 . 0)))))
    t)

;; GNU accepts an unchecked circular ARGLIST when constructing the object,
;; then rejects every call with the function object as the WNA datum.
(qa (let* ((arglist (read "#1=(x . #1#)"))
           (function (make-byte-code arglist (string-as-unibyte "") [] 0)))
      (mapcar (lambda (arguments)
                (condition-case err
                    (apply function arguments)
                  (wrong-number-of-arguments (eq (cadr err) function))))
              '(nil (1) (1 2))))
    '(t t t))

(defun qa-bytecode-call-error (function arguments)
  (condition-case err
      (list 'ok (apply function arguments))
    (error (list (car err) (eq (cadr err) function)))))
;; A dotted tail is diagnosed after the required prefix's WNA check.
;; Non-symbol entries and malformed marker sequences are invalid-function.
(qa (let* ((code (unibyte-string 192 135))
           (constants [ok])
           (dotted (make-byte-code '(x . y) code constants 1))
           (non-symbol (make-byte-code '(1) code constants 1))
           (bare-rest (make-byte-code '(&rest) code constants 1))
           (repeated-optional
            (make-byte-code '(&optional x &optional y) code constants 1)))
      (list (qa-bytecode-call-error dotted nil)
            (qa-bytecode-call-error dotted '(a))
            (qa-bytecode-call-error dotted '(a b))
            (qa-bytecode-call-error non-symbol nil)
            (qa-bytecode-call-error bare-rest nil)
            (qa-bytecode-call-error repeated-optional nil)))
    '((wrong-number-of-arguments t)
      (invalid-function t)
      (invalid-function t)
      (invalid-function t)
      (invalid-function t)
      (invalid-function t)))
;; The rest parameter is the first symbol after &rest; later arglist symbols
;; remain bound to nil, matching interpreted lambda binding.
(qa (let ((function
           (make-byte-code '(&rest x y)
                           (unibyte-string 8 135) [x] 1)))
      (list (funcall function) (funcall function 'a 'b)))
    '(nil (a b)))
(qa (byte-code (unibyte-string 192 135) [still-ok] 1) 'still-ok)
(qa-err-data (make-byte-code 0 nil [] 0) 'error '("Invalid byte-code object"))
(qa-err-data (make-byte-code 0 "" nil 0) 'error '("Invalid byte-code object"))
(qa-err-data (make-byte-code 'bad (unibyte-string 192 135) [ok] 1)
             'error '("Invalid byte-code object"))
(qa-err-data (make-byte-code "bad" (unibyte-string 192 135) [ok] 1)
             'error '("Invalid byte-code object"))
(qa-err-data (make-byte-code [bad] (unibyte-string 192 135) [ok] 1)
             'error '("Invalid byte-code object"))
(qa-err-data (make-byte-code 2305843009213693952
                             (unibyte-string 192 135) [ok] 1)
             'error '("Invalid byte-code object"))
(qa-err-data (make-byte-code 0 "" [] -1) 'error '("Invalid byte-code object"))
(qa-err-data (make-byte-code 0 "" [] nil) 'error '("Invalid byte-code object"))
(qa-err-data (make-byte-code 0 "" [] 0.0) 'error '("Invalid byte-code object"))
(qa-err-data (make-byte-code 0 "" [] 2305843009213693952)
             'error '("Invalid byte-code object"))
(qa (byte-code-function-p
     (make-byte-code 0 (unibyte-string 192 135) [ok] 2305843009213693951)) t)

;; The byte-code object's ARGLIST remains mutable after construction. A
;; non-list, non-integer replacement is an invalid function at call time and
;; during arity introspection, while FUNCALL still evaluates its arguments.

(qa (byte-code "Ç" [0 1 2 answer 4 5 6 7] 1) 'answer)
(qa (byte-code (string-to-multibyte (unibyte-string 192 135)) [old-format] 1) 'old-format)
(qa-err-data (byte-code (string-to-multibyte "3") [] 0)
             'error '("Invalid byte opcode: op=51, ptr=0"))
(qa-err-data (make-byte-code 0 "Ç" [0 1 2 answer 4 5 6 7] 1)
             'error '("Invalid byte-code object"))
(qa-err-data (make-byte-code 0 (string-to-multibyte (unibyte-string 192 135)) [ok] 1)
             'error '("Invalid byte-code object"))

(qa (let ((constants (make-vector 34 nil)))
      (aset constants 33 '(answer))
      (byte-code (string #x1887) constants 1)) 'answer)
(qa-err-data (byte-code (string #x200000) (make-vector 57 nil) 1)
             'error '("Invalid byte opcode: op=128, ptr=2"))
(qa (string-to-list (string-as-unibyte (string #x200000))) '(248 136 128 128 128))
(qa (string-to-list (string-as-unibyte (string #xd800))) '(237 160 128))
(qa (let ((function (read "#[0 \"Ç\" [0 1 2 answer 4 5 6 7] 1]")))
      (list (multibyte-string-p (aref function 1))
            (string-to-list (aref function 1)) (funcall function)))
    '(nil (195 135) answer))
(qa-err-data (read "#[0 \"\\300\\207\" [ok] 2305843009213693952]")
             'invalid-read-syntax '("Invalid byte-code object"))
(qa-err-data (read "#[2305843009213693952 \"\\300\\207\" [ok] 1]")
             'invalid-read-syntax '("Invalid byte-code object"))

(princ (qa--done))
