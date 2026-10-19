;;; -*- lexical-binding: nil; -*-
;; Issue #31: execute the actual boot-created function-valued menu fields.
(defun qa31-menu-item (name) (cdr (assq name (cdr menu-bar-edit-menu))))
(defun qa31-menu-keys (name) (plist-get (nthcdr 3 (qa31-menu-item name)) :keys))

(dolist (qa31-name '(cut copy paste))
  (qa (functionp (qa31-menu-keys qa31-name)) t)
  (qa (func-arity (qa31-menu-keys qa31-name)) '(0 . 0)))
(dolist (cua-mode '(nil t))
  (dolist (mark-active '(nil t))
    (qa (funcall (qa31-menu-keys 'cut))
        (if (and cua-mode mark-active) "\\[cua-cut-handler]" "\\[kill-region]"))
    (qa (funcall (qa31-menu-keys 'copy))
        (cond ((featurep 'ns) "\\[ns-copy-including-secondary]")
              ((and cua-mode mark-active) "\\[cua-copy-handler]")
              (t "\\[kill-ring-save]")))
    (qa (funcall (qa31-menu-keys 'paste)) (if cua-mode "\\[cua-paste]" "\\[yank]"))))
(qa (lookup-key (current-global-map) [menu-bar edit cut]) 'kill-region)
(qa (lookup-key (current-global-map) [menu-bar edit paste]) 'yank)
(qa (lookup-key (current-global-map) [menu-bar edit copy])
    (if (featurep 'ns) 'ns-copy-including-secondary 'kill-ring-save))
(dolist (qa31-name '(cut copy paste))
  (qa (let* ((map (cdar (accessible-keymaps (current-global-map) [menu-bar edit])))
             (item (cdr (assq qa31-name (cdr map)))))
        (eq (plist-get (nthcdr 3 item) :keys) (qa31-menu-keys qa31-name))) t))

;; Paste's enable predicate was also lost by the serialized snapshot.
(qa (let* ((form (plist-get (nthcdr 3 (qa31-menu-item 'paste)) :enable))
           (function (cadr (cadr form)))) (functionp function)) t)
(let ((saved (symbol-function 'gui-backend-selection-exists-p)))
  (unwind-protect
      (progn
        (fset 'gui-backend-selection-exists-p (lambda (_) nil))
        (dolist (buffer-read-only '(nil t))
          (let ((kill-ring nil) (yank-menu '(keymap)))
            (qa (eval (plist-get (nthcdr 3 (qa31-menu-item 'paste)) :enable)) nil))
          (let ((kill-ring '("text")) (yank-menu '(keymap (text . "text"))))
            (qa (eval (plist-get (nthcdr 3 (qa31-menu-item 'paste)) :enable))
                (and (not buffer-read-only)
                     (if (featurep 'ns) (cdr yank-menu) kill-ring))))))
    (fset 'gui-backend-selection-exists-p saved)))

;; Post-boot splices go through the optimizer and drop the unused menu.
(qa-prin1
 (eval '(let ((menu (make-sparse-keymap "E")))
          (define-key menu [cut] `(menu-item "Cut" kill-region :keys ,(lambda () "k")))
          (plist-get (nthcdr 3 (cdr (assq 'cut (cdr menu)))) :keys)) t)
 "#[nil (\"k\") (t)]")

(princ (qa--done)) (terpri)
