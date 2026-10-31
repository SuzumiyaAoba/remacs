;;; -*- lexical-binding: t -*-
;; macOS GUI regression for #3334. Run with an external process-group watchdog.
;; Use this as init.el in a dedicated --init-directory. The watchdog must set
;; REMACS_NATIVE_TEST_RESULT and saved integer pointer coordinates in
;; REMACS_NATIVE_TEST_POINTER_X/Y, and require the result file to contain "ok".
;; Restore the saved native pointer position externally if the process fails.
(let ((result-file (getenv "REMACS_NATIVE_TEST_RESULT"))
      (old-x (getenv "REMACS_NATIVE_TEST_POINTER_X"))
      (old-y (getenv "REMACS_NATIVE_TEST_POINTER_Y")))
  (condition-case error-data
      (progn
        (unless (and result-file old-x old-y)
          (error "Native pointer regression requires result and saved coordinates"))
        (unwind-protect
            (progn
              (let ((frame (make-frame '((name . "qa3334-pointer")
                                         (width . 40) (height . 12)
                                         (no-focus-on-map . t)))))
                (set-mouse-position frame 0 0)
                (set-mouse-pixel-position frame 17 23))
              (let ((frame (make-frame '((name . "qa3334-absolute")
                                         (width . 40) (height . 12)
                                         (no-focus-on-map . t)))))
                (select-frame frame)
                (set-mouse-absolute-pixel-position
                 (string-to-number old-x) (string-to-number old-y))))
          (ns-set-mouse-absolute-pixel-position
           (string-to-number old-x) (string-to-number old-y)))
        (write-region "ok" nil result-file nil 'silent)
        (kill-emacs 0))
    (error
     (when result-file
       (write-region (error-message-string error-data) nil result-file nil 'silent))
     (kill-emacs 1))))
