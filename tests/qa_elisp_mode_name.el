;;; qa_elisp_mode_name.el --- GNU lexical-binding mode indicator -*- lexical-binding: t; -*-

;; The mode name is a mode-line construct, not a preformatted string:
;; changing lexical-binding must select a different branch without rerunning
;; the major mode.  Keep the map object itself, including later key edits.
(defun qa-elisp-mode-name ()
  `("Elisp"
    (lexical-binding (:propertize "/l"
                      help-echo "Using lexical-binding mode")
                     (:propertize "/d"
                      help-echo "Using old dynamic scoping mode\n\
mouse-1: Enable lexical-binding mode"
                      face warning
                      mouse-face mode-line-highlight
                      local-map ,elisp--dynlex-modeline-map))))

(qa (with-temp-buffer (emacs-lisp-mode) mode-name)
    (qa-elisp-mode-name))
(qa (with-temp-buffer
      (emacs-lisp-mode)
      (setq-local lexical-binding t)
      mode-name)
    (qa-elisp-mode-name))
(qa (with-temp-buffer
      (emacs-lisp-mode)
      (setq-local lexical-binding nil)
      mode-name)
    (qa-elisp-mode-name))
(qa (with-temp-buffer
      (emacs-lisp-mode)
      (let ((name mode-name))
        (setq-local lexical-binding t)
        (setq-local lexical-binding nil)
        (eq name mode-name)))
    t)
(qa (with-temp-buffer
      (emacs-lisp-mode)
      (eq (plist-get (cddr (nth 2 (cadr mode-name))) 'local-map)
          elisp--dynlex-modeline-map))
    t)
(qa (with-temp-buffer
      (emacs-lisp-mode)
      (lookup-key (plist-get (cddr (nth 2 (cadr mode-name))) 'local-map)
                  [mode-line mouse-1]))
    'elisp-enable-lexical-binding)
(qa (let ((elisp--dynlex-modeline-map (make-sparse-keymap)))
      (define-key elisp--dynlex-modeline-map [mode-line mouse-1] 'ignore)
      (with-temp-buffer
        (emacs-lisp-mode)
        (lookup-key (plist-get (cddr (nth 2 (cadr mode-name))) 'local-map)
                    [mode-line mouse-1])))
    'ignore)
(qa (with-temp-buffer
      (emacs-lisp-mode)
      (let ((current-prefix-arg nil))
        (elisp-enable-lexical-binding)
        (list lexical-binding
              (buffer-substring-no-properties (point-min) (point-max))
              (equal mode-name (qa-elisp-mode-name)))))
    '(t ";; -*- lexical-binding: t; -*-\n" t))
(qa (commandp 'elisp-enable-lexical-binding) t)
(qa (with-temp-buffer
      (let ((emacs-lisp-mode-hook
             (list (lambda () (setq-local qa-elisp-hook-name mode-name)))))
        (emacs-lisp-mode)
        qa-elisp-hook-name))
    (qa-elisp-mode-name))
(qa (with-temp-buffer
      (emacs-lisp-mode)
      (lisp-interaction-mode)
      (list major-mode mode-name lexical-binding))
    '(lisp-interaction-mode "Lisp Interaction" t))
(qa (with-temp-buffer (lisp-mode) mode-name) "Lisp")
(qa (with-temp-buffer (lisp-data-mode) mode-name) "Lisp-Data")
(qa (with-temp-buffer
      (emacs-lisp-mode)
      (fundamental-mode)
      (emacs-lisp-mode)
      mode-name)
    (qa-elisp-mode-name))

(princ (qa--done))
(terpri)
