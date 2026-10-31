;;; -*- lexical-binding: t -*-
;; macOS GUI regression for #3339. Run with an external process-group watchdog.
;; Use this as init.el in a dedicated --init-directory.
;; The watchdog must set REMACS_NATIVE_TEST_RESULT and require its value "ok".
(let ((result-file (getenv "REMACS_NATIVE_TEST_RESULT")))
  (condition-case error-data
      (let ((frame (make-frame)))
        (select-frame frame)
        (ns-popup-color-panel frame)
        (when result-file
          (write-region "ok" nil result-file nil 'silent))
        (kill-emacs 0))
    (error
     (when result-file
       (write-region (error-message-string error-data) nil result-file nil 'silent))
     (kill-emacs 1))))
