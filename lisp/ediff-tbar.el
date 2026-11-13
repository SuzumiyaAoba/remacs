;;; ediff-tbar.el --- Ediff toolbar support  -*- lexical-binding:t -*-

;; Remacs: provide Ediff toolbar items through the native tool-bar-map API.

;;; Code:

(require 'tool-bar)

(declare-function ediff-in-control-buffer-p "ediff-init" (&optional meta-buf-p))
(declare-function ediff-has-toolbar-support-p "ediff-init" (&optional frame))
(declare-function ediff-use-toolbar-p "ediff-init" ())

(defvar ediff--tool-bar-map nil
  "Keymap used for Ediff's tool bar.")

(defvar-local ediff--saved-tool-bar-map nil)
(defvar-local ediff--saved-tool-bar-map-local-p nil)
(defvar-local ediff--saved-tool-bar-map-p nil)
(defvar ediff--toolbar-frame-states nil
  "Frames whose tool bar lines were enabled for Ediff.")
(defvar-local ediff--tool-bar-frame nil)

(defun ediff--toolbar-display-enabled-p (&optional frame)
  "Return non-nil if FRAME supports Ediff's tool bar."
  (ediff-has-toolbar-support-p (or frame (selected-frame))))

(defun ediff--control-frame (buffer)
  "Return the frame that displays Ediff control BUFFER."
  (let ((window (get-buffer-window buffer t)))
    (or (and window (window-frame window))
        (selected-frame))))

(defun ediff--claim-toolbar-frame ()
  "Make the current control buffer's frame show its tool bar."
  (let* ((buffer (current-buffer))
         (frame (or ediff--tool-bar-frame
                    (ediff--control-frame buffer)))
         (state (assq frame ediff--toolbar-frame-states)))
    (setq-local ediff--tool-bar-frame frame)
    (if state
        (unless (memq buffer (nth 2 state))
          (setcar (nthcdr 2 state) (cons buffer (nth 2 state))))
      (let ((lines (frame-parameter frame 'tool-bar-lines)))
        (when (<= (or lines 0) 0)
          (push (list frame lines (list buffer)) ediff--toolbar-frame-states)
          (set-frame-parameter frame 'tool-bar-lines 1))))
    (add-hook 'kill-buffer-hook
              #'ediff--release-toolbar-frame-for-current-buffer nil t)))

(defun ediff--release-toolbar-frame (buffer)
  "Restore BUFFER's frame tool bar lines if Ediff changed them."
  (let* ((frame (buffer-local-value 'ediff--tool-bar-frame buffer))
         (state (and frame (assq frame ediff--toolbar-frame-states))))
    (when state
      (let ((owners (delq buffer (nth 2 state))))
        (if owners
            (setcar (nthcdr 2 state) owners)
          (when (and (frame-live-p frame)
                     (= (or (frame-parameter frame 'tool-bar-lines) 0) 1))
            (set-frame-parameter frame 'tool-bar-lines (nth 1 state)))
          (setq ediff--toolbar-frame-states
                (delq state ediff--toolbar-frame-states)))))
    (when (buffer-live-p buffer)
      (with-current-buffer buffer
        (setq ediff--tool-bar-frame nil)))))

(defun ediff--release-toolbar-frame-for-current-buffer ()
  "Restore the tool bar when the current Ediff control buffer is killed."
  (ediff--release-toolbar-frame (current-buffer)))

(defun ediff--make-tool-bar-map ()
  "Return the keymap used by Ediff's tool bar."
  (or ediff--tool-bar-map
      (let ((map (make-sparse-keymap)))
        (tool-bar-local-item "prev-node" #'ediff-previous-difference 'ediff-prev
                             map :label "Previous"
                             :help "Go to the previous difference")
        (tool-bar-local-item "next-node" #'ediff-next-difference 'ediff-next
                             map :label "Next"
                             :help "Go to the next difference")
        (tool-bar-local-item "copy" #'ediff-copy-A-to-B 'ediff-copy-a-to-b
                             map :label "A to B"
                             :help "Copy the current difference from A to B")
        (tool-bar-local-item "copy" #'ediff-copy-B-to-A 'ediff-copy-b-to-a
                             map :label "B to A"
                             :help "Copy the current difference from B to A")
        (tool-bar-local-item "refresh" #'ediff-update-diffs 'ediff-update
                             map :label "Update"
                             :help "Recompute differences")
        (tool-bar-local-item "close" #'ediff-quit 'ediff-quit
                             map :label "Quit"
                             :help "Quit Ediff")
        (setq ediff--tool-bar-map map))))

(defun ediff--set-toolbar (enable)
  "Show Ediff's tool bar in the current control buffer if ENABLE is non-nil."
  (when (ediff-in-control-buffer-p)
    (if enable
        (progn
          (unless ediff--saved-tool-bar-map-p
            (setq ediff--saved-tool-bar-map-p t
                  ediff--saved-tool-bar-map-local-p
                  (local-variable-p 'tool-bar-map)
                  ediff--saved-tool-bar-map
                  (and ediff--saved-tool-bar-map-local-p tool-bar-map)))
          (setq-local tool-bar-map (ediff--make-tool-bar-map))
          (ediff--claim-toolbar-frame))
      (when ediff--saved-tool-bar-map-p
        (if ediff--saved-tool-bar-map-local-p
            (setq-local tool-bar-map ediff--saved-tool-bar-map)
          (kill-local-variable 'tool-bar-map))
        (setq ediff--saved-tool-bar-map-p nil
              ediff--saved-tool-bar-map-local-p nil
              ediff--saved-tool-bar-map nil))
      (ediff--release-toolbar-frame (current-buffer)))
    (force-mode-line-update)))

(defun ediff--setup-toolbar-for-control-buffer ()
  "Install Ediff's tool bar when the user enabled it before a session started."
  (let* ((window (get-buffer-window (current-buffer) t))
         (frame (if window (window-frame window) (selected-frame))))
    (when (and (ediff-in-control-buffer-p)
               (ediff-use-toolbar-p)
               (ediff--toolbar-display-enabled-p frame))
      (ediff--set-toolbar t))))

(add-hook 'ediff-mode-hook #'ediff--setup-toolbar-for-control-buffer)

(provide 'ediff-tbar)
;;; ediff-tbar.el ends here
