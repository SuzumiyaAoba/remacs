;;; cl-lib.el --- feature shim for GNU cl-lib -*- lexical-binding: t; -*-

;; remacs: the cl-* definitions (cl-defun, cl-loop, cl-defgeneric, ...)
;; live in the embedded prelude rather than in cl-lib.el/cl-macs.el,
;; so this file only needs to provide the cl-lib feature for GNU
;; libraries that (require 'cl-lib).

;;; Code:

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

(provide 'cl-lib)
(unless (load "cl-loaddefs" 'noerror 'quiet)
  ;; When bootstrapping, cl-loaddefs hasn't been built yet!
  (require 'cl-macs)
  (require 'cl-seq))

;;; cl-lib.el ends here
