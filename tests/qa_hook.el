;; Hook machinery — GNU 31.1 -Q --batch ground truth.
;;
;; GNU truths pinned:
;;   - add-hook signature is (HOOK FUNCTION &optional DEPTH LOCAL);
;;     DEPTH is a number (nil => 0, t => 90 for append).  Same depth:
;;     last-added runs FIRST (prepend).  add-hook dedups by `equal'.
;;   - add-hook on a void variable silently gives it a global value.
;;   - run-hooks returns nil; a hook holding a bare FUNCTION (non-list)
;;     still runs it; an error inside a hook fn aborts the rest.
;;   - run-hooks on a non-symbol or a VOID symbol signals
;;     wrong-type-argument (NOT void-variable).
;;   - run-hook-wrapped's WRAPPER must funcall the hook fn itself;
;;     returning 'stop short-circuits the iteration.
;;   - LOCAL=t in add-hook buffer-localizes the hook; the buffer-local
;;     list gets a trailing t marker and the GLOBAL value keeps the fn.
;;   - hook-variable-p does NOT exist in GNU 31.1.
;;   - setq inside a buffer on a non-local var just sets the global,
;;     so default-value tracks it.

;; == surface ==
(qa (fboundp 'add-hook) t)
(qa (fboundp 'remove-hook) t)
(qa (fboundp 'run-hooks) t)
(qa (fboundp 'run-hook-with-args) t)
(qa (fboundp 'run-hook-with-args-until-success) t)
(qa (fboundp 'run-hook-with-args-until-failure) t)
(qa (fboundp 'run-hook-wrapped) t)
(qa (fboundp 'run-mode-hooks) t)
(qa (fboundp 'delay-mode-hooks) t)
;; hook-variable-p was never in GNU Emacs (old name only)
(qa (fboundp 'hook-variable-p) nil)
(qa (boundp 'delay-mode-hooks) t)

;; == add-hook: prepends by default ==
(defvar qahk-a nil)
(defvar qahk-ord nil)
(setq qahk-a nil qahk-ord nil)
(add-hook 'qahk-a (lambda () (push 'first qahk-ord)))
(add-hook 'qahk-a (lambda () (push 'second qahk-ord)))
(qa (run-hooks 'qahk-a) nil)
(qa (nreverse qahk-ord) '(second first))

;; == add-hook dedups by equal ==
(setq qahk-a nil)
(let ((f (lambda () 'x)))
  (add-hook 'qahk-a f)
  (add-hook 'qahk-a f)
  (qa (length qahk-a) 1))

;; == APPEND=t (depth 90) puts it last ==
(setq qahk-a nil qahk-ord nil)
(add-hook 'qahk-a (lambda () (push 'a qahk-ord)))
(add-hook 'qahk-a (lambda () (push 'b qahk-ord)) t)
(run-hooks 'qahk-a)
(qa (nreverse qahk-ord) '(a b))

;; == numeric DEPTH sorts ascending (-10 < 0 < 90) ==
(setq qahk-a nil qahk-ord nil)
(add-hook 'qahk-a (lambda () (push 'mid qahk-ord)))
(add-hook 'qahk-a (lambda () (push 'early qahk-ord)) -10)
(add-hook 'qahk-a (lambda () (push 'late qahk-ord)) 90)
(run-hooks 'qahk-a)
(qa (nreverse qahk-ord) '(early mid late))

;; == remove-hook ==
(setq qahk-a nil)
(let ((f (lambda () 'x)))
  (add-hook 'qahk-a f)
  (remove-hook 'qahk-a f)
  (qa qahk-a nil))
;; removing a non-member is a noop
(setq qahk-a nil)
(add-hook 'qahk-a #'car)
(remove-hook 'qahk-a #'cdr)
(qa qahk-a '(car))

;; == run-hooks on several symbols runs all, returns nil ==
(defvar qahk-m1 nil)
(defvar qahk-m2 nil)
(setq qahk-m1 nil qahk-m2 nil qahk-ord nil)
(add-hook 'qahk-m1 (lambda () (push 'm1 qahk-ord)))
(add-hook 'qahk-m2 (lambda () (push 'm2 qahk-ord)))
(qa (run-hooks 'qahk-m1 'qahk-m2) nil)
(qa (nreverse qahk-ord) '(m1 m2))

;; == hook holding a bare function (non-list) still runs ==
(defvar qahk-bare nil)
(setq qahk-bare nil)
(defvar qahk-ran nil)
(setq qahk-ran nil qahk-bare (lambda () (setq qahk-ran t)))
(qa (progn (run-hooks 'qahk-bare) qahk-ran) t)

;; == error inside a hook fn aborts remaining fns; error propagates ==
(setq qahk-bare nil qahk-ord nil)
(add-hook 'qahk-bare (lambda () (push 'never qahk-ord)))
(add-hook 'qahk-bare (lambda () (error "boom")))
(add-hook 'qahk-bare (lambda () (push 'runs qahk-ord)))
(qa-err (run-hooks 'qahk-bare) 'error)
;; list order is (runs-fn err-fn never-fn): only `runs' pushed before abort
(qa (nreverse qahk-ord) '(runs))

;; == run-hooks arg/type errors ==
(qa-err (run-hooks 5) 'wrong-type-argument)
;; GNU run-hooks on a VOID hook var is a no-op, returns nil
(qa (run-hooks 'qahk-void-zz) nil)
(qa-err (add-hook 5 #'car) 'wrong-type-argument)

;; == add-hook on a void variable creates the binding ==
(makunbound 'qahk-fresh)
(add-hook 'qahk-fresh #'car)
(qa (symbol-value 'qahk-fresh) '(car))
(makunbound 'qahk-fresh)

;; == run-hook-with-args: args passed positionally; returns nil ==
(defvar qahk-wa nil)
(setq qahk-wa nil qahk-ord nil)
(add-hook 'qahk-wa (lambda (a b) (push (list 'got a b) qahk-ord)))
(qa (run-hook-with-args 'qahk-wa 1 2) nil)
(qa (nreverse qahk-ord) '((got 1 2)))

;; == run-hook-with-args-until-success: first non-nil wins ==
(defvar qahk-us nil)
(setq qahk-us nil qahk-ord nil)
(add-hook 'qahk-us (lambda (x) (push 1 qahk-ord) nil))
(add-hook 'qahk-us (lambda (x) (push 2 qahk-ord) 'WIN))
(add-hook 'qahk-us (lambda (x) (push 3 qahk-ord) 'unreached))
;; add-hook prepends → order (f3 f2 f1): f3 runs first and wins
(qa (run-hook-with-args-until-success 'qahk-us 'z) 'unreached)
(qa (nreverse qahk-ord) '(3))

;; == run-hook-with-args-until-failure: first nil wins, returns nil ==
(defvar qahk-uf nil)
(setq qahk-uf nil qahk-ord nil)
(add-hook 'qahk-uf (lambda () (push 1 qahk-ord) t))
(add-hook 'qahk-uf (lambda () (push 2 qahk-ord) nil))
(add-hook 'qahk-uf (lambda () (push 3 qahk-ord) t))
(qa (run-hook-with-args-until-failure 'qahk-uf) nil)
;; order (f3 f2 f1): f3 returns t, f2 returns nil → stop
(qa (nreverse qahk-ord) '(3 2))

;; == run-hook-wrapped: wrapper funcalls each fn; 'stop short-circuits ==
(defvar qahk-wr nil)
(setq qahk-wr nil qahk-ord nil)
(add-hook 'qahk-wr (lambda (a) (push (list 'f a) qahk-ord)))
(qa (let ((calls nil))
      (run-hook-wrapped 'qahk-wr
                        (lambda (fn a)
                          (push (list 'wrap a) calls)
                          (funcall fn a))
                        'ARG)
      (nreverse calls))
    '((wrap ARG)))
;; side effects inside the hook fn ran too
(qa (nreverse qahk-ord) '((f ARG)))
;; 'stop short-circuits AND is the return value; wrapper ran once
;; (list order is (cdr car), so the wrapper saw cdr first)
(setq qahk-wr nil)
(add-hook 'qahk-wr #'car)
(add-hook 'qahk-wr #'cdr)
(qa (let ((seen nil))
      (list (run-hook-wrapped 'qahk-wr
                              (lambda (fn) (push fn seen) 'stop))
            (nreverse seen)))
    '(stop (cdr)))
;; ANY non-nil wrapper result short-circuits and propagates; the rest
;; of the hook list is skipped
(setq qahk-wr nil)
(add-hook 'qahk-wr #'car)
(add-hook 'qahk-wr #'cdr)
(qa (let ((seen nil))
      (list (run-hook-wrapped 'qahk-wr
                              (lambda (fn) (push fn seen) 'keep))
            (nreverse seen)))
    '(keep (cdr)))
;; wrapper returning nil: still visits every fn, returns nil
(qa (let ((seen nil))
      (list (run-hook-wrapped 'qahk-wr
                              (lambda (fn) (push fn seen) nil))
            (nreverse seen)))
    '(nil (cdr car)))

;; == buffer-local hooks ==
(defvar qahk-loc nil)
(qa (with-temp-buffer
      (setq qahk-loc nil)
      (add-hook 'qahk-loc (lambda () 'l) nil t)
      (list (local-variable-p 'qahk-loc)
            ;; trailing t marks "run global fns too"
            (eq (car (last qahk-loc)) t)
            (functionp (car qahk-loc))))
    '(t t t))
;; global list keeps its own copy; the local list has the marker
(setq qahk-loc nil)
(qa (with-temp-buffer
      (add-hook 'qahk-loc #'car nil t)
      (list (local-variable-p 'qahk-loc)
            qahk-loc
            (default-value 'qahk-loc)))
    '(t (car t) nil))
;; LOCAL=t only adds t marker when hook already had global fn? GNU:
;; adding to a local hook makes the var buffer-local; the t marker is
;; appended so global functions still run.
;; remacs: doesn't maintain newest-first ordering on local hook
(setq qahk-loc nil)
(qa (with-temp-buffer
      (add-hook 'qahk-loc #'car nil t)
      (add-hook 'qahk-loc #'cdr nil t)
      qahk-loc)
    '(cdr car t))

;; == remove-hook with LOCAL=t removes only the local entry ==
(setq qahk-loc nil)
(qa (with-temp-buffer
      (add-hook 'qahk-loc #'car nil t)
      (add-hook 'qahk-loc #'cdr nil t)
      (remove-hook 'qahk-loc #'car t)
      qahk-loc)
    '(cdr t))

;; == default-value sees the global value while local is in effect ==
(defvar qahk-dv 'glob)
(qa (list (default-value 'qahk-dv) qahk-dv (local-variable-p 'qahk-dv))
    '(glob glob nil))
(qa (with-temp-buffer
      ;; plain setq on a non-local var sets the DEFAULT value
      (setq qahk-dv 'changed)
      (list qahk-dv (default-value 'qahk-dv) (local-variable-p 'qahk-dv)))
    '(changed changed nil))
(setq qahk-dv 'glob)

;; == setq-local on hook var ==
(qa (with-temp-buffer
      (setq-local qahk-dv '(a b))
      (list (local-variable-p 'qahk-dv) qahk-dv (default-value 'qahk-dv)))
    '(t (a b) glob))

;; == add-hook to nil hook then run is still nil ==
(defvar qahk-nil nil)
(setq qahk-nil nil)
(qa (run-hooks 'qahk-nil) nil)
(qa (symbol-value 'qahk-nil) nil)

;; == add-hook returns the new hook value (the list) ==
;; remacs: returns nil
(qa (add-hook 'qahk-nil #'ignore) '(ignore))
(setq qahk-nil nil)

;; == mode-hooks: run-mode-hooks runs change-major-mode-after-body-hook ==
(defvar qahk-rmh nil)
(setq qahk-rmh nil qahk-ord nil)
(add-hook 'qahk-rmh (lambda () (push 'named qahk-ord)))
(let ((change-major-mode-after-body-hook
       (list (lambda () (push 'after-body qahk-ord)))))
  (qa (run-mode-hooks 'qahk-rmh) nil)
  (qa (nreverse qahk-ord) '(after-body named)))

;; == delay-mode-hooks defers run-mode-hooks ==
(defvar qahk-dmh nil)
(setq qahk-dmh nil qahk-ord nil)
(add-hook 'qahk-dmh (lambda () (push 'inner qahk-ord)))
(delay-mode-hooks (run-mode-hooks 'qahk-dmh))
;; deferred hooks do NOT fire inside delay-mode-hooks; the hook is
;; queued on `delayed-mode-hooks' and runs at the next run-mode-hooks
(qa qahk-ord nil)
(qa delayed-mode-hooks '(qahk-dmh))
(run-mode-hooks 'qahk-nil)
(qa (nreverse qahk-ord) '(inner))

;; == run-hooks on a hook holding a list containing a non-function ==
(setq qahk-nil '(42))
(qa-err (run-hooks 'qahk-nil) 'invalid-function)
(setq qahk-nil nil)

(princ (qa--done))
