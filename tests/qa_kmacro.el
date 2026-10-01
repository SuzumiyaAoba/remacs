;; kmacro.el — GNU 31.1 pins (verified).
;;
;; remacs's `lisp/kmacro.el' is the full GNU port; `kmacro' itself is a
;; primitive subr delegating to `kmacro--impl' (like GNU, where the
;; byte-compiled defun sits in a subr slot).

(require 'kmacro)

;; == defined in remacs's subset ==
(qa (fboundp 'kmacro-start-macro) t)
(qa (fboundp 'kmacro-end-macro) t)
(qa (fboundp 'kmacro-start-macro-or-insert-counter) t)
(qa (fboundp 'kmacro-end-or-call-macro) t)
(qa (fboundp 'kmacro-end-or-call-macro-repeat) t)
(qa (fboundp 'kmacro-name-last-macro) t)

;; == kmacro itself is a subr in GNU 31.1 ==
(qa (subrp (symbol-function 'kmacro)) t)

;; == kmacro API surface ==
(qa (fboundp 'kmacro-p) t)
(qa (fboundp 'kmacro-ring-empty-p) t)
(qa (fboundp 'kmacro-call-macro) t)
(qa (fboundp 'kmacro-add-counter) t)
(qa (fboundp 'kmacro-set-counter) t)
(qa (fboundp 'kmacro-set-format) t)
(qa (fboundp 'kmacro-insert-counter) t)
(qa (fboundp 'kmacro-exec-ring-item) t)
(qa (fboundp 'kmacro-bind-to-key) t)
(qa (fboundp 'kmacro-step-edit-macro) t)
(qa (fboundp 'kmacro-cycle-ring-previous) t)
(qa (fboundp 'kmacro-cycle-ring-next) t)
(qa (fboundp 'kmacro-delete-ring-head) t)
(qa (fboundp 'kmacro-edit-macro) t)
(qa (fboundp 'kmacro-edit-lossage) t)
(qa (fboundp 'kmacro-view-macro) t)
(qa (fboundp 'kmacro-to-register) t)
(qa (fboundp 'kmacro-call-ring-2nd) t)
(qa (fboundp 'kmacro-call-ring-2nd-repeat) t)
(qa (fboundp 'kmacro-view-macro-repeat) t)
(qa (fboundp 'kmacro-edit-macro-repeat) t)
(qa (fboundp 'apply-macro-to-region-lines) t)

;; == kmacro-start-macro / end-macro run through start-kbd-macro ==
(qa (fboundp 'start-kbd-macro) t)
(qa (fboundp 'end-kbd-macro) t)
(qa (fboundp 'call-last-kbd-macro) t)

;; == counter / ring variables exist ==
(qa (boundp 'kmacro-counter) t)
(qa (boundp 'kmacro-counter-value-start) t)
(qa (boundp 'kmacro-counter-format) t)
(qa (boundp 'kmacro-ring) t)
(qa (boundp 'kmacro-ring-max) t)
;; == kmacro-p rejects non-macros ==
(qa (kmacro-p 42) nil)
(qa (kmacro-p 'sym) nil)
(qa (kmacro-p nil) nil)
(qa (kmacro-p '()) nil)

;; == kmacro-exec-ring-item is an OBSOLETE alias for funcall ==
;; GNU: (define-obsolete-function-alias 'kmacro-exec-ring-item #'funcall)
(qa (eq (symbol-function 'kmacro-exec-ring-item) #'funcall) t)

;; == kmacro-keymap exists (autoloaded keymap var) ==
(qa (fboundp 'kmacro-keymap) t)

;; == kmacro-start-macro / -end-macro — calling starts/stops recording ==
;; We can't fully exercise them in batch (no event loop), but the
;; commands are bound and callable.
(qa (commandp 'kmacro-start-macro) t)
(qa (commandp 'kmacro-end-macro) t)
(qa (commandp 'kmacro-end-or-call-macro) t)

(princ (qa--done))
