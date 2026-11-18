;; Events / mouse / command vars — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - event-* / posn-* are fns; mouse fns exist in batch.
;;   - event-basic-type on C-a → 97 (the base char).
;;   - event-modifiers on C-a → '(control).
;;   - event-convert-list returns an event integer.
;;   - command vars are bound; single-command-keys/raw-keys are NOT bound.
;;   - Hooks (pre-command-hook etc.) are bound vars.
;;   - recent-keys exists as a var.
;;   - unread-command-events exists.

;; == event fns ==
(qa (fboundp 'event-start) t)
(qa (fboundp 'event-end) t)
(qa (fboundp 'event-basic-type) t)
(qa (fboundp 'event-modifiers) t)
(qa (fboundp 'event-convert-list) t)

;; == posn-* (mouse position accessors) ==
(qa (fboundp 'posn-window) t)
(qa (fboundp 'posn-point) t)
(qa (fboundp 'posn-col-row) t)
(qa (fboundp 'posn-actual-col-row) t)
(qa (fboundp 'posn-image) t)
(qa (fboundp 'posn-object) t)
(qa (fboundp 'posn-string) t)
(qa (fboundp 'posn-area) t)
(qa (fboundp 'posn-x-y) t)
(qa (fboundp 'posn-object-x-y) t)
(qa (fboundp 'posn-object-width-height) t)
(qa (fboundp 'posn-timestamp) t)

;; == mouse fns ==
(qa (fboundp 'mouse-pixel-position) t)
(qa (fboundp 'mouse-position) t)
(qa (fboundp 'set-mouse-position) t)
(qa (fboundp 'set-mouse-pixel-position) t)
(qa (fboundp 'track-mouse) t)

;; == command vars ==
(qa (boundp 'this-command) t)
(qa (boundp 'last-command) t)
(qa (boundp 'last-command-event) t)
(qa (boundp 'last-nonmenu-event) t)
(qa (boundp 'last-input-event) t)
(qa (boundp 'last-event-frame) t)
(qa (boundp 'real-last-command) t)
(qa (boundp 'this-single-command-keys) nil)
(qa (boundp 'this-single-command-raw-keys) nil)
(qa (boundp 'this-command-keys) nil)
(qa (boundp 'this-command-keys-vector) nil)
(qa (boundp 'recent-keys) nil)
(qa (boundp 'unread-command-events) t)
(qa (boundp 'unread-input-method-events) t)
(qa (boundp 'unread-post-input-method-events) t)
(qa (boundp 'keyboard-coding-system) t)
(qa (boundp 'keyboard-translate-table) t)
(qa (boundp 'num-input-keys) t)
(qa (boundp 'num-nonmacro-input-events) t)

;; The internal C setter changes terminal state; the Lisp wrapper sets the
;; defcustom afterward.  A direct internal call must not mutate that variable
;; or notify its watchers.
(defvar qa-input-keyboard-coding-events nil)
(defun qa-input-keyboard-coding-watcher (_symbol value operation _where)
  (push (list operation value) qa-input-keyboard-coding-events))
(qa
 (let ((old-coding (keyboard-coding-system))
       (keyboard-coding-system 'no-conversion)
       (qa-input-keyboard-coding-events nil))
   (add-variable-watcher 'keyboard-coding-system
                         #'qa-input-keyboard-coding-watcher)
   (unwind-protect
       (progn
         (setq qa-input-keyboard-coding-events nil)
         (set-keyboard-coding-system-internal 'utf-8 nil)
         (list qa-input-keyboard-coding-events
               keyboard-coding-system
               (keyboard-coding-system)))
     (remove-variable-watcher 'keyboard-coding-system
                              #'qa-input-keyboard-coding-watcher)
     (set-keyboard-coding-system-internal old-coding nil)))
 '(nil no-conversion utf-8))

;; The Lisp wrapper sets the defcustom after the internal terminal update,
;; so its watcher must run exactly once.
(qa
 (let ((old-coding (keyboard-coding-system))
       (old-variable keyboard-coding-system)
       (qa-input-keyboard-coding-events nil))
   (add-variable-watcher 'keyboard-coding-system
                         #'qa-input-keyboard-coding-watcher)
   (unwind-protect
       (progn
         (set-keyboard-coding-system 'utf-8)
         (list qa-input-keyboard-coding-events
               keyboard-coding-system
               (keyboard-coding-system)))
     (remove-variable-watcher 'keyboard-coding-system
                              #'qa-input-keyboard-coding-watcher)
     (setq keyboard-coding-system old-variable)
     (set-keyboard-coding-system-internal old-coding nil)))
 '(((set utf-8-unix)) utf-8-unix utf-8-unix))

;; == hooks (bound vars) ==
(qa (boundp 'pre-command-hook) t)
(qa (boundp 'post-command-hook) t)
(qa (boundp 'post-gc-hook) t)
(qa (boundp 'before-init-hook) t)
(qa (boundp 'after-init-hook) t)
(qa (boundp 'emacs-startup-hook) t)
(qa (boundp 'term-setup-hook) t)
(qa (boundp 'window-setup-hook) t)
(qa (boundp 'auto-save-hook) t)
(qa (boundp 'suspend-hook) t)
(qa (boundp 'suspend-resume-hook) t)
(qa (boundp 'kill-emacs-hook) t)
(qa (boundp 'kill-buffer-hook) t)
(qa (boundp 'change-major-mode-hook) t)
(qa (boundp 'first-change-hook) t)
(qa (boundp 'buffer-list-update-hook) t)

;; == event-modifiers / event-basic-type ==
;; on a real key event
(qa (event-modifiers (aref [?\C-a] 0)) '(control))
(qa (event-basic-type (aref [?\C-a] 0)) 97)
;; on a symbol event
(qa (event-modifiers 'control) nil)
(qa (event-basic-type 'control) 'control)
;; M-x has meta modifier
(qa (event-modifiers (aref [?\M-x] 0)) '(meta))

;; == event-convert-list ==
;; (control ?a) → C-a char (1)
(qa (event-convert-list '(control ?a)) 1)
;; (meta ?x) → M-x char
(qa (event-convert-list '(meta ?x)) ?\M-x)

;; == this-command-keys in batch ==
(qa (this-command-keys) "")
(qa (this-command-keys-vector) [])

;; `internal-auto-fill' errors abort self-insert before post-self-insert-hook.
(qa
 (let ((qa-input-post-self-insert-ran nil))
   (with-temp-buffer
     (let ((auto-fill-function
            (lambda () (signal 'file-error '("auto-fill failed"))))
           (post-self-insert-hook
            (list (lambda () (setq qa-input-post-self-insert-ran t)))))
       (list (condition-case error
                 (self-insert-command 1 ?\s)
               (error (car error)))
             (buffer-string)
             qa-input-post-self-insert-ran))))
 '(file-error " " nil))

;; `yank-pop' is a Lisp sequence: watched `setq's and the final window update
;; must propagate failures instead of continuing with the replacement.
(defun qa-input-yank-pop-set-watcher (symbol _value operation _where)
  (when (and (eq operation 'set)
             (memq symbol '(this-command yank-undo-function)))
    (signal 'file-error '("yank-pop watcher"))))

(qa
 (with-temp-buffer
   (let ((last-command 'yank)
         (this-command 'qa-input-original-command))
     (add-variable-watcher 'this-command #'qa-input-yank-pop-set-watcher)
     (unwind-protect
         (list (condition-case error (yank-pop) (error (car error)))
               (buffer-string)
               this-command)
       (remove-variable-watcher 'this-command #'qa-input-yank-pop-set-watcher))))
 '(file-error "" qa-input-original-command))

(qa
 (with-temp-buffer
   (insert "abc")
   (set-mark 1)
   (goto-char (point-max))
   (let ((last-command 'yank)
         (kill-ring '("x"))
         (yank-window-start 1)
         (qa-input-yank-undo-ran nil)
         (yank-undo-function
          (lambda (_start _end) (setq qa-input-yank-undo-ran t))))
     (add-variable-watcher 'yank-undo-function #'qa-input-yank-pop-set-watcher)
     (unwind-protect
         (list (condition-case error (yank-pop) (error (car error)))
               (buffer-string)
               qa-input-yank-undo-ran
               (functionp yank-undo-function))
       (remove-variable-watcher 'yank-undo-function #'qa-input-yank-pop-set-watcher))))
 '(file-error "abc" t t))

(qa
 (with-temp-buffer
   (insert "abc")
   (set-mark 1)
   (goto-char (point-max))
   (let ((last-command 'yank)
         (kill-ring '("x"))
         (yank-window-start "bad")
         (yank-undo-function (lambda (_start _end) nil)))
     (list (condition-case error
               (yank-pop)
             (error (list (car error) (cadr error))))
           (buffer-string))))
 '((wrong-type-argument integer-or-marker-p) "abcx"))

(princ (qa--done))
