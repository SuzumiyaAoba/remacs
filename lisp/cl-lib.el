;;; cl-lib.el --- feature shim for GNU cl-lib -*- lexical-binding: t; -*-

;; remacs: the cl-* definitions (cl-defun, cl-loop, cl-seq functions,
;; cl-defgeneric, ...) live in the embedded prelude rather than in
;; cl-lib.el/cl-macs.el/cl-seq.el, so this file only needs to provide
;; the `cl-lib' feature for GNU libraries that (require 'cl-lib).

;;; Code:

(provide 'cl-lib)

;;; cl-lib.el ends here
