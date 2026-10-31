;;; -*- lexical-binding: t -*-
;; macOS GUI regression for #3340. Run with an external process-group watchdog.
;; Use this as init.el in a dedicated --init-directory.
;; REMACS_NATIVE_TEST_RESULT optionally names a result file for the watchdog.
(let ((result-file (getenv "REMACS_NATIVE_TEST_RESULT")))
  (condition-case error-data
      (let ((frame (make-frame)))
        (select-frame frame)
        (ns-emacs-info-panel)
        (when result-file
          (write-region "ok" nil result-file nil 'silent))
        (kill-emacs 0))
    (error
     (when result-file
       (write-region (error-message-string error-data) nil result-file nil 'silent))
     (kill-emacs 1))))
