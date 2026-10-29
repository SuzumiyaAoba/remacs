;; Special forms and control flow — GNU Emacs 31.1 semantics.
;; Complements qa_ctl.el / qa_errors.el: focuses on binding-order details,
;; mutation-visibility in loops, nonlocal-exit bookkeeping, and the
;; save-*/with-* wrappers' restore contracts.
;;
;; GNU-verified 31.1 notes:
;; - `setq' on a never-bound variable in a lexical-binding file works: it
;;   sets the variable's dynamic (special) value cell — no void-variable.
;; - `condition-case' handler clauses with NO body return nil (not the
;;   bound error data).
;; - `dotimes' iterates while I < COUNT, so a fractional count floors:
;;   count 1.5 runs i = 0,1 and binds i = 1.5 in the result clause.
;; - `dolist' walks the list's cons cells: destructive setcdr mid-loop is
;;   visible to iteration; a dotted tail errors wrong-type-argument listp.
;; - `save-excursion' restores point and current-buffer but NOT the mark
;;   (mark marker is left where the body last set it).
;; - `save-restriction' restores bounds but NOT point: point is merely
;;   clamped into the restored bounds.
;; - `makunbound' inside a dynamic `let' only kills the innermost binding;
;;   the outer binding reappears when the let exits.
;; - `eval-and-compile'/`eval-when-compile' act like progn under -l
;;   interpretation: body forms evaluate and the last value is returned.
;;   (eval-and-compile inside a NON-top-level form is evaluated eagerly at
;;   load time by the loader — a void-variable aborts the whole file, so
;;   only top-level/eval uses are pinned here.)
;; - `throw' bypasses `condition-case' handlers entirely (even `t');
;;   unwind-protect cleanup forms still run.
;; - cond clause `(quote q)' as a bare clause returns q (test is the
;;   quote form); a clause whose TEST is `((quote q))' errors
;;   invalid-function since `(quote q)' is not a valid function.
;; - `with-current-buffer' accepts a buffer OBJECT or a live buffer NAME.
;; - `(interactive)' evaluated at top level returns nil.

;; -- fixtures -------------------------------------------------------------
(defvar qasp-dyn 'top)                 ; dynamic variable for binding tests
(defvar qasp-let-specs nil)            ; mutable variable list for let eval-order tests
(defvar qasp-let-form nil)              ; active let call for argument-spine tests
(defvar qasp-let-side-effect nil)      ; late let-spec validation test
(defvar qasp-late)                     ; defvar with no init: unbound, special
(defvar qasp-mk2 1)                    ; makunbound-in-let target
(defvar qasp-docv 1 "qasp-docv docs")  ; docstring -> variable-documentation
(defvar qasp-docu)                     ; unbound defvar: no doc property
(defun qasp-get-dyn () qasp-dyn)

;; == let / let* ===========================================================
(qa (let nil 5) 5)                              ; nil binding list
(qa (let (a b) (list a b)) '(nil nil))          ; bare vars bind nil
(qa (let ((a 1) b (c 3)) (list a b c)) '(1 nil 3))
(qa (let ((x 1))) nil)                          ; no body -> nil
(qa (let* ()) nil)
(let ((x 1))
  (qa (let ((x 2) (y x)) (list x y)) '(2 1)))   ; parallel: init sees outer
(let ((x 1))
  (qa (let* ((x 2) (y x)) (list x y)) '(2 2)))  ; sequential: init sees inner
(qa (let* ((v 1) (v (+ v 1)) (v (+ v 1))) v) 3) ; same-name sequential
(let ((x 1))
  (let ((x 2))
    (qa (let ((x 3)) x) 3)
    (qa x 2))
  (qa x 1))                                     ; shadowing unwinds outward
;; init forms evaluate strictly left-to-right (let and let* alike).
(let ((o nil))
  (qa (let ((a (progn (push 'ia o) 1))
            (b (progn (push 'ib o) 2)))
        (list a b (reverse o)))
      '(1 2 (ia ib))))
(let ((o nil))
  (qa (let* ((a (progn (push 'ia o) 1))
             (b (progn (push 'ib o) 2)))
        (list a b (reverse o)))
      '(1 2 (ia ib))))
;; GNU reads each binding in order as it evaluates its initializer. Changes
;; to later binding specs and the binding-list tail therefore affect let and
;; let*, while parallel `let' reads variable names after all initializers.
(let ((qasp-let-specs nil))
  (setq qasp-let-specs
        (list (list 'qasp-let-x
                    '(progn (setcar (cdr qasp-let-specs) '(qasp-let-y 9)) 1))
              '(qasp-let-y 2)))
  (qa (eval (list 'let qasp-let-specs '(list qasp-let-x qasp-let-y))) '(1 9)))
(let ((qasp-let-specs nil))
  (setq qasp-let-specs
        (list (list 'qasp-let-x
                    '(progn (setcdr qasp-let-specs nil) 1))
              '(qasp-let-y 2)))
  (qa (eval (list 'let qasp-let-specs
                  '(list qasp-let-x (boundp 'qasp-let-y))))
      '(1 nil)))
(let ((qasp-let-specs nil))
  (setq qasp-let-specs
        (list '(qasp-let-x 1)
              (list 'qasp-let-y
                    '(progn (setcar qasp-let-specs '(qasp-let-z 3)) 2))))
  (qa (eval (list 'let qasp-let-specs
                  '(list (boundp 'qasp-let-x) (boundp 'qasp-let-z) qasp-let-y)))
      '(nil t 2)))
(let ((qasp-let-specs nil) (qasp-let-side-effect nil))
  (setq qasp-let-specs
        (list (list 'qasp-let-x '(setq qasp-let-side-effect 'ran))
              '(qasp-let-y 1 2)))
  (qa (condition-case err
          (eval (list 'let qasp-let-specs nil))
        (error (list (car err) qasp-let-side-effect)))
      '(error ran)))
(let ((qasp-let-specs nil))
  (setq qasp-let-specs
        (list (list 'qasp-let-x
                    '(progn (setcar (cdr qasp-let-specs) '(qasp-let-z 9)) 1))
              '(qasp-let-y 2)))
  (qa (eval (list 'let* qasp-let-specs
                  '(list qasp-let-x (boundp 'qasp-let-y)
                         (boundp 'qasp-let-z) qasp-let-z)))
      '(1 nil t 9)))
(let ((qasp-let-specs nil))
  (setq qasp-let-specs
        (list (list 'qasp-let-x
                    '(progn (setcar (cdr qasp-let-specs) '(qasp-let-y 9)) 1))
              '(qasp-let-y 2)))
  (qa (eval (list 'let qasp-let-specs '(list qasp-let-x qasp-let-y)) t) '(1 9)))
(let ((qasp-let-specs nil))
  (setq qasp-let-specs
        (list (list 'qasp-let-x
                    '(progn (setcar (cdr qasp-let-specs) '(qasp-let-z 9)) 1))
              '(qasp-let-y 2)))
  (qa (eval (list 'let* qasp-let-specs
                  '(list qasp-let-x qasp-let-z))
             t)
      '(1 9)))
(let ((qasp-let-specs nil))
  (setq qasp-let-specs
        (list (list 'qasp-let-x '(progn (setcdr qasp-let-specs nil) 1))
              '(qasp-let-y 2)))
  (qa (eval (list 'let* qasp-let-specs
                  '(list qasp-let-x (boundp 'qasp-let-y))))
      '(1 nil)))
(let ((qasp-let-specs nil))
  (setq qasp-let-specs
        (list (list 'qasp-let-x
                    '(progn (setcar (car qasp-let-specs) 'qasp-let-z) 1))))
  (qa (eval (list 'let* qasp-let-specs
                  '(list (boundp 'qasp-let-x) (boundp 'qasp-let-z))))
      '(t nil)))
;; GNU reads the body after the initializers. Mutating the active call's
;; argument cdr therefore removes body forms before either let variant runs.
(let ((qasp-let-form nil))
  (setq qasp-let-form
        (list 'let (list (list 'qasp-let-x
                               '(progn (setcdr (cdr qasp-let-form) nil) 1))
                         'qasp-let-x)))
  (qa (eval qasp-let-form) nil))
(let ((qasp-let-form nil))
  (setq qasp-let-form
        (list 'let* (list (list 'qasp-let-x
                                '(progn (setcdr (cdr qasp-let-form) nil) 1))
                          'qasp-let-x)))
  (qa (eval qasp-let-form) nil))
;; dynamic let is visible through function calls, lexical let is not.
(qa (let ((qasp-dyn 'inner)) (qasp-get-dyn)) 'inner)
(let ((v 9))
  (qa (qasp-get-dyn) 'top))
;; let on a dynamic var then a body setq updates only the inner binding.
(defvar qasp-sdv 0)
(let ((qasp-sdv 1))
  (setq qasp-sdv 2)
  (qa qasp-sdv 2))
(qa qasp-sdv 0)
;; makunbound inside a dynamic let removes only the current binding.
(let ((qasp-mk2 2))
  (makunbound 'qasp-mk2)
  (qa (boundp 'qasp-mk2) nil))
(qa (boundp 'qasp-mk2) t)
(qa qasp-mk2 1)

;; == setq / set / setq-local ==============================================
(let ((x 0)) (qa (setq x 9) 9))                 ; single pair returns value
(let ((x 0)) (qa (list (setq x 1 x 2) x) '(2 2)))
;; value forms evaluate left-to-right.
(let ((o nil))
  (setq qasp-s1 (progn (push 'v1 o) 1)
        qasp-s2 (progn (push 'v2 o) 2))
  (qa (list qasp-s1 qasp-s2 (reverse o)) '(1 2 (v1 v2))))
;; setq on a never-bound variable in lexical code: no error, sets the
;; dynamic value cell (31.1 semantics).
(qa (setq qasp-unbound 42) 42)
(qa (boundp 'qasp-unbound) t)
(qa (symbol-value 'qasp-unbound) 42)
;; `set' on a lexical variable writes the dynamic cell, not the binding;
;; the dynamic value persists afterwards (special-variable-p stays nil).
(qa-lex (let ((x 'lex))
          (list (set 'x 'dyn) x (symbol-value 'x)))
        '(dyn lex dyn))
;; `set' inside a dynamic let hits the innermost dynamic binding.
(qa (let ((qasp-dyn 'inner))
      (set 'qasp-dyn 'setd)
      (list qasp-dyn (symbol-value 'qasp-dyn)))
    '(setd setd))
(qa qasp-dyn 'top)
;; symbol-value on a symbol bound only lexically is void-variable.
(qa-err-data (eval '(let ((qasp-lexonly 'l)) (symbol-value 'qasp-lexonly)) t)
             'void-variable '(qasp-lexonly))
;; setq-local: returns value, creates buffer-local in current buffer.
(qa (with-temp-buffer
      (list (setq-local qasp-sl 'v) (local-variable-p 'qasp-sl)))
    '(v t))
(qa (local-variable-p 'qasp-sl) nil)            ; not local in this buffer
(defvar qasp-lv 'glob)
(let ((b (generate-new-buffer " qasp-lvb")))
  (unwind-protect
      (progn
        (with-current-buffer b (setq-local qasp-lv 'loc))
        (qa (buffer-local-value 'qasp-lv b) 'loc)
        (qa qasp-lv 'glob))                     ; global/default untouched
    (kill-buffer b)))

;; == defvar / defconst ====================================================
(qa (boundp 'qasp-late) nil)
;; NOTE: under an interpreted -l load a no-init defvar does NOT mark the
;; variable special (special-variable-p => nil); an init-form defvar does.
(qa (special-variable-p 'qasp-late) nil)
(setq qasp-late 11)
(qa qasp-late 11)
;; defvar inside progn still binds only when unbound and marks special.
(qa (progn (defvar qasp-dv9 1) (defvar qasp-dv9 2) qasp-dv9) 1)
(qa (special-variable-p 'qasp-dv9) t)
;; defconst DOES clobber and stores its docstring.
(qa (progn (defconst qasp-dc9 1) (defconst qasp-dc9 2 "doc") qasp-dc9) 2)
(qa (get 'qasp-dc9 'variable-documentation) "doc")
(qa (get 'qasp-docv 'variable-documentation) "qasp-docv docs")
(qa (get 'qasp-docu 'variable-documentation) nil) ; unbound var: none stored

;; == progn / prog1 / prog2 ================================================
(let ((n 0))
  (qa (prog1 1 (setq n 5)) 1)                   ; rest still evaluated
  (qa n 5))
(let ((n 0))
  (qa (prog2 (setq n 1) 'v (setq n 2)) 'v)      ; returns SECOND form
  (qa n 2))
(qa (prog2 'first 'second 'third 'fourth) 'second)

;; == cond / if / when / unless ============================================
;; clause test eval order: later clauses never evaluated after a hit.
(let ((o nil))
  (qa (cond ((progn (push 't1 o) nil))
            ((progn (push 't2 o) 'hit) (push 'b o) 'r)
            ((progn (push 't3 o) t)))
      'r)
  (qa (reverse o) '(t1 t2 b)))
(let ((o nil))
  (qa (cond ((progn (push 'a o) t) 'v) (t 'else)) 'v)
  (qa (reverse o) '(a)))
;; `(t)` is a clause whose test is t -> returns t (test value, no body).
(qa (cond (t)) t)
(qa (cond ('x)) 'x)                             ; clause test is (quote x)
(qa (cond (0 'zero) (t 'else)) 'zero)           ; any non-nil test wins
(qa (cond ("" 'mt)) 'mt)
(qa (cond ((list) 'zz)) nil)                    ; (list) => nil -> skipped
;; clause ((quote q)): test is a call with head (quote q) — GNU signals
;; invalid-function with the cons as data.
(qa-err-data (cond (('q))) 'invalid-function '((quote q)))
;; non-list clause / non-list clause list
(qa-err-data (cond 'x) 'void-variable '(quote))
(qa-err-data (cond 3) 'wrong-type-argument '(listp 3))
;; if: extra else forms are an implicit progn, skipped when test true.
(let ((n 0))
  (qa (if t 'yes (setq n 1)) 'yes)
  (qa n 0))
(qa (if nil 'a (progn 'm 'n)) 'n)
(qa (when t 'a 'b) 'b)
(let ((n 0))
  (qa (when t (setq n 1) (setq n 2) 'last) 'last)
  (qa n 2))

;; == and / or evaluation order ============================================
(let ((o nil))
  (qa (and (progn (push 'a o) nil) (progn (push 'b o) t)) nil)
  (qa (reverse o) '(a)))                        ; b never evaluated
(let ((o nil))
  (qa (or (progn (push 'a o) nil) (progn (push 'b o) 'x)
          (progn (push 'c o) 'y))
      'x)
  (qa (reverse o) '(a b)))                      ; c never evaluated
(let ((a nil) (b 2))
  (qa (or a b 'c) 2)
  (qa (and b 'z) 'z))

;; == while / dolist / dotimes =============================================
(let ((n 0) (i 0))
  (while (< i 3) (setq i (1+ i)) (setq n (1+ n)))
  (qa (list i n) '(3 3)))
(let ((n 0)) (while nil (setq n 1)) (qa n 0))    ; body never runs
(let ((i 0))
  (while (and (< i 2) 'sym) (setq i (1+ i)))
  (qa i 2))                                     ; non-t truthy test
(let ((i 0))
  (qa (catch 'done
        (while t (setq i (1+ i)) (when (> i 2) (throw 'done i))))
      3))
(qa (dotimes (i 3)) nil)                        ; no result clause -> nil
(qa (dolist (x '(a b))) nil)
;; fractional counts: loop while i < count; i binds the count in result.
(let ((acc nil))
  (qa (dotimes (i 1.5 acc) (push i acc)) '(1 0)))
(let ((acc nil))
  (qa (dotimes (i 0.5 acc) (push i acc)) '(0)))
(let ((acc nil))
  (qa (dotimes (i 2.9 acc) (push i acc)) '(2 1 0)))
(qa-err-data (dotimes (i "x" 'r) i) 'wrong-type-argument '(number-or-marker-p "x"))
(qa-err-data (dotimes (i nil 'r) i) 'wrong-type-argument '(number-or-marker-p nil))
(let ((r nil))
  (qa (dotimes (i 4 r) (setq r i)) 3))          ; result sees side effects
;; dolist walks the cons cells: setcdr mid-loop truncates iteration.
(let ((lst '(a b c)) (seen nil))
  (qa (dolist (x lst seen)
        (push x seen)
        (when (eq x 'a) (setcdr lst nil)))
      '(a)))
;; setcar of a later cell is visible when iteration reaches it.
(let ((lst '(a b c)) (seen nil))
  (dolist (x lst)
    (push x seen)
    (when (eq x 'a) (setcar (cdr lst) 'B)))
  (qa (reverse seen) '(a B c)))
;; rebinding the list VARIABLE does not disturb the cell walk.
(let ((lst '(a b c)) (seen nil))
  (qa (dolist (x lst (list (reverse seen) lst))
        (push x seen)
        (pop lst))
      '((a b c) nil)))
(let ((lst '(a b)))
  (qa (dolist (x lst 'done) (setq lst nil)) 'done))
;; dotted tail: cdr of last cell is not a list -> wrong-type-argument.
(qa-err-data (dolist (x '(a b . c) 'done) x)
             'wrong-type-argument '(listp c))
;; each iteration rebinds the var: closures capture distinct values.
(qa-lex (let ((fns nil))
          (dolist (x '(1 2 3)) (push (lambda () x) fns))
          (mapcar #'funcall fns))
        '(3 2 1))
;; dynamic loop var: result clause sees the restored outer binding.
(defvar qasp-dvx 'outer)
(qa (dolist (qasp-dvx '(a b) qasp-dvx)) 'outer)
;; throw exits the whole loop, result clause skipped.
(let ((seen nil))
  (qa (catch 'done
        (dolist (x '(a b c) 'finished)
          (push x seen)
          (when (eq x 'b) (throw 'done 'broke))))
      'broke)
  (qa (reverse seen) '(a b)))
;; empty list still evaluates the result clause.
(let ((n 0))
  (qa (dolist (x nil (setq n 7)) (setq n 1)) 7)
  (qa n 7))

;; == catch / throw ========================================================
;; tag AND value evaluate; catch's tag arg evaluates first.
(let ((o '()))
  (qa (catch (progn (push 'tag o) 't1)
        (throw (progn (push 'thrown o) 't1) (progn (push 'val o) 'v)))
      'v)
  (qa (reverse o) '(tag thrown val)))
;; catch body evaluates all forms on normal exit; returns last.
(let ((n 0))
  (qa (catch 't (setq n 1) (setq n 2) 'end) 'end)
  (qa n 2))
;; throw through several nested catches reaches the matching tag.
(qa (catch 'o (catch 'm (catch 'i (throw 'o 'deep)))) 'deep)
(qa (catch 't (throw 't nil)) nil)
(qa (catch 't (throw 't nil) 'after) nil)       ; 'after never runs
;; throw bypasses condition-case handlers entirely — even `t'.
(qa (catch 'done
      (condition-case e (throw 'done 'tv) (t 't-caught)))
    'tv)

;; == unwind-protect =======================================================
(qa (unwind-protect 'body 'cleanup1 'cleanup2) 'body) ; cleanup value lost
;; nested unwinds run innermost cleanup first.
(let ((o nil))
  (qa (catch 't
        (unwind-protect
            (unwind-protect (throw 't 'v) (push 'i o))
          (push 'o o)))
      'v)
  (qa o '(o i)))                                ; '(o i): outer pushed last
;; cleanup runs on error and the original error re-signals.
(let ((n 0))
  (qa (condition-case e
          (unwind-protect (error "x") (setq n (1+ n)))
        (error (list 'caught n)))
      '(caught 1)))
(let ((hit nil))
  (qa (condition-case e
          (unwind-protect (signal 'arith-error nil) (setq hit t))
        (arith-error (list 'caught hit)))
      '(caught t)))

;; == condition-case =======================================================
;; handler clause with NO body returns nil, not the error object.
(qa (condition-case e (car 5) (error)) nil)
(qa (condition-case nil (car 5) (error)) nil)
(qa (condition-case e (car 5) (error 'h1 'h2)) 'h2) ; last body form
(qa (condition-case nil (car 5) (error 'handled)) 'handled)
;; first matching handler wins.
(qa (condition-case e (signal 'arith-error nil)
      (arith-error 'p1) (error 'p2))
    'p1)
;; `debug' handler is inactive in batch (debugger not entered) -> next
;; handler catches.
(qa (condition-case e (car 5) (debug 'dbg) (error 'plain)) 'plain)
;; no matching handler: error propagates to the outer case.
(qa (condition-case e
        (condition-case v (car 5) (arith-error 'nope))
      (error (car e)))
    'wrong-type-argument)
;; a handler that errors replaces the original error outward.
(qa (condition-case e
        (condition-case v (car 5) (error (error "in-handler")))
      (error (cdr e)))
    '("in-handler"))
;; :success clause as only clause on an error -> propagates.
(qa (condition-case o
        (condition-case e (car 5) (:success 's))
      (error (car o)))
    'wrong-type-argument)
;; on success with no :success, handlers never run.
(let ((n 0))
  (qa (condition-case e 'ok (error (setq n 1))) 'ok)
  (qa n 0))

;; == save-excursion / save-restriction / save-selected-window =============
;; save-excursion restores point and buffer, but NOT the mark.
(qa (with-temp-buffer
      (insert "abcd")
      (goto-char 2) (set-mark 4)
      (save-excursion (goto-char 1) (set-mark 3))
      (list (point) (mark)))
    '(2 3))
(let ((b (generate-new-buffer " qasp-seb")))
  (unwind-protect
      (progn
        (save-excursion (set-buffer b))
        (qa (buffer-name (current-buffer)) "*scratch*"))
    (kill-buffer b)))
;; save-restriction restores bounds, not point (point is clamped).
(qa (with-temp-buffer
      (insert "abcde") (narrow-to-region 2 4) (goto-char 3)
      (save-restriction (widen) (goto-char 1))
      (list (point) (point-min) (point-max)))
    '(2 2 4))
(qa (save-selected-window 'ok) 'ok)

;; == with-current-buffer / with-temp-buffer ===============================
(let ((b (generate-new-buffer " qasp-wcb")))
  (unwind-protect
      (progn
        (qa (with-current-buffer b 'obj-ok) 'obj-ok)     ; buffer object
        (qa (with-current-buffer " qasp-wcb" 'str-ok) 'str-ok)) ; by name
    (kill-buffer b)))
;; buffer-local set inside with-current-buffer persists in that buffer.
(let ((b (generate-new-buffer " qasp-pers")))
  (unwind-protect
      (progn
        (with-current-buffer b
          (setq-local qasp-pvar 'kept))
        (qa (buffer-local-value 'qasp-pvar b) 'kept))
    (kill-buffer b)))
;; with-temp-buffer returns last form and kills its buffer.
(qa (let ((n (with-temp-buffer (buffer-name))))
      (get-buffer n))
    nil)
(qa (with-temp-buffer 'a 'b 'c) 'c)

;; == eval-and-compile / eval-when-compile =================================
;; under -l interpretation both act like progn: evaluate and return last.
(qa (eval-and-compile 7) 7)
(qa (eval-and-compile 'a 'b) 'b)
(qa (eval-and-compile) nil)
(qa (eval-when-compile 'a 'b) 'b)
(qa (eval '(eval-when-compile 7)) 7)
(qa (eval '(let ((y 1)) (eval-and-compile y))) 1)

;; == defun / defmacro / interactive / declare =============================
(qa (defun qasp-f2 () 'a 'b) 'qasp-f2)
(qa (qasp-f2) 'b)                               ; implicit progn body
(defun qasp-cmd (a b) "doc" (interactive "sA: \nsB: ") (list a b))
(qa (interactive) nil)                          ; top-level interactive -> nil
(qa (interactive-form 'qasp-cmd) '(interactive "sA: \nsB: "))
(qa (commandp 'qasp-cmd) t)
(qa (documentation 'qasp-cmd) "doc")
;; code-form interactive spec
(defun qasp-cmd3 () (interactive (list 1 2)) 'r)
(qa (interactive-form 'qasp-cmd3) '(interactive (list 1 2)))
;; declare after docstring is inert at runtime
(defun qasp-fd (x) "docstring" (declare (indent 1)) x)
(qa (qasp-fd 8) 8)
(qa (documentation 'qasp-fd) "docstring")
(defmacro qasp-dm2 (x) "docdm" (declare (indent 1)) `(1+ ,x))
(qa (qasp-dm2 4) 5)
(qa (documentation 'qasp-dm2) "docdm")
;; function cells: defmacro'd function is (macro . fn); special forms subrs.
(qa (car (symbol-function 'when)) 'macro)
(qa (subrp (symbol-function 'and)) t)           ; special form = subr
(qa (macrop 'defmacro) t)
(qa (macrop 'defun) t)
(qa (special-form-p 'defmacro) nil)
(qa (special-form-p 'quote) t)
(qa (special-form-p 'function) t)
(qa (function if) 'if)
(qa #'quote 'quote)

(princ (qa--done))
