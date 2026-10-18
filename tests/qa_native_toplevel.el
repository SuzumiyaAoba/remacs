;;; -*- lexical-binding: t; -*-
;; Issue #186: native file units retain the complete ordered load program.
;; Run with native-enabled GNU Emacs 31.1 or remacs with a C compiler.
(defvar qa186-directory (make-temp-file "remacs-qa-native-toplevel-" t))
(defvar qa186-output nil)
(defvar qa186-inner nil)
(defvar qa186-trace nil)
(defvar qa186-loaded nil)
(defvar qa186-counter 0)
(defvar qa186-closure nil)
(defvar qa186-fail t)
(defun qa186-mark (value)
  (setq qa186-trace (append qa186-trace (list value))) value)
(defun qa186-compile (name body &optional dynamic)
  (let ((source (expand-file-name (concat name ".el") qa186-directory))
        (output (expand-file-name (concat name ".eln") qa186-directory)))
    (with-temp-file source
      (insert (if dynamic ";;; -*- lexical-binding: nil; -*-\n"
                ";;; -*- lexical-binding: t; -*-\n") body))
    (native-compile source output)
    ;; The unit must be self-contained, even when its source is unavailable.
    (delete-file source)
    output))

(unwind-protect
    (let ((user-emacs-directory qa186-directory)
          (native-comp-eln-load-path (list qa186-directory)))
      (unless (native-comp-available-p)
        (error "qa_native_toplevel requires native compilation"))
      ;; Exact reported case; compilation must not run the load program.
      (setq qa186-output
            (qa186-compile "reported"
             "(setq qa186-loaded 42)\n(defun qa186-file-function () 7)\n(provide 'qa186-file-feature)\n"))
      (qa (list qa186-loaded (fboundp 'qa186-file-function)
                (featurep 'qa186-file-feature)) '(nil nil nil))
      (setq qa186-loaded nil)
      (native-elisp-load qa186-output)
      (qa (list qa186-loaded (featurep 'qa186-file-feature)
                (qa186-file-function)) '(42 t 7))
      (qa (native-comp-function-p (symbol-function 'qa186-file-function)) t)
      (setq qa186-loaded nil)
      (load qa186-output nil t)
      (qa qa186-loaded 42)

      ;; Definitions become visible at their source positions, including
      ;; redefinitions and top-level progn. Repeated load reruns the program.
      (setq qa186-trace nil)
      (setq qa186-output
            (qa186-compile "ordered"
             "(qa186-mark (list 'before (fboundp 'qa186-first) (fboundp 'qa186-second)))\n(defun qa186-first () 'first)\n(qa186-mark (list 'middle (qa186-first) (fboundp 'qa186-second)))\n(progn (defun qa186-second () 'second) (qa186-mark (qa186-second)))\n(defun qa186-first () 'replaced)\n(qa186-mark (qa186-first))\n"))
      (qa qa186-trace nil)
      (native-elisp-load qa186-output)
      (qa qa186-trace '((before nil nil) (middle first nil) second replaced))
      (qa (list (qa186-first) (qa186-second)) '(replaced second))
      (qa (native-comp-function-p (symbol-function 'qa186-second)) t)
      (setq qa186-trace nil)
      (fmakunbound 'qa186-first) (fmakunbound 'qa186-second)
      (native-elisp-load qa186-output)
      (qa qa186-trace '((before nil nil) (middle first nil) second replaced))

      ;; No defun is required. Quoted data survives serialization, and lexical
      ;; closures use a fresh file environment rather than the loader's scope.
      (setq qa186-output
            (qa186-compile "initializers"
             "(setq qa186-counter (1+ qa186-counter))\n(setq qa186-data '(alpha [1 \"日本語\\ntext\"] (a . b)))\n(let ((x 7)) (setq qa186-closure (lambda () (setq x (1+ x)))))\n"))
      (qa qa186-counter 0)
      (native-elisp-load qa186-output)
      (qa qa186-counter 1)
      (qa qa186-data '(alpha [1 "日本語\ntext"] (a . b)))
      (qa (list (funcall qa186-closure) (funcall qa186-closure)) '(8 9))
      (load qa186-output nil t)
      (qa (list qa186-counter (funcall qa186-closure)) '(2 8))
      (setq qa186-output
            (qa186-compile "dynamic"
             "(let ((qa186-dynamic-x 3)) (setq qa186-closure (lambda () qa186-dynamic-x)))\n" t))
      (native-elisp-load qa186-output)
      (qa-err-data (funcall qa186-closure) 'void-variable '(qa186-dynamic-x))

      ;; Initializers run only on load; defvar preserves existing values,
      ;; while defconst reinitializes. Declarations do not leak at compile.
      (setq qa186-trace nil)
      (setq qa186-output
            (qa186-compile "variables"
             "(defvar qa186-variable (qa186-mark 'var))\n(defvar qa186-bare)\n(defconst qa186-constant (qa186-mark 'const))\n(defun qa186-read-variable () qa186-variable)\n(defun qa186-bind-variable () (let ((qa186-variable 'bound)) (qa186-read-variable)))\n"))
      (qa qa186-trace nil)
      (qa (list (boundp 'qa186-variable) (special-variable-p 'qa186-variable)
                (boundp 'qa186-constant) (special-variable-p 'qa186-constant))
          '(nil nil nil nil))
      (native-elisp-load qa186-output)
      (qa qa186-trace '(var const))
      (qa (list qa186-variable qa186-constant (boundp 'qa186-bare)) '(var const nil))
      (qa (qa186-bind-variable) 'bound)
      (qa qa186-variable 'var)
      (setq qa186-variable 'changed qa186-constant 'changed qa186-trace nil)
      (native-elisp-load qa186-output)
      (qa (list qa186-variable qa186-constant qa186-trace) '(changed const (const)))

      (setq qa186-output
            (qa186-compile "dynamic-variables"
             "(defvar qa186-dynamic-variable 5)\n(defun qa186-dynamic-reader () qa186-dynamic-variable)\n(defun qa186-dynamic-binder () (let ((qa186-dynamic-variable 9)) (qa186-dynamic-reader)))\n" t))
      (qa (boundp 'qa186-dynamic-variable) nil)
      (native-elisp-load qa186-output)
      (qa (qa186-dynamic-binder) 9)
      (qa qa186-dynamic-variable 5)

      ;; Explicit compile-time evaluation, including nested phase forms.
      (setq qa186-trace nil)
      (setq qa186-output
            (qa186-compile "phases"
             "(qa186-mark 'ordinary)\n(eval-when-compile (qa186-mark 'compile-only))\n(eval-and-compile (qa186-mark 'both))\n(setq qa186-phase-one (eval-when-compile (qa186-mark 'nested-only)))\n(setq qa186-phase-two (eval-and-compile (qa186-mark 'nested-both)))\n"))
      (qa qa186-trace '(compile-only both nested-only nested-both))
      (qa (list (boundp 'qa186-phase-one) (boundp 'qa186-phase-two)) '(nil nil))
      (setq qa186-trace nil)
      (native-elisp-load qa186-output)
      (qa qa186-trace '(ordinary both nested-both))
      (qa (list qa186-phase-one qa186-phase-two) '(nested-only nested-both))
      (setq qa186-trace nil)
      (native-elisp-load qa186-output)
      (qa qa186-trace '(ordinary both nested-both))

      ;; File-local macro definitions are available during compilation and
      ;; restored afterward. Expanded forms do not rerun expanders on load.
      (defmacro qa186-existing () 'old)
      (setq qa186-trace nil)
      (setq qa186-output
            (qa186-compile "macros"
             "(defmacro qa186-existing () ''new)\n(defmacro qa186-macro (x) (eval-when-compile (qa186-mark 'macro-body)) `(setq qa186-loaded ',x))\n(qa186-macro first)\n(defun qa186-macro-user () (qa186-macro function))\n(setq qa186-existing-result (qa186-existing))\n"))
      (qa qa186-trace '(macro-body))
      (qa (fboundp 'qa186-macro) nil)
      (qa (macroexpand '(qa186-existing)) 'old)
      (setq qa186-loaded nil)
      (native-elisp-load qa186-output)
      (qa (list qa186-loaded qa186-existing-result) '(first new))
      (qa (macrop 'qa186-macro) t)
      (qa qa186-trace '(macro-body))
      (fset 'qa186-macro (lambda (&rest _) (error "expander must not run")))
      (qa (qa186-macro-user) 'function)
      (qa qa186-trace '(macro-body))

      ;; Original errors/data propagate. Earlier definitions remain safe to
      ;; call after failed initialization; later forms are skipped, and a
      ;; retry runs from the beginning without a stale pending error.
      (setq qa186-trace nil)
      (setq qa186-output
            (qa186-compile "errors"
             "(qa186-mark 'before)\n(defun qa186-before-error () 19)\n(if qa186-fail (signal 'wrong-type-argument '(integerp bad)))\n(qa186-mark 'after)\n(defun qa186-after-error () 23)\n"))
      (qa qa186-trace nil)
      (qa-err-data (native-elisp-load qa186-output)
                   'wrong-type-argument '(integerp bad))
      (qa qa186-trace '(before))
      (qa (qa186-before-error) 19)
      (qa (fboundp 'qa186-after-error) nil)
      (qa-err-data (load qa186-output nil t)
                   'wrong-type-argument '(integerp bad))
      (qa qa186-trace '(before before))
      (qa (qa186-file-function) 7)
      (setq qa186-fail nil qa186-trace nil)
      (native-elisp-load qa186-output)
      (qa qa186-trace '(before after))
      (qa (list (qa186-before-error) (qa186-after-error)) '(19 23))

      ;; Throws cross the C initializer only after returning to Rust.
      (setq qa186-trace nil)
      (setq qa186-output
            (qa186-compile "throw"
             "(qa186-mark 'before-throw)\n(defun qa186-before-throw () 29)\n(throw 'qa186-tag 'thrown)\n(qa186-mark 'after-throw)\n"))
      (qa (catch 'qa186-tag (native-elisp-load qa186-output) 'not-thrown) 'thrown)
      (qa qa186-trace '(before-throw))
      (qa (qa186-before-throw) 29)
      (qa (qa186-file-function) 7)

      ;; A nested native load reserves its own unit; metadata on both sides
      ;; of the nested load must still point at the correct parent unit.
      (setq qa186-inner (qa186-compile "inner" "(defun qa186-inner-function () 31)\n"))
      (setq qa186-output
            (qa186-compile "outer"
             "(defun qa186-outer-before () 37)\n(native-elisp-load qa186-inner)\n(defun qa186-outer-after () 41)\n"))
      (native-elisp-load qa186-output)
      (qa (list (qa186-outer-before) (qa186-inner-function) (qa186-outer-after)) '(37 31 41))
      (qa (equal (native-comp-unit-file (subr-native-comp-unit (symbol-function 'qa186-outer-before))) qa186-output) t)
      (qa (equal (native-comp-unit-file (subr-native-comp-unit (symbol-function 'qa186-outer-after))) qa186-output) t)
      (qa (equal (native-comp-unit-file (subr-native-comp-unit (symbol-function 'qa186-inner-function))) qa186-inner) t))
  (delete-directory qa186-directory t))
(princ (qa--done)) (terpri)
