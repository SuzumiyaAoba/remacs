;; keymaps — GNU 31.1 pins (verified): define-key/lookup-key/key-binding,
;; parents, copy-keymap, map-keymap, accessible-keymaps, command-remapping,
;; local/global map switching.  NOTE: `qa' evals FORMs dynamically, so all
;; `let' bindings must live INSIDE each form.
;;
;; GNU truths pinned:
;;   - (keymapp '(keymap)) is t — only the car matters.
;;   - lookup-key returns t... no: returns the integer N (key length
;;     position) when the sequence is longer than needed — here "ab" in a
;;     map binding "a" to a command yields 1.
;;   - define-key returns the definition it installed.
;;   - local-unset-key with no local map returns nil and creates none.
;;   - set-keymap-parent returns the PARENT (a keymap).
;;   - key-binding "C-x C-f" is nil in -Q batch (find-file binding is a
;;     menu-visible prefix lookup, not a direct command binding).
;;   - command-remapping honors [remap CMD] entries; key-binding applies
;;     the remap unless NO-REMAP.

;; == construction ==
(qa (keymapp (make-keymap)) t)
(qa (keymapp (make-sparse-keymap)) t)
(qa-same (car (make-sparse-keymap)) 'keymap)
(qa (keymapp '(keymap)) t)
(qa (keymapp '(keymap (97 . x))) t)
(qa (keymapp nil) nil)
(qa (keymapp '((not-keymap))) nil)
(qa (char-table-p (cadr (make-keymap))) t)
(qa (listp (cadr (make-sparse-keymap))) t)

;; == define-key returns the definition ==
(qa (define-key (make-sparse-keymap) "a" 'car) 'car)
(qa (define-key (make-keymap) "a" 'car) 'car)

;; == lookup-key ==
(qa (let ((k (make-sparse-keymap))) (lookup-key k "a")) nil)
(qa (let ((k (make-sparse-keymap)))
      (define-key k "a" 'cmd-a)
      (lookup-key k "a"))
    'cmd-a)
;; too-long key: binding reached before the end => 1
(qa (let ((k (make-sparse-keymap)))
      (define-key k "a" 'cmd-a)
      (lookup-key k "ab"))
    1)
;; multi-level prefix: "ab" makes "a" a keymap
(qa (let ((k (make-sparse-keymap)))
      (define-key k "ab" 'deep)
      (list (keymapp (lookup-key k "a"))
            (lookup-key k "ab")))
    '(t deep))
;; vector and integer-list keys
(qa (let ((k (make-sparse-keymap)))
      (define-key k [f5] 'f5-cmd)
      (lookup-key k [f5]))
    'f5-cmd)
(qa (let ((k (make-sparse-keymap)))
      (define-key k [?\C-a] 'ca-cmd)
      (lookup-key k "\C-a"))
    'ca-cmd)
;; function object as definition
(qa (let ((k (make-sparse-keymap)))
      (define-key k "a" #'car)
      (lookup-key k "a"))
    'car)
;; nil definition shadows parent
(qa (let ((parent (make-sparse-keymap))
          (child (make-sparse-keymap)))
      (define-key parent "a" 'pa)
      (set-keymap-parent child parent)
      (define-key child "a" nil)
      (lookup-key child "a"))
    nil)
;; inherited lookup through parent
(qa (let ((parent (make-sparse-keymap))
          (child (make-sparse-keymap)))
      (define-key parent "x" 'px)
      (set-keymap-parent child parent)
      (lookup-key child "x"))
    'px)
;; empty vector returns the keymap itself
(qa (keymapp (lookup-key (make-sparse-keymap) [])) t)
;; errors
(qa-err (lookup-key 5 "a") 'wrong-type-argument)
(qa-err (define-key 5 "a" 'x) 'wrong-type-argument)
(qa (condition-case nil (lookup-key (make-sparse-keymap) nil)
         (wrong-type-argument 'wta))
       'wta)

;; == default binding via [t] + ACCEPT-DEFAULTS ==
(qa (let ((k (make-sparse-keymap)))
      (define-key k [t] 'def-b)
      (lookup-key k "q" t))
    'def-b)

;; == copy-keymap is independent ==
(qa (let ((k (make-sparse-keymap)))
      (define-key k "a" 'a1)
      (let ((c (copy-keymap k)))
        (define-key c "b" 'b2)
        (list (lookup-key k "b") (lookup-key c "a") (lookup-key c "b"))))
    '(nil a1 b2))
(qa (let ((k (make-keymap)))
      (define-key k "a" 'a1)
      (lookup-key (copy-keymap k) "a"))
    'a1)

;; == set-keymap-parent returns the parent map ==
(qa (let ((p (make-sparse-keymap)))
      (keymapp (set-keymap-parent (make-sparse-keymap) p)))
    t)
(qa (let ((p (make-sparse-keymap))
          (c (make-sparse-keymap)))
      (set-keymap-parent c p)
      (eq (keymap-parent c) p))
    t)
(qa (keymap-parent (make-sparse-keymap)) nil)

;; == map-keymap / map-keymap-sorted ==
(qa (let ((k (make-sparse-keymap)) (acc nil))
      (define-key k "b" 'b-cmd)
      (define-key k "a" 'a-cmd)
      (map-keymap-sorted (lambda (ev b) (push (cons ev b) acc)) k)
      (nreverse acc))
    '((97 . a-cmd) (98 . b-cmd)))
(qa (let ((k (make-keymap)) (acc nil))
      (define-key k "a" 'dense-cmd)
      (map-keymap (lambda (ev b) (push b acc)) k)
      (not (null (memq 'dense-cmd acc))))
    t)

;; == keymap-prompt ==
(qa (keymap-prompt (make-sparse-keymap "MyPrompt")) "MyPrompt")
(qa (keymap-prompt (make-sparse-keymap)) nil)

;; == key-binding ==
(qa (key-binding "a") 'self-insert-command)
(qa (key-binding [?a]) 'self-insert-command)
(qa (key-binding [24 15]) 'delete-blank-lines)   ; C-x C-o
(qa (key-binding "C-x C-f") nil)                 ; -Q batch: not a command binding
(qa (key-binding "C-x") nil)                     ; prefix: returns nil
(qa (key-binding "a" t) 'self-insert-command)    ; ACCEPT-DEFAULTS arg
(qa (key-binding "a" nil t) 'self-insert-command); NO-REMAP arg

;; == command-remapping ==
(qa (command-remapping 'self-insert-command) nil)
(qa (let ((k (make-sparse-keymap)))
      (define-key k [remap self-insert-command] 'my-cmd)
      (command-remapping 'self-insert-command nil (list k)))
    'my-cmd)
;; key-binding applies remap; no-remap bypasses it
(qa (progn
         (define-key global-map [remap self-insert-command] 'my-remapped)
         (unwind-protect
             (list (command-remapping 'self-insert-command)
                   (key-binding "a")
                   (key-binding "a" nil t))
           (define-key global-map [remap self-insert-command] nil)))
       '(my-remapped my-remapped self-insert-command))

;; == global-set-key / global-unset-key ==
(qa (progn
      (global-set-key (kbd "M-g M-1") #'car)
      (unwind-protect
          (key-binding (kbd "M-g M-1"))
        (global-unset-key (kbd "M-g M-1"))))
    'car)
(qa (progn
      (global-set-key (kbd "M-g M-1") #'car)
      (global-unset-key (kbd "M-g M-1"))
      (key-binding (kbd "M-g M-1")))
    nil)
(qa (global-set-key (kbd "M-g M-1") #'car) 'car)
(qa (global-unset-key (kbd "M-g M-1")) nil)

;; == local-set-key / local-unset-key ==
(qa (with-temp-buffer
      (local-set-key "q" 'qa-cmd)
      (list (key-binding "q") (lookup-key (current-local-map) "q")))
    '(qa-cmd qa-cmd))
(qa (with-temp-buffer
      (local-unset-key "q"))
    nil)
;; local-unset-key does NOT create a local map when none exists
(qa (with-temp-buffer
         (local-unset-key "q")
         (current-local-map))
       nil)

;; == use-local-map / use-global-map ==
(qa (use-local-map (make-sparse-keymap)) nil)
(qa (with-temp-buffer
      (let ((m (make-sparse-keymap)))
        (use-local-map m)
        (eq (current-local-map) m)))
    t)
(qa (with-temp-buffer (current-local-map)) nil)
(qa (let ((old (current-global-map))
          (m (make-sparse-keymap)))
      (unwind-protect
          (progn
            (use-global-map m)
            (eq (current-global-map) m))
        (use-global-map old)))
    t)
(qa (keymapp (current-global-map)) t)
(qa (keymapp global-map) t)
;; local binding overrides global for key-binding
(qa (with-temp-buffer
      (let ((m (make-sparse-keymap)))
        (use-local-map m)
        (local-set-key "a" 'loc-a)
        (list (lookup-key m "a") (key-binding "a"))))
    '(loc-a loc-a))
;; POSITION arg of key-binding (buffer-pos → that buffer's local map)
(qa (with-temp-buffer
      (local-set-key "z" 'zz-cmd)
      (key-binding "z" nil nil (point-min)))
    'zz-cmd)
;; emacs-lisp-mode installs a local map
(qa (with-temp-buffer
      (emacs-lisp-mode)
      (keymapp (current-local-map)))
    t)

;; == accessible-keymaps ==
(qa (let ((aks (accessible-keymaps global-map)))
      (list (listp aks)
            (vectorp (car (car aks)))
            (keymapp (cdr (car aks)))))
    '(t t t))
(qa (let ((aks (accessible-keymaps global-map)))
      (not (null (member-if (lambda (e) (equal (car e) [27])) aks))))
    t)
(qa (let ((aks (accessible-keymaps global-map)))
      (not (null (member-if (lambda (e) (equal (car e) [24])) aks))))
    t)

;; == current-active-maps / minor-mode-key-binding ==
(qa (listp (current-active-maps)) t)
(qa (seq-every-p #'keymapp (current-active-maps)) t)
(qa (minor-mode-key-binding [f5]) nil)

(princ (qa--done))
