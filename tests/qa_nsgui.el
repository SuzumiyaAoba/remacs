;; GNU NS entry points in a batch session.
;; Headless-capable dock/selection predicates return nil; GUI-only calls
;; report the missing window system before validating their arguments.

(qa (ns-badge "a") nil)
(qa (ns-badge "~") nil)
(qa (ns-request-user-attention 'a) nil)
(qa (ns-request-user-attention '~) nil)
(qa (ns-selection-exists-p "a") nil)

(qa-err-data (ns-own-selection-internal "a" "a\\b")
             'error '("Window system is not in use or not initialized"))
(qa-err-data (ns-disown-selection-internal "a")
             'error '("Window system is not in use or not initialized"))
(qa-err-data (ns-get-selection "a" "a\\b")
             'error '("Window system is not in use or not initialized"))
(qa-err-data (ns-selection-owner-p "a")
             'error '("Window system is not in use or not initialized"))
(qa-err-data (ns-popup-color-panel "a")
             'error '("Window system is not in use or not initialized"))
(qa-err-data (ns-read-file-name 'bad-prompt)
             'error '("Window system is not in use or not initialized"))

(princ (qa--done))
