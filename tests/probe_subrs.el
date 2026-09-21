;; Missing-subr batch: new primitives, handler-bind, watchers.
;; Differential-verified against GNU Emacs 31.1.
(progn
  ;; trivial builtins
  (princ (list 'a (subr-primitive-p (symbol-function 'car))
               (subr-type (symbol-function 'car))
               (condition-case e (subr-type 5) (error (car e)))
               (subr-native-lambda-list (symbol-function 'car))
               (syntax-class-to-char 2) (syntax-class-to-char 15)
               (syntax-class-to-char 0)
               (condition-case e (syntax-class-to-char 99) (error (car e)))
               (matching-paren ?\() (matching-paren ?\)) (matching-paren ?a)
               (ngettext "x" "xs" 1) (ngettext "x" "xs" 3)
               (condition-case e (ngettext 'a 'b 1) (error (car e)))))
  (terpri)
  ;; access-file
  (let ((tf (make-temp-file "subrs-probe")))
    (princ (list 'b (access-file tf "ok %s")
                 (condition-case e (access-file "/nonexistent-dir-zzz/f" "msg %s")
                   (error (car e)))))
    (delete-file tf))
  (terpri)
  ;; single-char property changes with PROP/OBJECT/LIMIT
  (with-temp-buffer
    (insert "abcdef")
    (put-text-property 2 4 'face 'bold)
    (princ (list 'c (next-single-char-property-change 1 'face)
                 (next-single-char-property-change 2 'face)
                 (next-single-char-property-change 4 'face nil 6)
                 (next-single-char-property-change 6 'face)
                 (previous-single-char-property-change 5 'face)
                 (previous-single-char-property-change 4 'face)
                 (previous-single-char-property-change 2 'face)
                 (next-single-char-property-change 0 'face "abcd")
                 (previous-single-char-property-change 3 'face "abcd"))))
  (terpri)
  ;; base64url + get-byte
  (princ (list 'd (base64url-encode-string "a?b?") (base64url-encode-string "a?b?" t)
               (get-byte 0 "ABC") (get-byte 2 "ABC")
               (condition-case e (get-byte 0 "é") (error (cadr e)))))
  (terpri)
  ;; variable watchers: set/let/unlet/makunbound
  (let ((log nil))
    (add-variable-watcher 'vw-t1 (lambda (s n o w) (push (list s n o) log)))
    (setq vw-t1 1)
    (let ((vw-t1 2)) t)
    (makunbound 'vw-t1)
    (princ (list 'e (nreverse log) (length (get-variable-watchers 'vw-t1))))
    (remove-variable-watcher 'vw-t1 (lambda (s n o w) t))
    (terpri))
  ;; find-file-name-handler + misc
  (princ (list 'f (find-file-name-handler "/tmp/x" 'insert-file-contents)
               (combine-after-change-execute)
               (with-temp-buffer (make-overlay 1 1) (delete-all-overlays) t)
               (current-message)))
  (terpri)
  ;; replace-region-contents
  (with-temp-buffer
    (insert "hello world")
    (replace-region-contents 1 6 "goodbye ")
    (princ (list 'g (buffer-string))))
  (terpri)
  ;; bury-buffer-internal / make-indirect-buffer
  (princ (list 'h2 (with-temp-buffer
                     (bury-buffer-internal (current-buffer))
                     t)
               (bufferp (condition-case e
                            (make-indirect-buffer (current-buffer) "ind-probe")
                          (error e)))))
  (terpri)
  ;; handler-bind: caught signals do not fire handlers
  (let ((log nil))
    (handler-bind ((error (lambda (c) (push 'ran log))))
      (condition-case e (signal 'error '(x)) (error nil)))
    (princ (list 'h log)))
  (terpri)
  ;; handler ordering within one form: argument order, innermost first
  (let ((log nil))
    (catch 'done
      (handler-bind ((error (lambda (c) (push 'first log)))
                     (error (lambda (c) (push 'second log) (throw 'done 'esc))))
        (signal 'error '(boom))))
    (princ (list 'ord (nreverse log))))
  (terpri)
  ;; signal-hook-function sees caught signals; nested let so the hook
  ;; closure captures `log' (parallel `let' inits can't see own bindings).
  (let ((log nil))
    (let ((signal-hook-function (lambda (s d) (push (list 'hk s) log))))
      (condition-case e (signal 'arith-error '(z)) (error (push 'cc log))))
    (princ (list 'hook (nreverse log))))
  (terpri)
  ;; handler receives (sym . data); condition-list membership
  (let ((got nil))
    (catch 'out
      (handler-bind (((arith-error file-error)
                      (lambda (c) (setq got c) (throw 'out 'done))))
        (signal 'arith-error '(d1 d2))))
    (princ (list 'cond got)))
  (terpri)
  ;; macroexpand + throw-escape
  (princ (list 'i (macroexpand '(handler-bind ((error h)) b))))
  (terpri)
  (princ (list 'j (catch 'out
                    (handler-bind ((error (lambda (c) (throw 'out 'inner))))
                      (signal 'error '(e))))))
  (terpri)
  ;; condition-case-unless-debug
  (princ (list 'k (condition-case-unless-debug v (error "x") (error 'caught))))
  (terpri)
  ;; ignore-errors must suppress handler-bind handlers
  (let ((log nil))
    (handler-bind ((error (lambda (c) (push 'ran log))))
      (ignore-errors (signal 'error '(x))))
    (princ (list 'ie log)))
  (terpri)
  ;; autoload-do-load returns nil on non-autoload
  (princ (list 'l (autoload-do-load (symbol-function 'car) 'car)))
  (terpri)
  ;; load honors the lexical-binding cookie (nil cookie -> dynamic file)
  (let ((tf (make-temp-file "subrs-load" nil ".el")))
    (with-temp-file tf
      (insert "(defvar probe-dyn t) (let ((probe-dyn 'lex)) (setq probe-let-seen probe-dyn))"))
    (load tf)
    (delete-file tf))
  (princ (list 'load-dyn (boundp 'probe-dyn) (boundp 'probe-let-seen)))
  (terpri)
  (let ((tf (make-temp-file "subrs-load-lex" nil ".el")))
    (with-temp-file tf
      (insert ";; -*- lexical-binding: t -*-\n(defvar probe-lex t)"))
    (load tf)
    (delete-file tf))
  (princ (list 'load-lex (boundp 'probe-lex)))
  (terpri)
  (princ (list 'load-missing
               (condition-case e (load "/nonexistent-subrs-zzz.el")
                 (file-error (car e)))))
  (terpri)
  ;; make-indirect-buffer: API surface (name uniqueness, base link)
  (let ((b (get-buffer-create "subrs-base")))
    (with-current-buffer b (insert "HELLO"))
    (let ((ib (make-indirect-buffer b "subrs-ind" t)))
      (princ (list 'ind (bufferp ib) (buffer-name ib)
                   (eq (buffer-base-buffer ib) b)
                   (buffer-base-buffer b)
                   (with-current-buffer ib (buffer-string)))))
    (princ (list 'ind-dup
                 (condition-case e (make-indirect-buffer b "subrs-ind")
                   (error (car e)))))
    (kill-buffer "subrs-ind")
    (terpri))
  nil)
