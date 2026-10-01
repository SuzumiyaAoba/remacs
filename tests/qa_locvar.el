;; Variable locality / transient maps — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - variable-binding-locus on a dynamic var → nil; on a buffer-local
;;     var → buffer.
;;   - make-variable-buffer-local makes a var buffer-local; kill-all-local
;;     -variables clears binding but not permanent-local.
;;   - set-buffer-local is not a fn in 31.1.
;;   - make-variable-frame-local is not a fn.
;;   - defvar-keymap is a fn.
;;   - permanent-local prop keeps the buffer-local binding across
;;     kill-all-local-variables.
;;   - minor-mode-overriding-map-alist is bound.
;;   - set-transient-map returns the map.

;; == variable-binding-locus ==
(qa (fboundp 'variable-binding-locus) t)
(defvar qa--dynvar 1)
(qa (variable-binding-locus 'qa--dynvar) nil)
(defvar-local qa--bufvar 5)
;; variable-binding-locus is nil until a buffer-local binding exists
(with-temp-buffer
  (qa (variable-binding-locus 'qa--bufvar) nil)
  ;; setq makes the binding real; locus becomes the buffer
  (setq qa--bufvar 9)
  (qa (bufferp (variable-binding-locus 'qa--bufvar)) t))

;; == make-variable-buffer-local ==
(defvar qa--bvl 0)
(make-variable-buffer-local 'qa--bvl)
(with-temp-buffer
  (setq qa--bvl 99)
  (qa qa--bvl 99)
  (qa (default-value 'qa--bvl) 0)
  (kill-all-local-variables)
  (qa qa--bvl 0))

;; == permanent-local ==
(defvar qa--perm 1)
(put 'qa--perm 'permanent-local t)
(make-variable-buffer-local 'qa--perm)
(with-temp-buffer
  (setq qa--perm 42)
  (kill-all-local-variables)
  ;; buffer-local binding survives thanks to permanent-local
  (qa qa--perm 42)
  (qa (local-variable-p 'qa--perm) t))

;; == misc vars/fns ==
(qa (boundp 'minor-mode-overriding-map-alist) t)
(qa (boundp 'permanent-local-hook) nil)
(qa (fboundp 'set-buffer-local) nil)
(qa (fboundp 'make-variable-frame-local) nil)
;; == set-transient-map ==
;; returns a byte-code FUNCTION (a deactivation closure), not the keymap
(let ((km (make-sparse-keymap)))
  (define-key km "a" 'cmd-a)
  (qa (functionp (set-transient-map km)) t))
(qa (fboundp 'current-minor-mode-maps) t)
(qa (listp (current-minor-mode-maps)) t)


;; == defvar-keymap builds a real keymap ==
(defvar-keymap qa--dkm
  "a" #'cmd-a
  "b" #'cmd-b)
(qa (keymapp qa--dkm) t)
(qa (lookup-key qa--dkm "a") 'cmd-a)
(qa (lookup-key qa--dkm "b") 'cmd-b)

;; == make-local-variable / kill-local-variable ==
(defvar qa--mlv 7)
(with-temp-buffer
  (make-local-variable 'qa--mlv)
  (setq qa--mlv 11)
  (qa qa--mlv 11)
  (qa (default-value 'qa--mlv) 7)
  (kill-local-variable 'qa--mlv)
  (qa qa--mlv 7)
  (qa (local-variable-p 'qa--mlv) nil))

;; == local-variable-if-set-p + buffer-local-variables ==
(with-temp-buffer
  ;; not set yet → nil
  (qa (local-variable-if-set-p 'qa--nope) nil)
  (setq-local qa--nope 1)
  (qa (local-variable-if-set-p 'qa--nope) t))

;; buffer-local-variables returns an alist; buffer-read-only present in GNU
(with-temp-buffer
  (qa (listp (buffer-local-variables)) t)
  (qa (if (assq 'buffer-read-only (buffer-local-variables)) t nil) t))


;; == buffer-local-value reads a specific buffer's value ==
(defvar qa--blv 0)
(make-variable-buffer-local 'qa--blv)
(let ((b1 (generate-new-buffer "qa-blv1"))
      (b2 (generate-new-buffer "qa-blv2")))
  (unwind-protect
      (progn
        (with-current-buffer b1 (setq qa--blv 11))
        (with-current-buffer b2 (setq qa--blv 22))
        (qa (buffer-local-value 'qa--blv b1) 11)
        (qa (buffer-local-value 'qa--blv b2) 22)
        (qa (default-value 'qa--blv) 0))
    (kill-buffer b1)
    (kill-buffer b2)))

(princ (qa--done))
