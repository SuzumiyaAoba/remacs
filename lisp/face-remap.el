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

(provide 'face-remap)

;;; face-remap.el ends here
