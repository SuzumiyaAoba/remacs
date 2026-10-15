;; Obarray/interning — GNU 31.1 pins:
;;   - `intern' is idempotent: same name → same symbol.  NAME must be a
;;     string: (intern 'sym)/(intern nil) → wrong-type-argument.
;;   - `intern-soft' returns the symbol or nil; a SYMBOL arg is returned
;;     as-is when interned, nil when not — an UNINTERNED symbol arg → nil.
;;   - Custom obarray: (obarray-make &optional SIZE) — make-vector does
;;     NOT produce an obarray (obarrayp → nil).  Symbols interned into a
;;     custom obarray are NOT in the default `obarray'.
;;   - `unintern' is strictly 2-arg (NAME OBARRAY) — the doc shows
;;     &optional but subr-arity is (2 . 2).  Uninterning a missing name
;;     → nil; uninterning an uninterned TWIN symbol → nil (interned twin
;;     survives); string vs symbol NAME both work.
;;   - `obarray-size' is obsolete since 30.1 ("obarrays now grow
;;     automatically") and returns the constant 4 for ANY obarray; its
;;     byte-obsolete-info is a (STRING . VER) cons.
;;   - `make-symbol' produces an uninterned symbol — full symbol cell
;;     (value/function/plist usable), not findable via intern-soft.
;;   - `gensym' → uninterned symbol named PREFIX+gensym-counter, bumping
;;     the counter; arg semantics: integer N replaces the "g" prefix
;;     ((gensym 99) with counter 0 → "990"), string P becomes the prefix.
;;   - `obarray' bound to the default obarray.
;;   - `mapatoms' iterates all symbols of an obarray; returns nil.
;;     A non-function FUNC arg → invalid-function; non-obarray OBARRAY
;;     → wrong-type-argument.

;; == intern ==
(qa (eq (intern "foo") (intern "foo")) t)
(qa (eq 'foo (intern "foo")) t)
(qa (eq (intern "car") 'car) t)
(qa (intern "a b") 'a\ b) ;; spaces are part of the name
(qa (intern "\\") '\\)   ;; backslash name → printed escaped
(qa (intern "#") '\#)
(qa-err-data (intern 5) 'wrong-type-argument '(stringp 5))
(qa-err-data (intern 'sym) 'wrong-type-argument '(stringp sym))
(qa-err-data (intern nil) 'wrong-type-argument '(stringp nil))

;; == intern-soft ==
(qa (intern-soft "definitely-not-a-symbol-xyzzy") nil)
(qa (progn (intern "qa-ob-xyzzy") (intern-soft "qa-ob-xyzzy"))
    'qa-ob-xyzzy)
(qa (intern-soft 'car) 'car)
(qa (intern-soft (make-symbol "no-such-sym")) nil)
(qa (eq (intern-soft "car") 'car) t)

;; == custom obarray via obarray-make ==
(qa (fboundp 'obarray-make) t)
(qa (subr-arity (symbol-function 'obarray-make)) '(0 . 1))
(qa (let ((ob (obarray-make 11)))
      (and (obarrayp ob)
           (progn (intern "aa" ob) t)
           (eq (intern "aa" ob) (intern-soft "aa" ob))))
    t)
;; a name interned into a custom obarray is NOT in the default one
(qa (let ((ob (obarray-make 7)))
      (intern "aa" ob)
      (intern-soft "aa"))
    nil)
;; same name in two different obarrays → different symbols
(qa (let ((ob (obarray-make 7)))
      (eq (intern "aa" ob) (intern "aa")))
    nil)
;; obarrayp: only real obarrays — not vectors, not the raw make-vector
(qa (obarrayp (obarray-make)) t)
(qa (obarrayp obarray) t)
(qa (obarrayp (make-vector 5 0)) nil) ;; make-vector is NOT an obarray!
(qa (obarrayp (make-vector 1511 0)) nil)
(qa (obarrayp [1 2 3]) nil)
(qa (obarrayp "x") nil)
(qa (obarrayp nil) nil)
;; the `obarray' variable holds the default
(qa (eq (intern-soft "car" obarray) 'car) t)

;; == obarray-size is obsolete (30.1) and always returns 4 ==
(qa (obarray-size obarray) 4)
(qa (obarray-size (obarray-make 1)) 4)
(qa (obarray-size (obarray-make 11)) 4)
(qa (obarray-size (obarray-make 1511)) 4)
;; and the doc records the obsolescence
(qa (let ((info (get 'obarray-size 'byte-obsolete-info)))
        (and (consp info) (stringp (car info))))
    t)

;; == unintern (strictly 2-arg in GNU 31.1) ==
(qa (subr-arity (symbol-function 'unintern)) '(2 . 2))
(qa-err-data (unintern 'car) 'wrong-number-of-arguments '(unintern 1))
(qa (let ((s (intern "qa-ob-tmp-123")))
      (unintern s nil))
    t) ;; nil obarray arg → default obarray
(qa (let ((s (intern "qa-ob-tmp-456")))
      (unintern s obarray)
      (intern-soft "qa-ob-tmp-456"))
    nil)
(qa (unintern "not-in-ob" (obarray-make 5)) nil)
;; uninterning an uninterned twin: nil; the interned twin survives
(qa (let ((s1 (intern "qa-dup")) (s2 (make-symbol "qa-dup")))
        (list (unintern s2 obarray)
              (intern-soft "qa-dup")
              (eq s1 s2)))
      '(nil qa-dup nil))
;; failed unintern doesn't disturb other symbols
(qa (let ((s (intern "keep-me")))
      (unintern 'other obarray)
      (eq (intern-soft "keep-me") s))
    t)
;; unintern by STRING name works too
(qa (let ((s1 (intern "str-unintern")))
      (unintern "str-unintern" obarray)
      (intern-soft "str-unintern"))
    nil)
;; NOTE: (unintern nil obarray) deletes `nil' itself — GNU crash trap,
;; not probed.

;; == make-symbol ==
(qa (let ((s (make-symbol "uninterned")))
      (list (symbolp s)
            (eq (intern-soft "uninterned") s)
            (eq s s)
            (string= (symbol-name s) "uninterned")))
    '(t nil t t))
;; uninterned symbols have full usable cells
(qa (let ((s (make-symbol "ghost")))
      (set s 42)
      (fset s (lambda () 7))
      (list (symbol-value s) (funcall s) (boundp s) (fboundp s)
            (symbol-name s) (symbol-plist s)))
    '(42 7 t t "ghost" nil))
;; symbol name may contain NUL (length counts bytes incl. the NUL)
(qa (let ((s (make-symbol "a\0b")))
      (list (symbol-name s) (length (symbol-name s))))
    '("a\0b" 3))

;; == gensym ==
;; name = PREFIX (default "g") + printed gensym-counter; counter then
;; increments by 1
(qa (let ((g (gensym)))
        (list (symbolp g)
              (eq (intern (symbol-name g)) g)
              (and (string-match "\\`g[0-9]+\\'" (symbol-name g)) t)))
    '(t nil t))
(qa (let ((g2 (gensym)) (g3 (gensym)))
      (list (eq g2 g3)
            (string= (symbol-name g2) (symbol-name g3))))
    '(nil nil)) ;; unique names each call
;; (gensym N): integer arg replaces "g" prefix; combined as N + counter
(qa (progn (setq gensym-counter 0)
           (symbol-name (gensym 99)))
    "990")
(qa (progn (setq gensym-counter 1)
           (symbol-name (gensym 0)))
    "01")
(qa (progn (setq gensym-counter 2)
           (symbol-name (gensym -5)))
    "-52")
;; (gensym "pre"): string arg becomes the prefix
(qa (progn (setq gensym-counter 3)
           (symbol-name (gensym "pre")))
    "pre3")
;; too many args → error
(qa-err (gensym "x" 5 6) 'wrong-number-of-arguments)
;; gensym-counter is a settable int; each gensym bumps it
(qa (progn (setq gensym-counter 42)
           (symbol-name (gensym)))
    "g42")
(qa (progn (setq gensym-counter 10)
           (list (symbol-name (gensym)) gensym-counter
                 (symbol-name (gensym 77)) gensym-counter))
    '("g10" 11 "7711" 12))

;; == mapatoms ==
(qa (let ((ob (obarray-make 3)))
      (intern "b1" ob) (intern "b2" ob)
      (let ((seen nil))
        (mapatoms (lambda (s) (push (symbol-name s) seen)) ob)
        (sort seen #'string<)))
    '("b1" "b2"))
;; mapatoms returns nil
(qa (let ((ob (obarray-make 3)))
      (mapatoms #'identity ob))
    nil)
;; mapatoms finds core syms in the default obarray
(qa (let ((found nil))
      (mapatoms (lambda (s) (when (eq s 'car) (setq found t))))
      found)
    t)
;; mapatoms over the default obarray yields a large count
(qa (let ((n 0)) (mapatoms (lambda (_) (setq n (1+ n)))) (> n 1000)) t)
;; mapatoms on a custom obarray: count = interned symbols
(qa (let ((ob (obarray-make 5)) (n 0))
      (intern "x1" ob) (intern "x2" ob) (intern "x3" ob)
      (mapatoms (lambda (_) (setq n (1+ n))) ob)
      n)
    3)
;; mapatoms arg-type errors: non-func → invalid-function, non-obarray →
;; wrong-type-argument
(qa-err-data (mapatoms 5 obarray) 'invalid-function '(5))
(qa-err (mapatoms #'1+ 5) 'wrong-type-argument)

;; == intern creates distinct syms across obarrays but eq within one ==
(qa (let ((ob1 (obarray-make 3)) (ob2 (obarray-make 3)))
      (list (eq (intern "n" ob1) (intern "n" ob1))
            (eq (intern "n" ob1) (intern "n" ob2))))
    '(t nil))

(princ (qa--done))
