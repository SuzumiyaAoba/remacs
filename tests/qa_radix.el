;; radix-tree.el — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - radix-tree is a pure-Lisp immutable trie.
;;   - radix-tree-insert returns a NEW tree (doesn't mutate).
;;   - radix-tree-delete is NOT a fn in 31.1 (use cl-removef / similar).
;;   - radix-tree-empty is a VARIABLE bound to the empty tree.
;;   - radix-tree-count counts elements.

(require 'radix-tree)

;; == surface ==
(qa (fboundp 'radix-tree-make) nil)   ; not a fn — use radix-tree-empty
(qa (fboundp 'radix-tree-insert) t)
(qa (fboundp 'radix-tree-lookup) t)
(qa (fboundp 'radix-tree-delete) nil) ; not a fn
(qa (fboundp 'radix-tree-iter-subtrees) t)
(qa (fboundp 'radix-tree-iter-mappings) t)
(qa (fboundp 'radix-tree-iter-maps) nil)
(qa (fboundp 'radix-tree-count) t)
(qa (fboundp 'radix-tree-empty-p) nil)
(qa (fboundp 'radix-tree-print) nil)
;; empty marker
(qa (boundp 'radix-tree-empty) t)

;; == insert + lookup ==
(let ((t1 radix-tree-empty))
  ;; insert returns the new tree
  (setq t1 (radix-tree-insert t1 "foo" 1))
  (setq t1 (radix-tree-insert t1 "foobar" 2))
  (setq t1 (radix-tree-insert t1 "fop" 3))
  ;; lookups
  (qa (radix-tree-lookup t1 "foo") 1)
  (qa (radix-tree-lookup t1 "foobar") 2)
  (qa (radix-tree-lookup t1 "fop") 3)
  ;; prefix "fo" isn't a key
  (qa (radix-tree-lookup t1 "fo") nil)
  ;; missing
  (qa (radix-tree-lookup t1 "missing") nil)
  (qa (radix-tree-lookup t1 "") nil))

;; == insert overwrites ==
(let ((t1 radix-tree-empty))
  (setq t1 (radix-tree-insert t1 "k" 'v1))
  (setq t1 (radix-tree-insert t1 "k" 'v2))
  (qa (radix-tree-lookup t1 "k") 'v2))

;; == different data types ==
(let ((t1 radix-tree-empty))
  (setq t1 (radix-tree-insert t1 "sym" 'symbol))
  (setq t1 (radix-tree-insert t1 "num" 42))
  (setq t1 (radix-tree-insert t1 "str" "value"))
  (setq t1 (radix-tree-insert t1 "list" '(a b)))
  (qa (radix-tree-lookup t1 "sym") 'symbol)
  (qa (radix-tree-lookup t1 "num") 42)
  (qa (radix-tree-lookup t1 "str") "value")
  (qa (radix-tree-lookup t1 "list") '(a b)))

;; == empty string key ==
(let ((t1 radix-tree-empty))
  (setq t1 (radix-tree-insert t1 "" 'empty-key))
  (qa (radix-tree-lookup t1 "") 'empty-key))

;; == count ==
(let ((t1 radix-tree-empty))
  (setq t1 (radix-tree-insert t1 "a" 1))
  (setq t1 (radix-tree-insert t1 "ab" 2))
  (setq t1 (radix-tree-insert t1 "abc" 3))
  (setq t1 (radix-tree-insert t1 "b" 4))
  (qa (radix-tree-count t1) 4)
  ;; re-insert existing key doesn't add
  (setq t1 (radix-tree-insert t1 "a" 'new))
  (qa (radix-tree-count t1) 4))

;; == radix-tree-iter-subtrees — walks IMMEDIATE subtrees ==
;; "a" has a nested subtree for "ab" — iteration yields only top-level prefixes
(let ((t1 radix-tree-empty)
      (acc nil))
  (setq t1 (radix-tree-insert t1 "a" 1))
  (setq t1 (radix-tree-insert t1 "ab" 2))
  (setq t1 (radix-tree-insert t1 "b" 3))
  (radix-tree-iter-subtrees
   t1
   (lambda (prefix subtree)
     (push prefix acc)))
  ;; newest first — sort for stability
  (qa (sort acc #'string<) '("a" "b")))

;; == radix-tree-iter-mappings — (key . value) pairs for terminals ==
(let ((t1 radix-tree-empty)
      (acc nil))
  (setq t1 (radix-tree-insert t1 "a" 1))
  (setq t1 (radix-tree-insert t1 "ab" 2))
  (radix-tree-iter-mappings
   t1
   (lambda (key val)
     (push (cons key val) acc)))
  (qa (sort acc (lambda (x y) (string< (car x) (car y))))
      '(("a" . 1) ("ab" . 2))))

;; == immutability ==
(let ((t1 radix-tree-empty)
      (t2 nil))
  (setq t2 (radix-tree-insert t1 "x" 1))
  ;; original t1 still empty
  (qa (radix-tree-lookup t1 "x") nil)
  (qa (radix-tree-lookup t2 "x") 1))

(princ (qa--done))
