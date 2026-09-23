;; winner window-configuration undo/redo, GNU-compatible.
;; Self-asserting: any mismatch signals `error'.

;; ---------- startup/dump availability (GNU: autoload cookie) ----------
(cl-assert (autoloadp (symbol-function 'winner-mode)))
(cl-assert (not (featurep 'winner)))
(require 'winner)
(cl-assert (featurep 'winner))
(cl-assert (fboundp 'winner-mode))
(cl-assert (fboundp 'winner-undo))
(cl-assert (fboundp 'winner-redo))
(cl-assert (fboundp 'winner-set))
(cl-assert (fboundp 'winner-conf))
(cl-assert (fboundp 'winner-ring))
(cl-assert (fboundp 'winner-sorted-window-list))
(cl-assert (fboundp 'winner-win-data))
(cl-assert (fboundp 'winner-change-fun))
(cl-assert (fboundp 'winner-save-old-configurations))
(cl-assert (fboundp 'winner-insert-if-new))

;; ---------- defcustoms / defvars ----------
(cl-assert (eq winner-ring-size 200))
(cl-assert (equal winner-boring-buffers '("*Completions*")))
(cl-assert (null winner-boring-buffers-regexp))
(cl-assert (null winner-dont-bind-my-keys))
(cl-assert (null winner-currents))
(cl-assert (null winner-ring-alist))
(cl-assert (null winner-mode-hook))
(cl-assert (null winner-mode-off-hook))
(cl-assert (null select-enable-primary))

;; ---------- keymaps (GNU defaults: C-c <left>/<right>) ----------
(cl-assert (keymapp winner-mode-map))
(cl-assert (eq (lookup-key winner-mode-map (kbd "C-c <left>")) 'winner-undo))
(cl-assert (eq (lookup-key winner-mode-map (kbd "C-c <right>")) 'winner-redo))
;; repeat-mode properties from (defvar-keymap ... :repeat t)
(cl-assert (eq (get 'winner-undo 'repeat-map) 'winner-repeat-map))
(cl-assert (eq (get 'winner-redo 'repeat-map) 'winner-repeat-map))
(cl-assert (keymapp winner-repeat-map))
(cl-assert (eq (lookup-key winner-repeat-map (kbd "<left>")) 'winner-undo))
(cl-assert (eq (lookup-key winner-repeat-map (kbd "<right>")) 'winner-redo))

;; winner-dont-bind-my-keys toggles via its custom :set.
(setopt winner-dont-bind-my-keys t)
(cl-assert (null (lookup-key winner-mode-map (kbd "C-c <left>"))))
(cl-assert (null (lookup-key winner-mode-map (kbd "C-c <right>"))))
(setopt winner-dont-bind-my-keys nil)
(cl-assert (eq (lookup-key winner-mode-map (kbd "C-c <left>")) 'winner-undo))

;; ---------- winner-undo errors when mode is off ----------
(cl-assert (eq (condition-case e (progn (winner-undo) nil)
                 (error (car e)))
               'error))

;; ---------- mode toggling wires/unwires hooks ----------
;; `post-command-hook' is a per-buffer (DEFVAR_PER_BUFFER) variable in
;; GNU: `add-hook' without LOCAL writes the default binding, so the
;; membership check goes through `default-value'.
(cl-assert (local-variable-p 'post-command-hook))
(cl-assert (local-variable-p 'pre-command-hook))
(winner-mode 1)
(cl-assert winner-mode)
(cl-assert (memq 'winner-change-fun window-configuration-change-hook))
(cl-assert (memq 'winner-save-old-configurations
                 (default-value 'post-command-hook)))
(cl-assert (memq 'winner-save-unconditionally minibuffer-setup-hook))
;; enabling snapshots all live frames into winner-modified-list then
;; drains it through winner-save-old-configurations
(cl-assert (null winner-modified-list))
;; the initial configuration was pushed to the frame's ring
(let ((ring (cdr (assq (selected-frame) winner-ring-alist))))
  (cl-assert (ring-p ring))
  (cl-assert (= (ring-length ring) 1)))

;; ---------- simulated command loop (batch has no real one) ----------
(defun probe-winner--step (cmd)
  (setq this-command cmd)
  (winner-change-fun)
  (winner-save-old-configurations)
  (setq last-command cmd))

;; C1: remember the 1-window configuration.
(probe-winner--step 'c1)
;; C2: split -> ring records the OLD (1-window) conf, currents updates.
(split-window)
(probe-winner--step 'c2)
;; C3: split the selected window below.
(split-window nil nil 'below)
(probe-winner--step 'c3)
(cl-assert (= (length (window-list)) 3))
(let ((ring (cdr (assq (selected-frame) winner-ring-alist))))
  ;; ring holds [conf-at-3wins, conf-at-2wins] -- newest first.
  (cl-assert (= (ring-length ring) 2))
  (cl-assert (= (length (cdr (ring-ref ring 0))) 2))
  (cl-assert (= (length (cdr (ring-ref ring 1))) 1)))

;; ---------- undo chain ----------
;; A real command loop sets `this-command' before each command and
;; `last-command' after; simulate that for interactive commands.
(defun probe-winner--cmd (cmd fn)
  (setq this-command cmd)
  (funcall fn)
  (setq last-command cmd))
;; first undo goes back to the 2-window configuration
(probe-winner--cmd 'winner-undo #'winner-undo)
(cl-assert (= (length (window-list)) 2))
(cl-assert (= winner-undo-counter 1))
;; repeated undos walk further back
(probe-winner--cmd 'winner-undo #'winner-undo)
(cl-assert (= (length (window-list)) 1))
;; ring exhausted -> "No further" (message only, stays put)
(probe-winner--cmd 'winner-undo #'winner-undo)
(cl-assert (= (length (window-list)) 1))
;; redo restores the configuration as of the undo sequence start
(probe-winner--cmd 'winner-redo #'winner-redo)
(cl-assert (= (length (window-list)) 3))
;; redo without a preceding winner-undo signals user-error
(setq last-command 'something-else)
(cl-assert (eq (condition-case e (progn (winner-redo) nil)
                 (user-error (car e)))
               'user-error))

;; ---------- boring buffers are not restored ----------
(let ((scratch (current-buffer)))
  (delete-other-windows)
  (get-buffer-create "*Completions*")
  (probe-winner--step 'b1)
  (split-window)
  (set-window-buffer (next-window) "*Completions*")
  (probe-winner--step 'b2)
  (probe-winner--cmd 'winner-undo #'winner-undo)
  ;; the *Completions* window is dropped on restore
  (cl-assert (not (memq "*Completions*"
                       (mapcar (lambda (w)
                                 (buffer-name (window-buffer w)))
                               (window-list)))))
  (cl-assert (eq (window-buffer (selected-window)) scratch)))

;; ---------- point/mark preservation across undo ----------
;; GNU restores the pre-split configuration (1 window, *scratch* at
;; point 1): `winner-point-alist' keeps the current points so undo does
;; not resurrect stale positions.
(let ((b (get-buffer-create "wp")))
  (delete-other-windows)
  (with-current-buffer b
    (insert "abcdef")
    (goto-char 3))
  (probe-winner--step 'p1)
  (set-window-buffer (selected-window) b)
  (set-window-point (selected-window) 5)
  (split-window)
  (probe-winner--step 'p2)
  (probe-winner--cmd 'winner-undo #'winner-undo)
  (cl-assert (= (window-point (selected-window)) 1))
  (cl-assert (eq (window-buffer (selected-window))
                 (get-buffer "*scratch*")))
  (cl-assert (= (length (window-list)) 1)))

;; ---------- mode off removes hooks ----------
(winner-mode -1)
(cl-assert (not winner-mode))
(cl-assert (not (memq 'winner-change-fun window-configuration-change-hook)))
(cl-assert (not (memq 'winner-save-old-configurations
                     (default-value 'post-command-hook))))
(cl-assert (not (memq 'winner-save-unconditionally minibuffer-setup-hook)))

;; ---------- real window configurations ----------
;; identity: GNU reuses the same window objects on restore.
(delete-other-windows)
(let* ((c (current-window-configuration))
       (w1 (selected-window)))
  (split-window)
  (let ((w2 (next-window)))
    (set-window-configuration c)
    (cl-assert (eq w1 (selected-window)))
    ;; GNU kills windows the configuration does not contain
    (cl-assert (not (window-live-p w2)))
    (cl-assert (= (length (window-list)) 1))
    ;; restored window is the same object
    (split-window)
    (set-window-configuration c)
    (cl-assert (eq w1 (selected-window)))))
;; restoring a 2-window config over 1 window revives the saved object
(delete-other-windows)
(let* ((w1 (selected-window)))
  (split-window)
  (let* ((c2 (current-window-configuration))
         (w2 (next-window)))
    (delete-other-windows)
    (cl-assert (not (window-live-p w2)))
    (set-window-configuration c2)
    (cl-assert (window-live-p w2))
    (cl-assert (= (length (window-list)) 2))
    (cl-assert (eq (next-window) w2))))
;; window-configuration-equal-p / window-configuration-frame
(let ((c1 (current-window-configuration))
      (c2 (current-window-configuration)))
  (cl-assert (window-configuration-equal-p c1 c2))
  (cl-assert (eq (window-configuration-frame c1) (selected-frame)))
  (split-window)
  (let ((c3 (current-window-configuration)))
    (cl-assert (not (window-configuration-equal-p c1 c3)))))
;; type checks: non-configuration arguments signal
;; wrong-type-argument window-configuration-p on either side
(cl-assert (eq (condition-case e (window-configuration-equal-p 1 2)
                 (wrong-type-argument (car e)))
               'wrong-type-argument))
(cl-assert (eq (condition-case e
                   (window-configuration-equal-p 'x
                                                 (current-window-configuration))
                 (wrong-type-argument (car e)))
               'wrong-type-argument))
;; restore keeps the buffer/point association per window
(let ((b (get-buffer-create "wc1")))
  (with-current-buffer b (insert "0123456789"))
  (set-window-buffer (selected-window) b)
  (set-window-point (selected-window) 7)
  (let ((c (current-window-configuration)))
    (goto-char (point-min))
    (set-window-configuration c)
    (cl-assert (eq (window-buffer (selected-window)) b))
    (cl-assert (= (window-point (selected-window)) 7))
    (cl-assert (eq (current-buffer) b))))

;; ---------- cl-letf generalized places (needed by winner-set-conf) ----------
(let ((w (selected-window)))
  ;; bare (PLACE) spec: save + restore only
  (let ((saved (window-point w)))
    (cl-letf (((window-point w)))
      (set-window-point w (point-min)))
    (cl-assert (= (window-point w) saved)))
  ;; valued spec: set + restore
  (let ((x (list 1 2)))
    (cl-letf (((car x) 'tmp))
      (cl-assert (eq (car x) 'tmp)))
    (cl-assert (eq (car x) 1))))
;; symbol-function place restores unbound-ness
(cl-assert (not (fboundp 'probe-winner--undef-fn)))
(cl-letf (((symbol-function 'probe-winner--undef-fn) (lambda () 9)))
  (cl-assert (= (funcall 'probe-winner--undef-fn) 9)))
(cl-assert (not (fboundp 'probe-winner--undef-fn)))
;; unbound variable spec restores unbound-ness
(cl-letf ((probe-winner--undef-var 5))
  (cl-assert (= probe-winner--undef-var 5)))
(cl-assert (not (boundp 'probe-winner--undef-var)))

;; ---------- with-selected-frame / setopt / cl-loop additions ----------
(cl-assert (= (with-selected-frame (selected-frame) (+ 1 2)) 3))
;; bare cl-loop body forms run until cl-return
(cl-assert (= (let ((n 0)) (cl-loop (when (> (incf n) 5) (cl-return n)))) 6))
;; while is tested after for-stepping (GNU textual order)
(cl-assert (equal (cl-loop for a in '(1 2 3) while (< a 3) collect a)
                  '(1 2)))
(cl-assert (equal (cl-loop for a in '(1 2 3) until (> a 2) collect a)
                  '(1 2)))
;; plist-get tolerates non-lists (returns nil); plist-member errors plistp
(cl-assert (null (plist-get t :x)))
(cl-assert (null (plist-get 'sym :x)))
(cl-assert (null (plist-get 5 :x)))
(cl-assert (eq (condition-case e (plist-member t :x)
                 (wrong-type-argument (car e)))
               'wrong-type-argument))

;; ---------- gv-setter declare ----------
(defun probe-winner--ar ()
  (declare (gv-setter (lambda (store) `(ignore ,store))))
  nil)
(cl-assert (get 'probe-winner--ar 'gv-expander))
(cl-assert (fboundp 'winner-active-region))
(cl-assert (get 'winner-active-region 'gv-expander))

;; ---------- ring (used by winner rings) ----------
(let ((r (make-ring 2)))
  (ring-insert r 'a)
  (ring-insert r 'b)
  (cl-assert (eq (ring-ref r 0) 'b))
  (cl-assert (eq (ring-ref r 1) 'a))
  ;; full ring drops oldest
  (ring-insert r 'c)
  (cl-assert (eq (ring-ref r 0) 'c))
  (cl-assert (eq (ring-ref r 1) 'b))
  (cl-assert (= (ring-length r) 2))
  (cl-assert (eq (ring-remove r 0) 'c))
  (cl-assert (= (ring-length r) 1))
  (cl-assert (not (ring-empty-p r))))

;; ---------- winner-undo is interactive & errors off-mode ----------
(winner-mode -1)
(cl-assert (commandp 'winner-undo))
(cl-assert (commandp 'winner-redo))

(winner-mode 1)
(provide 'probe-winner)
;;; probe_winner.el ends here
