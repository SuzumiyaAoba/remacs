;;; -*- lexical-binding: nil -*-
;; Issue #264: evaluation reads the accessible, live buffer stream.
(defvar qa264-events nil)
(defvar qa264-beg nil)
(defvar qa264-mid nil)
(defvar qa264-end nil)
(defvar qa264-whole nil)
(defun qa264-bound (value)
  (cond ((eq value 'beg) qa264-beg) ((eq value 'mid) qa264-mid)
        ((eq value 'end) qa264-end) ((eq value 'whole) qa264-whole)
        (t value)))
(defun qa264-run (kind start end &optional at-middle)
  (let (qa264-events qa264-beg qa264-mid qa264-end qa264-whole)
    (with-temp-buffer
      (insert "(push 'outside qa264-events)\n")
      (setq qa264-beg (point))
      (insert "(push 'inside qa264-events)\n")
      (setq qa264-mid (point))
      (insert "(push 'second qa264-events)\n")
      (setq qa264-end (point))
      (insert "(push 'hidden-tail qa264-events)\n")
      (setq qa264-whole (point))
      (narrow-to-region qa264-beg qa264-end)
      (goto-char (if at-middle qa264-mid qa264-beg))
      (let* ((old-point (point))
             (result (condition-case e
                         (if (eq kind 'buffer) (eval-buffer)
                           (eval-region (qa264-bound start) (qa264-bound end)))
                       (error e))))
        (list result (nreverse qa264-events) (= (point) old-point)
              (= (point-min) qa264-beg) (= (point-max) qa264-end))))))

(qa (qa264-run 'buffer nil nil) '(nil (inside second) t t t))
(qa (qa264-run 'buffer nil nil t) '(nil (inside second) t t t))
(qa (qa264-run 'region 'beg 'end) '(nil (inside second) t t t))
;; START is clipped before reading; even inaccessible values are legal.
(dolist (start '(1 0 -1 -1000000))
  (qa (qa264-run 'region start 'end) '(nil (inside second) t t t)))
(qa (qa264-run 'region 'beg nil) '(nil (inside second) t t t))
(qa (qa264-run 'region 'mid 'end) '(nil (second) t t t))
(qa (qa264-run 'region 'beg 'mid) '(nil (inside) t t t))
;; Empty/reversed regions do not evaluate a form or rearrange bounds.
(qa (qa264-run 'region 'mid 'mid) '(nil nil t t t))
(qa (qa264-run 'region 'end 'beg) '(nil nil t t t))
(qa (qa264-run 'region 'beg 1) '(nil nil t t t))
(dolist (start '(end whole 1000000))
  (qa (qa264-run 'region start 'end) '(nil nil t t t)))
;; Nil START means use live point; END is not consulted on this path.
(qa (qa264-run 'region nil nil) '(nil (inside second) nil t t))
(qa (qa264-run 'region nil nil t) '(nil (second) nil t t))
(dolist (end '(1 0 -4 1.5 bogus))
  (qa (qa264-run 'region nil end) '(nil (inside second) nil t t)))
(qa (qa264-run 'region nil (make-marker)) '(nil (inside second) nil t t))
(qa (car (qa264-run 'region 1.5 'end)) '(wrong-type-argument integer-or-marker-p 1.5))
(qa (car (qa264-run 'region 'beg 1.5)) '(wrong-type-argument integer-or-marker-p 1.5))
(qa (car (qa264-run 'region (make-marker) 'end)) '(error "Marker does not point anywhere"))
(qa (car (qa264-run 'region 'beg (make-marker))) '(error "Marker does not point anywhere"))
;; END validation is against the whole buffer and occurs before evaluation.
(qa (let ((r (qa264-run 'region 'beg 0))) (list (caar r) (cdr r)))
    '(args-out-of-range (nil t t t)))
(qa (let ((r (qa264-run 'region 'beg 1000000))) (list (caar r) (cdr r)))
    '(args-out-of-range (nil t t t)))
;; The issue's explicit full-buffer bounds must not execute a hidden prefix.
(qa (let (qa264-events)
      (with-temp-buffer
        (insert "(push 'outside qa264-events)\n(push 'inside qa264-events)")
        (let ((end (point-max)))
          (goto-char 1) (forward-line 1) (narrow-to-region (point) end)
          (list (eval-region 1 end) (nreverse qa264-events))))) '(nil (inside)))
;; Live and foreign markers contribute positions, not buffer identities.
(qa (let (qa264-events)
      (with-temp-buffer
        (insert "(push 'outside qa264-events)\n(push 'inside qa264-events)")
        (let ((start (copy-marker 1)) (end (point-max-marker)))
          (goto-char 1) (forward-line 1) (narrow-to-region (point) (point-max))
          (list (eval-region start end) (nreverse qa264-events))))) '(nil (inside)))
(qa (let ((foreign (generate-new-buffer " *qa264-foreign*")) qa264-events)
      (unwind-protect
          (let ((start (with-current-buffer foreign (insert "x") (copy-marker 1))))
            (with-temp-buffer
              (insert "(push 'inside qa264-events)")
              (list (eval-region start (point-max)) (nreverse qa264-events))))
        (kill-buffer foreign))) '(nil (inside)))
;; Unicode offsets are character positions; hidden invalid syntax is not read.
(qa (let (qa264-events)
      (with-temp-buffer
        (insert ") 日本語\n")
        (let ((beg (point)))
          (insert "(push 'inside qa264-events)")
          (let ((end (point)))
            (insert "\n) hidden") (narrow-to-region beg end)
            (list (eval-buffer) (nreverse qa264-events)))))) '(nil (inside)))
(qa (with-temp-buffer (insert ") hidden") (narrow-to-region 3 3) (eval-buffer)) nil)
;; No hidden prefix side effect may happen before a visible evaluation error.
(qa (let (qa264-events)
      (with-temp-buffer
        (insert "(push 'outside qa264-events)\n")
        (let ((beg (point)))
          (insert "(push 'inside qa264-events)\n(error \"stop\")\n(push 'after qa264-events)")
          (narrow-to-region beg (point-max))
          (list (condition-case e (eval-buffer) (error e)) (nreverse qa264-events)))))
    '((error "stop") (inside)))
;; Regions restore their point/restriction even on reader failure.
(qa (with-temp-buffer
      (insert ";; hidden\n")
      (let ((beg (point)))
        (insert "(list 1") (narrow-to-region beg (point-max))
        (goto-char beg)
        (let ((end (point-max)))
          (list (condition-case e (eval-region beg end)
                  (end-of-file (eq (cadr e) (current-buffer))))
                (= (point) beg) (= (point-min) beg) (= (point-max) end))))) '(t t t t))
;; The evaluated forms see the original region, not the temporary read limit.
(qa (let (qa264-events)
      (with-temp-buffer
        (insert "(setq qa264-events (list (point) (point-min) (point-max)))\n999")
        (let ((end (- (point-max) 3)) (whole (point-max)))
          (goto-char whole) (eval-region 1 end)
          (equal qa264-events (list whole 1 whole))))) t)
;; Evaluation can change narrowing and modify the remaining live input.
(qa (let (qa264-events)
      (with-temp-buffer
        (insert "(narrow-to-region (point-min) (point))\n(push 'after qa264-events)")
        (eval-buffer) qa264-events)) nil)
(qa (let (qa264-events)
      (with-temp-buffer
        (insert "(save-excursion (goto-char (point-max)) (insert \"(push 'added qa264-events)\"))\n")
        (eval-buffer) qa264-events)) '(added))
;; Source markers follow edits during evaluation without repeating forms.
(qa (let (qa264-events)
      (with-temp-buffer
        (insert "(save-excursion (goto-char 1) (insert \";; prefix\\n\"))\n(push 'after qa264-events)\n")
        (goto-char (point-max))
        (eval-region 1 (point-max))
        (list qa264-events (= (point) (point-max))))) '((after) t))
;; Custom readers receive the actual narrowed stream and EOF stops callbacks.
(qa (let ((calls 0) qa264-events)
      (with-temp-buffer
        (insert "(push 'outside qa264-events)\n")
        (let ((beg (point)))
          (insert "(push 'inside qa264-events)\n ; tail\n")
          (narrow-to-region beg (point-max))
          (eval-region 1 (point-max) nil
                       (lambda (stream) (setq calls (1+ calls)) (read stream)))
          (list calls qa264-events)))) '(1 (inside)))
(qa (let ((outer-input standard-input) (outer-list eval-buffer-list) qa264-events)
      (with-temp-buffer
        (insert "(setq qa264-events (list (eq standard-input (current-buffer)) (eq (car eval-buffer-list) (current-buffer))))")
        (eval-buffer))
      (list qa264-events (eq standard-input outer-input) (eq eval-buffer-list outer-list))) '((t t) t t))
;; Named source buffers keep their restriction and evaluate in the caller.
(qa (let ((source (generate-new-buffer " *qa264-source*")) qa264-events)
      (unwind-protect
          (with-temp-buffer
            (let ((caller (current-buffer)))
              (with-current-buffer source
                (insert "(error \"hidden\")\n")
                (let ((beg (point)))
                  (insert "(setq qa264-events (eq (current-buffer) caller))")
                  (narrow-to-region beg (point-max))))
              (eval-buffer (buffer-name source))
              (list qa264-events (eq (current-buffer) caller))))
        (kill-buffer source))) '(t t))
(qa (car (qa264-run 'region 1000000000000000000000000 nil))
    '(wrong-type-argument integer-or-marker-p 1000000000000000000000000))
(qa (car (qa264-run 'region -1000000000000000000000000 nil))
    '(wrong-type-argument integer-or-marker-p -1000000000000000000000000))
(dolist (kind '(buffer region))
  (qa (with-temp-buffer
        (insert ";; hidden\n")
        (let ((beg (point)))
          (insert ";; visible\n   )") (narrow-to-region beg (point-max))
          (condition-case e
              (if (eq kind 'buffer) (eval-buffer) (eval-region 1 (point-max)))
            (error e)))) '(invalid-read-syntax ")" 2 4)))
;; Indirect buffers synchronize their shared text while switching sources.
(qa (let ((base (generate-new-buffer " *qa264-base*")) indirect qa264-events)
      (unwind-protect
          (progn
            (with-current-buffer base (insert "(push 'inside qa264-events)"))
            (setq indirect (make-indirect-buffer base " *qa264-indirect*"))
            (with-current-buffer indirect
              (eval-region 1 (point-max)) qa264-events))
        (when (buffer-live-p indirect) (kill-buffer indirect))
        (kill-buffer base))) '(inside))

;; Explicit reads and custom load readers share the same source position.
(qa (let (qa264-events)
      (with-temp-buffer
        (insert "(setq qa264-events (read standard-input))\n(error \"not evaluated\")")
        (eval-buffer) qa264-events)) '(error "not evaluated"))
(qa (let ((calls 0) qa264-events
          (load-read-function (lambda (stream) (setq calls (1+ calls)) (read stream))))
      (with-temp-buffer
        (insert "(push 'inside qa264-events)\n ; comment\n")
        (eval-buffer) (list calls qa264-events))) '(1 (inside)))
(qa (let ((old-input standard-input) (old-list eval-buffer-list)
          (old-output standard-output))
      (with-temp-buffer
        (insert "(error \"stop\")")
        (condition-case e (eval-buffer) (error nil)))
      (list (eq old-input standard-input) (eq old-list eval-buffer-list)
            (eq old-output standard-output))) '(t t t))
;; Output controls and result values still use the dynamically bound stream.
(qa (with-output-to-string
      (with-temp-buffer (insert "(princ \"discard\") 42") (eval-buffer))) "")
(qa (let ((output (generate-new-buffer " *qa264-output*")) (values nil))
      (unwind-protect
          (with-temp-buffer
            (insert "1 2")
            (list (eval-buffer nil output) values
                  (with-current-buffer output (buffer-string))))
        (kill-buffer output))) '(nil (2 1) "\n1\n\n2\n"))
(qa (let ((output (generate-new-buffer " *qa264-output*")))
      (unwind-protect
          (with-temp-buffer
            (insert "(progn (setq standard-output (get-buffer \" *qa264-output*\")) 7)")
            (eval-buffer nil t)
            (with-current-buffer output (buffer-string)))
        (kill-buffer output))) "\n7\n")
;; A reader reaching point-max ends the loop even if its form moves point.
(qa (let ((calls 0))
      (with-temp-buffer
        (insert "(goto-char 1)")
        (list (eval-region 1 (point-max) nil
                           (lambda (stream) (setq calls (1+ calls)) (read stream))) calls)))
    '(nil 1))


(qa (let ((saved-read (symbol-function 'read)) (calls 0) qa264-events)
      (unwind-protect
          (progn
            (fset 'read (lambda (stream) (setq calls (1+ calls)) (funcall saved-read stream)))
            (with-temp-buffer (insert "(push 'inside qa264-events)") (eval-buffer))
            (list calls qa264-events))
        (fset 'read saved-read))) '(1 (inside)))
;; Same-length edits invalidate the cache even when the bounds stay fixed.
(qa (let (qa264-events)
      (with-temp-buffer
        (insert "(subst-char-in-region (point) (point-max) ?X ?Y)\n(push 'X qa264-events)")
        (eval-buffer) qa264-events)) '(Y))
;; Text identity matters even if its modification tick and length coincide.
(qa (let ((other (generate-new-buffer " *qa264-swap*")) qa264-events)
      (unwind-protect
          (with-temp-buffer
            (insert "(buffer-swap-text other)\n")
            (let ((next (point)))
              (insert "(push 'old qa264-events)")
              (with-current-buffer other
                (insert "(buffer-swap-text other)\n(push 'new qa264-events)")
                (put-text-property 1 2 'qa264 t)
                (insert " ") (delete-char -1)
                (goto-char next)))
            (let ((source (current-buffer)) (tick (buffer-chars-modified-tick)))
              (eval-buffer)
              (list qa264-events (= tick (buffer-chars-modified-tick source)))))
        (kill-buffer other))) '((new) t))


;; With nil START a read error leaves point after the offending delimiter.
(dolist (text '(")" "]" "."))
  (qa (with-temp-buffer
        (insert "hidden\n")
        (let ((beg (point)))
          (insert text) (narrow-to-region beg (point-max)) (goto-char beg)
          (list (condition-case e (eval-region nil nil) (error (car e)))
                (= (point) (point-max))))) '(invalid-read-syntax t)))

;; Issue #265: PRINTFLAG chooses discard/output and evaluation-result printing.
(defvar qa265-events nil)
(defun qa265-run (kind mode body)
 (let ((outer (generate-new-buffer " *qa265-outer*"))
       (target (generate-new-buffer " *qa265-target*"))
       (qa265-events nil))
  (unwind-protect
   (let ((standard-output outer) (values nil))
    (let ((flag (cond ((eq mode 'buffer) target)
                      ((eq mode 'marker) (with-current-buffer target (point-marker)))
                      ((eq mode 'function) (lambda (char) (push char qa265-events)))
                      (t nil))))
     (list
      (with-temp-buffer
       (insert ";;; -*- lexical-binding: nil; -*-\n" body) (goto-char 1)
       (condition-case err
        (progn (if (eq kind 'buffer) (eval-buffer nil flag)
                 (eval-region (point-min) (point-max) flag)) 'done)
        (error (list (car err) (cdr err)))))
      (with-current-buffer outer (buffer-string))
      (with-current-buffer target (buffer-string))
      (apply #'string (nreverse qa265-events))
      (eq standard-output outer) values)))
   (kill-buffer outer) (kill-buffer target))))
(qa
(qa265-run 'buffer 'discard "(princ \"payload\")")
'(done "" "" "" t nil)
)

(qa
(qa265-run 'buffer 'discard "(+ 1 2) (+ 3 4)")
'(done "" "" "" t nil)
)

(qa
(qa265-run 'buffer 'buffer "(princ \"payload\")")
'(done "" "payload\n\"payload\"\n" "" t ("payload"))
)

(qa
(qa265-run 'buffer 'buffer "(+ 1 2) (+ 3 4)")
'(done "" "\n3\n\n7\n" "" t (7 3))
)

(qa
(qa265-run 'buffer 'marker "(princ \"payload\")")
'(done "" "payload\n\"payload\"\n" "" t ("payload"))
)

(qa
(qa265-run 'buffer 'marker "(+ 1 2) (+ 3 4)")
'(done "" "\n3\n\n7\n" "" t (7 3))
)

(qa
(qa265-run 'buffer 'function "(princ \"payload\")")
'(done "" "" "payload\n\"payload\"\n" t ("payload"))
)

(qa
(qa265-run 'buffer 'function "(+ 1 2) (+ 3 4)")
'(done "" "" "\n3\n\n7\n" t (7 3))
)

(qa
(qa265-run 'region 'discard "(princ \"payload\")")
'(done "" "" "" t nil)
)

(qa
(qa265-run 'region 'discard "(+ 1 2) (+ 3 4)")
'(done "" "" "" t nil)
)

(qa
(qa265-run 'region 'buffer "(princ \"payload\")")
'(done "" "payload\n\"payload\"\n" "" t ("payload"))
)

(qa
(qa265-run 'region 'buffer "(+ 1 2) (+ 3 4)")
'(done "" "\n3\n\n7\n" "" t (7 3))
)

(qa
(qa265-run 'region 'marker "(princ \"payload\")")
'(done "" "payload\n\"payload\"\n" "" t ("payload"))
)

(qa
(qa265-run 'region 'marker "(+ 1 2) (+ 3 4)")
'(done "" "\n3\n\n7\n" "" t (7 3))
)

(qa
(qa265-run 'region 'function "(princ \"payload\")")
'(done "" "" "payload\n\"payload\"\n" t ("payload"))
)

(qa
(qa265-run 'region 'function "(+ 1 2) (+ 3 4)")
'(done "" "" "\n3\n\n7\n" t (7 3))
)

(princ (qa--done)) (terpri)
