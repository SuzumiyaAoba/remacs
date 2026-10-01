;; tabulated-list.el — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - tabulated-list-mode sets up a derived-mode for tables.
;;   - tabulated-list-print renders entries.
;;   - tabulated-list-get-entry returns the row at point.

(require 'tabulated-list)

;; == surface ==
(qa (fboundp 'tabulated-list-mode) t)
(qa (fboundp 'tabulated-list-print) t)
(qa (fboundp 'tabulated-list-get-entry) t)
(qa (fboundp 'tabulated-list-get-id) t)
(qa (fboundp 'tabulated-list-delete-entry) t)
(qa (fboundp 'tabulated-list-put-tag) t)
(qa (fboundp 'tabulated-list-print-entry) t)
(qa (fboundp 'tabulated-list-init-header) t)
(qa (fboundp 'tabulated-list-clear-all-tags) t)
(qa (fboundp 'tabulated-list-sort) t)
(qa (fboundp 'tabulated-list-col-sort) t)
(qa (fboundp 'tabulated-list-revert) t)
;; NOT fns in 31.1
(qa (fboundp 'tabulated-list-get-selection) nil)
(qa (fboundp 'tabulated-list-set-col-alignment) nil)
(qa (fboundp 'tabulated-list-clear-column) nil)
(qa (fboundp 'tabulated-list-sort-column) nil)

;; == vars ==
(qa (boundp 'tabulated-list-format) t)
(qa (boundp 'tabulated-list-entries) t)
(qa (boundp 'tabulated-list-padding) t)
(qa (boundp 'tabulated-list-sort-key) t)
(qa (boundp 'tabulated-list-use-header-line) t)
(qa (boundp 'tabulated-list-printer) t)
(qa (boundp 'tabulated-list-groups) t)
;; NOT vars in 31.1
(qa (boundp 'tabulated-list-tag-marker) nil)
(qa (boundp 'tabulated-list-margin) nil)
(qa (boundp 'tabulated-list-widen-current-column) nil)

;; == basic table print — cell text has text-props; match on plain ==
(let ((buf (generate-new-buffer " *tl*")))
  (unwind-protect
      (with-current-buffer buf
        (setq tabulated-list-format [("A" 10 t) ("B" 10 t)])
        (setq tabulated-list-entries '((id1 ["x1" "y1"])
                                       (id2 ["x2" "y2"])))
        (tabulated-list-mode)
        (tabulated-list-init-header)
        (tabulated-list-print)
        ;; the buffer has the two rows — text starts at col 0
        (qa (string-match-p "x1" (buffer-string)) 0)
        (qa (string-match-p "y2" (buffer-string)) 25))
    (kill-buffer buf)))

;; == get-id returns the entry's id at point ==
(let ((buf (generate-new-buffer " *tl2*")))
  (unwind-protect
      (with-current-buffer buf
        (setq tabulated-list-format [("A" 10 t)])
        (setq tabulated-list-entries '((alpha ["x"])))
        (tabulated-list-mode)
        (tabulated-list-init-header)
        (tabulated-list-print)
        (goto-char (point-min))
        (qa (tabulated-list-get-id) 'alpha))
    (kill-buffer buf)))

;; == get-entry returns the row vector ==
(let ((buf (generate-new-buffer " *tl3*")))
  (unwind-protect
      (with-current-buffer buf
        (setq tabulated-list-format [("A" 10 t) ("B" 10 t)])
        (setq tabulated-list-entries '((alpha ["1" "2"])))
        (tabulated-list-mode)
        (tabulated-list-init-header)
        (tabulated-list-print)
        (goto-char (point-min))
        (qa (tabulated-list-get-entry) ["1" "2"]))
    (kill-buffer buf)))

;; == tabulated-list-sort-key var exists ==
(let ((buf (generate-new-buffer " *tl*")))
  (unwind-protect
      (with-current-buffer buf
        (tabulated-list-mode)
        (qa (boundp 'tabulated-list-sort-key) t))
    (kill-buffer buf)))

;; == tabulated-list-mode is a derived major mode ==
(let ((buf (generate-new-buffer " *tl*")))
  (unwind-protect
      (with-current-buffer buf
        (tabulated-list-mode)
        (qa major-mode 'tabulated-list-mode))
    (kill-buffer buf)))

;; == tabulated-list-print on empty entries → empty table ==
(let ((buf (generate-new-buffer " *tl*")))
  (unwind-protect
      (with-current-buffer buf
        (setq tabulated-list-format [("A" 10 t)])
        (setq tabulated-list-entries nil)
        (tabulated-list-mode)
        (tabulated-list-init-header)
        (tabulated-list-print)
        ;; no rows
        (qa (buffer-string) ""))
    (kill-buffer buf)))

(princ (qa--done))
