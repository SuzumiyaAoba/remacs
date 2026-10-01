;; File-local & directory-local variables — -*- prop lines, Local Variables
;; sections, safe/risky classification, hack-* application, dir-locals
;; classes/files, and the add/delete commands that rewrite file text.
;; GNU 31.1 -Q --batch ground truth (verified by probing):
;; - `hack-local-variables' populates `file-local-variables-alist' AND
;;   applies the safe entries (fill-column gets buffer-local value).
;; - A `mode' entry is stored as (mode . text) — the VALUE lacks the
;;   -mode suffix; application interns "<val>-mode".  So
;;   `-*- mode: text-mode -*-' yields `text-mode-mode', an unknown
;;   function, and the buffer stays in fundamental-mode (GNU does the
;;   same bug-compatible thing).
;; - `hack-local-variables' applied to a -*- line does NOT itself change
;;   major-mode via the (mode . X) entry? — it DOES via
;;   hack-one-local-variable -> set-auto-mode-0.  But `set-auto-mode'
;;   applies ONLY the mode and ignores other -*- variables
;;   (fill-column stays default).
;; - In batch (noninteractive), unsafe or risky entries are silently
;;   dropped: `eval:' requires enable-local-eval (default `maybe') and
;;   an unknown/unsafe variable like `Fill-Column' (case-preserved
;;   symbol has no safe-local-variable property) taints the WHOLE -*-
;;   line, so even the mode is not applied.
;; - `risky-local-variable-p' returns a string-match INDEX (e.g. 4),
;;   not t, for name-suffix matches; the `eval' symbol returns t from
;;   its risky-local-variable property.
;; - `hack-one-local-variable-eval-safep' returns the index from
;;   "-mode\\'" string-match (4 for text-mode), the tail of
;;   `safe-local-eval-forms' via `member', t for whitelisted put
;;   forms, t for any atom, nil otherwise.
;; - `file-local-variables-alist' is permanent-local: it survives
;;   kill-all-local-variables while fill-column is reset.
;; - Duplicate entries: the LAST one wins; only the FIRST
;;   "Local Variables:" block in the buffer is read.
;; - lexical-binding is only honored from the -*- line, not the
;;   Local Variables section; it is in
;;   `permanently-enabled-local-variables' so it applies even when
;;   enable-local-variables is nil.
;; - `dir-locals-find-file' returns: nil, a directory-name string (a
;;   dir-locals file was found), or a (DIR CLASS MTIME) cache list.
;; - `dir-locals-read-from-dir' returns a CLASS SYMBOL (interned dir
;;   name), not the variable alist.
;; - In batch, `hack-local-variables-confirm' returns nil (its whole
;;   body is guarded by `noninteractive').
;; - add/delete-file-local-variable need no visited file to edit the
;;   buffer text, and `save-buffer' must be called explicitly — the
;;   commands only modify the buffer.
;; - The mode alist keys written by add-dir-local-variable print like
;;   (emacs-lisp-mode (fill-column . 66)) — cdr is a LIST of pairs.

(setq inhibit-message t)

;; Dynamic targets for eval: checks (setq inside eval needs a special
;; variable, not a lexical let binding).
(defvar qafl-ev-target nil)
(defvar qafl-ev-target2 nil)

;; == fboundp / variables presence ==
(qa (fboundp 'hack-local-variables) t)
(qa (fboundp 'hack-local-variables-prop-line) t)
(qa (fboundp 'hack-local-variables--find-variables) t)
(qa (fboundp 'hack-local-variables-filter) t)
(qa (fboundp 'hack-local-variables-apply) t)
(qa (fboundp 'hack-local-variables-confirm) t)
(qa (fboundp 'hack-one-local-variable) t)
(qa (fboundp 'hack-one-local-variable-quotep) t)
(qa (fboundp 'hack-one-local-variable-eval-safep) t)
(qa (fboundp 'safe-local-variable-p) t)
(qa (fboundp 'risky-local-variable-p) t)
(qa (fboundp 'inhibit-local-variables-p) t)
(qa (fboundp 'normal-mode) t)
(qa (fboundp 'set-auto-mode) t)
(qa (fboundp 'set-auto-mode-0) t)
(qa (fboundp 'set-auto-mode-1) t)
(qa (fboundp 'dir-locals-find-file) t)
(qa (fboundp 'dir-locals-read-from-dir) t)
(qa (fboundp 'dir-locals-set-class-variables) t)
(qa (fboundp 'dir-locals-set-directory-class) t)
(qa (fboundp 'dir-locals-get-class-variables) t)
(qa (fboundp 'hack-dir-local-variables) t)
(qa (fboundp 'hack-dir-local-variables-non-file-buffer) t)
(qa (fboundp 'add-file-local-variable) t)
(qa (fboundp 'delete-file-local-variable) t)
(qa (fboundp 'add-file-local-variable-prop-line) t)
(qa (fboundp 'delete-file-local-variable-prop-line) t)
(qa (fboundp 'add-dir-local-variable) t)
(qa (fboundp 'delete-dir-local-variable) t)
(qa (fboundp 'copy-file-locals-to-dir-locals) t)
(qa (fboundp 'locate-dominating-file) t)
(qa (fboundp 'major-mode-remap) t)

;; == default values ==
(qa (boundp 'file-local-variables-alist) t)
(qa (boundp 'dir-local-variables-alist) t)
(qa enable-local-variables t)
(qa enable-local-eval 'maybe)
(qa enable-dir-local-variables t)
(qa local-enable-local-variables t)
(qa enable-remote-dir-locals nil)
(qa dir-locals-file ".dir-locals.el")
(qa (boundp 'dir-locals-class-alist) t)
(qa (boundp 'dir-locals-directory-cache) t)
(qa safe-local-variable-values nil)
(qa safe-local-variable-directories nil)
(qa ignored-local-variable-values nil)
(qa permanently-enabled-local-variables
    '(lexical-binding read-symbol-shorthands))
(qa (memq 'file-local-variables-alist ignored-local-variables)
    '(file-local-variables-alist dir-local-variables-alist))
(qa hack-dir-local-get-variables-functions '(hack-dir-local--get-variables))
(qa inhibit-local-variables-ignore-case t)
(qa (boundp 'inhibit-local-variables-regexps) t)
(qa (car (member "\\.tar\\'" inhibit-local-variables-regexps)) "\\.tar\\'")
(qa (consp inhibit-local-variables-suffixes) t)
(qa (boundp 'file-auto-mode-skip) t)
(qa (boundp 'file-name-handler-alist) t)
(qa (boundp 'before-hack-local-variables-hook) t)
(qa (boundp 'hack-local-variables-hook) t)
(qa (boundp 'major-mode-remap-alist) t)

;; == safe-local-variable-p ==
(qa (safe-local-variable-p 'fill-column 70) t)
(qa (safe-local-variable-p 'fill-column "x") nil)
(qa (safe-local-variable-p 'indent-tabs-mode nil) t)
;; a non-boolean is NOT safe for booleanp-marked indent-tabs-mode
(qa (safe-local-variable-p 'indent-tabs-mode 5) nil)
(qa (safe-local-variable-p 'eval '(foo)) nil)
(qa (safe-local-variable-p 'load "x") nil)
(qa (safe-local-variable-p 'nonexistent-var-xyz 1) nil)
;; user option pairs and the safe-local-variable property
(let ((safe-local-variable-values '((qafl-v9 . 7))))
  ;; `member' returns the tail of safe-local-variable-values
  (qa (safe-local-variable-p 'qafl-v9 7) '((qafl-v9 . 7)))
  (qa (safe-local-variable-p 'qafl-v9 8) nil))
(put 'qafl-vip 'safe-local-variable 'integerp)
(qa (safe-local-variable-p 'qafl-vip 5) t)
(qa (safe-local-variable-p 'qafl-vip "x") nil)
;; string (non-symbol) variable name errors out
(qa-err (safe-local-variable-p "fill-column" 1) 'wrong-type-argument)

;; == risky-local-variable-p ==
;; `eval' is risky via its risky-local-variable property -> t.
(qa (risky-local-variable-p 'eval) t)
(qa (risky-local-variable-p 'fill-column) nil)
(qa (risky-local-variable-p 'random-name) nil)
;; name-suffix matches return the string-match INDEX, not t.
(qa (risky-local-variable-p 'some-hook) 4)
(qa (risky-local-variable-p 'some-function) 4)
(qa (risky-local-variable-p 'some-map) 4)
(qa (risky-local-variable-p 'some-program) 4)
(qa (risky-local-variable-p 'some-command) 4)
(qa (risky-local-variable-p 'some-predicate) 4)
(qa (risky-local-variable-p 'some-form) 4)
(qa (risky-local-variable-p 'some-frame-alist) 4)
(qa (risky-local-variable-p 'some-map-alist) 4)
(qa (risky-local-variable-p 'some-bindat-spec) 4)
;; auto-mode-alist also carries a risky-local-variable property -> t.
(qa (risky-local-variable-p 'auto-mode-alist) t)
;; font-lock-keyword* alternatives match at position 0.
(qa (risky-local-variable-p 'font-lock-keywords) 0)
(qa (risky-local-variable-p 'font-lock-keywords-3) 0)
(qa-err (risky-local-variable-p "eval") 'wrong-type-argument)

;; == hack-one-local-variable-eval-safep / quotep ==
(qa (hack-one-local-variable-eval-safep 'foo) t)
(qa (hack-one-local-variable-eval-safep 42) t)
(qa (hack-one-local-variable-eval-safep '(setq x 1)) nil)
(qa (hack-one-local-variable-eval-safep '(eval (setq x 1))) nil)
(qa (hack-one-local-variable-eval-safep '(progn)) nil)
(qa (hack-one-local-variable-eval-safep '(message "hi")) nil)
;; (foo-mode) / (foo-mode 1|0|-1) are considered safe; value is the
;; index of "-mode" inside the symbol name ("text-mode" -> 4).
(qa (hack-one-local-variable-eval-safep '(text-mode)) 4)
(qa (hack-one-local-variable-eval-safep '(text-mode 1)) 4)
;; member on safe-local-eval-forms returns the tail (= whole list for
;; the first element).
(qa (hack-one-local-variable-eval-safep
     '(add-hook 'write-file-hooks 'time-stamp))
    safe-local-eval-forms)
(qa (equal safe-local-eval-forms
           '((add-hook 'write-file-hooks 'time-stamp)
             (add-hook 'write-file-functions 'time-stamp)
             (add-hook 'before-save-hook 'time-stamp nil t)
             (add-hook 'before-save-hook 'delete-trailing-whitespace nil t)))
    t)
;; whitelisted `put' property settings
(qa (hack-one-local-variable-eval-safep '(put 's 'lisp-indent-function 1)) t)
(qa (hack-one-local-variable-eval-safep '(put 's 'lisp-indent-function 'foo)) nil)
(qa (hack-one-local-variable-eval-safep '(put 's 'edebug-form-spec 'x)) nil)
(qa (hack-one-local-variable-eval-safep '(put 'x 'y 1)) nil)
(qa (hack-one-local-variable-quotep ''a) t)
(qa (hack-one-local-variable-quotep 'x) nil)
(qa (hack-one-local-variable-quotep '(quote)) nil)

;; == hack-one-local-variable ==
(qa (with-temp-buffer
      (hack-one-local-variable 'fill-column 88)
      (list fill-column (local-variable-p 'fill-column)))
    '(88 t))
;; mode value must NOT carry the -mode suffix; it is appended.
(qa (with-temp-buffer
      (hack-one-local-variable 'mode 'text)
      major-mode)
    'text-mode)
;; 'text-mode' -> interns text-mode-mode (unknown) -> stays fundamental.
(qa (with-temp-buffer
      (hack-one-local-variable 'mode 'text-mode)
      major-mode)
    'fundamental-mode)
;; 'eval evaluates the form regardless of safep (this is the applier,
;; not the checker); returns the eval result.
(qa (with-temp-buffer
      (hack-one-local-variable 'eval '(setq qafl-ev-target2 6)))
    6)
(qa qafl-ev-target2 6)

;; == hack-local-variables-prop-line ==
(qa (with-temp-buffer
      (insert "-*- fill-column: 70 -*-\ntext\n")
      (hack-local-variables-prop-line))
    '((fill-column . 70)))
;; simple "-*- MODENAME -*-" form returns nil unless handle-mode is t
(qa (with-temp-buffer
      (insert "-*- text -*-\nx\n")
      (hack-local-variables-prop-line))
    nil)
(qa (with-temp-buffer
      (insert "-*- text -*-\nx\n")
      (hack-local-variables-prop-line t))
    'text-mode)
;; hairy form: entries are pushed, so order is reversed; mode key is
;; downcased to the symbol `mode' but its value keeps no -mode suffix.
(qa (with-temp-buffer
      (insert "-*- mode: c; fill-column: 70 -*-\nx\n")
      (hack-local-variables-prop-line))
    '((fill-column . 70) (mode . c)))
;; handle-mode neither nil nor t: mode entry dropped
(qa (with-temp-buffer
      (insert "-*- mode: c; fill-column: 70 -*-\nx\n")
      (hack-local-variables-prop-line 'no-mode))
    '((fill-column . 70)))
(qa (with-temp-buffer
      (insert "-*- mode: c; fill-column: 70 -*-\nx\n")
      (hack-local-variables-prop-line t))
    'c-mode)
;; coding: is always ignored on the prop line
(qa (with-temp-buffer
      (insert "-*- coding: utf-8; fill-column: 33 -*-\nx\n")
      (hack-local-variables-prop-line))
    '((fill-column . 33)))
;; "Mode:" is case-insensitive for the key; OTHER variable names keep
;; their case (Fill-Column is a distinct, unsafe symbol).
(qa (with-temp-buffer
      (insert "-*- Mode: c;Fill-Column: 3-*-\nx\n")
      (hack-local-variables-prop-line))
    '((Fill-Column . 3) (mode . c)))

;; == hack-local-variables: -*- prop line ==
(qa (with-temp-buffer
      (insert "-*- fill-column: 70 -*-\ntext\n")
      (hack-local-variables)
      file-local-variables-alist)
    '((fill-column . 70)))
;; the variables are also APPLIED buffer-locally
(qa (with-temp-buffer
      (insert "-*- fill-column: 70 -*-\ntext\n")
      (hack-local-variables)
      (list fill-column (local-variable-p 'fill-column)))
    '(70 t))
(qa (with-temp-buffer
      (insert "-*- mode: text-mode -*-\ntext\n")
      (hack-local-variables)
      (list major-mode file-local-variables-alist))
    '(fundamental-mode ((mode . text-mode))))
(qa (with-temp-buffer
      (insert "-*- mode: text -*- \ntext\n")
      (hack-local-variables)
      (list major-mode file-local-variables-alist))
    '(text-mode ((mode . text))))
(qa (with-temp-buffer
      (insert "-*- mode: text-mode; fill-column: 70 -*-\ntext\n")
      (hack-local-variables)
      (list major-mode file-local-variables-alist))
    '(fundamental-mode ((mode . text-mode) (fill-column . 70))))
(qa (with-temp-buffer
      (insert "-*- mode: text; fill-column: 61 -*- \nx\n")
      (hack-local-variables)
      (list major-mode fill-column))
    '(text-mode 61))
;; after hacking, file-local-variables-alist is a buffer-local var
(qa (with-temp-buffer
      (insert "-*- fill-column: 33 -*-\nx\n")
      (hack-local-variables)
      (local-variable-p 'file-local-variables-alist))
    t)
;; and it is permanent-local: survives kill-all-local-variables
(qa (with-temp-buffer
      (insert "-*- fill-column: 33 -*-\nx\n")
      (hack-local-variables)
      (kill-all-local-variables)
      (list fill-column file-local-variables-alist major-mode))
    '(70 ((fill-column . 33)) fundamental-mode))
;; unsafe -*- line (eval: with enable-local-eval=maybe in batch):
;; everything is dropped, alist stays nil
(qa (with-temp-buffer
      (insert "-*- eval: (message \"x\") -*-\nx\n")
      (hack-local-variables)
      file-local-variables-alist)
    nil)
;; unknown/unsafe variable name (case-preserved Fill-Column) taints the
;; whole line in batch: alist nil AND mode not applied.
(qa (with-temp-buffer
      (insert "-*- Mode: c;Fill-Column: 3-*-\nx\n")
      (hack-local-variables)
      (list major-mode file-local-variables-alist))
    '(fundamental-mode nil))
;; string value for an integerp-safe var is unsafe -> dropped
(qa (with-temp-buffer
      (insert "-*- fill-column: \"abc\" -*-\nx")
      (hack-local-variables)
      file-local-variables-alist)
    nil)

;; == hack-local-variables: Local Variables section ==
(qa (with-temp-buffer
      (insert "text\n;; Local Variables:\n;; fill-column: 55\n;; End:\n")
      (hack-local-variables)
      (list major-mode file-local-variables-alist))
    '(fundamental-mode ((fill-column . 55))))
(qa (with-temp-buffer
      (insert "x\n;; Local Variables:\n;; mode: emacs-lisp\n;; fill-column: 50\n;; End:\n")
      (hack-local-variables)
      (list major-mode file-local-variables-alist))
    '(emacs-lisp-mode ((mode . emacs-lisp) (fill-column . 50))))
;; eval: in the section is dropped under enable-local-eval=maybe
(qa (with-temp-buffer
      (insert "x\n;; Local Variables:\n;; eval: (setq qafl-ev-target 1)\n;; End:\n")
      (hack-local-variables)
      file-local-variables-alist)
    nil)
;; but with enable-local-eval=t it runs (and is recorded)
(let ((enable-local-eval t))
  (qa (progn
        (with-temp-buffer
          (insert "x\n;; Local Variables:\n;; eval: (setq qafl-ev-target 5)\n;; End:\n")
          (hack-local-variables))
        qafl-ev-target)
      5))
;; handle-mode = t returns just the mode symbol
(qa (with-temp-buffer
      (insert "x\n;; Local Variables:\n;; mode: text\n;; End:\n")
      (hack-local-variables t))
    'text-mode)
(qa (with-temp-buffer
      (insert "-*- c -*-\nx\n")
      (hack-local-variables t))
    'c-mode)
(qa (with-temp-buffer
      (insert "x\n;; Local Variables:\n;; fill-column: 50\n;; End:\n")
      (hack-local-variables t))
    nil)
;; handle-mode 'no-mode drops mode entries entirely
(qa (with-temp-buffer
      (insert "x\n;; Local Variables:\n;; mode: text\n;; fill-column: 5\n;; End:\n")
      (hack-local-variables 'no-mode)
      (list major-mode file-local-variables-alist))
    '(fundamental-mode ((fill-column . 5))))
;; duplicate entries: the LAST value wins, one alist entry
(qa (with-temp-buffer
      (insert "x\n;; Local Variables:\n;; fill-column: 5\n;; fill-column: 9\n;; End:\n")
      (hack-local-variables)
      (list fill-column file-local-variables-alist))
    '(9 ((fill-column . 9))))
;; only the FIRST Local Variables: block is read
(qa (with-temp-buffer
      (insert "x\n;; Local Variables:\n;; fill-column: 5\n;; End:\n\n;; Local Variables:\n;; fill-column: 9\n;; End:\n")
      (hack-local-variables)
      fill-column)
    5)
;; a formfeed before the section does not hide it
(qa (with-temp-buffer
      (insert "x\014\n;; Local Variables:\n;; fill-column: 5\n;; End:\n")
      (hack-local-variables)
      fill-column)
    5)
;; lexical-binding from a Local Variables SECTION is ignored entirely
;; (it is only honored on the -*- line).
(qa (with-temp-buffer
      (insert "x\n;; Local Variables:\n;; lexical-binding: t\n;; End:\n")
      (hack-local-variables)
      lexical-binding)
    nil)
;; -*- lexical-binding: t applies buffer-locally
(qa (with-temp-buffer
      (insert "-*- lexical-binding: t -*-\nx\n")
      (hack-local-variables)
      lexical-binding)
    t)

;; == inhibit paths ==
;; enable-local-variables nil: everything ignored EXCEPT the
;; permanently-enabled lexical-binding.
(qa (with-temp-buffer
      (insert "-*- lexical-binding: t -*-\nx\n")
      (let ((enable-local-variables nil))
        (hack-local-variables))
      (list lexical-binding file-local-variables-alist))
    '(t ((lexical-binding . t))))
(qa (with-temp-buffer
      (insert "-*- fill-column: 70 -*-\nx\n")
      (let ((enable-local-variables nil))
        (hack-local-variables))
      (list fill-column file-local-variables-alist))
    '(70 nil))
;; local-enable-local-variables nil blocks all local variable handling
(qa (with-temp-buffer
      (insert "-*- fill-column: 70 -*-\nx\n")
      (let ((local-enable-local-variables nil))
        (hack-local-variables))
      (list fill-column file-local-variables-alist))
    '(70 nil))
;; inhibit-locals argument
(qa (with-temp-buffer
      (insert "-*- fill-column: 70 -*-\nx\n")
      (hack-local-variables nil t)
      file-local-variables-alist)
    nil)
;; inhibit-local-variables-p: buffer/file name match against regexps
(qa (with-temp-buffer (inhibit-local-variables-p)) nil)
;; returns the matching tail of inhibit-local-variables-regexps (cons)
(qa (with-temp-buffer
      (rename-buffer "qafl-xxx.tar" t)
      (insert "x\n;; Local Variables:\n;; fill-column: 5\n;; End:\n")
      (list (consp (inhibit-local-variables-p))
            (hack-local-variables)
            file-local-variables-alist))
    '(t nil nil))
;; in batch, confirmation is a no-op returning nil
(qa (with-temp-buffer
      (hack-local-variables-confirm '((fill-column . 1)) nil nil nil))
    nil)

;; == hooks ==
;; hack-local-variables-hook runs even with an empty alist;
;; before-hack-local-variables-hook only when vars were recorded.
(let ((qafl-h nil) (qafl-h2 nil))
  (let ((before-hack-local-variables-hook (list (lambda () (setq qafl-h t))))
        (hack-local-variables-hook (list (lambda () (setq qafl-h2 t)))))
    (with-temp-buffer (hack-local-variables)))
  (qa (list qafl-h qafl-h2) '(nil t)))
(let ((qafl-h nil) (qafl-h2 nil))
  (let ((before-hack-local-variables-hook (list (lambda () (setq qafl-h t))))
        (hack-local-variables-hook (list (lambda () (setq qafl-h2 t)))))
    (with-temp-buffer
      (insert "-*- fill-column: 5 -*-\n")
      (hack-local-variables)))
  (qa (list qafl-h qafl-h2) '(t t)))

;; == set-auto-mode / set-auto-mode-0 / set-auto-mode-1 ==
;; applies ONLY the mode from the -*- line (fill-column untouched)
(qa (with-temp-buffer
      (insert "-*- mode: c; fill-column: 9 -*-\nx\n")
      (set-auto-mode)
      (list major-mode fill-column))
    '(c-mode 70))
(qa (with-temp-buffer
      (insert "-*- mode: text-mode -*-\ntext\n")
      (set-auto-mode)
      major-mode)
    'fundamental-mode)
(qa (with-temp-buffer
      (insert "-*- mode: emacs-lisp -*-\ntext\n")
      (set-auto-mode)
      major-mode)
    'emacs-lisp-mode)
(qa (with-temp-buffer
      (insert "-*- mode: no-such-mode-xyz -*-\ntext\n")
      (set-auto-mode)
      major-mode)
    'fundamental-mode)
;; mode: in the Local Variables section is honored too
(qa (with-temp-buffer
      (insert "x\n;; Local Variables:\n;; mode: text\n;; End:\n")
      (set-auto-mode)
      major-mode)
    'text-mode)
;; -*- c -*- returns the applied mode symbol
(qa (with-temp-buffer
      (insert "-*- c -*-\nx\n")
      (list (set-auto-mode) major-mode))
    '(c-mode c-mode))
;; keep-mode-if-same returns :keep when the mode would not change
(qa (with-temp-buffer
      (c-mode)
      (insert "-*- c -*-\nx")
      (goto-char (point-min))
      (set-auto-mode t))
    ':keep)
;; shebang / interpreter lookup
(qa (with-temp-buffer
      (insert "#!/usr/bin/env python\nx\n")
      (set-auto-mode)
      major-mode)
    'python-mode)
;; filename lookup via auto-mode-alist
(qa (with-temp-buffer
      (setq buffer-file-name "/tmp/qafl-nonexist.el")
      (set-auto-mode)
      major-mode)
    'emacs-lisp-mode)
(qa (with-temp-buffer
      (setq buffer-file-name "/tmp/qafl-nonexist.txt")
      (set-auto-mode)
      major-mode)
    'text-mode)
(qa (with-temp-buffer
      (setq buffer-file-name "/tmp/qafl-nonexist.tex")
      (set-auto-mode)
      major-mode)
    'latex-mode)
;; set-auto-mode-0 direct: returns applied mode, nil for unknown,
;; :keep when same.
(qa (with-temp-buffer (set-auto-mode-0 'c-mode)) 'c-mode)
(qa (with-temp-buffer (set-auto-mode-0 'no-such-mode-xyz)) nil)
(qa (with-temp-buffer (text-mode) (set-auto-mode-0 'text-mode t)) ':keep)
(qa (with-temp-buffer (text-mode) (set-auto-mode-0 'c-mode t) major-mode)
    'c-mode)
;; set-auto-mode-1 returns the end position of the -*- spec
(qa (with-temp-buffer
      (insert "-*- c -*-\nx\n")
      (goto-char (point-min))
      (integerp (set-auto-mode-1)))
    t)

;; == normal-mode ==
;; returns nil; NOTE: normal-mode hacks file-local vars and THEN runs
;; set-auto-mode, whose mode call kills all (non-permanent) local vars —
;; the -*- fill-column ends up back at the default 70.
(qa (with-temp-buffer
      (insert "-*- mode: text; fill-column: 41 -*-\nx\n")
      (list (normal-mode) major-mode fill-column))
    '(nil text-mode 70))
(qa (with-temp-buffer
      (insert "-*- fill-column: 70 -*-\nx\n")
      (list (normal-mode) major-mode fill-column))
    '(nil fundamental-mode 70))
;; shebang through normal-mode
(qa (with-temp-buffer
      (setq buffer-file-name "/tmp/qafl-nm.pl")
      (insert "#!/usr/bin/perl\n")
      (normal-mode)
      major-mode)
    'perl-mode)
;; nothing found: resets to default (fundamental) mode
(qa (with-temp-buffer
      (c-mode)
      (normal-mode)
      major-mode)
    'fundamental-mode)
;; run-mode-hooks runs change-major-mode-after-body-hook FIRST,
;; then the mode's own hook, then after-change-major-mode-hook.
(qa-lex (with-temp-buffer
          (let ((order nil))
            (add-hook 'text-mode-hook (lambda () (push 'body order)))
            (add-hook 'change-major-mode-after-body-hook (lambda () (push 'after order)))
            (add-hook 'after-change-major-mode-hook (lambda () (push 'post order)))
            (text-mode)
            (nreverse order)))
        '(after body post))
;; buffer-local after-change-major-mode-hook never fires on a mode
;; switch: kill-all-local-variables removes it before hooks run.
(qa (with-temp-buffer
      (let ((flag nil))
        (add-hook 'after-change-major-mode-hook (lambda () (setq flag t)) nil t)
        (c-mode)
        (list flag major-mode)))
    '(nil c-mode))

;; == auto-mode-alist / interpreter / file-name handlers ==
(qa (assoc-default "foo.el" auto-mode-alist 'string-match) 'emacs-lisp-mode)
(qa (assoc-default "foo.txt" auto-mode-alist 'string-match) 'text-mode)
(qa (assoc-default "perl" interpreter-mode-alist 'string-match) 'perl-mode)
(qa (assoc-default "x.gpg" file-name-handler-alist 'string-match)
    'epa-file-handler)
(qa (assoc-default "/:x" file-name-handler-alist 'string-match)
    'file-name-non-special)
(qa (assoc-default "/ssh:h/x" file-name-handler-alist 'string-match)
    'tramp-autoload-file-name-handler)
(qa (length file-name-handler-alist) 4)

;; == dir-locals: class + directory cache ==
;; find-file on a dir with no .dir-locals.el and no cache -> nil
(let ((dir (make-temp-file "qafl-dlnil" t)))
  (unwind-protect
      (qa (dir-locals-find-file (expand-file-name "x.el" dir)) nil)
    (delete-directory dir t)))
(qa-err (dir-locals-set-directory-class "/tmp/qafl-nope/" 'qafl-no-such-class)
        'error)
(dir-locals-set-class-variables 'qafl-dclass '((nil . ((fill-column . 42)))))
(qa (dir-locals-get-class-variables 'qafl-dclass)
    '((nil . ((fill-column . 42)))))
(dir-locals-set-directory-class "/tmp/qafl-clsdir/" 'qafl-dclass)
;; cache entries are (DIR CLASS MTIME); directly-set ones have nil mtime
(qa (car dir-locals-directory-cache) '("/tmp/qafl-clsdir/" qafl-dclass nil))
(qa (dir-locals-find-file "/tmp/qafl-clsdir/sub/y.c")
    '("/tmp/qafl-clsdir/" qafl-dclass nil))
;; a file buffer below the class dir picks up the class variables
(qa (with-temp-buffer
      (setq buffer-file-name "/tmp/qafl-clsdir/x.txt")
      (hack-dir-local-variables)
      (list dir-local-variables-alist file-local-variables-alist))
    '(((fill-column . 42)) ((fill-column . 42))))
;; enable-dir-local-variables nil disables the whole mechanism
(qa (let ((enable-dir-local-variables nil))
      (with-temp-buffer
        (setq buffer-file-name "/tmp/qafl-clsdir/x.txt")
        (hack-dir-local-variables)
        dir-local-variables-alist))
    nil)

;; == dir-locals: .dir-locals.el file on disk ==
(let* ((dir (make-temp-file "qafl-dlrd" t))
       (dlf (expand-file-name ".dir-locals.el" dir)))
  (unwind-protect
      (progn
        (write-region "((nil . ((fill-column . 55))))" nil dlf nil 'quiet)
        ;; before read: dir-locals-find-file returns the directory string
        (qa (dir-locals-find-file (expand-file-name "a.txt" dir))
            (file-name-as-directory dir))
        ;; read-from-dir interns a CLASS named after the directory
        (qa (symbolp (dir-locals-read-from-dir dir)) t)
        ;; afterwards the lookup hits the cache: (DIR CLASS MTIME)
        (let ((r (dir-locals-find-file (expand-file-name "a.txt" dir))))
          (qa (car r) (file-name-as-directory dir))
          (qa (symbolp (nth 1 r)) t)
          (qa (consp (nth 2 r)) t))
        ;; non-file buffer in that directory gets the variables applied
        (qa (with-temp-buffer
              (setq default-directory (file-name-as-directory dir))
              (hack-dir-local-variables-non-file-buffer)
              (list fill-column dir-local-variables-alist
                    file-local-variables-alist))
            '(55 ((fill-column . 55)) ((fill-column . 55)))))
    (delete-directory dir t)))

;; == add/delete-file-local-variable ==
(let ((f (make-temp-file "qafl-alfv" nil ".txt" "hello\n")))
  (unwind-protect
      (qa (let ((b (find-file-noselect f)))
            (unwind-protect
                (with-current-buffer b
                  (add-file-local-variable 'fill-column 77)
                  (save-buffer)
                  (with-temp-buffer (insert-file-contents f) (buffer-string)))
              (kill-buffer b)))
          "hello\n\n;; Local Variables:\n;; fill-column: 77\n;; End:\n")
    (delete-file f)))
(let ((f (make-temp-file "qafl-dflv" nil ".txt"
                       "x\n;; Local Variables:\n;; fill-column: 5\n;; indent-tabs-mode: t\n;; End:\n")))
  (unwind-protect
      (qa (let ((b (find-file-noselect f)))
            (unwind-protect
                (with-current-buffer b
                  (delete-file-local-variable 'fill-column)
                  (save-buffer)
                  (with-temp-buffer (insert-file-contents f) (buffer-string)))
              (kill-buffer b)))
          "x\n;; Local Variables:\n;; indent-tabs-mode: t\n;; End:\n")
    (delete-file f)))
;; prop-line variants rewrite the -*- line instead
(let ((f (make-temp-file "qafl-aflp" nil ".txt" "hello\n")))
  (unwind-protect
      (qa (let ((b (find-file-noselect f)))
            (unwind-protect
                (with-current-buffer b
                  (add-file-local-variable-prop-line 'fill-column 77)
                  (save-buffer)
                  (with-temp-buffer (insert-file-contents f) (buffer-string)))
              (kill-buffer b)))
          ";;; -*- fill-column: 77; -*-\nhello\n")
    (delete-file f)))
(let ((f (make-temp-file "qafl-dflp" nil ".txt"
                       "-*- fill-column: 70; indent-tabs-mode: t -*-\nhi\n")))
  (unwind-protect
      (qa (let ((b (find-file-noselect f)))
            (unwind-protect
                (with-current-buffer b
                  (delete-file-local-variable-prop-line 'fill-column)
                  (buffer-string))
              (kill-buffer b)))
          "-*- indent-tabs-mode: t -*-\nhi\n")
    (delete-file f)))
;; add-file-local-variable with local vars disabled is a silent no-op
(qa (with-temp-buffer
      (let ((enable-local-variables nil))
        (add-file-local-variable 'fill-column 1))
      (buffer-string))
    "")
;; find-file-noselect applies -*- specs when visiting
(let ((f (make-temp-file "qafl-ff" nil ".txt" "-*- fill-column: 71 -*-\nhi\n")))
  (unwind-protect
      (qa (let ((b (find-file-noselect f)))
            (unwind-protect
                (with-current-buffer b
                  (list major-mode fill-column file-local-variables-alist))
              (kill-buffer b)))
          '(text-mode 71 ((fill-column . 71))))
    (delete-file f)))

;; == add/delete-dir-local-variable rewrite .dir-locals.el ==
(let* ((dir (make-temp-file "qafl-adlv" t))
       (dlf (expand-file-name ".dir-locals.el" dir)))
  (unwind-protect
      (progn
        (add-dir-local-variable 'emacs-lisp-mode 'fill-column 66 dlf)
        (let ((b (find-buffer-visiting dlf)))
          (unwind-protect
              (qa (progn
                    (with-current-buffer b (save-buffer))
                    (with-temp-buffer
                      (insert-file-contents dlf)
                      (goto-char (point-min))
                      (read (current-buffer))))
                  '((emacs-lisp-mode (fill-column . 66))))
            (kill-buffer b))))
    (delete-directory dir t)))
(let* ((dir (make-temp-file "qafl-ddlv" t))
       (dlf (expand-file-name ".dir-locals.el" dir)))
  (unwind-protect
      (progn
        (write-region "((c-mode . ((fill-column . 55) (indent-tabs-mode . t))))"
                      nil dlf nil 'quiet)
        (delete-dir-local-variable 'c-mode 'fill-column dlf)
        (let ((b (find-buffer-visiting dlf)))
          (unwind-protect
              (qa (progn
                    (with-current-buffer b (save-buffer))
                    (with-temp-buffer
                      (insert-file-contents dlf)
                      (goto-char (point-min))
                      (read (current-buffer))))
                  '((c-mode (indent-tabs-mode . t))))
            (kill-buffer b))))
    (delete-directory dir t)))
;; copy-file-locals-to-dir-locals writes buffer's file-local vars under
;; the buffer's major-mode key
(let ((dir (make-temp-file "qafl-cfl" t)))
  (unwind-protect
      (qa (with-temp-buffer
            (setq file-local-variables-alist '((fill-column . 51)))
            (let ((major-mode 'text-mode))
              (copy-file-locals-to-dir-locals
               (expand-file-name ".dir-locals.el" dir))
              (let ((b (find-buffer-visiting
                        (expand-file-name ".dir-locals.el" dir))))
                (unwind-protect
                    (progn
                      (with-current-buffer b (save-buffer))
                      (with-temp-buffer
                        (insert-file-contents
                         (expand-file-name ".dir-locals.el" dir))
                        (goto-char (point-min))
                        (read (current-buffer))))
                  (kill-buffer b)))))
          '((text-mode (fill-column . 51))))
    (delete-directory dir t)))

(princ (qa--done))
