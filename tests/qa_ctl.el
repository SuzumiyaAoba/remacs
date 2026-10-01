;; control flow, special forms, defvar family, funcall/apply, setf places.
;; GNU-verified 31.1 notes: `progv', `subr-p', `psetf', `defsetf' were all
;; removed (void-function) — use cl-progv/subrp/cl-psetf instead.  `if' takes
;; 2+ args; extras are the else body: (if t 1 2 3) => 1.  `mapcar' accepts
;; exactly ONE sequence now (3 args => wrong-number-of-arguments).  `dolist'
;; evaluates its result clause OUTSIDE the loop variable's scope (lexical
;; binding => void-variable), while `dotimes' binds the counter to the count
;; there.  `track-mouse'/`when'/`setq-default' are NOT special forms in 31.1.
;; `setopt' exists and is a macro, but CALLING it in a -l-loaded file poisons
;; subsequent `condition-case' handlers under GNU (load aborts) — only its
;; fboundp/macrop status is pinned below.
(require 'cl-lib)

;; -- test fixtures -------------------------------------------------------
(defvar qact-dyn nil)          ; special variable for dynamic-binding checks
(defvar qact-noval)            ; defvar with no init: stays unbound
(defvar qact-keep 1)
(defvar qact-keep 2)           ; second defvar must NOT clobber
(defconst qact-con 1)
(defconst qact-con 2)          ; defconst DOES clobber
(defvar qact-mk 1)
(makunbound 'qact-mk)
(defvar qact-mk 3)             ; defvar after makunbound rebinds
(defun qact-real () 'rv)
(defalias 'qact-a1 'qact-real)
(defalias 'qact-a2 'qact-a1)
(defun qact-cmd (x) (interactive) x)
(defun qact-decl (x) (declare (indent 1)) (* x 2))
(defmacro qact-m (x) `(+ ,x 1))
(defun qact-fm () 1)
(fmakunbound 'qact-fm)
(defun qact-cadrcar (obj) (car (cadr obj)))
(defun qact-cadrcar-set (obj v) (setcar (cadr obj) v))
(gv-define-simple-setter qact-cadrcar qact-cadrcar-set)

;; == quote / function / closure ==========================================
(qa (quote (a b)) '(a b))
(qa 'selfeval-sym 'selfeval-sym)
(qa-lex (funcall (let ((x 5)) (lambda () x))) 5)   ; lexical closure
(qa (funcall #'car '(1 2)) 1)
(qa (eq #'car 'car) t)                          ; (function sym) => sym
(qa (function if) 'if)                          ; special form through `function'
(qa #'if 'if)
(qa ((lambda (x) x) 5) 5)                       ; literal lambda call
(qa (funcall '(lambda (x) (* x 10)) 5) 50)      ; lambda as funcall arg
(qa (functionp (let ((x 1)) #'(lambda () x))) t)
(qa (functionp 'car) t)
(qa (functionp (symbol-function 'car)) t)
(qa (functionp 'qact-nosuchfn) nil)

;; == eval + environment ===================================================
(qa (eval '(+ 1 2) t) 3)
(qa (eval '(+ a 1) '((a . 5))) 6)               ; alist env: lexical bindings
(qa (eval '(+ a b) '((a . 1) (b . 2))) 3)
(qa (eval 'x '((x))) nil)                       ; bare symbol entry binds nil
(qa-err-data (eval '(list a b) '((a . 1))) 'void-variable '(b))
(let ((a 100))
  (qa (eval 'a '((a . 5))) 5))                  ; env overrides lexical binding
(defvar qact-evd 4)
(qa (eval 'qact-evd nil) 4)                     ; nil env: dynamic lookup

;; == and / or =============================================================
(qa (and) t)
(qa (or) nil)
(qa (and 1 2 'x) 'x)                            ; last value, not t
(qa (or nil 'y 3) 'y)                           ; first non-nil value
(qa (and 1) 1)
(qa (or 'sym) 'sym)
(qa (or nil nil) nil)
(let ((n 0))
  (qa (and nil (setq n 99) t) nil)
  (qa n 0))                                     ; skipped forms never ran
(let ((n 0))
  (qa (or (setq n 1) (setq n 99)) 1)
  (qa n 1))                                     ; or returns the non-nil value
(let ((n 0))
  (qa (list (or (progn (setq n (1+ n)) nil)
                (progn (setq n (1+ n)) 'hit)
                (progn (setq n 99) 'never))
            n)
      '(hit 2)))                                ; strict left-to-right
(let ((n 0))
  (qa (list (and (progn (setq n (1+ n)) t)
                 (progn (setq n (1+ n)) 'last)
                 (progn (setq n 99) 'never))
            n)
      '(never 99)))                             ; all ran: last value returned

;; == cond / when / unless / if ============================================
(qa (cond) nil)
(qa (cond (nil)) nil)
(let ((x 1)) (qa (cond ((= x 1))) t))           ; no body: returns TEST value
(let ((x 2)) (qa (cond ((= x 1)) (t 'fall)) 'fall))
(qa (cond (5)) 5)
(qa (cond (5 'x)) 'x)                           ; body after true test
(qa (cond (nil) (t 'ok)) 'ok)
(qa-err-data (cond 5) 'wrong-type-argument '(listp 5))
(qa (when t 'a 'b) 'b)                          ; last body form
(qa (when nil 'a) nil)
(qa (unless t 'a) nil)
(qa (unless nil 'a 'b) 'b)
(qa (when t) nil)
(qa (if t 1 2 3) 1)                             ; extra args = else body
(qa (if nil 1 2 3) 3)
(qa (if 0 'y 'n) 'y)                            ; any non-nil is true
(qa (if "" 'y 'n) 'y)
(qa (if '() 'y 'n) 'n)
(qa-err-data (if t) 'wrong-number-of-arguments '(if 1))
(qa-err-data (if) 'wrong-number-of-arguments '(if 0))
(qa-err (if (car 5) 'y 'n) 'wrong-type-argument)

;; == while / dolist / dotimes =============================================
(let ((i 0) (s nil))
  (qa (while (< i 3) (push i s) (setq i (1+ i))) nil)  ; while returns nil
  (qa s '(2 1 0)))
(qa (while nil 'never) nil)
(qa (let ((i 3)) (while (> i 0) (setq i (1- i))) i) 0)
(qa (let ((i 2) (c 0)) (while i (setq i nil c 1)) c) 1) ; non-t test value
(qa (dolist (x '(a b) 'done) x) 'done)
(qa (dolist (x nil 'done)) 'done)
;; GNU/lexical: dolist's result clause is outside the loop var's scope.
(qa-err-data (dolist (x '(a b) x)) 'void-variable '(x))
(qa-err-data (dolist (x "ab" 'done)) 'wrong-type-argument '(listp "ab"))
(qa (dotimes (i 3 i)) 3)                        ; dotimes result: i = count
(qa (dotimes (i 0 'zero)) 'zero)
(let ((acc nil))
  (qa (dotimes (i 3 acc) (push i acc)) '(2 1 0))
  (qa acc '(2 1 0)))
(let ((lst nil))
  (qa (dotimes (i 4 (reverse lst)) (push i lst)) '(0 1 2 3)))

;; == catch / throw / unwind-protect =======================================
(qa (catch 't) nil)
(qa (catch 't 1 2 3) 3)                         ; last body form when no throw
(qa (catch 'sym (throw 'sym 1)) 1)
(qa (catch 1 (throw 1 'ok)) 'ok)                ; fixnum tags work (eq match)
(let ((tag (list 'x)))
  (qa (catch tag (throw tag 'ok)) 'ok))         ; shared-object tag works
;; fresh literals are distinct objects: eq match fails -> no-catch
(qa-err-data (catch "a" (throw "a" 'ok)) 'no-catch '("a" ok))
(qa-err-data (catch '(a) (throw '(a) 'ok)) 'no-catch '((a) ok))
(qa-err-data (catch 1.5 (throw 1.5 'ok)) 'no-catch '(1.5 ok))
(qa-err-data (throw 'qact-notag 1) 'no-catch '(qact-notag 1))
(let ((tg 'mytag))
  (qa (catch tg (throw 'mytag 1)) 1))           ; tag arg is evaluated
(qa (catch 'a (catch 'b (throw 'a 'outer)) 'inner) 'outer)
(qa (catch 'a (catch 'b (throw 'b 'hit-b)) 'after) 'after)
(qa (catch 't (or (throw 't 'boom) 'never)) 'boom)
(qa (catch 't (prog1 (throw 't 'v) 'x)) 'v)     ; throw exits prog1
(qa (unwind-protect 'v) 'v)
(qa (unwind-protect 7 'cleanup) 7)
(let ((n 0))
  (qa (list (catch 't1 (unwind-protect (throw 't1 5) (setq n (1+ n)))) n)
      '(5 1)))                                  ; cleanup ran during unwind
(let ((n 0))
  (qa (unwind-protect (setq n 1) (setq n (1+ n))) 1)
  (qa n 2))
(qa (catch 'a (unwind-protect (throw 'a 'v) 'done)) 'v)

;; == prog1 / prog2 / progn ================================================
(qa (progn) nil)
(qa (progn 42) 42)
(qa (prog1 'a 'b 'c) 'a)
(qa (prog1 7) 7)
(qa (prog2 'x 'y 'z) 'y)
(qa-err-data (prog1) 'wrong-number-of-arguments '(prog1 0))
;; NOTE: (prog2 x) is a macro-arity error raised at load time under -l
;; (eager macroexpansion aborts the file), so it cannot be checked here.
;; progv was removed from GNU 31.1 (it is void-function); cl-progv exists.
(qa (fboundp 'progv) nil)
(qa (special-form-p 'progv) nil)
(qa (fboundp 'cl-progv) t)

;; == let / let* ===========================================================
(let ((x 1))
  (qa (let ((x 2) (y x)) y) 1))                 ; parallel: init sees outer x
(let ((x 1))
  (qa (let* ((x 2) (y x)) y) 2))                ; sequential: init sees new x
(qa (let* ((a 1) (b (+ a 1)) (c (+ b 1))) (list a b c)) '(1 2 3))
(qa (let ((a)) a) nil)                          ; bare var => nil
(qa (let* ((a 1) (a 2)) a) 2)                   ; later binding wins
(let ((x 1))
  (qa (let ((x 2)) x) 2))
(let ((n 0))
  (qa (let ((a (setq n 1)) (b (setq n 2))) (list a b n)) '(1 2 2)))
;; let on an unproclaimed symbol in a dynamic file binds dynamically —
;; boundp inside sees it
(qa (boundp 'qact-lex) nil)
(let ((qact-lex 3))
  (qa (boundp 'qact-lex) t))
;; dynamic let DOES affect boundp/symbol-value
(let ((qact-dyn 5))
  (qa (boundp 'qact-dyn) t))
(defvar qact-sv nil)
(let ((qact-sv 1))
  (qa (symbol-value 'qact-sv) 1))

;; == setq / set / setq-default ============================================
(qa (setq qact-m1 1 qact-m2 2) 2)               ; returns last value
(qa (let ((x 1)) (setq x 2) x) 2)
(qa (set 'qact-set1 'v) 'v)
(qa (setq-default qact-sd2 7) 7)
(qa (default-value 'qact-sd2) 7)
(qa (symbol-value 'qact-sd2) 7)
;; setq-default on a dynamically-let var updates the current binding
(let ((qact-dyn 9))
  (qa (setq-default qact-dyn 11) 11)
  (qa qact-dyn 11))
(qa (symbol-value 'qact-dyn) nil)
(qa (fboundp 'setopt) t)                        ; macro; not called (see header)
(qa (consp (macrop 'setopt)) t)
;; constants
(qa-err-data (setq nil 1) 'setting-constant '(nil))
(qa-err-data (setq t 1) 'setting-constant '(t))
(qa-err-data (setq :k 1) 'setting-constant '(:k))
(qa-err-data (let ((nil 1)) nil) 'setting-constant '(nil))
(qa-err-data (let ((t 1)) t) 'setting-constant '(t))
;; remacs: (setq x) silently returns nil instead of signaling
(qa-err-data (setq qact-odd) 'wrong-number-of-arguments '(setq 1))
(qa :k :k)                                      ; keywords self-evaluate
(qa (boundp ':k) t)
(qa (boundp 't) t)
(qa (boundp 'nil) t)
(qa (fboundp ':k) nil)
(qa (keywordp :k) t)
(qa (keywordp 'k) nil)
(qa (eq :k ':k) t)

;; == defvar / defconst / makunbound / defalias ============================
(qa (boundp 'qact-noval) nil)                   ; defvar w/o init stays unbound
(qa qact-keep 1)                                ; second defvar did not clobber
(qa qact-con 2)                                 ; defconst DID clobber
(qa qact-mk 3)                                  ; defvar rebinds after makunbound
(qa (boundp 'qact-mk) t)
(qa (defvar qact-ret1 1) 'qact-ret1)            ; def* forms return the symbol
(qa (defvar qact-ret2) 'qact-ret2)
(qa (defconst qact-ret3 1) 'qact-ret3)
(qa (defun qact-ret4 () 1) 'qact-ret4)
(qa (defmacro qact-ret5 () 1) 'qact-ret5)
(qa (defalias 'qact-al 'car) 'qact-al)
(qa (makunbound 'qact-mu2) 'qact-mu2)
(qa (fmakunbound 'qact-fm2) 'qact-fm2)
(qa (qact-decl 5) 10)                           ; declare form ignored at runtime
(qa (qact-a2) 'rv)                              ; alias chain resolves
(qa (eq (indirect-function 'qact-a2) (symbol-function 'qact-real)) t)
(qa (funcall 'qact-al '(9 8)) 9)
(qa (indirect-function 'qact-nosuchfn) nil)
(qa (fboundp 'qact-fm) nil)
(qa (boundp 'qact-mu) nil)
(defvar qact-mu 3)
(makunbound 'qact-mu)
(qa (boundp 'qact-mu) nil)
(qa (special-variable-p 'qact-keep) t)          ; defvar makes special
(let ((qact-scoped 1))
  (qa (special-variable-p 'qact-scoped) nil))
(defvar qact-scoped-dv nil)
(qa (special-variable-p 'qact-scoped-dv) t)
(qa (default-boundp 'fill-column) t)
(qa (default-boundp 'qact-nosuchvar) nil)
(qa (default-value 'fill-column) 70)
(qa-err-data (symbol-value 'qact-nosuchvar) 'void-variable '(qact-nosuchvar))
(qa (symbol-function 'qact-nosuchfn) nil)       ; no error, returns nil

;; == special-form-p / subr predicates =====================================
(qa (mapcar #'special-form-p
            '(and catch cond defconst defvar function if let* let or progn
                  quote save-current-buffer save-excursion save-restriction
                  setq unwind-protect while interactive condition-case))
    '(t t t t t t t t t t t t t t t t t t t t))
;; NOT special forms in GNU 31.1:
(qa (mapcar #'special-form-p '(track-mouse when dolist setq-default car
                               progv mapcar))
    '(nil nil nil nil nil nil nil))
(qa (macrop 'when) t)
(qa (macrop 'if) nil)
(qa (macrop 'car) nil)
;; subr-p was removed; subrp takes the FUNCTION OBJECT, not the symbol.
(qa (fboundp 'subr-p) nil)
(qa (subrp (symbol-function 'car)) t)
(qa (subrp (symbol-function 'if)) t)            ; special forms are subrs too
(qa (subrp 'car) nil)                           ; symbol is not a subr
(qa (subrp (lambda (x) x)) nil)
(qa (subrp (indirect-function 'car)) t)
(qa (subr-primitive-p (symbol-function 'car)) t)
(qa (subr-arity (symbol-function 'car)) '(1 . 1))
(qa (subr-arity (symbol-function '+)) '(0 . many))
(qa (subr-arity (symbol-function 'if)) '(2 . unevalled))
(qa (commandp 'qact-cmd) t)
(qa (commandp (lambda (x) (interactive) x)) t)
(qa (commandp (lambda (x) x)) nil)
(qa (commandp 'car) nil)
(qa (interactive) nil)

;; == funcall / apply / mapcar =============================================
(qa (funcall #'car '(1 2)) 1)
(qa (funcall 'car '(1)) 1)                      ; subr symbol funcalls fine
(qa (funcall (symbol-function 'car) '(9)) 9)
(qa (funcall #'qact-real) 'rv)
(qa (apply #'+ 1 2 '(3 4)) 10)                  ; args + final list spliced
(qa (apply #'+ '(1 2 3)) 6)
(qa (apply #'list nil) nil)
(qa (apply #'list '(1)) '(1))
(qa (apply #'list 1 '(2)) '(1 2))
;; special forms / macros are invalid functions through funcall/apply
(qa-err (funcall 'if t 1) 'invalid-function)
(qa-err (funcall 'setq 'qact-ff 1) 'invalid-function)
(qa-err (funcall (symbol-function 'if) t 1 2) 'invalid-function)
(qa-err (apply 'if '(t 1 2)) 'invalid-function)
(qa-err (apply 'when '(t 1)) 'invalid-function)
;; error data carries the subr object, not the symbol
(qa-err-data (funcall #'car) 'wrong-number-of-arguments
             (list (symbol-function 'car) 0))
(qa-err-data (funcall #'car '(1) '(2)) 'wrong-number-of-arguments
             (list (symbol-function 'car) 2))
(qa-err-data (funcall) 'wrong-number-of-arguments '(funcall 0))
(qa-err-data (apply #'+ 1 2 3) 'wrong-type-argument '(listp 3))
(qa-err-data (apply #'list) 'wrong-type-argument '(listp list))
(qa-err-data (apply 'qact-nosuchfn '(1)) 'void-function '(qact-nosuchfn))
;; mapcar: one sequence only in 31.1; string input still yields a LIST
(qa (mapcar 'car '((1) (2))) '(1 2))
(qa (mapcar (lambda (x) (* x x)) '(1 2 3)) '(1 4 9))
(qa (mapcar #'1+ nil) nil)
(qa (mapcar #'upcase "ab") '(65 66))
(qa-err-data (mapcar #'+ '(1 2) '(10 20)) 'wrong-number-of-arguments '(mapcar 3))

;; == macroexpand on file-local macros =====================================
(qa (qact-m 9) 10)
(qa (macroexpand-1 '(qact-m y)) '(+ y 1))
(qa (macroexpand '(qact-m (qact-m2 z))) '(+ (qact-m2 z) 1))
(defmacro qact-m2 (x) `(+ ,x 1))
(qa (macroexpand '(qact-m2 (qact-m2 z))) '(+ (qact-m2 z) 1))
(qa (macroexpand-1 '(car x)) '(car x))          ; non-macro: unchanged
(qa (macroexpand-1 'plain-sym) 'plain-sym)

;; == setf / incf / push / pop places ======================================
(let ((l '(1 2 3)))
  (setf (car l) 9)
  (qa l '(9 2 3)))
(let ((l '(a b)))
  (setf (cdr l) 'z)
  (qa l '(a . z)))
(let ((l '(1 x)))
  (setf (car l) 1 (cdr l) '(x))                 ; multiple place/value pairs
  (qa l '(1 x)))
(let ((v [1 2 3]))
  (setf (aref v 1) 'mid)
  (qa v [1 mid 3]))
(let ((l '(1 2 3)))
  (setf (nth 2 l) 'z)
  (qa l '(1 2 z)))
(let ((x '((a . 1))))
  (setf (cdr (car x)) 9)
  (qa x '((a . 9))))
(let ((s '()))
  (push 'a s) (push 'b s)
  (qa s '(b a)))
(let ((s '(a b c)))
  (qa (pop s) 'a)
  (qa s '(b c)))
(let ((q '(x y)))
  (qa (prog1 (pop q) q) 'x))
(let ((n 5))
  (qa (incf n) 6)
  (qa n 6)
  (qa (incf n 3) 9)
  (qa (decf n 2) 7)
  (qa n 7))
(let ((x '(1 2 3)))
  (incf (car x) 10)
  (qa x '(11 2 3)))
(let ((s '(b c)))
  (cl-pushnew 'a s)
  (cl-pushnew 'b s)                             ; already member: unchanged
  (qa s '(a b c)))
(let ((a 1) (b 2))
  (cl-psetf a b b a)                            ; parallel swap
  (qa (list a b) '(2 1)))
(let ((p '(a 1)))
  (setf (plist-get p 'a) 2)
  (qa p '(a 2)))
(let ((h (make-hash-table)))
  (setf (gethash 'k h) 'v)
  (qa (gethash 'k h) 'v)
  (setf (gethash 'k h) 1)
  (incf (gethash 'k h))
  (qa (gethash 'k h) 2))
(qa (setf (symbol-value 'qact-gv1) 3) 3)
(defvar qact-gv1 0)
(let ((qact-gv1 5))
  (setf (symbol-value 'qact-gv1) 7))
(qa qact-gv1 3)                                 ; defvar kept existing value
(set 'qact-symv nil)
(setf (symbol-value 'qact-symv) 7)
(qa (symbol-value 'qact-symv) 7)
(setf (get 'qact-prop 'k) 'pv)
(qa (get 'qact-prop 'k) 'pv)
(qa (put 'qact-zz 'k 'v) 'v)
(qa (symbol-plist 'qact-zz) '(k v))
(let ((s (make-symbol "qact-ms")))
  (setf (symbol-plist s) '(a 1 b 2))
  (qa (symbol-plist s) '(a 1 b 2)))
;; custom gv place (reader + gv-define-simple-setter)
(let ((x '(a (1 2))))
  (setf (qact-cadrcar x) 9)
  (qa x '(a (9 2))))
(let ((x '(a (1 2))))
  (incf (qact-cadrcar x) 10)
  (qa x '(a (11 2))))
(qa (fboundp 'gv-define-setter) t)
(qa (fboundp 'gv-define-expander) t)

;; == save-* wrappers ======================================================
(qa (save-excursion 77) 77)
(qa (save-current-buffer 88) 88)
(qa (save-window-excursion 99) 99)
(qa (save-restriction 33) 33)
(qa (with-temp-buffer
      (insert "abc") (goto-char 2)
      (save-excursion (goto-char 3))
      (point))
    2)                                          ; point restored on exit
(qa (with-temp-buffer
      (insert "abc") (goto-char 2)
      (list (save-excursion (goto-char 3) (point)) (point)))
    '(3 2))
(qa (with-temp-buffer
      (insert "abcde") (narrow-to-region 2 4)
      (save-restriction (widen) (list (point-min) (point-max))))
    '(1 6))                                     ; widened inside
(qa (with-temp-buffer
      (insert "abcde") (narrow-to-region 2 4)
      (save-restriction (widen))
      (list (point-min) (point-max)))
    '(2 4))                                     ; narrowing restored
(let ((outer (current-buffer))
      (tmp (generate-new-buffer " qact-tmp")))
  (unwind-protect
      (qa (list (save-current-buffer (set-buffer tmp) 'inner)
                (eq (current-buffer) outer))
          '(inner t))
    (kill-buffer tmp)))
(qa (with-temp-buffer (save-excursion (insert "ab") (point))) 3)

;; == evaluation order =====================================================
(let ((n 0))
  (qa (list (progn (setq n 1) n) (progn (setq n 2) n) (progn (setq n 3) n))
      '(1 2 3)))                                ; list args eval left-to-right
(let ((x 0))
  (qa ((lambda (a b) (list a b x)) (setq x 1) (setq x 2)) '(1 2 2)))
(let ((n 0))
  (qa (funcall #'list (setq n 1) (setq n 2)) '(1 2)))

;; == recursion / limits ===================================================
(qa (recursion-depth) 0)
(qa (boundp 'max-specpdl-size) t)
(qa (boundp 'max-lisp-eval-depth) t)
(qa (integerp max-specpdl-size) t)
(qa (integerp max-lisp-eval-depth) t)

(princ (qa--done))
