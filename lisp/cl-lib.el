;;; cl-lib.el --- feature shim for GNU cl-lib -*- lexical-binding: t; -*-

;; remacs: the cl-* definitions (cl-defun, cl-loop, cl-defgeneric, ...)
;; live in the embedded prelude rather than in cl-lib.el/cl-macs.el,
;; so this file only needs to provide the cl-lib feature for GNU
;; libraries that (require 'cl-lib).

;;; Code:

;; remacs: `cl-loaddefs' calls `set-advertised-calling-convention',
;; which is still a stub; install the fallbacks first so requiring
;; cl-lib cannot abort half-way.
(require 'remacs-compat)

;; GNU binds these from the dumped cl-macs/cl-seq .elc files rather
;; than via cl-loaddefs; give them autoload stubs so `fboundp' and the
;; first call both behave like GNU.  GNU's internal helpers
;; (cl--check-*, cl--position, ...) stay unbound, matching GNU's
;; startup state.
(unless (fboundp 'cl-subst)
  (fset 'cl-subst
        '(autoload "cl-seq" "Substitute NEW for OLD everywhere in TREE.

Keywords supported:  :test :test-not :key

(fn NEW OLD TREE [KEYWORD VALUE]...)" nil nil)))
(unless (fboundp 'cl--do-subst)
  (fset 'cl--do-subst
        '(autoload "cl-seq" nil nil nil)))

;; GNU's cl-lib.elc also carries the non-autoloaded cl-* definitions.
;; Our dump-time copies are hidden for -Q parity; restore them here so
;; `(require 'cl-lib)' exposes the same public function cells.
(dolist (sym '(cl--block-throw cl--block-wrapper cl--compiling-file
               cl--defalias cl--old-struct-type-of
               cl--set-buffer-substring cl--set-substring
               cl-acons cl-adjoin cl-caaaar cl-caaadr cl-caaar cl-caadar
               cl-caaddr cl-caadr cl-cadaar cl-cadadr cl-cadar cl-caddar
               cl-cadddr cl-caddr cl-cdaaar cl-cdaadr cl-cdaar cl-cdadar
               cl-cdaddr cl-cdadr cl-cddaar cl-cddadr cl-cddar cl-cdddar
               cl-cddddr cl-cdddr cl-constantly cl-copy-list cl-copy-seq
               cl-decf cl-declaim cl-digit-char-p cl-eighth cl-evenp
               cl-fifth cl-first cl-floatp-safe cl-fourth cl-ldiff
               cl-list* cl-mapcar cl-minusp cl-multiple-value-apply
               cl-multiple-value-call cl-multiple-value-list cl-ninth
               cl-nth-value cl-oddp cl-pairlis cl-plusp cl-proclaim
               cl-pushnew cl-rest cl-second cl-seventh cl-sixth cl-svref
               cl-tenth cl-third cl-values cl-values-list))
  (unless (fboundp sym)
    (let ((def (get sym 'remacs--dump-fn)))
      (when def (fset sym def)))))

;; These are aliases in GNU, not independent subrs/lambdas.
(defalias 'cl-evenp #'evenp)
(defalias 'cl-oddp #'oddp)
(defalias 'cl-minusp #'minusp)
(defalias 'cl-plusp #'plusp)
(defalias 'cl-floatp-safe #'floatp)

(provide 'cl-lib)
(unless (load "cl-loaddefs" 'noerror 'quiet)
  ;; When bootstrapping, cl-loaddefs hasn't been built yet!
  (require 'cl-macs)
  (require 'cl-seq))

;;; cl-lib.el ends here
