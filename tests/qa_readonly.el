;; GNU `verify_interval_modification' read-only surface — verified
;; against GNU Emacs 31.1 -Q --batch (2026-10-15).
;;
;; GNU truths pinned:
;; - Insert before read-only text is allowed unless the right char's
;;   `front-sticky' covers `read-only'; insert after it is blocked
;;   unless the left char's `rear-nonsticky' covers `read-only'.
;; - `inhibit-read-only' non-list non-nil bypasses everything; a LIST
;;   bypasses only the `read-only' VALUES it memq-contains.
;; - The `inhibit-read-only' TEXT PROPERTY marks an interval
;;   expressly writable.
;; - `read-only' resolves through `category' and
;;   `char-property-alias-alist' (lookup_char_property).
;; - `default-text-properties' NEVER gates: GNU skips
;;   verify_interval_modification when the buffer has no interval
;;   tree, and when it does, defaults only consult inside a real
;;   lookup... actually GNU's `lookup_char_property' DOES read
;;   default-text-properties, but only after a buffer already has
;;   intervals.  A props-free buffer ignores them.
;; - `text-read-only' carries the offending VALUE as data only when
;;   that value is a string (GNU text_read_only).
;; - `buffer-read-only' beats `text-read-only': barf fires before the
;;   interval walk (GNU prepare_to_modify_buffer_1 order).
;; - `put/add/set-text-properties' signal buffer-read-only even when
;;   they would change nothing; `remove-text-properties' and
;;   `remove-list-of-text-properties' return nil early when no listed
;;   property exists in the range — WITHOUT touching the read-only
;;   checks.
(let ((mk (lambda (setup body)
            (with-temp-buffer
              (insert "abcd")
              (funcall setup)
              (condition-case e (progn (funcall body) :ok)
                (error (car e)))))))
  ;; insert to the LEFT of read-only char: allowed (no front-sticky).
  (qa (funcall mk (lambda () (put-text-property 3 4 'read-only t))
               (lambda () (goto-char 3) (insert "X")))
      :ok)
  ;; same, but the right char is front-sticky for read-only: blocked.
  (qa (funcall mk (lambda ()
                    (let ((inhibit-read-only t))
                      (put-text-property 3 4 'read-only t)
                      (put-text-property 3 4 'front-sticky '(read-only))))
               (lambda () (goto-char 3) (insert "X")))
      'text-read-only)
  ;; inside a read-only span: blocked.
  (qa (funcall mk (lambda () (put-text-property 2 4 'read-only t))
               (lambda () (goto-char 3) (insert "X")))
      'text-read-only)
  ;; right AFTER the read-only char: blocked unless rear-nonsticky.
  (qa (funcall mk (lambda () (put-text-property 2 3 'read-only t))
               (lambda () (goto-char 3) (insert "X")))
      'text-read-only)
  (qa (funcall mk (lambda ()
                    (let ((inhibit-read-only t))
                      (put-text-property 2 3 'read-only t)
                      (put-text-property 2 3 'rear-nonsticky '(read-only))))
               (lambda () (goto-char 3) (insert "X")))
      :ok)
  ;; t rear-nonsticky covers every property.
  (qa (funcall mk (lambda ()
                    (let ((inhibit-read-only t))
                      (put-text-property 2 3 'read-only t)
                      (put-text-property 2 3 'rear-nonsticky t)))
               (lambda () (goto-char 3) (insert "X")))
      :ok)
  ;; inhibit-read-only values.
  (qa (let ((inhibit-read-only t))
        (funcall mk (lambda () (put-text-property 2 4 'read-only t))
                 (lambda () (delete-region 2 3))))
      :ok)
  (qa (let ((inhibit-read-only '(myro)))
        (funcall mk (lambda () (put-text-property 2 4 'read-only 'myro))
                 (lambda () (delete-region 2 3))))
      :ok)
  (qa (let ((inhibit-read-only '(other)))
        (funcall mk (lambda () (put-text-property 2 4 'read-only 'myro))
                 (lambda () (delete-region 2 3))))
      'text-read-only)
  ;; the inhibit-read-only PROPERTY grants writability.
  (qa (funcall mk (lambda () (let ((inhibit-read-only t))
                               (put-text-property 2 4 'read-only t)
                               (put-text-property 2 4 'inhibit-read-only t)))
               (lambda () (delete-region 2 3)))
      :ok)
  ;; category-supplied read-only blocks deletes...
  (qa (funcall mk (lambda () (put 'qa-cat-ro 'read-only t)
                    (put-text-property 2 4 'category 'qa-cat-ro))
               (lambda () (delete-region 2 3)))
      'text-read-only)
  ;; ...but insertion BEFORE the category-read-only char is allowed:
  ;; `after' is non-nil (category resolves it) yet front-sticky only
  ;; covers `category', not `read-only'.
  (qa (funcall mk (lambda () (put 'qa-cat-ro2 'read-only t)
                    (put-text-property 3 4 'category 'qa-cat-ro2))
               (lambda () (goto-char 3) (insert "X")))
      :ok)
  ;; char-property-alias-alist: (read-only locked) makes `locked' an
  ;; alias consulted during the read-only lookup.  GNU keeps the
  ;; variable GLOBAL (local-variable-p → nil in -Q *scratch*), so a
  ;; `setq' in the temp buffer is the portable setup — a `let' around
  ;; the call would bind buffer-locally in whichever buffer the eval
  ;; happens in and miss the temp buffer's lookups.
  (qa (with-temp-buffer
        (insert "abcd")
        (setq char-property-alias-alist '((read-only locked)))
        (put-text-property 2 4 'locked t)
        (condition-case e (progn (delete-region 2 3) :ok)
          (error (car e))))
      'text-read-only)
  ;; default-text-properties: NO interval tree → not consulted.
  (qa (let ((default-text-properties '(read-only t)))
        (with-temp-buffer (insert "ab")
                          (condition-case e (delete-region 1 2)
                            (error (car e)))))
      nil)
  ;; ...and once the writer builds the interval tree, defaults gate
  ;; the whole buffer — the `put-text-property' call itself signals.
  (qa (let ((default-text-properties '(read-only t)))
        (with-temp-buffer
          (insert "ab")
          (condition-case e (progn (put-text-property 1 2 'face 'bold) :ok)
            (error (car e)))))
      'text-read-only)
  ;; the put writes the props under inhibit-read-only → delete on a
  ;; defaulted region still reads read-only from the defaults.
  (qa (let ((default-text-properties '(read-only t)))
        (with-temp-buffer
          (insert "ab")
          (let ((inhibit-read-only t)) (put-text-property 1 2 'face 'bold))
          (condition-case e (delete-region 2 3)
            (error (car e)))))
      'text-read-only)
  ;; error data: string read-only value rides along, others don't.
  (qa-err-data (with-temp-buffer
                 (insert "abcd") (put-text-property 2 4 'read-only "nope")
                 (delete-region 2 3))
               'text-read-only '("nope"))
  (qa-err-data (with-temp-buffer
                 (insert "abcd") (put-text-property 2 4 'read-only 'sym)
                 (delete-region 2 3))
               'text-read-only '())
  ;; buffer-read-only fires before the interval checks; its datum is
  ;; the buffer object.
  (qa-err (with-temp-buffer
            (insert "ab") (setq buffer-read-only t)
            (insert "X"))
          'buffer-read-only)
  (qa-err (with-temp-buffer
            (insert "ab") (setq buffer-read-only t)
            (put-text-property 1 2 'face 'bold))
          'buffer-read-only)
  (qa-err (with-temp-buffer
            (insert "ab") (setq buffer-read-only t)
            (add-text-properties 1 2 '(face bold)))
          'buffer-read-only)
  ;; set-text-properties nil PROPS on a no-interval buffer returns
  ;; early — GNU's validate_interval_range soft path means
  ;; buffer-read-only is never consulted.
  (qa (with-temp-buffer
        (insert "ab") (setq buffer-read-only t)
        (condition-case e (progn (set-text-properties 1 2 nil) :ok)
          (error (car e))))
      :ok)
  ;; ...but once ANY interval exists, the same call signals.
  (qa-err (with-temp-buffer
            (insert "ab") (put-text-property 1 2 'face 'bold)
            (setq buffer-read-only t)
            (set-text-properties 2 3 nil))
          'buffer-read-only)
  ;; add/put skip everything when the range already holds the same
  ;; `eq' values — GNU's interval_has_all_properties gate means even
  ;; buffer-read-only goes unchecked.
  (qa (with-temp-buffer
        (insert "ab") (put-text-property 1 3 'face 'bold)
        (setq buffer-read-only t)
        (condition-case e (progn (put-text-property 1 2 'face 'bold) :ok)
          (error (car e))))
      :ok)
  ;; ...but an `equal'-but-not-`eq' value does NOT qualify — GNU
  ;; compares with EQ, so the write proceeds and hits the check.
  (qa-err (with-temp-buffer
            (insert "ab") (put-text-property 1 3 'face (list 'a))
            (setq buffer-read-only t)
            (put-text-property 1 2 'face (list 'a)))
          'buffer-read-only)
  ;; Writing nil to an ABSENT property still counts as a change.
  (qa-err (with-temp-buffer
            (insert "abcd") (put-text-property 2 4 'read-only t)
            (put-text-property 2 3 'other nil))
          'text-read-only)
  ;; empty-range writes return early — no read-only consult.
  (qa (funcall mk (lambda () (put-text-property 1 3 'read-only t))
               (lambda () (put-text-property 2 2 'face 'bold)))
      :ok)
  ;; strings never consult read-only state.
  (qa (let ((s (copy-sequence "ab")))
        (put-text-property 0 1 'read-only t s)
        (condition-case e (progn (put-text-property 0 1 'face 'bold s) :ok)
          (error (car e))))
      :ok)
  ;; remove-* are lazy: no listed property in range → silent nil,
  ;; even in a read-only buffer / on read-only text.
  (qa (funcall mk (lambda () (setq buffer-read-only t))
               (lambda () (remove-text-properties 2 3 '(face))))
      :ok)
  (qa (funcall mk (lambda () (setq buffer-read-only t))
               (lambda () (remove-list-of-text-properties 2 3 '(face))))
      :ok)
  ;; ...but when the property IS there, remove signals like a write.
  (qa-err (with-temp-buffer
            (insert "abcd") (setq buffer-read-only t)
            (let ((inhibit-read-only t))
              (put-text-property 2 3 'face 'bold))
            (remove-text-properties 2 3 '(face)))
          'buffer-read-only)
  ;; text read-only also gates property writes.
  ;; text read-only also gates property writes — qa-err wants the
  ;; RAW signalling form (framework catches it).
  (qa-err (with-temp-buffer
            (insert "abcd") (put-text-property 2 4 'read-only t)
            (put-text-property 2 3 'face 'bold))
          'text-read-only))

(princ (qa--done))
