;;; qa_hooks.el — GNU-compat checks for modification-hook machinery:
;;; text-property `modification-hooks', `insert-in-front-hooks',
;;; `insert-behind-hooks', overlay hook variants, change-signal
;;; ordering/args, inhibit-modification-hooks, and
;;; combine-after-change-calls deferral.  Every expected value was
;;; verified against GNU Emacs 31.1 batch.
;;;
;;; Buffer positions are 1-based throughout (hook args, put-text-
;;; property bounds); a property on (S E) covers chars S..E-1.

(defun qa-hooks--purge ()
  "Drain GNU's global `interval_insert_*_hooks' state.
The pending lists survive every report and are only reset by a
`verify_interval_modification' on a buffer that HAS an interval tree;
without this, a previous check's insert hooks would replay into this
check's first insert.  Any stale functions fired here push into their
dead lexical captures, so the drain is invisible."
  (with-temp-buffer
    (insert "a")
    (put-text-property 1 2 'qa-hooks--purge t)
    ;; Insert at point-max (outside the interval): verifies collect
    ;; both lists and find them nil, clearing the stale state.
    (goto-char 3)
    (insert "b")))

;; ---------- text-property modification-hooks ----------

;; Overlapping delete fires the interval's list once with (BEG END).
(qa-lex (let (l)
      (qa-hooks--purge)
      (with-temp-buffer
        (insert "abcd")
        (put-text-property 2 4 'modification-hooks
                           (list (lambda (b e) (push (list b e) l))))
        (delete-region 2 4))
      l)
    '((2 4)))

;; A delete that ends exactly where the property starts does NOT fire
;; it — GNU's overlap test is strict (`i->position < end').
(qa-lex (let (l)
      (qa-hooks--purge)
      (with-temp-buffer
        (insert "abcd")
        (put-text-property 2 4 'modification-hooks
                           (list (lambda (b e) (push (list b e) l))))
        (delete-region 1 2))
      l)
    nil)

;; ...and neither does a delete starting at the property's end.
(qa-lex (let (l)
      (qa-hooks--purge)
      (with-temp-buffer
        (insert "abcd")
        (put-text-property 2 4 'modification-hooks
                           (list (lambda (b e) (push (list b e) l))))
        (delete-region 4 5))
      l)
    nil)

;; Pure insertions do not run modification-hooks (verify_interval_
;; modification only collects insert-in-front/behind for start==end).
(qa-lex (let (l)
      (qa-hooks--purge)
      (with-temp-buffer
        (insert "abcd")
        (put-text-property 2 4 'modification-hooks
                           (list (lambda (b e) (push (list b e) l))))
        (goto-char 3)
        (insert "X"))
      l)
    nil)

;; put-text-property over an interval runs the hooks with the
;; property-change range (GNU signals it as a text modification).
(qa-lex (let (l)
      (qa-hooks--purge)
      (with-temp-buffer
        (insert "ab")
        (put-text-property 1 2 'modification-hooks
                           (list (lambda (b e) (push (list b e) l))))
        (put-text-property 1 2 'x 1))
      l)
    '((1 2)))

(qa-lex (let (l)
      (qa-hooks--purge)
      (with-temp-buffer
        (insert "ab")
        (put-text-property 1 2 'modification-hooks
                           (list (lambda (b e) (push (list b e) l))))
        (remove-text-properties 1 2 '(modification-hooks nil)))
      l)
    '((1 2)))

;; ---------- insert-in-front-hooks / insert-behind-hooks ----------

;; insert-behind-hooks fire when the char BEFORE the insertion point
;; is inside the interval (prev = interval containing start-1): at
;; the interval's end boundary and strictly inside, not at its start.
(qa-lex (let (l)
      (qa-hooks--purge)
      (with-temp-buffer
        (insert "abcd")
        (put-text-property 2 4 'insert-behind-hooks
                           (list (lambda (b e) (push (list b e) l))))
        (goto-char 4)
        (insert "X"))
      l)
    '((4 5)))

(qa-lex (let (l)
      (qa-hooks--purge)
      (with-temp-buffer
        (insert "abcd")
        (put-text-property 2 4 'insert-behind-hooks
                           (list (lambda (b e) (push (list b e) l))))
        (goto-char 3)
        (insert "X"))
      l)
    '((3 4)))

(qa-lex (let (l)
      (qa-hooks--purge)
      (with-temp-buffer
        (insert "abcd")
        (put-text-property 2 4 'insert-behind-hooks
                           (list (lambda (b e) (push (list b e) l))))
        (goto-char 2)
        (insert "X"))
      l)
    nil)

;; insert-in-front-hooks fire when the char AT the insertion point is
;; inside the interval: at its start and inside, not at its end.
(qa-lex (let (l)
      (qa-hooks--purge)
      (with-temp-buffer
        (insert "abcd")
        (put-text-property 2 4 'insert-in-front-hooks
                           (list (lambda (b e) (push (list b e) l))))
        (goto-char 2)
        (insert "X"))
      l)
    '((2 3)))

(qa-lex (let (l)
      (qa-hooks--purge)
      (with-temp-buffer
        (insert "abcd")
        (put-text-property 2 4 'insert-in-front-hooks
                           (list (lambda (b e) (push (list b e) l))))
        (goto-char 4)
        (insert "X"))
      l)
    nil)

;; Insert hook args are the inserted range (BEG END of new text).
(qa-lex (let (l)
      (qa-hooks--purge)
      (with-temp-buffer
        (insert "ab")
        (put-text-property 1 3 'insert-in-front-hooks
                           (list (lambda (b e) (push (list b e) l))))
        (goto-char 2)
        (insert "XY"))
      l)
    '((2 4)))

;; GNU's interval_insert_*_hooks are C globals that outlive the
;; report: the last verify that collected them leaves them set, and a
;; later insert into a property-free buffer (whose verify returns
;; before the reset) replays them with the new range.
(qa-lex (let (l)
      (qa-hooks--purge)
      (with-temp-buffer
        (insert "ab")
        (put-text-property 1 3 'insert-in-front-hooks
                           (list (lambda (b e) (push (list b e) l))))
        ;; Collect AND fire inside [1,3): pendings stay [λ].
        (goto-char 2)
        (insert "X"))
      ;; This buffer has no intervals: verify returns early and the
      ;; stale list fires on its first insert.
      (with-temp-buffer
        (insert "zz"))
      l)
    '((1 3) (2 3)))

;; ---------- overlay hooks ----------

;; before- and after-phase calls: (OVERLAY AFTER-P BEG END) before,
;; plus LEN (deleted length) after.
(qa-lex (let (l)
      (qa-hooks--purge)
      (with-temp-buffer
        (insert "abcd")
        (let ((ov (make-overlay 2 4)))
          (overlay-put ov 'modification-hooks
                       (list (lambda (o ap b e &optional len)
                               (push (list ap b e len) l))))
          (delete-region 3 4)))
      (nreverse l))
    '((nil 3 4 nil) (t 3 3 1)))

;; Insert at the overlay's start runs insert-in-front-hooks; at its
;; end runs insert-behind-hooks (overlay boundaries are markers).
(qa-lex (let (l)
      (qa-hooks--purge)
      (with-temp-buffer
        (insert "abcd")
        (let ((ov (make-overlay 2 4)))
          (overlay-put ov 'insert-in-front-hooks
                       (list (lambda (o ap b e &optional len)
                               (push (list ap b e len) l))))
          (goto-char 2)
          (insert "Q")))
      (nreverse l))
    '((nil 2 2 nil) (t 2 3 0)))

(qa-lex (let (l)
      (qa-hooks--purge)
      (with-temp-buffer
        (insert "abcd")
        (let ((ov (make-overlay 2 4)))
          (overlay-put ov 'insert-behind-hooks
                       (list (lambda (o ap b e &optional len)
                               (push (list ap b e len) l))))
          (goto-char 4)
          (insert "Q")))
      (nreverse l))
    '((nil 4 4 nil) (t 4 5 0)))

;; An insert strictly inside the overlay runs modification-hooks.
(qa-lex (let (l)
      (qa-hooks--purge)
      (with-temp-buffer
        (insert "abcd")
        (let ((ov (make-overlay 2 4)))
          (overlay-put ov 'modification-hooks
                       (list (lambda (o ap b e &optional len)
                               (push (list ap b e len) l))))
          (goto-char 3)
          (insert "Q")))
      (nreverse l))
    '((nil 3 3 nil) (t 3 4 0)))

;; ---------- ordering / args / flags ----------

;; after-change-functions see (BEG END LEN-DELETED); for a property
;; change LEN is the range length (GNU signals it as a modification).
(qa-lex (let (l)
      (qa-hooks--purge)
      (with-temp-buffer
        (insert "ab")
        (add-hook 'after-change-functions
                  (lambda (b e len) (push (list b e len) l)) nil t)
        (put-text-property 1 2 'x 1))
      l)
    '((1 2 1)))

;; interval modification-hooks fire before before-change-functions
;; (GNU runs verify_interval_modification first).
(qa-lex (let (l)
      (qa-hooks--purge)
      (with-temp-buffer
        (insert "ab")
        (put-text-property 1 2 'modification-hooks
                           (list (lambda (b e) (push 'mod l))))
        (add-hook 'before-change-functions
                  (lambda (b e) (push 'bcf l)) nil t)
        (delete-region 1 2))
      (nreverse l))
    '(mod bcf))

;; `inhibit-modification-hooks' suppresses everything (the same flag
;; `with-silent-modifications' binds).
(qa-lex (let (l)
      (qa-hooks--purge)
      (with-temp-buffer
        (insert "ab")
        (put-text-property 1 2 'modification-hooks
                           (list (lambda (b e) (push 'm l))))
        (let ((inhibit-modification-hooks t))
          (delete-region 1 2)))
      l)
    nil)

;; An erroring after-change function resets the hook variable
;; (GNU `reset_var_on_error').
(qa-lex (progn
      (qa-hooks--purge)
      (with-temp-buffer
        (insert "ab")
        (add-hook 'after-change-functions (lambda (b e l) (error "x")) nil t)
        (condition-case nil (insert "z") (error nil))
        after-change-functions))
    nil)

;; put-text-property signals buffer-read-only on a read-only buffer
;; (GNU `prepare_to_modify_buffer' runs barf-if-buffer-read-only).
(qa-lex (progn
      (qa-hooks--purge)
      (with-temp-buffer
        (insert "ab")
        (setq buffer-read-only t)
        (condition-case e (progn (put-text-property 1 2 'x 1) :no)
          (error (car e)))))
    'buffer-read-only)

;; OBJECT buffer arg: hooks fire in the OBJECT buffer (GNU
;; set_buffer_internal), not the current one.
(qa-lex (let (l)
      (qa-hooks--purge)
      (let ((ob (get-buffer-create " *qa-ob*")))
        (with-current-buffer ob (insert "ab"))
        (with-temp-buffer
          (put-text-property 1 2 'modification-hooks
                             (list (lambda (b e) (push (list b e) l)))
                             ob)
          (with-current-buffer ob (delete-region 1 2))))
      (nreverse l))
    '((1 2)))

;; ---------- combine-after-change-calls ----------

;; While bound, each change just records a (BEG END CHANGE) triple —
;; no hooks run until the binding drops; the next real change flushes
;; one merged call before its own.  (combine-after-change-execute
;; inside the let re-defers in GNU.)
(qa-lex (let (l)
      (qa-hooks--purge)
      (with-temp-buffer
        (add-hook 'after-change-functions
                  (lambda (b e len) (push (list b e len) l)) nil t)
        (let ((combine-after-change-calls t))
          (insert "ab")
          (insert "cd")
          (combine-after-change-execute))
        ;; The deferred flush inside this change's signal_after_change
        ;; delivers ONE merged call spanning the earlier edits, then
        ;; this change's own call.
        (insert "ef"))
      (nreverse l))
    '((1 7 2) (5 7 0)))

;; Buffer deleted before flush: nothing runs, list clears (GNU's
;; BUFFER_LIVE_P check).
(qa-lex (let (l)
      (qa-hooks--purge)
      (let ((ob (get-buffer-create " *qa-dead*")))
        (with-current-buffer ob
          (add-hook 'after-change-functions
                    (lambda (b e len) (push (list b e len) l)) nil t)
          (let ((combine-after-change-calls t))
            (insert "ab"))
          (kill-buffer ob))
        (combine-after-change-execute))
      l)
    nil)

(princ (qa--done))
