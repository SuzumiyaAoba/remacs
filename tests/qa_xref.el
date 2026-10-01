;; xref.el — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - xref-find-definitions / -references / -apropos / -at-mouse
;;     exist (autoloaded).
;;   - xref-marker-stack-empty-p checks the marker stack.
;;   - xref-backend-* are generic methods (cl-defgeneric).
;;   - xref-make + xref-file-location build xref items.

(require 'xref)

;; == surface — find commands ==
(qa (fboundp 'xref-find-definitions) t)
(qa (fboundp 'xref-find-references) t)
(qa (fboundp 'xref-find-apropos) t)
(qa (fboundp 'xref-find-definitions-at-mouse) t)
(qa (fboundp 'xref-go-back) t)
(qa (fboundp 'xref-go-forward) t)
(qa (fboundp 'xref-marker-stack-empty-p) t)

;; == xref-backend generics ==
(qa (fboundp 'xref-backend-definitions) t)
(qa (fboundp 'xref-backend-references) t)
(qa (fboundp 'xref-backend-apropos) t)
(qa (fboundp 'xref-backend-identifier-at-point) t)
(qa (fboundp 'xref-backend-identifier-completion-table) t)
(qa (fboundp 'xref-backend-identifier-completion-ignore-case) t)

;; == xref-item constructors ==
(qa (fboundp 'xref-make) t)
(qa (fboundp 'xref-make-file-location) t)
(qa (fboundp 'xref-make-bogus-location) t)
(qa (fboundp 'xref-make-buffer-location) t)
(qa (fboundp 'xref-make-elisp-location) t)

;; == item accessors ==
(qa (fboundp 'xref-item-summary) t)
(qa (fboundp 'xref-item-location) t)
(qa (fboundp 'xref-location-marker) t)
(qa (fboundp 'xref-location-group) t)
(qa (fboundp 'xref-location-line) t)

;; == file-location accessors ==
(qa (fboundp 'xref-file-location-file) t)
(qa (fboundp 'xref-file-location-line) t)
(qa (fboundp 'xref-file-location-column) t)

;; == xref-make + xref-file-location produces an item ==
(let ((it (xref-make "foo" (xref-make-file-location "/tmp/x.el" 3 5))))
  (qa (xref-item-summary it) "foo")
  (qa (type-of (xref-item-location it)) 'xref-file-location)
  (qa (xref-file-location-file (xref-item-location it)) "/tmp/x.el")
  (qa (xref-file-location-line (xref-item-location it)) 3)
  (qa (xref-file-location-column (xref-item-location it)) 5))

;; == xref-item accessors on the constructed item ==
(let ((it (xref-make "bar" (xref-make-file-location "/tmp/y.el" 10 0))))
  (qa (xref-item-summary it) "bar")
  (qa (xref-file-location-line (xref-item-location it)) 10))

;; == bogus-location ==
(let ((it (xref-make "bs" (xref-make-bogus-location "msg"))))
  (qa (type-of (xref-item-location it)) 'xref-bogus-location))

;; == buffer-location — needs a live buffer ==
(let ((buf (generate-new-buffer " *x*")))
  (unwind-protect
      (with-current-buffer buf
        (let ((it (xref-make "loc" (xref-make-buffer-location buf (point-min)))))
          (qa (type-of (xref-item-location it)) 'xref-buffer-location)))
    (kill-buffer buf)))

;; == marker ring is initially empty ==
(qa (xref-marker-stack-empty-p) t)

;; marker ring is a private var, not user-facing
(qa (boundp 'xref-marker-ring) nil)
;; xref--history is bound
(qa (boundp 'xref--history) t)


;; == find-backend / -identifier-methods are on a cl-generic ==
(qa (fboundp 'xref-find-backend) t)

(princ (qa--done))
