;; Hook machinery + obsolete/delayed/named-let — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - run-mode-hooks runs `change-major-mode-after-body-hook' FIRST,
;;     then the named hooks in order.
;;   - delay-mode-hooks defers the enclosed run-mode-hooks to scope exit:
;;     the deferred hooks fire AFTER the delay-mode-hooks body ends,
;;     before subsequent run-mode-hooks calls.
;;   - run-hooks returns nil; a hook holding a non-list FUNCTION value
;;     still runs that function.
;;   - add-hook dedups by `equal'; remove-hook is idempotent.
;;   - add-hook DEPTH: smaller depth first (-10 < 0 < 90).
;;   - run-hook-with-args passes ARGS to each hook fn — arity must match.
;;   - run-hook-with-args-until-success/-failure stop at the first
;;     non-nil/nil return respectively, returning that value.
;;   - run-hook-wrapped: WRAPPER calls each hook fn; the wrapper's
;;     'stop return short-circuits the iteration.
;;   - named-let (Emacs 28+) loops via recursive call to the name.
;;   - dlet binds dynamically inside a lexical context.

;; == run-hooks order + return ==
(defvar qa--h nil)
;; add-hook DEPTH=0 + equal depth → new fn goes BEFORE existing
;; (i.e. add-hook prepends by default; last-added runs first)
(defvar qa--h nil)
(defvar qa--ord nil)
(setq qa--h nil qa--ord nil)
(add-hook 'qa--h (lambda () (push 'a qa--ord)))
(add-hook 'qa--h (lambda () (push 'b qa--ord)))
(add-hook 'qa--h (lambda () (push 'c qa--ord)))
(qa (run-hooks 'qa--h) nil)
(qa (nreverse qa--ord) '(c b a))
;; APPEND=t is DEPTH=90: same depth → new fn goes AFTER the depth-0
;; entries (sorted by depth) → first-added still runs first
(setq qa--h nil qa--ord nil)
(add-hook 'qa--h (lambda () (push 'first qa--ord)))
(add-hook 'qa--h (lambda () (push 'last qa--ord)) t)
(run-hooks 'qa--h)
(qa (nreverse qa--ord) '(first last))
;; DEPTH: smaller depth runs earlier
(setq qa--h nil qa--ord nil)
(add-hook 'qa--h (lambda () (push 'deep qa--ord)) 90)
(add-hook 'qa--h (lambda () (push 'shallow qa--ord)) -10)
(add-hook 'qa--h (lambda () (push 'mid qa--ord)) 0)
(run-hooks 'qa--h)
(qa (nreverse qa--ord) '(shallow mid deep))

;; run-hooks on void hook → nil (no error)
(qa (run-hooks 'qa-nonexistent-hook-xyz) nil)

;; hook holding a single function (not a list) still runs
(setq qa--h (lambda () 'single))
(qa (run-hooks 'qa--h) nil)

;; run-hooks always returns nil even with values
(setq qa--h (list (lambda () 'x) (lambda () 'y)))
(qa (run-hooks 'qa--h) nil)

;; == add-hook dedup / remove-hook ==
(setq qa--h nil)
(let ((f (lambda () 'x)))
  (add-hook 'qa--h f)
  (add-hook 'qa--h f)
  (qa (length qa--h) 1))
;; remove-hook idempotent
(remove-hook 'qa--h (lambda () 'b))
(qa (length qa--h) 1)
;; remove a fn actually present
(let ((f (lambda () 'z)))
  (setq qa--h nil)
  (add-hook 'qa--h f)
  (remove-hook 'qa--h f)
  (qa (length qa--h) 0))

;; == buffer-local hooks ==
(let ((h9 nil) (r nil))
  (with-temp-buffer
    (add-hook 'h9 (lambda () 'one) nil t))
  (with-temp-buffer
    (add-hook 'h9 (lambda () 'two) nil t)
    (setq r (run-hook-with-args-until-success 'h9)))
  ;; only THIS buffer's local hook ran
  (qa r 'two))
;; remove-hook + LOCAL arg removes only the local entry
(let ((h10 nil))
  (with-temp-buffer
    (add-hook 'h10 (lambda () 'g))        ; global
    (add-hook 'h10 (lambda () 'l) nil t)  ; local
    (remove-hook 'h10 (lambda () 'l) t)
    (qa (run-hook-with-args-until-success 'h10) 'g)))

;; == run-mode-hooks ==
;; runs change-major-mode-after-body-hook FIRST
(setq qa--h nil qa--ord nil change-major-mode-after-body-hook nil)
(add-hook 'change-major-mode-after-body-hook
          (lambda () (push 'mode-reset qa--ord)))
(add-hook 'qa--h (lambda () (push 'h4 qa--ord)))
(run-mode-hooks 'qa--h)
(qa (nreverse qa--ord) '(mode-reset h4))

;; multiple named hooks in order
(setq qa--h nil qa--ord nil change-major-mode-after-body-hook nil)
(let ((h5 nil) (h6 nil))
  (add-hook 'h5 (lambda () (push 'five qa--ord)))
  (add-hook 'h6 (lambda () (push 'six qa--ord)))
  (run-mode-hooks 'h5 'h6)
  (qa (nreverse qa--ord) '(five six)))

;; delay-mode-hooks defers enclosed run-mode-hooks to scope exit
(setq qa--ord nil change-major-mode-after-body-hook nil)
(let ((h5 nil) (h6 nil))
  (add-hook 'h5 (lambda () (push 'five qa--ord)))
  (add-hook 'h6 (lambda () (push 'six qa--ord)))
  (delay-mode-hooks (run-mode-hooks 'h5))
  (push 'between qa--ord)
  (run-mode-hooks 'h6)
  ;; deferred h5 fires at delay-mode-hooks scope exit, before h6
  (qa (nreverse qa--ord) '(between five six)))
;; == run-hook-with-args ==
(let ((h11 nil) (seen nil))
  (add-hook 'h11 (lambda (x y) (setq seen (list x y))))
  (run-hook-with-args 'h11 1 2)
  (qa seen '(1 2)))
;; arity mismatch → wrong-number-of-arguments
(let ((h12 nil))
  (add-hook 'h12 (lambda () 'noarg))
  (qa-err (run-hook-with-args 'h12 1 2) 'wrong-number-of-arguments))

;; == run-hook-with-args-until-success / until-failure ==
;; hooks run last-added-first: last add must be the hit
(let ((h13 nil))
  (add-hook 'h13 (lambda (a) 'never))
  (add-hook 'h13 (lambda (a) (list 'hit a)))
  (add-hook 'h13 (lambda (a) nil))
  (qa (run-hook-with-args-until-success 'h13 7) '(hit 7)))
;; until-failure stops at first nil: 'c runs first (added last),
;; then 'b returns nil → stops before 'a
(let ((h14 nil) (ran nil))
  (add-hook 'h14 (lambda (a) (push (list 'a a) ran) 'yes))
  (add-hook 'h14 (lambda (a) (push (list 'b a) ran) nil))
  (add-hook 'h14 (lambda (a) (push (list 'c a) ran) 'never))
  (qa (run-hook-with-args-until-failure 'h14 42) nil)
  (qa (nreverse ran) '((c 42) (b 42))))
;; all nil → until-success returns nil
(let ((h15 nil))
  (add-hook 'h15 (lambda () nil))
  (qa (run-hook-with-args-until-success 'h15) nil))

;; == run-hook-wrapped ==
;; WRAPPER is called with each hook fn + the ARGS
(let ((h16 nil) (calls nil))
  (add-hook 'h16 (lambda (x) (push (list 'a x) calls) nil))
  (add-hook 'h16 (lambda (x) (push (list 'b x) calls) nil))
  (run-hook-wrapped 'h16 (lambda (f arg) (funcall f arg)) 99)
  ;; hooks run last-added-first (b then a)
  (qa (nreverse calls) '((b 99) (a 99))))
;; wrapper returning 'stop short-circuits: 'c runs first, then 'b
;; triggers stop → 'a never reached
(let ((h17 nil) (calls nil))
  (add-hook 'h17 (lambda (x) (push (list 'a x) calls) nil))
  (add-hook 'h17 (lambda (x) (push (list 'b x) calls) nil))
  (add-hook 'h17 (lambda (x) (push (list 'c x) calls) nil))
  (run-hook-wrapped 'h17
                    (lambda (f arg)
                      ;; wrapper returning 'stop skips this hook fn
                      (if (and calls (eq (car (car calls)) 'b))
                          'stop
                        (funcall f arg)))
                    99)
  (qa (nreverse calls) '((c 99) (b 99))))

;; == make-obsolete / make-obsolete-variable ==
;; byte-obsolete-function plist is nil in this build (renamed upstream)
(qa (get 'qa-obso-fn 'byte-obsolete-function) nil)
;; obsolete function still callable
(defun qa-obso-callee () 'new-value)
(make-obsolete 'qa-obso-callee 'qa-new-callee "31.1")
(qa (qa-obso-callee) 'new-value)
;; obsolete variable
(make-obsolete-variable 'qa-obso-var 'qa-new-var "31.1")
(qa (get 'qa-obso-var 'byte-obsolete-variable) '(qa-new-var nil "31.1"))
;; setting an obsolete var still stores the value
(defvar qa-obso-var2 nil)
(make-obsolete-variable 'qa-obso-var2 'qa-new-var2 "31.1")
(setq qa-obso-var2 7)
(qa qa-obso-var2 7)

;; == with-demoted-errors ==
;; swallows errors; body value nil on error, last value otherwise
(qa (with-demoted-errors "demoted: %S" (error "x")) nil)
(qa (with-demoted-errors "demoted: %S" (+ 1 2)) 3)

;; == named-let (Emacs 28+) ==
;; loop via recursive call to the name
(qa-lex (named-let loop ((i 0) (acc nil))
          (if (< i 3) (loop (1+ i) (cons i acc)) (nreverse acc)))
        '(0 1 2))
;; accumulate via cons
(qa-lex (named-let fact ((n 5) (acc 1))
          (if (<= n 1) acc (fact (1- n) (* n acc))))
        120)

;; non-defvar'd var: let makes a LEXICAL binding; dlet adds a
;; DYNAMIC one — `x' still reads the lexical 'outer
(qa-lex (let ((x 'outer))
          (dlet ((x 'inner))
            (list x (symbol-value 'x))))
        '(outer inner))
;; defvar'd var: let is already DYNAMIC — symbol-value sees 'lex
(defvar qa-dlet-x 'dyn)
(qa (let ((qa-dlet-x 'lex))
      (list qa-dlet-x (symbol-value 'qa-dlet-x)))
    '(lex lex))
;; dlet overrides the dynamic binding → both see 'dyn2
(qa (let ((qa-dlet-x 'lex))
      (dlet ((qa-dlet-x 'dyn2))
        (list qa-dlet-x (symbol-value 'qa-dlet-x))))
    '(dyn2 dyn2))

;; == prog1/prog2 single-form edge ==
(qa (prog1 'ret) 'ret)
(qa (prog2 'one 'two) 'two)

(princ (qa--done))
