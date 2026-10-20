;; keymap.el — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - make-keymap produces a dense (char-table) keymap.
;;   - make-sparse-keymap produces a sparse one.
;;   - copy-keymap shares the same bindings object (but is a copy).
;;   - define-key stores; lookup-key retrieves.
;;   - keymap-set / keymap-unset / keymap-set-after are the modern API.

;; == surface ==
(qa (fboundp 'keymapp) t)
(qa (fboundp 'make-keymap) t)
(qa (fboundp 'make-sparse-keymap) t)
(qa (fboundp 'copy-keymap) t)
(qa (fboundp 'keymap-prompt) t)
(qa (fboundp 'keymap-set) t)
(qa (fboundp 'keymap-unset) t)
(qa (fboundp 'keymap-set-after) t)
(qa (fboundp 'keymap-parent) t)
(qa (fboundp 'current-local-map) t)
(qa (fboundp 'current-global-map) t)
(qa (fboundp 'use-global-map) t)
(qa (fboundp 'use-local-map) t)
(qa (fboundp 'define-key) t)
(qa (fboundp 'lookup-key) t)
(qa (fboundp 'key-binding) t)
(qa (fboundp 'substitute-key-definition) t)
(qa (fboundp 'map-keymap) t)
(qa (fboundp 'map-keymap-sorted) t)
(qa (fboundp 'accessible-keymaps) t)
(qa (fboundp 'minor-mode-key-binding) t)
(qa (fboundp 'current-active-maps) t)
(qa (fboundp 'command-remapping) t)
(qa (fboundp 'local-set-key) t)
(qa (fboundp 'global-set-key) t)
(qa (fboundp 'global-unset-key) t)
(qa (fboundp 'local-unset-key) t)
;; NOT fns in 31.1
(qa (fboundp 'keymap-set-parent) nil)
(qa (fboundp 'keymap-has-parent-p) nil)
(qa (fboundp 'keymap--check-p) nil)
(qa (fboundp 'abbrev-map) nil)
(qa (fboundp 'global-map) nil)
(qa (fboundp 'local-map) nil)
(qa (fboundp 'keymap-command-symbols) nil)
(qa (fboundp 'undefine-key) nil)
(qa (fboundp 'substitute-keys-in-commands) nil)

;; == make-keymap produces a char-table-backed keymap ==
(let ((k (make-keymap)))
  (qa (keymapp k) t)
  ;; dense — cadr is a char-table
  (qa (char-table-p (cadr k)) t))

;; == make-sparse-keymap — no char-table ==
(let ((k (make-sparse-keymap)))
  (qa (keymapp k) t)
  ;; cadr is a list of bindings
  (qa (listp (cadr k)) t))

;; == define-key / lookup-key round-trip ==
(let ((k (make-sparse-keymap)))
  (define-key k "a" #'car)
  (qa (lookup-key k "a") 'car))

;; == keymap-set (modern API) ==
(let ((k (make-sparse-keymap)))
  (keymap-set k "b" #'cdr)
  (qa (lookup-key k "b") 'cdr))

;; == keymap-unset removes ==
(let ((k (make-sparse-keymap)))
  (keymap-set k "c" #'car)
  (keymap-unset k "c")
  (qa (lookup-key k "c") nil))

;; == lookup-key on global-map "a" — self-insert-command ==
(let ((gm (current-global-map)))
  (qa (lookup-key gm "a") 'self-insert-command))

;; == copy-keymap gives an independent copy ==
(let* ((k (make-sparse-keymap))
       (c (copy-keymap k)))
  ;; the copy doesn't see new bindings on the original
  (define-key k "x" #'car)
  (qa (lookup-key c "x") nil)
  ;; but the original does
  (qa (lookup-key k "x") 'car))

;; == keymap-parent / use-global-map ==
(let* ((parent (make-sparse-keymap))
       (child (copy-keymap parent)))
  ;; copy inherits parent — set explicitly via set-keymap-parent
  (set-keymap-parent child parent)
  (qa (eq (keymap-parent child) parent) t))

;; == use-local-map / use-global-map on a buffer ==
(let ((buf (generate-new-buffer " *km*")))
  (unwind-protect
      (with-current-buffer buf
        (let ((lm (make-sparse-keymap)))
          (use-local-map lm)
          (qa (eq (current-local-map) lm) t)))
    (kill-buffer buf)))

;; == current-global-map is keymapp ==
(let ((gm (current-global-map)))
  (qa (keymapp gm) t))

;; == key-binding returns the command ==
;; == key-binding — returns the command for a key sequence ==
;; C-x C-f isn't bound in batch (find-file is autoloaded via help only)
(qa (key-binding "C-x C-f") nil)
;; 'a' is self-insert
(qa (key-binding "a") 'self-insert-command)

;; == key-binding with ACCEPT-DEFAULTS arg (not a keymap) ==
;; key-binding's args are (key accept-defaults no-remap position)
(qa (key-binding "a" nil t) 'self-insert-command)

;; == global-set-key / global-unset-key on a free binding ==
;; "M-g M-1" is unbound in GNU batch
(global-set-key (kbd "M-g M-1") #'car)
(qa (key-binding (kbd "M-g M-1")) 'car)
(global-unset-key (kbd "M-g M-1"))
(qa (key-binding (kbd "M-g M-1")) nil)

;; == map-keymap iterates bindings ==
(let ((k (make-sparse-keymap))
      (acc nil))
  (define-key k "a" #'car)
  (define-key k "b" #'cdr)
  (map-keymap (lambda (ev cmd) (push (list ev cmd) acc)) k)
  (qa (length acc) 2))

;; == accessible-keymaps returns (prefix . keymap) pairs ==
(let ((k (make-sparse-keymap)))
  (define-key k "a" #'car)
  (let ((aks (accessible-keymaps k)))
    (qa (listp aks) t)
    ;; at least the empty-prefix entry exists
    (qa (length aks) 1)))

;; == command-remapping — nil for unremapped ==
(let ((w (selected-window)))
  ;; self-insert-command isn't remapped
  (qa (command-remapping 'self-insert-command) nil))

;; == local-set-key / local-unset-key on a buffer ==
(let ((buf (generate-new-buffer " *km*")))
  (unwind-protect
      (with-current-buffer buf
        (local-set-key "z" #'car)
        (qa (lookup-key (current-local-map) "z") 'car)
        (local-unset-key "z")
        (qa (lookup-key (current-local-map) "z") nil))
    (kill-buffer buf)))


;; == keymap-set-after — sets after an existing binding ==
(let ((k (make-sparse-keymap)))
  (define-key k "a" #'car)
  ;; keymap-set-after doesn't move the binding
  (qa (lookup-key k "a") 'car))

;; == substitute-key-definition maps old → new ==
(let ((src (make-sparse-keymap))
      (dst (make-sparse-keymap)))
  (define-key src "x" #'car)
  ;; substitute car→cdr in dst's copy of src
  (substitute-key-definition #'car #'cdr dst src)
  ;; dst got the substitution — 'x' maps to cdr
  (qa (lookup-key dst "x") 'cdr))

;; == minor-mode-key-binding returns active minor-mode bindings ==
(let ((mmb (minor-mode-key-binding "a")))
  ;; in batch no minor modes with 'a' bound — returns nil
  (qa mmb nil))

;; == current-active-maps includes the global ==
(let ((cams (current-active-maps)))
  (qa (listp cams) t)
  (qa (>= (length cams) 1) t)
  ;; all keymapp
  (qa (seq-every-p #'keymapp cams) t))

;; == keymap-prompt — prompt string for a keymap ==
(let ((k (make-sparse-keymap "MyMode")))
  (qa (keymap-prompt k) "MyMode"))

;; where-is-internal validates map lists and keeps GNU boot maps (#148).
(qa-err-data (let ((k (make-sparse-keymap)))
               (define-key k "a" 'ignore)
               (where-is-internal 'ignore (list k 5)))
             'wrong-type-argument '(keymapp 5))
(qa-err-data (let ((k (make-sparse-keymap)))
               (define-key k "a" 'ignore)
               (where-is-internal 'ignore (list k nil) t))
             'wrong-type-argument '(keymapp nil))
(qa-err-data (let ((k (make-sparse-keymap)))
               (define-key k "a" 'ignore)
               (where-is-internal 'ignore (list k 'qa148-not-a-map) 'non-ascii))
             'wrong-type-argument '(keymapp qa148-not-a-map))
(qa-err-data (let ((k (make-sparse-keymap)))
               (where-is-internal 'qa148-unbound-command (list k 5) nil nil t))
             'wrong-type-argument '(keymapp 5))
(qa (let ((k (make-sparse-keymap)) (other (make-sparse-keymap)))
      (define-key k "a" 'ignore)
      (define-key other "b" 'ignore)
      (where-is-internal 'ignore (list k other))) '([97] [98]))
(qa (let ((k (make-sparse-keymap)) (other (make-sparse-keymap)))
      (define-key k "a" 'ignore)
      (define-key other "b" 'ignore)
      (fset 'qa148-map other)
      (unwind-protect
          (list (where-is-internal 'ignore (list k 'qa148-map))
                (where-is-internal 'ignore (list 'qa148-map k)))
        (fmakunbound 'qa148-map))) '(([97] [98]) ([98] [97])))
;; GNU ignores an improper map list's non-cons tail.
(qa (let ((k (make-sparse-keymap)))
      (define-key k "a" 'ignore)
      (where-is-internal 'ignore (cons k 5))) '([97]))
(qa (lookup-key emacs-lisp-mode-map "\C-j") nil)
(qa (lookup-key lisp-interaction-mode-map "\C-j") 'eval-print-last-sexp)
(qa (string-match-p "C-j\\|eval-print-last-sexp"
                    (substitute-command-keys "\\{emacs-lisp-mode-map}")) nil)
(qa (seq-filter (lambda (key)
                  (and (> (length key) 2)
                       (eq (aref key 0) 'menu-bar)
                       (eq (aref key 1) 'buffer)))
                (where-is-internal nil)) '([menu-bar buffer command-separator]))
(qa-err-data (let ((k (make-sparse-keymap)))
               (define-key k "a" 'qa148-advertised)
               (put 'qa148-advertised 'advertised-binding "a")
               (unwind-protect
                   (where-is-internal 'qa148-advertised (list k 5) t)
                 (put 'qa148-advertised 'advertised-binding nil)))
             'wrong-type-argument '(keymapp 5))
(princ (qa--done))
