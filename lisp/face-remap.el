;;; face-remap.el --- face remapping -*- lexical-binding: t -*-
;; The face-remap API (face-remap-add-relative, face-remap-set-base,
;; text-scale-*) is defined in the prelude, matching GNU's dumped
;; state where the functions are bound but the feature is unprovided.
;; Loading this file restores their registrations and provides the feature,
;; like GNU's (require 'face-remap).

;;; Code:

;; ;Remacs: Startup removes the folded define-minor-mode registrations.
;; Restore the same lighter forms when autoload or require loads the library.
(add-minor-mode 'text-scale-mode '(" " text-scale-mode-lighter))
(add-minor-mode 'buffer-face-mode " BufFace")

;;Remacs: GNU makes these settings buffer-local when face-remap loads.
(defvar text-scale-mode-amount 0
  "Number of steps that `text-scale-mode' will increase/decrease text height.")
(defvar text-scale-remap-header-line nil
  "If non-nil, text scaling may change font size of header lines too.")
(make-variable-buffer-local 'text-scale-mode-amount)
(make-variable-buffer-local 'text-scale-remap-header-line)

(provide 'face-remap)

;;; face-remap.el ends here
