;; load/require/provide/autoload — GNU 31.1 pins:
;; * `featurep'/`require' consult the C-level features vector, NOT the
;;   dynamic `features' binding: a let-shadowed `features' is ignored.
;; * `provide' returns the feature symbol; `(provide nil)' legitimately
;;   adds nil to features (featurep nil => t, require nil => nil).
;; * `featurep' SUBFEATURE consults the feature's `subfeatures' plist.
;; * `autoload' returns nil when FUNCTION is already fbound, else the
;;   symbol; FILE must be a string (nil/5 => wrong-type-argument).
;; * `autoloadp' is t for ANY cons starting with `autoload' (even
;;   `(autoload)') — it never follows symbols.
;; * `macrop' on a `macro' autoload is (macro t); `commandp' honors the
;;   INTERACTIVE field of an autoload cell.
;; * `load' NOSUFFIX means "name literally"; MUST-SUFFIX is an
;;   exists-predicate: a bare name still loads when name.el exists, and
;;   even a suffixless file satisfies it.
;; * `load' prefers .eln > .elc > .el > bare; `load-prefer-newer' picks
;;   the newer of .el/.elc.
;; * `require' NOERROR only suppresses file-missing; a file that loads
;;   but fails to provide still signals plain `error' with message
;;   "Loading file FILE failed to provide feature ‘FEATURE’".
;; * `eval-after-load' keys: strings become file regexps fired after
;;   the load unwinds (load-file-name already restored); symbol keys
;;   fire inside `provide' (load-file-name = providing file) OR
;;   immediately if the feature is already provided.  String keys also
;;   fire immediately when a matching file is in load-history.
;; * Identical eval-after-load forms deduplicate (fires once); distinct
;;   forms for one file run oldest-registration-first.
;; * `load-history' is newest-first, (FILE . ENTRIES); entries record
;;   defun/defvar/require/provide; reloading replaces, not appends;
;;   `unload-feature' leaves the entry; a file that errors mid-load
;;   leaves NO entry even if it provided a feature first.
;; * `unload-feature' on a bare `(provide ...)' (no file) dies inside
;;   file-dependents with wrong-type-argument (stringp nil); on a
;;   never-loaded feature signals error "X is not a currently loaded
;;   feature"; on cl-lib signals error listing dependent libraries.
;; * `load-read-function' is called WITH the stream arg; its signals
;;   (incl. end-of-file) propagate; a non-function value gives
;;   invalid-function.
;; * `eval-when-compile' DOES evaluate its body under interpreted load
;;   (internal-macroexpand-for-load); `eval-and-compile' also evals.
;; * `load' on a directory signals file-error "Is a directory".
;; * .el.gz files load transparently via load-file-rep-suffixes.

;; == API surface ==
(qa (fboundp 'load) t)
(qa (fboundp 'load-file) t)
(qa (fboundp 'require) t)
(qa (fboundp 'provide) t)
(qa (fboundp 'featurep) t)
(qa (fboundp 'autoload) t)
(qa (fboundp 'autoloadp) t)
(qa (fboundp 'autoload-do-load) t)
(qa (fboundp 'unload-feature) t)
(qa (fboundp 'locate-library) t)
;; find-library-name lives in find-func.el: unbound in -Q batch
(qa (fboundp 'find-library-name) nil)
(qa (fboundp 'eval-after-load) t)
(qa (boundp 'load-path) t)
(qa (boundp 'load-history) t)
(qa (boundp 'features) t)
(qa (boundp 'load-suffixes) t)
(qa (boundp 'load-file-rep-suffixes) t)
(qa (boundp 'load-prefer-newer) t)
(qa (boundp 'load-in-progress) t)
(qa (boundp 'load-file-name) t)
(qa (boundp 'load-true-file-name) t)
(qa (boundp 'load-read-function) t)
(qa (boundp 'current-load-list) t)
(qa (boundp 'after-load-alist) t)
(qa (boundp 'after-load-functions) t)
(qa (boundp 'load-force-doc-strings) t)
(qa (boundp 'byte-compile-current-file) nil)

;; == load-path / suffix variables ==
(qa (listp load-path) t)
(let ((qal-ok t))
  (dolist (d load-path)
    (unless (stringp d) (setq qal-ok nil)))
  (qa qal-ok t))
(qa (member ".el" load-suffixes) '(".el"))
(qa (member ".elc" load-suffixes) '(".elc" ".el"))
(qa (member "" load-file-rep-suffixes) '("" ".gz"))
(qa load-prefer-newer nil)
(qa load-force-doc-strings nil)
;; load-read-function's default binding is `read' (while this file loads)
(qa load-read-function 'read)

;; == provide / featurep / features ==
(qa (featurep 'qa-load-nonexistent-xyz) nil)
(qa (provide 'qa-lp-feat) 'qa-lp-feat)
(qa (featurep 'qa-lp-feat) t)
(qa (and (memq 'qa-lp-feat features) t) t)
;; providing twice keeps a single entry
(provide 'qa-lp-feat)
(let ((n 0))
  (dolist (f features) (when (eq f 'qa-lp-feat) (setq n (1+ n))))
  (qa n 1))
;; newest provided feature becomes (car features)
(provide 'qa-lp-a)(provide 'qa-lp-b)
(qa (car features) 'qa-lp-b)
;; nil is a legitimate feature name
(qa (provide nil) nil)
(qa (featurep nil) t)
(qa (require nil) nil)
(setq features (delq nil features))
(qa (featurep nil) nil)
;; argument type errors
(qa-err (provide "x") 'wrong-type-argument)
(qa-err (provide 'qa-lp-bad 5) 'wrong-type-argument)
(qa-err (featurep 5) 'wrong-type-argument)
(qa-err (featurep "x") 'wrong-type-argument)
;; subfeatures: recorded on the feature's plist, checked by featurep
(provide 'qa-lp-sub '(sfa sfb))
(qa (featurep 'qa-lp-sub 'sfa) t)
(qa (featurep 'qa-lp-sub 'sfc) nil)
(qa (featurep 'qa-lp-sub nil) t)
(qa (get 'qa-lp-sub 'subfeatures) '(sfa sfb))
;; 2-arg featurep on an unloaded feature is nil
(qa (featurep 'qa-lp-missing 'any) nil)
;; `features' is NOT a special variable in 31.1: a lexical `let' does
;; not shadow what `featurep'/`require' consult (pin via `(eval … t)'
;; — a bare `let' in this dynamic-scope file DOES shadow, per GNU).
(qa (eval '(let ((features '(shadowed))) (featurep 'shadowed)) t) nil)
(qa (eval '(let ((features '(shadowed))) (featurep 'seq)) t) t)
(qa (eval '(let ((features (cons 'qa-lp-fake features)))
             (featurep 'qa-lp-fake))
          t)
    nil)
(qa-err (eval '(let ((features (cons 'qa-lp-fake features)))
                 (require 'qa-lp-fake))
              t)
        'file-missing)
;; ...but a DYNAMIC binding does shadow (the Gnus cycle-breaker trick).
(qa (dlet ((features '(qa-dlet-fake))) (featurep 'qa-dlet-fake)) t)
(qa (dlet ((features '(qa-dlet-fake))) (require 'qa-dlet-fake)) 'qa-dlet-fake)
(qa (dlet ((features nil)) (featurep 'seq)) nil)

;; == require ==
(qa (require 'seq) 'seq)
(qa (require 'seq) 'seq)                  ; idempotent
(qa-err-data (require 'qa-no-such-feat-xyz) 'file-missing
             '("Cannot open load file" "No such file or directory"
               "qa-no-such-feat-xyz"))
(qa-err-data (require 'qa-no-such2 "/tmp/qa-load-missing-xyz") 'file-missing
             '("Cannot open load file" "No such file or directory"
               "/tmp/qa-load-missing-xyz"))
;; NOERROR suppresses only file-missing
(qa (require 'qa-definitely-missing-xyz nil t) nil)
(qa (require 'qa-definitely-missing2 "/tmp/qa-load-missing2" t) nil)
;; a loaded file that fails to provide the requested feature -> error.
;; GNU's message embeds the RESOLVED file name.
(with-temp-file "/tmp/qa_load_np.el" (insert "(setq qa-load-np t)\n"))
(qa-err-data (require 'qa-wanted "/tmp/qa_load_np") 'error
                '("Loading file /tmp/qa_load_np.el failed to provide feature ‘qa-wanted’"))
(qa (boundp 'qa-load-np) t)               ; the file did evaluate
;; NOERROR does not swallow the failed-to-provide error
(qa-err-data (require 'qa-wanted "/tmp/qa_load_np" t) 'error
                '("Loading file /tmp/qa_load_np.el failed to provide feature ‘qa-wanted’"))
;; filename override: correct feature -> returns feature symbol
(with-temp-file "/tmp/qa_load_ovr.el" (insert "(provide 'qa-override)\n"))
(qa (require 'qa-override "/tmp/qa_load_ovr") 'qa-override)
(qa (featurep 'qa-override) t)
;; file providing a different feature: error, but feature stays provided
(with-temp-file "/tmp/qa_load_ovr2.el" (insert "(provide 'qa-wrong-feat)\n"))
(qa-err-data (require 'qa-wanted-ovr "/tmp/qa_load_ovr2") 'error
                '("Loading file /tmp/qa_load_ovr2.el failed to provide feature ‘qa-wanted-ovr’"))
(qa (featurep 'qa-wrong-feat) t)
;; non-symbol feature arg
(qa-err (require 5) 'wrong-type-argument)
(qa-err (require "x") 'wrong-type-argument)

;; == load: basics ==
(with-temp-file "/tmp/qa_load_b.el"
  (insert "(setq qa-lb 42)\n(setq qa-lbn load-file-name)\n"))
(qa (load "/tmp/qa_load_b.el" nil t) t)
(qa qa-lb 42)
(qa qa-lbn "/tmp/qa_load_b.el")
(qa (load "/tmp/qa_load_b" nil t) t)      ; .el appended automatically
(setq qa-lb 0)
(qa (load "/tmp/qa_load_b.el" nil t) t)   ; reload re-evaluates
(qa qa-lb 42)
;; missing file
(qa-err-data (load "/tmp/qa_load_no_such_xyz") 'file-missing
             '("Cannot open load file" "No such file or directory"
               "/tmp/qa_load_no_such_xyz"))
(qa (load "/tmp/qa_load_no_such_xyz" t t) nil)
;; empty file name
(qa-err-data (load "" nil t) 'file-missing
             '("Cannot open load file" "No such file or directory" ""))
;; non-string FILE
(qa-err-data (load 5) 'wrong-type-argument '(stringp 5))
(qa-err-data (load 'sym) 'wrong-type-argument '(stringp sym))
;; directory is not loadable
(qa-err-data (load "/tmp" nil t) 'file-error
                '("Cannot open load file" "Is a directory" "/tmp"))
(qa (load "/tmp" t t) nil)
;; empty and comment-only files load fine
(with-temp-file "/tmp/qa_load_empty.el" (insert ""))
(with-temp-file "/tmp/qa_load_cmt.el" (insert ";; nothing here\n"))
(qa (load "/tmp/qa_load_empty.el" nil t) t)
(qa (load "/tmp/qa_load_cmt.el" nil t) t)

;; == load: NOSUFFIX / MUST-SUFFIX ==
(qa-err (load "/tmp/qa_load_b" nil t t) 'file-missing)   ; bare name not tried
(qa (load "/tmp/qa_load_b.el" nil t t) t)                ; exact name, nosuffix
;; mustsuffix: bare name + existing .el still loads
(qa (load "/tmp/qa_load_b" nil t nil t) t)
;; mustsuffix is an existence predicate: a suffixless file satisfies it
(with-temp-file "/tmp/qa_load_plain" (insert "(setq qa-plain t)\n"))
(qa (load "/tmp/qa_load_plain" nil t nil t) t)
;; name carrying a suffix is used literally under mustsuffix
(qa-err (load "/tmp/qa_load_b.elc" nil t nil t) 'file-missing)
;; without nosuffix, a bare file (no ext) still loads as last resort
(qa (load "/tmp/qa_load_plain" nil t) t)
;; but an existing .el beats the bare file
(with-temp-file "/tmp/qa_load_both" (insert "(setq qa-both 'bare)\n"))
(with-temp-file "/tmp/qa_load_both.el" (insert "(setq qa-both 'el)\n"))
(qa (load "/tmp/qa_load_both" nil t) t)
(qa qa-both 'el)
;; relative name resolved via dynamic load-path binding
(let ((load-path (cons "/tmp" load-path)))
  (qa (load "qa_load_b" nil t) t))
;; relative name not on load-path fails (default-directory is ignored)
(let ((default-directory "/tmp/")
      (load-path '("/qa-load-nowhere")))
  (qa-err (load "qa_load_b" nil t) 'file-missing))
;; name with a directory part also resolves through load-path
(make-directory "/tmp/qa_load_sub" t)
(with-temp-file "/tmp/qa_load_sub/qa_sub.el" (insert "(setq qa-sub t)\n"))
(let ((load-path (cons "/tmp" load-path)))
  (qa (load "qa_load_sub/qa_sub" nil t) t))
(qa (boundp 'qa-sub) t)

;; == load: dynamic bindings visible inside the file ==
(with-temp-file "/tmp/qa_load_dyn.el"
  (insert "(setq qa-ldyn (list load-in-progress load-file-name\n"
          "                   load-true-file-name load-read-function))\n"))
(qa (load "/tmp/qa_load_dyn.el" nil t) t)
(qa (car qa-ldyn) t)                                     ; load-in-progress
(qa (nth 1 qa-ldyn) "/tmp/qa_load_dyn.el")               ; load-file-name
(qa (nth 2 qa-ldyn) "/tmp/qa_load_dyn.el")
(qa (nth 3 qa-ldyn) 'read)                               ; load-read-function
;; nested load: inner file sees its own name, outer name restored after
(with-temp-file "/tmp/qa_load_in.el"
  (insert "(setq qa-in-name load-file-name)\n"))
(with-temp-file "/tmp/qa_load_out.el"
  (insert "(setq qa-out1 load-file-name)\n"
          "(load \"/tmp/qa_load_in.el\" nil t)\n"
          "(setq qa-out2 load-file-name)\n"))
(qa (load "/tmp/qa_load_out.el" nil t) t)
(qa qa-out1 "/tmp/qa_load_out.el")
(qa qa-in-name "/tmp/qa_load_in.el")
(qa qa-out2 "/tmp/qa_load_out.el")

;; == load-read-function ==
;; called WITH the stream; returning a form evaluates it; signaling
;; end-of-file propagates as an error (not clean EOF).  The read
;; function is only consulted while file content remains — an empty
;; file never reaches it.
(with-temp-file "/tmp/qa_load_rfi.el" (insert "(setq qa-rfi-marker t)\n"))
(defvar qa-lrf-n 0)
(setq qa-lrf-n 0)
(let ((load-read-function
       (lambda (s) (if (zerop qa-lrf-n)
                       (progn (setq qa-lrf-n 1) '(setq qa-lrf 3))
                     (signal 'end-of-file nil)))))
  (qa-err (load "/tmp/qa_load_rfi.el" nil t) 'end-of-file))
(qa (boundp 'qa-lrf) t)
(qa qa-lrf 3)
;; the file's own form was never read (read function replaced `read')
(qa (boundp 'qa-rfi-marker) nil)
;; an error from the read function propagates
(let ((load-read-function (lambda (s) (error "readfun boom"))))
  (qa-err (load "/tmp/qa_load_rfi.el" nil t) 'error))
;; a non-function read function signals invalid-function
(let ((load-read-function 5))
  (qa-err-data (load "/tmp/qa_load_rfi.el" nil t) 'invalid-function '(5)))

;; == load: errors mid-file ==
;; an error aborts the rest of the file but keeps earlier definitions
(with-temp-file "/tmp/qa_load_err.el"
  (insert "(setq qa-err-before t)\n(error \"boom\")\n(setq qa-err-after t)\n"))
(qa-err (load "/tmp/qa_load_err.el" nil t) 'error)
(qa (boundp 'qa-err-before) t)
(qa (boundp 'qa-err-after) nil)
;; a file that errors leaves NO load-history entry, even after provide
(qa (assoc "/tmp/qa_load_err.el" load-history) nil)
(with-temp-file "/tmp/qa_load_perr.el"
  (insert "(provide 'qa-perr)\n(error \"x\")\n"))
(qa-err (load "/tmp/qa_load_perr.el" nil t) 'error)
(qa (featurep 'qa-perr) t)
(qa (assoc "/tmp/qa_load_perr.el" load-history) nil)
;; a reader error propagates (end-of-file)
(with-temp-file "/tmp/qa_load_readerr.el"
  (insert "(setq qa-rerr t)\n(unmatched\n"))
(qa-err (load "/tmp/qa_load_readerr.el" nil t) 'end-of-file)
(qa (boundp 'qa-rerr) t)

;; == lexical-binding cookie / shebang ==
(with-temp-file "/tmp/qa_load_lex.el"
  (insert ";; -*- lexical-binding: t -*-\n"
          "(setq qa-lexf (let ((y 8)) #'(lambda () y)))\n"))
(load "/tmp/qa_load_lex.el" nil t)
(qa (funcall qa-lexf) 8)
(with-temp-file "/tmp/qa_load_dynsc.el"
  (insert "(setq qa-dynf (let ((y 9)) #'(lambda () y)))\n"))
(load "/tmp/qa_load_dynsc.el" nil t)
(qa-err (funcall qa-dynf) 'void-variable)
;; a #! first line is skipped; a cookie on line 2 still applies
(with-temp-file "/tmp/qa_load_shb.el"
  (insert "#!ignored\n;; -*- lexical-binding: t -*-\n"
          "(setq qa-shbf (let ((y 7)) #'(lambda () y)))\n"))
(qa (load "/tmp/qa_load_shb.el" nil t) t)
(qa (funcall qa-shbf) 7)
;; eval-when-compile DOES run under interpreted load (eager expansion)
(with-temp-file "/tmp/qa_load_ewc.el"
  (insert "(eval-when-compile (setq qa-ewc t))\n"
          "(eval-and-compile (setq qa-eac t))\n"))
(qa (load "/tmp/qa_load_ewc.el" nil t) t)
(qa (boundp 'qa-ewc) t)
(qa (boundp 'qa-eac) t)

;; == .elc preference / load-prefer-newer ==
(with-temp-file "/tmp/qa_load_pref.el" (insert "(setq qa-pref 'el)\n"))
(when (and (fboundp 'byte-compile-file)
           (ignore-errors (byte-compile-file "/tmp/qa_load_pref.el")))
  (setq qa-pref nil)
  (qa (load "/tmp/qa_load_pref" nil t) t)
  (qa qa-pref 'el)                       ; .elc preferred over .el
  (let ((load-prefer-newer t))
    (sleep-for 0.05)
    (with-temp-file "/tmp/qa_load_pref.el"
      (insert "(setq qa-pref 'el-newer)\n"))
    (setq qa-pref nil)
    (qa (load "/tmp/qa_load_pref" nil t) t)
    (qa qa-pref 'el-newer)))

;; == eval-after-load ==
;; string key: fires after the file loads
(setq qa-eal1 nil)
(eval-after-load "qa_load_eal" '(setq qa-eal1 'loaded))
(qa qa-eal1 nil)
(with-temp-file "/tmp/qa_load_eal.el"
  (insert "(setq qa-eal-inner t)\n(provide 'qa-eal-feat)\n"))
(qa (load "/tmp/qa_load_eal.el" nil t) t)
(qa qa-eal1 'loaded)
;; string key fires immediately when a matching file is in load-history
(setq qa-eal2 nil)
(eval-after-load "qa_load_eal" '(setq qa-eal2 'ran))
(qa qa-eal2 'ran)
;; file key matches even when the file provides nothing
(setq qa-eal3 nil)
(eval-after-load "qa_load_ealnp" '(setq qa-eal3 t))
(with-temp-file "/tmp/qa_load_ealnp.el" (insert "(setq qa-ealnp-inner t)\n"))
(qa (load "/tmp/qa_load_ealnp.el" nil t) t)
(qa qa-eal3 t)
;; string-keyed hooks see the OUTER load's bindings: by the time they
;; run, the inner load has unwound
(setq qa-eal-lfn 'unset)
(eval-after-load "qa_load_lfn" '(setq qa-eal-lfn load-file-name))
(with-temp-file "/tmp/qa_load_lfn.el" (insert "(provide 'qa-lfn)\n"))
(load "/tmp/qa_load_lfn.el" nil t)
(qa qa-eal-lfn load-file-name)
;; load-in-progress is still t (an outer load is in progress)
(setq qa-eal-lip nil)
(eval-after-load "qa_load_lip" '(setq qa-eal-lip load-in-progress))
(with-temp-file "/tmp/qa_load_lip.el" (insert "(provide 'qa-lip)\n"))
(load "/tmp/qa_load_lip.el" nil t)
(qa qa-eal-lip t)
;; identical forms deduplicate
(setq qa-eal-dup 0)
(eval-after-load "qa_load_dup" '(setq qa-eal-dup (1+ qa-eal-dup)))
(eval-after-load "qa_load_dup" '(setq qa-eal-dup (1+ qa-eal-dup)))
(with-temp-file "/tmp/qa_load_dup.el" (insert "(provide 'qa-dup)\n"))
(load "/tmp/qa_load_dup.el" nil t)
(qa qa-eal-dup 1)
;; multiple distinct forms for one file run oldest-registration-first
(setq qa-eal-ord nil)
(eval-after-load "qa_load_ord" '(push 'a qa-eal-ord))
(eval-after-load "qa_load_ord" '(push 'b qa-eal-ord))
(with-temp-file "/tmp/qa_load_ord.el" (insert "(provide 'qa-ord)\n"))
(load "/tmp/qa_load_ord.el" nil t)
(qa qa-eal-ord '(b a))
;; symbol key: fires at `provide' time even when the feature's file
;; name doesn't match the key
(setq qa-eal-sym nil)
(eval-after-load 'qa-mismatch-feat '(setq qa-eal-sym 'provided))
(with-temp-file "/tmp/qa_load_mm.el" (insert "(provide 'qa-mismatch-feat)\n"))
(load "/tmp/qa_load_mm.el" nil t)
(qa qa-eal-sym 'provided)
;; symbol key on an already-provided feature evaluates immediately
(provide 'qa-pre-feat)
(setq qa-eal-pre nil)
(eval-after-load 'qa-pre-feat '(setq qa-eal-pre t))
(qa qa-eal-pre t)
;; symbol-keyed hooks also run after the load unwinds (they go through
;; after-load-functions): load-file-name is again the outer file
(setq qa-eal-slfn 'unset)
(eval-after-load 'qa-slfn-feat '(setq qa-eal-slfn load-file-name))
(with-temp-file "/tmp/qa_load_slfn.el" (insert "(provide 'qa-slfn-feat)\n"))
(load "/tmp/qa_load_slfn.el" nil t)
(qa qa-eal-slfn load-file-name)
;; return value is nil
(qa (eval-after-load "qa_load_never" '(setq qa-nv 1)) nil)
;; non-string non-symbol key is a type error
(qa-err (eval-after-load 5 '(setq qa-nv2 1)) 'wrong-type-argument)
;; hooks fire when `require' does the loading too
(setq qa-eal-req nil)
(eval-after-load 'qa-req-feat '(setq qa-eal-req t))
(with-temp-file "/tmp/qa_load_rf.el" (insert "(provide 'qa-req-feat)\n"))
(require 'qa-req-feat "/tmp/qa_load_rf.el")
(qa qa-eal-req t)

;; == autoload ==
(qa (autoload 'qa-afn "qa-alib") 'qa-afn)
(qa (symbol-function 'qa-afn) '(autoload "qa-alib" nil nil nil))
(qa (autoloadp (symbol-function 'qa-afn)) t)
;; autoloadp checks only (eq (car x) 'autoload) — never follows symbols
(qa (autoloadp 'qa-afn) nil)
(qa (autoloadp '(autoload)) t)
(qa (autoloadp '(autoloadx "f")) nil)
(qa (autoloadp (lambda (x) x)) nil)
;; docstring/interactive/type fields land verbatim in the cell
(autoload 'qa-afn2 "qa-alib2" "Doc here" t)
(qa (symbol-function 'qa-afn2) '(autoload "qa-alib2" "Doc here" t nil))
(autoload 'qa-afn3 "qa-alib3" nil nil 'macro)
(qa (symbol-function 'qa-afn3) '(autoload "qa-alib3" nil nil macro))
;; an unrecognized TYPE is stored verbatim
(autoload 'qa-afn4 "qa-alib4" "doc" nil 'keymap)
(qa (symbol-function 'qa-afn4) '(autoload "qa-alib4" "doc" nil keymap))
;; FILE must be a string
(qa-err (autoload 'qa-abad nil) 'wrong-type-argument)
(qa-err (autoload 'qa-abad2 5) 'wrong-type-argument)
;; FUNCTION must be a symbol
(qa-err (autoload "str" "x") 'wrong-type-argument)
;; already fbound: no-op, returns nil (not the symbol)
(defun qa-already-defined () 1)
(qa (autoload 'qa-already-defined "qa-nope") nil)
(qa (qa-already-defined) 1)
;; documentation comes from the autoload's DOC field
(autoload 'qa-adoc "qa-adoclib" "My docstring." t nil)
(qa (documentation 'qa-adoc) "My docstring.")
;; macrop / commandp consult the cell's TYPE/INTERACTIVE fields
(qa (macrop 'qa-afn3) '(macro t))
(qa (macrop 'qa-afn) nil)
(qa (commandp 'qa-afn2) t)
(qa (commandp 'qa-afn3) nil)             ; macro autoloads aren't commands
(qa (indirect-function 'qa-afn) '(autoload "qa-alib" nil nil nil))
(qa (fboundp 'qa-afn) t)
;; calling an autoloaded function loads the file and installs the def
(makunbound 'qa-called-fn)
(with-temp-file "/tmp/qa_load_af.el"
  (insert "(defun qa-called-fn (x) (list 'real x))\n(provide 'qa-af)\n"))
(let ((load-path (cons "/tmp" load-path)))
  (autoload 'qa-called-fn "qa_load_af")
  (qa (qa-called-fn 'z) '(real z)))
(qa (autoloadp (symbol-function 'qa-called-fn)) nil)
;; file that fails to define the function -> error on call
(makunbound 'qa-wrong-fn)
(with-temp-file "/tmp/qa_load_awf.el" (insert "(provide 'qa-awf)\n"))
(let ((load-path (cons "/tmp" load-path)))
  (autoload 'qa-wrong-fn "qa_load_awf")
  (qa-err (qa-wrong-fn) 'error))
;; autoload-do-load on a missing file signals file-missing
(autoload 'qa-dlm "qa_load_dlm_missing")
(qa-err (autoload-do-load (symbol-function 'qa-dlm) 'qa-dlm) 'file-missing)
;; autoload-do-load returns the real definition on success
(with-temp-file "/tmp/qa_load_adl.el"
  (insert "(defun qa-adl-fn (x) (+ x 1))\n(provide 'qa-adl)\n"))
(let ((load-path (cons "/tmp" load-path)))
  (autoload 'qa-adl-fn "qa_load_adl")
  (qa (autoloadp (symbol-function 'qa-adl-fn)) t)
  (qa (functionp (autoload-do-load (symbol-function 'qa-adl-fn) 'qa-adl-fn)) t)
  (qa (qa-adl-fn 41) 42))

;; == load-history ==
(qa (listp load-history) t)
(qa (consp (car load-history)) t)
;; newest load sits at the front
(with-temp-file "/tmp/qa_load_hA.el" (insert "(provide 'qa-ha)\n"))
(with-temp-file "/tmp/qa_load_hB.el" (insert "(provide 'qa-hb)\n"))
(load "/tmp/qa_load_hA.el" nil t)
(load "/tmp/qa_load_hB.el" nil t)
(qa (car (car load-history)) "/tmp/qa_load_hB.el")
;; entries record (defun . f), bare var symbols, provide, require
(with-temp-file "/tmp/qa_load_hist.el"
  (insert "(defun qa-hf () 1)\n(defvar qa-hv 2)\n(provide 'qa-hist)\n"))
(load "/tmp/qa_load_hist.el" nil t)
(let ((e (cdr (assoc "/tmp/qa_load_hist.el" load-history))))
  (qa (member '(defun . qa-hf) e) '((defun . qa-hf) qa-hv (provide . qa-hist)))
  (qa (memq 'qa-hv e) '(qa-hv (provide . qa-hist)))
  (qa (member '(provide . qa-hist) e) '((provide . qa-hist))))
;; require entries are recorded too
(with-temp-file "/tmp/qa_load_hreq.el"
  (insert "(require 'seq)\n(provide 'qa-hreq)\n"))
(load "/tmp/qa_load_hreq.el" nil t)
(let ((e (cdr (assoc "/tmp/qa_load_hreq.el" load-history))))
  (qa (member '(require . seq) e) '((require . seq) (provide . qa-hreq))))
;; reloading replaces the old entry (still a single assoc)
(let ((n 0))
  (dolist (e load-history)
    (when (equal (car e) "/tmp/qa_load_hA.el") (setq n (1+ n))))
  (qa n 1))
(load "/tmp/qa_load_hA.el" nil t)
(let ((n 0))
  (dolist (e load-history)
    (when (equal (car e) "/tmp/qa_load_hA.el") (setq n (1+ n))))
  (qa n 1))
;; a file whose only tracked form is setq still gets an entry
(with-temp-file "/tmp/qa_load_hc.el" (insert "(setq qa-hc t)\n"))
(load "/tmp/qa_load_hc.el" nil t)
(qa (assoc "/tmp/qa_load_hc.el" load-history) '("/tmp/qa_load_hc.el"))

;; == locate-library ==
(let ((r (locate-library "subr-x")))
  (qa-xf (and (stringp r) (string-suffix-p "subr-x.elc" r)) t
         "remacs ships only .el sources in lisp/ — no .elc exists on
         disk for locate-library to find, so .el is the physically
         correct answer here (unlike GNU's installed tree which has
         byte-compiled files)"))
;; explicit .el finds the source (.el.gz counts via load-file-rep-suffixes)
(let ((r (locate-library "subr-x.el")))
  (qa (and (stringp r)
           (or (string-suffix-p "subr-x.el" r)
               (string-suffix-p "subr-x.el.gz" r)))
      t))
(qa (locate-library "qa-load-no-such-lib-xyz") nil)
;; NOSUFFIX arg restricts the search
(qa (locate-library "subr-x" nil '(".xyz")) nil)
;; locate-library honors dynamic load-path
(with-temp-file "/tmp/qa_load_loc.el" (insert "(provide 'qa-loc)\n"))
(let ((load-path (cons "/tmp" load-path)))
  (qa (locate-library "qa_load_loc") "/tmp/qa_load_loc.el"))
;; find-library-name lives in find-func
(require 'find-func)
(qa (fboundp 'find-library-name) t)
(let ((r (and (fboundp 'find-library-name)
              (find-library-name "subr-x"))))
  (qa (and (stringp r)
           (or (string-suffix-p "subr-x.el" r)
               (string-suffix-p "subr-x.el.gz" r)))
      t))

;; == unload-feature ==
;; a bare (provide ...) has no file: GNU dies inside file-dependents
(provide 'qa-unload-bare)
(qa-err (unload-feature 'qa-unload-bare) 'wrong-type-argument)
(qa (featurep 'qa-unload-bare) t)         ; still provided after the error
;; never-loaded feature -> plain error
(qa-err (unload-feature 'qa-never-loaded-xyz) 'error)
;; nil feature -> error mentioning "not a currently loaded feature"
(qa-err (unload-feature nil) 'error)
(qa-err (unload-feature 5) 'wrong-type-argument)
;; a loaded file's feature unloads: feature removed, defs unbound
(defvar qa-unl-ran nil)
(with-temp-file "/tmp/qa_load_unl.el"
  (insert "(defun qa-unl-fn () 1)\n"
          "(defun qa_load_unl-unload-function () (setq qa-unl-ran t) nil)\n"
          "(provide 'qa_load_unl)\n"))
(let ((load-path (cons "/tmp" load-path)))
  (qa (require 'qa_load_unl) 'qa_load_unl))
(qa (featurep 'qa_load_unl) t)
(qa (fboundp 'qa-unl-fn) t)
(qa (unload-feature 'qa_load_unl) nil)    ; returns nil on success
(qa (featurep 'qa_load_unl) nil)
(qa (fboundp 'qa-unl-fn) nil)
(qa qa-unl-ran t)                         ; FEATURE-unload-function ran
;; unload removes the file's load-history entry
(qa (assoc "/tmp/qa_load_unl.el" load-history) nil)
;; re-require reloads the file
(let ((load-path (cons "/tmp" load-path)))
  (qa (require 'qa_load_unl) 'qa_load_unl))
(qa (featurep 'qa_load_unl) t)
;; unloading cl-lib errors when another loaded file requires it.
;; The temp file's (require 'cl-lib) can hang remacs inside load —
;; skip the load there; the unload check itself still runs.
(with-temp-file "/tmp/qa_load_dep.el"
  (insert "(require 'cl-lib)\n(provide 'qa-dep)\n"))
(qa (load "/tmp/qa_load_dep.el" nil t) t)
(qa-err (unload-feature 'cl-lib) 'error)
(qa (featurep 'cl-lib) t)

;; == with-demoted-errors ==
(qa (with-demoted-errors "demoted: %s" (error "demoted") 7) nil)
(qa (with-demoted-errors "ok: %s" 7) 7)
(let ((debug-ignored-errors nil))
  (qa (with-demoted-errors "fmt %s" (error "y") 5) nil))
;; load under with-demoted-errors: signals are demoted, load returns nil
(qa (with-demoted-errors "load: %s" (load "/tmp/qa_load_no_such_xyz" nil t))
    nil)

;; == .gz loading via load-file-rep-suffixes ==
(with-temp-file "/tmp/qa_load_gz.el" (insert "(setq qa-gz-ok t)\n"))
(qa (call-process "gzip" nil nil nil "-kf" "/tmp/qa_load_gz.el") 0)
(qa (load "/tmp/qa_load_gz" nil t) t)      ; finds .el.gz via rep-suffixes
(qa (boundp 'qa-gz-ok) t)
(qa (load "/tmp/qa_load_gz.el.gz" nil t) t)

;; == autoload target vs loading-file basename collision ==
;; A file being loaded whose basename equals an autoload's library
;; ("/tmp/qa_load_coll/sort.el" vs the bundled "sort") must not trip
;; the recursion guard: GNU compares resolved paths in
;; `Vloads_in_progress', never basenames.
(make-directory "/tmp/qa_load_coll" t)
(with-temp-file "/tmp/qa_load_coll/sort.el"
  (insert "(setq qa-sort-file (with-temp-buffer
                              (insert \"c\\na\\nb\\n\")
                              (sort-lines nil (point-min) (point-max))
                              (buffer-string)))\n"))
(qa (load "/tmp/qa_load_coll/sort.el" nil t) t)
(qa qa-sort-file "a\nb\nc\n")

;; == Recursive load ==
;; GNU Fload conses `found' onto Vloads_in_progress and signals
;; "Recursive load" when the same path nests a 4th time.
(with-temp-file "/tmp/qa_load_recur.el" (insert "(load \"/tmp/qa_load_recur.el\" nil t)\n"))
(qa-err (load "/tmp/qa_load_recur.el" nil t) 'error)
;; Three nested loads of the same file are allowed.
(with-temp-file "/tmp/qa_load_recur2.el"
  (insert "(setq qa-recur-depth (1+ (or (bound-and-true-p qa-recur-depth) 0)))
(when (< qa-recur-depth 3) (load \"/tmp/qa_load_recur2.el\" nil t))\n"))
(setq qa-recur-depth nil)
(qa (load "/tmp/qa_load_recur2.el" nil t) t)
(qa qa-recur-depth 3)

;; == cleanup ==
(dolist (f '("/tmp/qa_load_np.el" "/tmp/qa_load_ovr.el" "/tmp/qa_load_ovr2.el"
             "/tmp/qa_load_b.el" "/tmp/qa_load_empty.el" "/tmp/qa_load_cmt.el"
             "/tmp/qa_load_plain" "/tmp/qa_load_both" "/tmp/qa_load_both.el"
             "/tmp/qa_load_dyn.el" "/tmp/qa_load_in.el" "/tmp/qa_load_out.el"
             "/tmp/qa_load_err.el" "/tmp/qa_load_perr.el" "/tmp/qa_load_readerr.el"
             "/tmp/qa_load_lex.el" "/tmp/qa_load_dynsc.el" "/tmp/qa_load_shb.el"
             "/tmp/qa_load_ewc.el" "/tmp/qa_load_pref.el" "/tmp/qa_load_pref.elc"
             "/tmp/qa_load_eal.el" "/tmp/qa_load_ealnp.el" "/tmp/qa_load_lfn.el"
             "/tmp/qa_load_lip.el" "/tmp/qa_load_dup.el" "/tmp/qa_load_ord.el"
             "/tmp/qa_load_mm.el" "/tmp/qa_load_slfn.el" "/tmp/qa_load_rf.el"
             "/tmp/qa_load_af.el" "/tmp/qa_load_awf.el" "/tmp/qa_load_adl.el"
             "/tmp/qa_load_hA.el" "/tmp/qa_load_hB.el" "/tmp/qa_load_hist.el"
             "/tmp/qa_load_hreq.el" "/tmp/qa_load_hc.el" "/tmp/qa_load_loc.el"
             "/tmp/qa_load_unl.el" "/tmp/qa_load_dep.el" "/tmp/qa_load_gz.el"
             "/tmp/qa_load_gz.el.gz" "/tmp/qa_load_sub/qa_sub.el"
             "/tmp/qa_load_rfi.el" "/tmp/qa_load_recur.el"
             "/tmp/qa_load_recur2.el" "/tmp/qa_load_coll/sort.el"))
  (when (file-exists-p f) (delete-file f)))
(delete-directory "/tmp/qa_load_sub" t)
(delete-directory "/tmp/qa_load_coll" t)

(princ (qa--done))
