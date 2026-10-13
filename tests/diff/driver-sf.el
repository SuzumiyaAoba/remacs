;;; tests/diff/driver-sf.el --- special-form/macro differential driver -*- lexical-binding: t -*-
;;
;; Companion to driver.el: GNU sweeps subrs (funcall-able); this driver
;; sweeps special forms and macros by *evaluating* generated forms, so
;; unevaluated-argument semantics get real coverage.
;;
;; Row protocol identical to driver.el:
;;   DIF|<name>|<seq>|<ok|err|unbound>|<payload>    on stdout
;;   DMARK <name> <seq>                            on stderr (hang triage)
;;
;; For every (NAME . MACROP) in dif--targets (loaded from
;; $DIFF_DIR/targets-sf.el):
;;   - generic rows: (NAME A0), (NAME A0 A1), (NAME A0 A1 A2) built from
;;     the shared corpus — exercises macroexpansion, arity checks and
;;     argument-shape validation.  Names in dif--sf-no-generic (e.g.
;;     `while', which loops forever on a truthy test) skip this.
;;   - semantic rows: hand-written forms per name in dif--sf-cases —
;;     exercises binding/return/order/catch semantics.
;;   - buffer rows: every form is ALSO evaluated inside the fixture
;;     buffer (`dif--call-buf'-equivalent) — seq offsets by +1000.

(load (expand-file-name "common.el" (or (getenv "DIFF_DIR") ".")))

;;; ---- generic-sweep exclusions ------------------------------------------
;; `while' loops forever on any truthy TEST — semantic rows only.
;; `throw' is a subr, not swept here.
(defvar dif--sf-no-generic
  '("while" "named-let" "do-while" "loop" "comment" "track-mouse"
    "invisible"))

;;; ---- semantic cases ----------------------------------------------------
;; Hand-written deterministic forms per name.  Evaluated with `eval' in
;; the driver buffer; %b marker = run under dif--sf-buf context instead.
;; Keep the table a plain alist: (NAME . (FORM ...)).
(defvar dif--sf-cases
  '(("and" (and) (and nil (error "no")) (and t 1 2) (and 1 t "x")
      (and dif--undef (error "no")) (and nil nil nil))
    ("catch" (catch 'dif-t 1) (catch 'dif-t (throw 'dif-t 2))
      (catch 'dif-t (throw 'dif-other 3))
      (catch nil 4) (catch 'dif-t)
      (catch 'dif-t (progn 5 6)))
    ("cond" (cond) (cond (nil 1)) (cond (t 2)) (cond ((= 1 2) 3) (t 4))
      (cond (1)) (cond (nil) (2 3 4)))
    ("condition-case"
      (condition-case nil (error "boom") (error 'caught))
      (condition-case x (error "boom") (error x))
      (condition-case x (signal 'arith-error nil)
        (arith-error (list 'got (car x))))
      (condition-case x 42 (error 'no))
      (condition-case x (error "boom") (file-error 'f) (error 'e))
      (condition-case x (error "boom") (t 'any))
      (condition-case nil (error "boom"))
      (condition-case x (error "boom") ((arith-error file-error) 'ae))
      (condition-case x 7 (:success (list 's x)))
      (condition-case x (error "boom") (error 'e1) (:success 's2)))
    ("defconst" (progn (defconst dif-c1 5 "doc") dif-c1)
      (progn (defconst dif-c2 6) (defconst dif-c2 7) dif-c2))
    ("defvar" (progn (defvar dif-v1 9 "doc") dif-v1)
      (progn (defvar dif-v2) (boundp 'dif-v2))
      (progn (setq dif-v3 11) (defvar dif-v3 12) dif-v3))
    ("function" (function car) (function (lambda (x) x))
      (function nil) #'identity)
    ("if" (if t 1) (if nil 1) (if nil 1 2) (if nil 1 2 3)
      (if t) (if nil) (if (error "x") 1 2))
    ("interactive" (interactive) (interactive "sfoo: ")
      (interactive 5) (interactive (list 1 2))
      (interactive 'bogus))
    ("let" (let ((a 1) (b 2)) (+ a b)) (let (a) a)
      (let ((a 1) (a 2)) a) (let (x (y 3)) (list x y))
      (let ((dif-b 1)) (let ((dif-b 2)) dif-b))
      (let ((x 5)) (setq x 6) x)
      (let ((x 5)) (unwind-protect (throw 'dif-t x) (setq x 9)))
      (let ((i 0)) (while (< i 3) (setq i (1+ i))) i))
    ("let*" (let* ((a 1) (b a)) (list a b))
      (let* ((a 1) (a (1+ a))) a)
      (let* (a (b 2)) (list a b)))
    ("named-let" (named-let lp ((i 3)) (if (<= i 0) i (lp (1- i))))
      (named-let lp nil 8))
    ("prog1" (prog1 1 2 3) (prog1 (progn 7)))
    ("prog2" (prog2 1 2 3) (prog2 (error "x") 2))
    ("progn" (progn) (progn 1) (progn 1 2 3)
      (progn (error "b") 2))
    ("quote" 'a '(1 2) ''a '() 'nil)
    ("save-current-buffer"
      (save-current-buffer (current-buffer))
      (save-current-buffer (set-buffer "*scratch*") (buffer-name)))
    ("save-excursion" (save-excursion 1 2)
      (with-temp-buffer (insert "abc") (goto-char 2)
        (save-excursion (goto-char 3)) (point))
      (with-temp-buffer (insert "abc") (goto-char 2)
        (save-excursion (goto-char 3) (point)) (point)))
    ("save-restriction"
      (save-restriction 1)
      (with-temp-buffer (insert "abcdefghij") (goto-char 5)
        (narrow-to-region 2 8)
        (save-restriction (widen) (point-max))
        (point-max)))
    ("setq" (progn (setq dif-s1 3) dif-s1)
      (progn (setq dif-s2 1 dif-s3 2) (list dif-s2 dif-s3))
      (setq dif-s4)
      (progn (setq dif-s5 (+ 1 2)) dif-s5))
    ("setq-default" (progn (setq-default dif-sd1 4) dif-sd1)
      (progn (setq-default indent-tabs-mode t) (default-value 'indent-tabs-mode)))
    ("track-mouse" (track-mouse 1))
    ("unwind-protect"
      (progn (setq dif-u1 0) (unwind-protect 1 (setq dif-u1 2))
             dif-u1)
      (progn (setq dif-u2 0)
             (condition-case nil
                 (unwind-protect (error "x") (setq dif-u2 5))
               (error nil))
             dif-u2)
      (unwind-protect 7)
      (catch 'dif-t (unwind-protect (throw 'dif-t 1) (setq dif-u3 9))))
    ("while" (progn (setq dif-w1 0) (while (< dif-w1 4) (setq dif-w1 (1+ dif-w1))) dif-w1)
      (progn (setq dif-w2 0) (while nil (setq dif-w2 1)) dif-w2)
      (let ((i 0)) (while (progn (setq i (1+ i)) (< i 2)) i)))
    ("eval-and-compile" (eval-and-compile 3))
    ("eval-when-compile" (eval-when-compile 4) (eval-when-compile (error "c")))
    ("with-internal-interpreter" (with-internal-interpreter 2))))

;;; ---- driver ------------------------------------------------------------

(defun dif--sf-eval (name seq form &optional buf)
  "Eval FORM under condition-case; emit a DIF row."
  (message "DMARK %s %s" name seq)
  (let ((res (condition-case e
                 (if buf
                     (with-temp-buffer
                       (insert "Alpha beta gamma.\nDelta epsilon zeta.\nEta theta.\n")
                       (goto-char 9)
                       (cons 'ok (eval form)))
                   (cons 'ok (eval form)))
               (error (cons 'err e)))))
    (dif--restore-vars)
    (if (eq (car res) 'ok)
        (dif--emit name seq 'ok (dif--norm (cdr res)))
      (dif--emit name seq 'err (dif--norm-err (cdr res))))))

(defun dif--sf-form (name args)
  "Build (intern NAME . ARGS) as a data list for `eval'."
  (cons (intern name) args))

;;; ---- main loop ---------------------------------------------------------

(progn
  (setq gc-cons-threshold 800000000)
  (random "difftest-seed")
  (load (expand-file-name "targets-sf.el"
                          (or (getenv "DIFF_DIR") ".")))
  (let ((shard (getenv "DIFF_SHARD")))
    (when (and shard (string-match "\\`\\([0-9]+\\)/\\([0-9]+\\)\\'" shard))
      (let ((k (string-to-number (match-string 1 shard)))
            (n (string-to-number (match-string 2 shard)))
            (out nil) (i 0))
        (dolist (tgt dif--targets)
          (when (= (% i n) k) (push tgt out))
          (setq i (1+ i)))
        (setq dif--targets (nreverse out)))))
  (setq dif--fxbuf (get-buffer-create " dfx"))
  (with-current-buffer dif--fxbuf
    (insert "Alpha beta gamma.\nDelta epsilon zeta.\nEta theta.\n"))
  (let ((corpus (setq dif--corpus (dif--mk)))
        (_ (dif--snapshot-vars))
        (only (getenv "DIFF_ONLY"))
        (from (getenv "DIFF_FROM"))
        (auto (getenv "DIFF_AUTOSKIP"))
        (total 0))
    (when (and auto (not (equal auto "")))
      (dolist (n (split-string auto ","))
        (push n dif--skip)))
    (dolist (tgt dif--targets)
      (let ((name (car tgt))
            (seq 0))
        (when (and (or (null only) (equal only "")
                       (member name (split-string only ",")))
                   (or (null from) (equal from "") (not (string< name from)))
                   (not (dif--skip-p name)))
          (let ((sym (intern-soft name)))
            (if (or (null sym) (not (fboundp sym)))
                (progn (setq seq (1+ seq) total (1+ total))
                       (dif--emit name seq 'unbound nil))
              (progn
                ;; semantic rows first (deterministic ids 1..)
                (dolist (form (cdr (assoc name dif--sf-cases)))
                  (setq seq (1+ seq) total (1+ total))
                  (dif--sf-eval name seq form)
                  (setq seq (1+ seq) total (1+ total))
                  (dif--sf-eval name seq form t))
                ;; generic corpus-arg rows (1,2,3 args), plain + buffer ctx
                (unless (member name dif--sf-no-generic)
                  (dolist (n '(1 2 3))
                    (let ((args (dif--args n seq corpus)))
                      (setq seq (1+ seq) total (1+ total))
                      (dif--sf-eval name seq (dif--sf-form name args))
                      (setq seq (1+ seq) total (1+ total))
                      (dif--sf-eval name seq (dif--sf-form name args) t))))))))))
    (message "DIFDONE %d rows" total)
    (message "DIF|TOTAL|%d" total)))
;;; driver-sf.el ends here
