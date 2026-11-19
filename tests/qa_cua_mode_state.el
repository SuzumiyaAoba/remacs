;;; -*- lexical-binding: nil; -*-

(require 'cua-base)

(qa (let ((transient-mark-mode t)
          (cua-highlight-region-shift-only t)
          (cua-delete-selection nil)
          (cua-delete-copy-to-register-0 nil))
      (unwind-protect
          (progn
            (cua-mode 1)
            (let ((during-enable transient-mark-mode))
              (cua-mode -1)
              (list during-enable transient-mark-mode)))
        (when cua-mode
          (cua-mode -1))))
    '(nil t))

(princ (qa--done))
