;; Buffer objects — GNU 31.1 -Q --batch ground truth.
;;
;; GNU truths pinned:
;;   - In batch the current buffer is *scratch*, EMPTY (size 0).
;;   - generate-new-buffer uniquifies ("x" then "x" → "x<2>"); names
;;     starting with a space are hidden but still in buffer-list.
;;   - get-buffer-create is idempotent; get-buffer returns nil for a
;;     dead or missing buffer, and accepts a buffer object itself.
;;   - kill-buffer returns t; after kill, buffer-live-p → nil and
;;     buffer-name → nil.  kill-buffer nil kills the CURRENT buffer.
;;   - set-buffer accepts buffer objects AND names.
;;   - bury-buffer keeps the buffer alive; (bury-buffer) → nil.
;;   - with-current-buffer/save-current-buffer restore the buffer.

;; == surface ==
(qa (fboundp 'generate-new-buffer) t)
(qa (fboundp 'get-buffer-create) t)
(qa (fboundp 'get-buffer) t)
(qa (fboundp 'current-buffer) t)
(qa (fboundp 'set-buffer) t)
(qa (fboundp 'buffer-name) t)
(qa (fboundp 'buffer-string) t)
(qa (fboundp 'buffer-size) t)
(qa (fboundp 'buffer-live-p) t)
(qa (fboundp 'buffer-list) t)
(qa (fboundp 'kill-buffer) t)
(qa (fboundp 'bury-buffer) t)
(qa (fboundp 'other-buffer) t)
(qa (fboundp 'bufferp) t)
(qa (fboundp 'buffer-modified-p) t)
(qa (fboundp 'buffer-file-name) t)

;; == batch current buffer ==
(qa (bufferp (current-buffer)) t)
(qa (buffer-name (current-buffer)) "*scratch*")
(qa (buffer-size) 0)
(qa (list (point-min) (point-max) (point)) '(1 1 1))
(qa (bufferp nil) nil)
(qa (bufferp "x") nil)

;; == generate-new-buffer ==
(qa (let ((buf (generate-new-buffer " *qab-t1*")))
      (prog1 (list (bufferp buf)
                   (buffer-name buf)
                   (buffer-live-p buf)
                   (eq buf (current-buffer))
                   (with-current-buffer buf (buffer-size)))
        (kill-buffer buf)))
    '(t " *qab-t1*" t nil 0))
;; uniquification
(qa (let ((b1 (generate-new-buffer "qab-gn"))
          (b2 (generate-new-buffer "qab-gn")))
      (prog1 (list (buffer-name b1) (buffer-name b2) (eq b1 b2))
        (kill-buffer b1)
        (kill-buffer b2)))
    '("qab-gn" "qab-gn<2>" nil))

;; == get-buffer-create / get-buffer ==
(qa (get-buffer "qab-nosuch-zz") nil)
(qa (let ((b (get-buffer-create "qab-nb")))
      (prog1
          (list (bufferp b)
                (eq (get-buffer "qab-nb") b)
                (eq (get-buffer-create "qab-nb") b) ; idempotent
                (eq (get-buffer b) b)              ; accepts objects
                (buffer-live-p b))
        (kill-buffer b)))
    '(t t t t t))
;; dead buffer: live-p nil, name nil, get-buffer nil
(qa (let ((b (get-buffer-create "qab-dead")))
      (kill-buffer b)
      (list (buffer-live-p b) (buffer-name b) (get-buffer "qab-dead")))
    '(nil nil nil))
;; get-buffer-create on existing name reuses contents
(qa (let ((b (get-buffer-create "qab-nb3")))
      (with-current-buffer b (insert "zz"))
      (prog1 (with-current-buffer (get-buffer-create "qab-nb3")
               (buffer-string))
        (kill-buffer b)))
    "zz")

;; == set-buffer / with-current-buffer / save-current-buffer ==
(qa (let ((buf (generate-new-buffer " *qab-t2*")))
      (prog1
          (list (let ((orig (current-buffer)))
                  (set-buffer buf)
                  (prog1 (eq (current-buffer) buf)
                    (set-buffer orig)))
                (let ((orig (current-buffer)))
                  (set-buffer (buffer-name buf))   ; name accepted
                  (prog1 (eq (current-buffer) buf)
                    (set-buffer orig)))
                (save-current-buffer (set-buffer buf) (insert "X") 'done)
                (with-current-buffer buf (buffer-string)))
        (kill-buffer buf)))
    '(t t done "X"))
;; with-current-buffer accepts names
(qa (let ((buf (generate-new-buffer " *qab-t2b*")))
      (with-current-buffer buf (insert "a"))
      (prog1 (with-current-buffer (buffer-name buf) (point))
        (kill-buffer buf)))
    2)

;; == with-temp-buffer ==
(qa (with-temp-buffer (buffer-name)) " *temp*")
(qa (with-temp-buffer (insert "abc") (buffer-string)) "abc")

;; == insert / erase / size ==
(qa (with-temp-buffer
      (list (insert "hello")
            (buffer-size)
            (point)
            (buffer-string)
            (erase-buffer)
            (buffer-size)
            (point)
            (buffer-string)))
    '(nil 5 6 "hello" nil 0 1 ""))
;; buffer-size counts CHARS not bytes for multibyte
(qa (with-temp-buffer
      (insert "αβ")
      (list (buffer-size) (multibyte-string-p (buffer-string))))
    '(2 t))

;; == kill-buffer ==
(qa (kill-buffer (generate-new-buffer " *qab-t5*")) t)
(qa (let ((b (get-buffer-create "qab-kb")))
      (list (kill-buffer "qab-kb") (buffer-live-p b))) ; by name
    '(t nil))
;; no arg kills current; nil arg too (nil = current buffer)
(qa (let ((b (generate-new-buffer " *qab-t6*")))
      (list (with-current-buffer b (kill-buffer))
            (buffer-live-p b)))
    '(t nil))
(qa (let ((b (generate-new-buffer " *qab-t6b*")))
      (list (with-current-buffer b (kill-buffer nil))
            (buffer-live-p b)))
    '(t nil))
;; killing an already-dead buffer returns nil
(qa (let ((b (generate-new-buffer " *qab-t7*")))
      (kill-buffer b)
      (kill-buffer b))
    nil)
;; errors
(qa-err (set-buffer "qab-nosuch-zz") 'error)
(qa-err (buffer-name 5) 'wrong-type-argument)
(qa-err (get-buffer 5) 'wrong-type-argument)

;; == buffer-list / bury-buffer / other-buffer ==
(qa (consp (buffer-list)) t)
(qa (if (memq (current-buffer) (buffer-list)) t nil) t)
(qa (let ((b (generate-new-buffer " *qab-t8*")))
      ;; hidden (leading-space) buffers still appear in buffer-list
      (prog1 (if (memq b (buffer-list)) t nil)
        (bury-buffer b)
        (kill-buffer b)))
    t)
(qa (let ((b (generate-new-buffer " *qab-t8b*")))
      (bury-buffer b)
      (prog1 (buffer-live-p b) (kill-buffer b)))
    t)
(qa (bury-buffer) nil)
(qa (bufferp (other-buffer)) t)
;; other-buffer with an arg EXCLUDES that buffer from the result
(qa (let ((b (generate-new-buffer "qab-ob")))
      (prog1 (eq (other-buffer b) b) (kill-buffer b)))
    nil)
;; Killing the current buffer switches to a NON-displayed buffer
;; (GNU kill_buffer → replace_buffer_in_windows uses other-buffer's
;; "prefer not shown in windows" rule).
(qa (let ((b (get-buffer-create "qab-kb"))
          (shown (window-buffer (selected-window))))
      (prog2 (set-buffer b)
          (progn (kill-buffer b)
                 (and shown
                      (not (eq (current-buffer)
                               (window-buffer (selected-window))))))
        nil))
    t)


;; == buffer-file-name / modified ==
(qa (with-temp-buffer (buffer-file-name)) nil)
(qa (with-temp-buffer (buffer-modified-p)) nil)
(qa (with-temp-buffer
      (insert "x")
      (list (buffer-modified-p)
            (progn (set-buffer-modified-p nil) (buffer-modified-p))))
    '(t nil))

;; == buffer-substring keeps properties ==
(qa (with-temp-buffer
      (insert "ab")
      (put-text-property 1 2 'x 1)
      (list (get-text-property 0 'x (buffer-string))
            (get-text-property 1 'x (buffer-string))
            (buffer-substring-no-properties 1 3)))
    '(1 nil "ab"))

;; == point / bounds ==
(qa (with-temp-buffer
      (insert "abc")
      (list (point-min) (point-max) (buffer-size) (point)
            (progn (goto-char 1) (point))
            (bobp) (eobp)))
    '(1 4 3 4 1 t nil))


;; == indirect buffers (GNU Emacs 31.1 verified) ==
(qa (let* ((b (get-buffer-create "qaib")) (i (make-indirect-buffer b "qaib-i")))
      (unwind-protect
          (list (eq (buffer-base-buffer i) b) (buffer-base-buffer b)
                (progn (with-current-buffer i (insert "via-i"))
                       (with-current-buffer b (buffer-string)))
                (progn (with-current-buffer b (insert "via-b"))
                       (with-current-buffer i (buffer-string))))
        (mapc 'kill-buffer (list i b))))
    '(t nil "via-i" "via-bvia-i"))
(qa (let* ((b (get-buffer-create "qanb")) (i (make-indirect-buffer b "qanb-i")))
      (unwind-protect
          (progn (with-current-buffer b
                   (insert "abcdefghij") (goto-char 3) (narrow-to-region 2 8))
                 (with-current-buffer i
                   (list (buffer-string) (point-min) (point-max) (point))))
        (mapc 'kill-buffer (list i b))))
    '("abcdefghij" 1 11 1))
(qa (let* ((b (get-buffer-create "qakb")) (i (make-indirect-buffer b "qakb-i")))
      (kill-buffer b)
      (unwind-protect
          (list (buffer-name i) (buffer-live-p i)
                (condition-case e (progn (with-current-buffer i (buffer-string)) :ok)
                  (error (car e)))
                (condition-case e (progn (buffer-base-buffer i) :ok)
                  (error (car e))))
        (kill-buffer i)))
    '(nil nil error :ok))
(qa (let* ((b (get-buffer-create "qasb")) (i (make-indirect-buffer b "qasb-i")))
      (unwind-protect
          (condition-case e (progn (with-current-buffer i (buffer-swap-text b)) :ok)
            (error (cdr e)))
        (mapc 'kill-buffer (list i b))))
    '("Cannot swap indirect buffers’s text"))
(qa (let* ((b (get-buffer-create "qamb")) (i (make-indirect-buffer b "qamb-i")))
      (unwind-protect
          (condition-case e (progn (with-current-buffer i (set-buffer-multibyte nil)) :ok)
            (error (cdr e)))
        (mapc 'kill-buffer (list i b))))
    '("Cannot do ‘set-buffer-multibyte’ on an indirect buffer"))
(qa (let* ((b (get-buffer-create "qam2")) (i (make-indirect-buffer b "qam2-i")))
      (unwind-protect
          (list (progn (with-current-buffer b (insert "x")) (buffer-modified-p i))
                (progn (with-current-buffer i (set-buffer-modified-p nil))
                       (buffer-modified-p b))
                (progn (with-current-buffer i (set-buffer-modified-p t))
                       (buffer-modified-p b)))
        (mapc 'kill-buffer (list i b))))
    '(t nil t))


;; == insert-byte: GNU BYTE8_TO_CHAR (insdel.c) ==
;; bytes >=0x80 in a multibyte buffer become the eight-bit-graphic
;; escape char 0x3FFF00+byte; in a unibyte buffer the raw byte lands.
(qa (with-temp-buffer
      (insert-byte 233 1)
      (car (string-to-list (buffer-string))))
    4194281)
(qa (with-temp-buffer
      (insert-byte 65 2)
      (buffer-string))
    "AA")
(qa (with-temp-buffer
      (set-buffer-multibyte nil)
      (insert-byte 233 1)
      (car (string-to-list (buffer-string))))
    233)
(qa (condition-case e (with-temp-buffer (insert-byte 256 1) :ok) (error (car e)))
    'args-out-of-range)
(qa (condition-case e (with-temp-buffer (insert-byte -1 1) :ok) (error (car e)))
    'args-out-of-range)
(qa (with-temp-buffer (insert-byte 200 2) (buffer-size)) 2)

(princ (qa--done))
