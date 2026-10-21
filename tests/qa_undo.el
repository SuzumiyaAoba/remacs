;; Undo machinery — GNU 31.1 pins (verified).
;;
;; GNU truths pinned (batch-verified; `undo' itself is command-driven
;; and merges all groups in batch — use undo-start+undo-more for
;; deterministic group-level checks):
;;   - buffer-undo-list starts as t in a fresh with-temp-buffer
;;     (undo recording DISABLED).
;;   - (setq buffer-undo-list nil) re-enables recording.
;;   - undo-start + undo-more 1 undoes exactly ONE group; each
;;     further undo-more 1 undoes the previous group.
;;   - pending-undo-list is the WORKING list consumed by undo-more.
;;   - undo-boundary inserts a nil marker into buffer-undo-list.
;;   - undo-in-region is interned but NOT fbound in GNU 31.1.

;; == temp-buffer default: undo disabled ==
(qa (with-temp-buffer buffer-undo-list) t)
;; enabling recording
(with-temp-buffer
  (setq buffer-undo-list nil)
  (qa buffer-undo-list nil)
  (insert "x")
  (qa (listp buffer-undo-list) t))

;; == undo-start + undo-more: deterministic group-level undo ==
(with-temp-buffer
  (setq buffer-undo-list nil)
  (insert "a")
  (undo-boundary)
  (insert "b")
  (undo-boundary)
  (insert "c")
  (undo-start)
  (undo-more 1)               ; undoes "c" → "ab"
  (qa (buffer-string) "ab")
  (undo-more 1)               ; undoes "b" → "a"
  (qa (buffer-string) "a")
  (undo-more 1)               ; undoes "a" → ""
  (qa (buffer-string) ""))

;; == pending-undo-list: t while recording; consumed by undo-more ==
(with-temp-buffer
  (setq buffer-undo-list nil)
  (insert "p")
  (qa pending-undo-list t)
  (qa (listp buffer-undo-list) t))
;; == undo-boundary inserts nil marker ==
(with-temp-buffer
  (setq buffer-undo-list nil)
  (insert "ab")
  (undo-boundary)
  (insert "cd")
  (qa (if (memq nil buffer-undo-list) t nil) t))

;; == undo-limit/undo-strong-limit/undo-outer-limit ==
(qa undo-limit 160000)
(qa undo-strong-limit 240000)
(qa undo-outer-limit nil)

;; == fns exist / absent ==
(qa (fboundp 'undo-in-region) nil)     ; interned, NOT fbound in 31.1
(qa (fboundp 'undo-redo) t)
(qa (fboundp 'undo-only) t)
(qa (fboundp 'undo-more) t)
(qa (fboundp 'undo-auto-amalgamate) t)
(qa (fboundp 'undo-boundary) t)
(qa (fboundp 'primitive-undo) t)
(qa (fboundp 'undo-start) t)

;; == primitive-undo replays raw list ==
(with-temp-buffer
  (setq buffer-undo-list nil)
  (insert "xy")
  (undo-boundary)
  (insert "z")
  (primitive-undo 1 buffer-undo-list)
  (qa (buffer-string) "xy"))

;; == buffer-enable-undo / buffer-disable-undo ==
(with-temp-buffer
  (setq buffer-undo-list nil)
  (insert "z")
  (buffer-disable-undo)
  (qa buffer-undo-list t)
  (buffer-enable-undo)
  (qa buffer-undo-list nil))

;; == undo-no-redo / undo-equiv-table presence ==
(qa (boundp 'undo-equiv-table) t)
(qa (fboundp 'undo-no-redo) nil)       ; not a function in 31.1

;; == amalgamating-undo-limit value ==
(qa amalgamating-undo-limit 20)

;; GH #131: empty histories have no boundary, but still remember point.
(qa (with-temp-buffer (buffer-enable-undo) (setq buffer-undo-list nil)
      (undo-boundary) buffer-undo-list) nil)
(qa (with-temp-buffer (buffer-enable-undo) (setq buffer-undo-list nil)
      (undo-boundary) (undo-boundary) buffer-undo-list) nil)
(qa (with-temp-buffer (buffer-enable-undo) (setq buffer-undo-list nil)
      (undo-boundary) (insert "ab") buffer-undo-list) '((1 . 3) (t . 0)))
(qa (with-temp-buffer (buffer-enable-undo) (setq buffer-undo-list nil)
      (insert "ab") (undo-boundary) (undo-boundary) buffer-undo-list) '(nil (1 . 3) (t . 0)))
(qa (with-temp-buffer (setq buffer-undo-list t) (undo-boundary) buffer-undo-list) t)
(qa (with-temp-buffer (insert "ab") (buffer-enable-undo)
      (setq buffer-undo-list nil) (goto-char 2) (undo-boundary) (goto-char 1)
      (insert "x") buffer-undo-list) '((1 . 2) 2))

;; Issue #281: undo edits and callbacks run with inhibit-read-only bound.
(qa
(with-temp-buffer
  (insert "abc") (put-text-property 1 4 'read-only nil)
  (setq buffer-undo-list nil buffer-read-only nil)
  (let ((inhibit-read-only '(locked)))
    (list
     (condition-case e (progn (primitive-undo 1 '((1 . 4))) 'undone)
       (error e))
     (buffer-substring-no-properties (point-min) (point-max))
     inhibit-read-only)))
'(undone "" (locked))
)

(qa
(with-temp-buffer
  (insert "abc") (put-text-property 1 4 'read-only nil)
  (setq buffer-undo-list nil buffer-read-only t)
  (let ((inhibit-read-only '(locked)))
    (list
     (condition-case e (progn (primitive-undo 1 '((1 . 4))) 'undone)
       (error e))
     (buffer-substring-no-properties (point-min) (point-max))
     inhibit-read-only)))
'(undone "" (locked))
)

(qa
(with-temp-buffer
  (insert "abc") (put-text-property 1 4 'read-only t)
  (setq buffer-undo-list nil buffer-read-only nil)
  (let ((inhibit-read-only '(locked)))
    (list
     (condition-case e (progn (primitive-undo 1 '((1 . 4))) 'undone)
       (error e))
     (buffer-substring-no-properties (point-min) (point-max))
     inhibit-read-only)))
'(undone "" (locked))
)

(qa
(with-temp-buffer
  (insert "abc") (put-text-property 1 4 'read-only t)
  (setq buffer-undo-list nil buffer-read-only t)
  (let ((inhibit-read-only '(locked)))
    (list
     (condition-case e (progn (primitive-undo 1 '((1 . 4))) 'undone)
       (error e))
     (buffer-substring-no-properties (point-min) (point-max))
     inhibit-read-only)))
'(undone "" (locked))
)

(qa
(with-temp-buffer
  (insert "abc") (put-text-property 1 4 'read-only 'locked)
  (setq buffer-undo-list nil buffer-read-only nil)
  (let ((inhibit-read-only '(locked)))
    (list
     (condition-case e (progn (primitive-undo 1 '((1 . 4))) 'undone)
       (error e))
     (buffer-substring-no-properties (point-min) (point-max))
     inhibit-read-only)))
'(undone "" (locked))
)

(qa
(with-temp-buffer
  (insert "abc") (put-text-property 1 4 'read-only 'locked)
  (setq buffer-undo-list nil buffer-read-only t)
  (let ((inhibit-read-only '(locked)))
    (list
     (condition-case e (progn (primitive-undo 1 '((1 . 4))) 'undone)
       (error e))
     (buffer-substring-no-properties (point-min) (point-max))
     inhibit-read-only)))
'(undone "" (locked))
)

(qa
(with-temp-buffer
  (insert "abc") (setq buffer-undo-list nil)
  (let (events)
    (let
	((before-change-functions
	  (list
	   (lambda (beg end)
	     (push (list 'before beg end inhibit-read-only) events))))
	 (after-change-functions
	  (list
	   (lambda (beg end old)
	     (push (list 'after beg end old inhibit-read-only) events)))))
      (primitive-undo 1 '((1 . 4))))
    (list (nreverse events) inhibit-read-only (buffer-string))))
'(((before 1 4 t) (after 1 1 3 t)) nil "")
)

(qa
(with-temp-buffer
  (setq buffer-undo-list nil)
  (let ((inhibit-read-only 'old) seen)
    (primitive-undo 1
		    (list
		     (list 'apply
			   (lambda nil (setq seen inhibit-read-only)))))
    (list seen inhibit-read-only buffer-undo-list)))
'(t old ((apply cdr nil)))
)

(qa
(with-temp-buffer
  (insert "abc") (setq buffer-undo-list nil)
  (let
      ((inhibit-read-only 'old)
       (before-change-functions
	(list (lambda (&rest _) (error "undo hook")))))
    (list (condition-case e (primitive-undo 1 '((1 . 4))) (error e))
	  inhibit-read-only (buffer-string))))
'((error "undo hook") old "abc")
)

(qa
(with-temp-buffer
  (let ((inhibit-read-only 'old) seen)
    (list
     (catch 'qa281
       (primitive-undo 1
		       (list
			(list 'apply
			      (lambda nil
				(setq seen inhibit-read-only)
				(throw 'qa281 'jump))))))
     seen inhibit-read-only)))
'(jump t old)
)

(qa
(with-temp-buffer
  (let ((inhibit-read-only 'old))
    (list (condition-case e (primitive-undo 'bad nil) (error e))
	  inhibit-read-only)))
'((wrong-type-argument number-or-marker-p bad) old)
)


;; Issue #282: only apply entries can require a compensating undo record.
(qa
(with-temp-buffer
  (insert "abc") (goto-char 1) (setq buffer-undo-list nil)
  (list (primitive-undo 1 '(2)) (point) buffer-undo-list))
'(nil 2 nil)
)

(qa
(with-temp-buffer
  (insert "abc") (goto-char 1) (setq buffer-undo-list t)
  (list (primitive-undo 1 '(2)) (point) buffer-undo-list))
'(nil 2 t)
)

(qa
(with-temp-buffer
  (insert "abc") (goto-char 1) (setq buffer-undo-list nil)
  (list (primitive-undo 1 '(2 3 nil 1)) (point) buffer-undo-list))
'((1) 3 nil)
)

(qa
(with-temp-buffer
  (insert "abc") (goto-char 1) (setq buffer-undo-list nil)
  (list (primitive-undo 1.5 '(2 nil 3)) (point) buffer-undo-list))
'(nil 3 nil)
)

(qa
(with-temp-buffer
  (insert "abc") (goto-char 1) (setq buffer-undo-list nil)
  (list (primitive-undo 0 '(2)) (point) buffer-undo-list))
'((2) 1 nil)
)

(qa
(with-temp-buffer
  (insert "abc") (goto-char 1) (setq buffer-undo-list nil)
  (list (primitive-undo 1 '(nil nil)) (point) buffer-undo-list))
'((nil) 1 nil)
)

(qa
(with-temp-buffer
  (setq buffer-undo-list nil)
  (list (primitive-undo 1 '((apply ignore))) buffer-undo-list))
'(nil ((apply cdr nil)))
)

(qa
(with-temp-buffer
  (setq buffer-undo-list t)
  (list (primitive-undo 1 '((apply ignore))) buffer-undo-list))
'(nil ((apply cdr nil) . t))
)

(qa
(with-temp-buffer
  (insert "abc") (goto-char 1) (setq buffer-undo-list nil)
  (primitive-undo 1 '((apply goto-char 2)))
  (list (point) buffer-undo-list))
'(2 ((apply cdr nil)))
)

(qa
(with-temp-buffer
  (setq buffer-undo-list nil)
  (primitive-undo 1
		  (list
		   (list 'apply
			 (lambda nil (setq buffer-undo-list '(custom))))))
  buffer-undo-list)
'(custom)
)

(qa
(with-temp-buffer
  (setq buffer-undo-list nil)
  (primitive-undo 1
		  (list
		   (list 'apply (lambda nil (setq buffer-undo-list t)))))
  buffer-undo-list)
't
)

(qa
(with-temp-buffer
  (insert "abc") (goto-char 1)
  (let ((old (list 'old)))
    (setq buffer-undo-list old) (primitive-undo 1 '(2))
    (list (point) (eq buffer-undo-list old) buffer-undo-list)))
'(2 t (old))
)

(qa
(with-temp-buffer
  (insert "abc") (setq buffer-undo-list t)
  (primitive-undo 1 '((1 . 4)))
  (list (buffer-string) buffer-undo-list))
'("" t)
)


(princ (qa--done))
