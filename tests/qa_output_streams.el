;;; -*- lexical-binding: nil -*-
(defun q271-call (operation stream mode)
  (let ((args (pcase operation
                ('princ '("payload"))
                ('princ-empty '(""))
                ('prin1 '("payload"))
                ('print '("payload"))
                ('write-char '(88))
                (_ nil)))
        (fn (if (eq operation 'princ-empty) 'princ
              (if (eq operation 'terpri-ensure) 'terpri operation))))
    (if (eq mode 'explicit)
        (apply fn (append args (list stream) (if (eq operation 'terpri-ensure) '(t))))
      (let ((standard-output stream))
        (apply fn (append args (if (or (eq mode 'nil) (eq operation 'terpri-ensure)) '(nil))
                          (if (eq operation 'terpri-ensure) '(t))))))))
(defun q271-invalid (operation kind mode &optional bol)
  (let* ((target (generate-new-buffer " *q271-dead*"))
         (marker (with-current-buffer target (point-marker)))
         (stream (pcase kind ('buffer target) ('detached (make-marker)) (_ marker))))
    (kill-buffer target)
    (with-temp-buffer
      (insert "original")
      (goto-char (if bol 1 4))
      (set-buffer-modified-p nil)
      (let* ((before (current-buffer))
             (tick (buffer-modified-tick))
             (chars-tick (buffer-chars-modified-tick))
             (undo buffer-undo-list)
             (hooks 0)
             (before-change-functions (list (lambda (&rest _) (setq hooks (1+ hooks)))))
             (after-change-functions (list (lambda (&rest _) (setq hooks (1+ hooks)))))
             (result (condition-case err (list 'ok (q271-call operation stream mode))
                       (error err))))
        (list result (eq before (current-buffer)) (buffer-string) (point)
              (buffer-modified-p) (= tick (buffer-modified-tick))
              (marker-position marker) (marker-buffer marker)
              (= chars-tick (buffer-chars-modified-tick))
              (equal undo buffer-undo-list) hooks)))))

;; Invalid explicit and dynamically selected streams signal before touching
;; text, point, modified ticks, undo data, or modification hooks.  Empty
;; strings and terpri's ENSURE shortcut still validate their destinations.
(dolist (operation '(princ princ-empty prin1 print write-char terpri terpri-ensure))
  (dolist (kind '(buffer detached killed-marker))
    (dolist (mode '(explicit omitted nil))
      (qa (q271-invalid operation kind mode)
          (list (list 'error (if (eq kind 'buffer)
                                "Selecting deleted buffer"
                              "Marker does not point anywhere"))
                t "original" 4 nil t nil nil t t 0)))))

(dolist (kind '(buffer detached killed-marker))
  (dolist (mode '(explicit omitted nil))
    (qa (q271-invalid 'terpri-ensure kind mode t)
        (list (list 'error (if (eq kind 'buffer)
                              "Selecting deleted buffer"
                            "Marker does not point anywhere"))
              t "original" 1 nil t nil nil t t 0))))

;; A bad destination is rejected before an unreadable-object callback can
;; run arbitrary Lisp, including edits to the caller's buffer.
(defun q271-invalid-callback (operation kind)
  (let* ((target (generate-new-buffer " *q271-callback*"))
         (stream (if (eq kind 'buffer) target (make-marker))))
    (kill-buffer target)
    (with-temp-buffer
      (insert "original")
      (let* ((calls 0)
             (print-unreadable-function
              (lambda (&rest _) (setq calls (1+ calls)) (insert "BAD") "callback"))
             (result (condition-case err
                         (funcall operation (current-buffer) stream)
                       (error err))))
        (list result calls (buffer-string))))))
(dolist (operation '(princ prin1 print))
  (dolist (kind '(buffer detached))
    (qa (q271-invalid-callback operation kind)
        (list (list 'error (if (eq kind 'buffer)
                              "Selecting deleted buffer"
                            "Marker does not point anywhere"))
              0 "original"))))

;; Argument validation retains GNU's ordering relative to stream checking.
(qa-err-data (let ((dead (generate-new-buffer " *q271-type*")))
               (kill-buffer dead) (prin1 'x dead 1))
             'wrong-type-argument '(consp 1))
(qa-err-data (let ((dead (generate-new-buffer " *q271-type*")))
               (kill-buffer dead) (write-char "x" dead))
             'wrong-type-argument '(fixnump "x"))
(qa-err-data (let ((dead (generate-new-buffer " *q271-type*")))
               (kill-buffer dead) (write-char -1 dead))
             'error '("Selecting deleted buffer"))

;; Valid buffer and marker streams keep their insertion/point behavior.
(defun q271-live (operation kind readonly)
  (let ((target (generate-new-buffer " *q271-live*")))
    (unwind-protect
        (let ((marker (with-current-buffer target
                        (insert "ab") (goto-char 1)
                        (setq buffer-read-only readonly)
                        (copy-marker (if readonly 1 2)))))
          (with-temp-buffer
            (insert "original") (goto-char 4)
            (let* ((caller (current-buffer))
                   (stream (if (eq kind 'buffer) target marker))
                   (result (condition-case err (q271-call operation stream 'explicit)
                             (error (list 'error (car err))))))
              (list result (eq caller (current-buffer)) (buffer-string) (point)
                    (with-current-buffer target (list (buffer-string) (point)))
                    (marker-position marker)))))
      (kill-buffer target))))
(dolist (operation '(princ prin1 print write-char terpri))
  (dolist (kind '(buffer marker))
    (qa (q271-live operation kind t)
        '((error buffer-read-only) t "original" 4 ("ab" 1) 1))))
(qa (q271-live 'princ 'buffer nil)
    '("payload" t "original" 4 ("payloadab" 8) 9))
(qa (q271-live 'princ 'marker nil)
    '("payload" t "original" 4 ("apayloadb" 1) 9))
(qa (q271-live 'prin1 'marker nil)
    '("payload" t "original" 4 ("a\"payload\"b" 1) 11))
(qa (q271-live 'print 'marker nil)
    '("payload" t "original" 4 ("a\n\"payload\"\nb" 1) 13))
(qa (q271-live 'write-char 'marker nil)
    '(88 t "original" 4 ("aXb" 1) 3))
(qa (q271-live 'terpri 'marker nil)
    '(t t "original" 4 ("a\nb" 1) 3))
(qa (q271-live 'princ-empty 'marker nil)
    '("" t "original" 4 ("ab" 1) 2))

;; The current-buffer marker control from the issue, and ordinary captured
;; and function output, remain unchanged.
(qa (with-temp-buffer
      (insert "ab") (goto-char 1)
      (let ((marker (copy-marker 2)))
        (princ "X" marker)
        (list (buffer-string) (point) (marker-position marker))))
    '("aXb" 1 3))
(qa (with-output-to-string (princ "a") (prin1 "b") (write-char 99) (terpri))
    "a\"b\"c\n")
(qa (let ((chars nil))
      (princ "ab" (lambda (char) (push char chars)))
      (nreverse chars))
    '(97 98))
(qa (with-temp-buffer (list (terpri (current-buffer) t) (buffer-string)))
    '(nil ""))
(qa (let ((target (generate-new-buffer " *q271-resolved*"))
          (other (generate-new-buffer " *q271-other*")))
      (unwind-protect
          (let ((standard-output target)
                (print-unreadable-function
                 (lambda (&rest _) (setq standard-output other) "callback")))
            (prin1 (current-buffer))
            (list (with-current-buffer target (buffer-string))
                  (with-current-buffer other (buffer-string))))
        (kill-buffer target) (kill-buffer other)))
    '("callback" ""))
(princ (qa--done))
