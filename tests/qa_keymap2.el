;; Keymap helpers + translation tables + misc keymap fns — GNU 31.1 pins.
;;
;; GNU truths pinned:
;;   - function-and/function-ormap/keymap-global-unset/obsolete-face-p
;;     are NOT fns in the dump (31.1 doesn't have them / lazy).
;;   - keymap-global-unset IS bound (keymap.el).
;;   - keymap-canonicalize / keymap--merge-atom are unbound internals.
;;   - key-description round-trips key vectors.
;;   - make-translation-table-from-alist produces a char-table.
;;   - define-key-after inserts AFTER a given key in a keymap.
;;   - define-prefix-command creates a sparse keymap bound to a symbol.

;; == function-and / function-ormap (not in 31.1) ==
(qa (fboundp 'function-and) nil)
(qa (fboundp 'function-ormap) nil)
(qa (fboundp 'obsolete-face-p) nil)
(qa (fboundp 'keymap-canonicalize) t)
;; == keymap helpers ==
(qa (fboundp 'keymap-global-unset) t)
(qa (fboundp 'keymap--get-keyelt) t)
(qa (fboundp 'keymap--merge-atom) nil)
(qa (fboundp 'keymap-prompt) t)
(qa (fboundp 'key-description) t)
(qa (fboundp 'define-key-after) t)
(qa (fboundp 'define-prefix-command) t)
(qa (fboundp 'accessibility-keys) nil)

;; == key-description ==
(qa (key-description [?\C-x ?\C-f]) "C-x C-f")
(qa (key-description [f1]) "<f1>")
(qa (key-description [?\M-a]) "M-a")
(qa (key-description [?\C-?]) "DEL")
(qa (key-description [32]) "SPC")

;; == translation tables ==
(qa (fboundp 'make-translation-table) t)
(qa (fboundp 'make-translation-table-from-alist) t)
(qa (fboundp 'make-translation-table-from-vector) t)
(qa (fboundp 'translation-table-get-input) nil)
;; from-alist produces a char-table
(let ((tt (make-translation-table-from-alist '((?a . ?A)))))
  (qa (char-table-p tt) t))

;; == define-key-after / define-prefix-command ==
(let ((km (make-sparse-keymap)))
  (define-key km "a" 'cmd-a)
  (define-key km "b" 'cmd-b)
  ;; insert "z" after "a" — remacs: define-key-after doesn't insert
  (define-key-after km "z" 'cmd-z "a")
  (qa (lookup-key km "z") 'cmd-z)
  (qa (lookup-key km "b") 'cmd-b))
;; define-prefix-command: a sparse keymap bound to the symbol
(define-prefix-command 'qa--pmap)
(qa (keymapp qa--pmap) t)
(qa (keymapp (symbol-function 'qa--pmap)) t)
;; keys defined under the prefix map work
(define-key qa--pmap "a" 'cmd-a)
(qa (lookup-key qa--pmap "a") 'cmd-a)

;; == lookup-key on the global map (sanity) ==
(qa (commandp (lookup-key global-map "\C-x\C-f")) t)
(qa (lookup-key global-map "\C-x\C-f") 'find-file)

;; == keymap-local-set / global-set / unset ==
(with-temp-buffer
  (keymap-local-set "a" #'ignore)
  (qa (lookup-key (current-local-map) "a") 'ignore))
(keymap-global-set "C-c q" #'ignore)
(qa (lookup-key global-map "\C-cq") 'ignore)
(keymap-global-unset "C-c q")
(qa (lookup-key global-map "\C-cq") nil)

;; == keymap-substitute ==
(let ((km (make-sparse-keymap)))
  (define-key km "a" 'cmd-a)
  (define-key km "b" 'cmd-b)
  (keymap-substitute km 'cmd-a 'cmd-replacement)
  (qa (lookup-key km "a") 'cmd-replacement)
  (qa (lookup-key km "b") 'cmd-b))

(princ (qa--done))
