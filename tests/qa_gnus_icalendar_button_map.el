;;; Calendar action buttons do not use the MIME part context menu.

;; Read the real source but evaluate only the button definitions.  Loading
;; the full Gnus stack needs unrelated face backends unavailable in this QA.
(let ((source (expand-file-name "lisp/gnus-icalendar.el" default-directory))
      (have-map nil)
      (have-insert nil))
  (with-temp-buffer
    (set-syntax-table emacs-lisp-mode-syntax-table)
    (insert-file-contents source)
    (goto-char (point-min))
    (while (progn
             (forward-comment (point-max))
             (not (eobp)))
      (let ((form (read (current-buffer))))
        (when (and (eq (car-safe form) 'defvar)
                   (eq (cadr form) 'gnus-icalendar-button-map))
          (eval form t)
          (setq have-map t))
        (when (and (eq (car-safe form) 'defun)
                   (eq (cadr form) 'gnus-icalendar-insert-button))
          (eval form t)
          (setq have-insert t)))))
  (qa (list have-map have-insert) '(t t)))

(qa (list (eq (lookup-key gnus-icalendar-button-map [mouse-2])
              #'gnus-article-push-button)
          (lookup-key gnus-icalendar-button-map [down-mouse-3]))
    '(t nil))

(qa (let ((gnus-article-button-face nil))
      (with-temp-buffer
        (gnus-icalendar-insert-button "Reply" #'ignore nil)
        (list (eq (get-text-property (point-min) 'keymap)
                  gnus-icalendar-button-map)
              (get-text-property (point-min) 'gnus-callback))))
    '(t ignore))

(princ (qa--done))
