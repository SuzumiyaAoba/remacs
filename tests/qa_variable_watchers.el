;;; -*- lexical-binding: t; -*-
(qa (let ((v (make-symbol "qa137")))
      (add-variable-watcher v 'ignore) (add-variable-watcher v 'car)
      (get-variable-watchers v)) '(car ignore))
(qa (let ((v (make-symbol "qa137")))
      (add-variable-watcher v 'ignore) (add-variable-watcher v 'ignore)
      (get-variable-watchers v)) '(ignore))
(qa (let* ((v (make-symbol "qa137"))
           (first (list 'lambda '(s n o w) nil))
           (second (copy-tree first)))
      (add-variable-watcher v first) (add-variable-watcher v second)
      (list (length (get-variable-watchers v))
            (eq (car (get-variable-watchers v)) first))) '(1 t))
(qa (let* ((v (make-symbol "qa137"))
           (watch (list 'lambda '(s n o w) nil)))
      (add-variable-watcher v watch)
      (remove-variable-watcher v (copy-tree watch))
      (get-variable-watchers v)) nil)
(qa (let ((v (make-symbol "qa137")))
      (add-variable-watcher v 'ignore) (add-variable-watcher v 'car)
      (add-variable-watcher v 'ignore)
      (get-variable-watchers v)) '(car ignore))
(qa (let* ((v (make-symbol "qa137")) (events nil)
           (first (lambda (&rest args) (push 'first events)))
           (second (lambda (&rest args) (push 'second events))))
      (add-variable-watcher v first) (add-variable-watcher v second)
      (set v 1) (nreverse events)) '(second first))
(qa (let* ((alias (make-symbol "qa137-alias")) (base (make-symbol "qa137-base"))
           (events nil)
           (watch (lambda (s n o w)
                    (push (list (eq s alias) (eq n base) o w
                                (eq (indirect-variable alias) alias)) events))))
      (set base 7) (add-variable-watcher alias watch)
      (defvaralias alias base) events)
    '((t t defvaralias nil t)))
(qa (let* ((alias (make-symbol "qa137-alias")) (base (make-symbol "qa137-base")))
      (set base 7) (add-variable-watcher alias 'ignore)
      (defvaralias alias base)
      (list (get-variable-watchers alias) (get-variable-watchers base))) '(nil nil))
(qa (let* ((alias (intern "qa137-interned-alias")) (base (intern "qa137-interned-base"))
           (events nil)
           (watch (lambda (s n o w) (push (list (eq s base) n o w) events))))
      (unwind-protect
          (progn (set base 7) (defvaralias alias base)
                 (add-variable-watcher alias watch) (set alias 8)
                 (list events (eq (car (get-variable-watchers base)) watch)
                       (eq (car (get-variable-watchers alias)) watch)))
        (remove-variable-watcher base watch) (unintern alias nil) (unintern base nil)))
    '(((t 8 set nil)) t t))
(qa (let* ((alias (intern "qa137-remove-alias")) (base (intern "qa137-remove-base")))
      (unwind-protect
          (progn (set base 7) (defvaralias alias base)
                 (add-variable-watcher base 'ignore)
                 (remove-variable-watcher alias 'ignore)
                 (list (get-variable-watchers base) (get-variable-watchers alias)))
        (remove-variable-watcher base 'ignore) (unintern alias nil) (unintern base nil))) '(nil nil))
(qa (let* ((alias (make-symbol "qa137-error-alias")) (base (make-symbol "qa137-base"))
           (watch (lambda (&rest args) (error "stop"))))
      (set base 7) (add-variable-watcher alias watch)
      (condition-case nil (defvaralias alias base) (error nil))
      (eq (indirect-variable alias) alias)) t)
;; The report's final example incorrectly claims that per-buffer setq
;; skips watchers. GNU 31.1 notifies for both setq and set.
(qa (with-temp-buffer
      (let* ((events nil)
             (watch (lambda (s n o w)
                      (push (list n o (eq w (current-buffer))) events))))
        (unwind-protect
            (progn (add-variable-watcher 'fill-column watch)
                   (setq fill-column 42) (set 'fill-column 43)
                   (kill-local-variable 'fill-column)
                   (list (nreverse events) fill-column))
          (remove-variable-watcher 'fill-column watch))))
    '(((42 set t) (43 set t)) 70))

;; A condition-case handler's dynamic variable can fail to bind.  Errors
;; raised while unwinding that failed binding replace the original error.
(defvar qa137-handler-var nil)
(defvar qa137-handler-events nil)
(defun qa137-handler-watcher (_symbol _value operation _where)
  (push operation qa137-handler-events)
  (cond ((eq operation 'let) (signal 'file-error '("bind")))
        ((eq operation 'unlet) (signal 'arith-error nil))))
(qa (let ((target (make-symbol "qa137-handler")))
      (setq qa137-handler-events nil)
      (add-variable-watcher target #'qa137-handler-watcher)
      (unwind-protect
          (let ((signal-handler
                 (condition-case e
                     (eval (list 'condition-case target
                                 '(error "body")
                                 (list 'error 'handled)))
                   (error (car e)))))
            (setq qa137-handler-events nil)
            (list signal-handler
                  (condition-case e
                      (eval (list 'condition-case target 7
                                  (list :success 'handled)))
                    (error (car e)))
                  (nreverse qa137-handler-events)))
        (remove-variable-watcher target #'qa137-handler-watcher)))
    '(arith-error arith-error (let unlet)))

;; GNU pops the dynamic-binding record before the unlet watcher runs. If
;; that watcher signals, the binding value remains visible after the unwind.
(defvar qa137-unlet-error-variable nil)
(defun qa137-unlet-error-watcher (_symbol _value operation _where)
  (when (eq operation 'unlet)
    (signal 'arith-error '("unlet watcher"))))
(qa (let ((qa137-unlet-error-variable 'outer))
      (add-variable-watcher 'qa137-unlet-error-variable
                            #'qa137-unlet-error-watcher)
      (unwind-protect
          (list (condition-case err
                    (let ((qa137-unlet-error-variable 'inner)) 'body)
                  (error (car err)))
                qa137-unlet-error-variable)
        (remove-variable-watcher 'qa137-unlet-error-variable
                                 #'qa137-unlet-error-watcher)))
    '(arith-error inner))

;; A local binding saves its original value before its `let' watcher runs.
;; A watcher may write the variable, but unwinding the binding restores the
;; value that was present when `let' began.
(defvar qa137-local-let-variable nil)
(qa
 (with-temp-buffer
   (setq-local qa137-local-let-variable 'outer)
   (let ((watch (lambda (_symbol _value operation _where)
                  (when (eq operation 'let)
                    (setq-local qa137-local-let-variable 'mutated)))))
     (add-variable-watcher 'qa137-local-let-variable watch)
     (unwind-protect
         (list (let ((qa137-local-let-variable 'inner))
                 qa137-local-let-variable)
               qa137-local-let-variable)
       (remove-variable-watcher 'qa137-local-let-variable watch))))
 '(inner outer))

;; A `let' watcher can change the current buffer before the dynamic value is
;; stored.  The original local cell still unwinds in its original buffer.
(defvar qa137-local-let-buffer-switch nil)
(qa
 (let* ((first (generate-new-buffer "*qa-local-let-first*"))
        (second (generate-new-buffer "*qa-local-let-second*"))
        (watch (lambda (_symbol _value operation _where)
                 (when (eq operation 'let)
                   (set-buffer second)))))
   (unwind-protect
       (with-current-buffer first
         (setq-local qa137-local-let-buffer-switch 'outer)
         (add-variable-watcher 'qa137-local-let-buffer-switch watch)
         (unwind-protect
             (list (let ((qa137-local-let-buffer-switch 'inner))
                     (list (eq (current-buffer) second)
                           qa137-local-let-buffer-switch
                           (with-current-buffer first
                             qa137-local-let-buffer-switch)))
                   (with-current-buffer first qa137-local-let-buffer-switch)
                   (with-current-buffer second qa137-local-let-buffer-switch))
           (remove-variable-watcher 'qa137-local-let-buffer-switch watch)))
     (kill-buffer first)
     (kill-buffer second)))
 '((t inner outer) outer inner))

(defvar qa137-local-let-error-variable nil)
(defvar qa137-local-let-error-events nil)
(defun qa137-local-let-error-watcher (_symbol _value operation _where)
  (push operation qa137-local-let-error-events)
  (when (eq operation 'let)
    (signal 'file-error '("local bind"))))
(qa
 (with-temp-buffer
   (setq-local qa137-local-let-error-variable 'outer)
   (let ((qa137-local-let-error-events nil)
         (watch #'qa137-local-let-error-watcher))
     (add-variable-watcher 'qa137-local-let-error-variable watch)
     (unwind-protect
         (list (condition-case err
                   (let ((qa137-local-let-error-variable 'inner)) 'body)
                 (error (car err)))
               qa137-local-let-error-variable
               (nreverse qa137-local-let-error-events))
       (remove-variable-watcher 'qa137-local-let-error-variable watch))))
 '(file-error outer (let unlet)))

;; Text-change hook scopes must stop on bind errors and propagate cleanup
;; errors after the change has already happened.
(defun qa137-before-change-watcher (_symbol _value operation _where)
  (if (eq operation 'let) (signal 'file-error '("before-change bind")) nil))
(qa (with-temp-buffer
      (add-variable-watcher 'inhibit-modification-hooks
                            #'qa137-before-change-watcher)
      (unwind-protect
          (list (condition-case e (insert "x") (error (car e)))
                (buffer-string) inhibit-modification-hooks)
        (remove-variable-watcher 'inhibit-modification-hooks
                                 #'qa137-before-change-watcher)))
    '(file-error "" nil))

(defvar qa137-after-change-bind-count 0)
(defun qa137-after-change-watcher (_symbol _value operation _where)
  (if (eq operation 'let)
      (progn
        (setq qa137-after-change-bind-count
              (1+ qa137-after-change-bind-count))
        (if (= qa137-after-change-bind-count 2)
            (signal 'file-error '("after-change bind"))
          nil))
    nil))
(qa (let ((qa137-after-change-bind-count 0))
      (with-temp-buffer
        (add-variable-watcher 'inhibit-modification-hooks
                              #'qa137-after-change-watcher)
        (unwind-protect
            (list (condition-case e (insert "x") (error (car e)))
                  (buffer-string) inhibit-modification-hooks)
          (remove-variable-watcher 'inhibit-modification-hooks
                                   #'qa137-after-change-watcher))))
    '(file-error "x" nil))

;; GNU's reset_var_on_error bypasses variable watchers when clearing a failing
;; before-change hook, so the next edit proceeds without invoking it again.
(defun qa137-reset-hook-fail (&rest _args)
  (signal 'file-error '("hook failure")))
(defun qa137-reset-hook-watcher (_symbol _value operation _where)
  (when (eq operation 'set)
    (signal 'arith-error '("reset watcher failure"))))
(qa (let ((before-change-functions '(qa137-reset-hook-fail)))
      (add-variable-watcher 'before-change-functions
                            #'qa137-reset-hook-watcher)
      (unwind-protect
          (with-temp-buffer
            (let ((first (condition-case err
                             (progn (insert "x") 'ok)
                           (error (car err)))))
              (list first before-change-functions
                    (condition-case err
                        (progn (insert "y") 'ok)
                      (error (car err)))
                    (buffer-string))))
        (remove-variable-watcher 'before-change-functions
                                 #'qa137-reset-hook-watcher)))
    '(file-error nil ok "y"))

(qa (let ((after-change-functions '(qa137-reset-hook-fail)))
      (add-variable-watcher 'after-change-functions
                            #'qa137-reset-hook-watcher)
      (unwind-protect
          (with-temp-buffer
            (let ((first (condition-case err
                             (progn (insert "x") 'ok)
                           (error (car err)))))
              (list first after-change-functions
                    (condition-case err
                        (progn (insert "y") 'ok)
                      (error (car err)))
                    (buffer-string))))
        (remove-variable-watcher 'after-change-functions
                                 #'qa137-reset-hook-watcher)))
    '(file-error nil ok "xy"))

(qa (with-temp-buffer
      (setq-local before-change-functions '(qa137-reset-hook-fail))
      (add-variable-watcher 'before-change-functions
                            #'qa137-reset-hook-watcher)
      (unwind-protect
          (let ((first (condition-case err
                           (progn (insert "x") 'ok)
                         (error (car err)))))
            (list first before-change-functions
                  (condition-case err
                      (progn (insert "y") 'ok)
                    (error (car err)))
                  (buffer-string)))
        (remove-variable-watcher 'before-change-functions
                                 #'qa137-reset-hook-watcher)))
    '(file-error nil ok "y"))

;; Minor-mode variables are assigned with Lisp setq semantics: a signaling
;; watcher aborts the command and leaves the binding unchanged.
(defun qa137-fail-blink-mode-set (_symbol _value operation _where)
  (when (eq operation 'set)
    (signal 'file-error '("blink mode watcher"))))
(qa
 (let ((blink-cursor-mode nil))
   (add-variable-watcher 'blink-cursor-mode #'qa137-fail-blink-mode-set)
   (unwind-protect
       (list (condition-case error
                 (blink-cursor-mode 1)
               (error (car error)))
             blink-cursor-mode)
     (remove-variable-watcher 'blink-cursor-mode #'qa137-fail-blink-mode-set)))
 '(file-error nil))
(qa
 (let ((blink-cursor-mode nil))
   (blink-cursor-mode 1)
   (let ((enabled blink-cursor-mode))
     (blink-cursor-mode 0)
     (list enabled blink-cursor-mode)))
 '(t nil))
(qa
 (let ((blink-cursor-mode nil)
       (qa137-blink-mode-hook-ran nil)
       (blink-cursor-mode-hook
        (list (lambda () (setq qa137-blink-mode-hook-ran t)))))
   (blink-cursor-mode 1)
   qa137-blink-mode-hook-ran)
 't)

;; C's new-fontset registry updates its forwarded alias table directly, so
;; Lisp variable watchers are not called for this internal assignment.
(defvar qa137-fontset-watcher-events nil)
(defun qa137-fontset-watcher (symbol value operation where)
  (push (list symbol value operation where) qa137-fontset-watcher-events))
(qa
 (let ((fontset-alias-alist nil)
       (qa137-fontset-watcher-events nil))
   (add-variable-watcher 'fontset-alias-alist #'qa137-fontset-watcher)
   (unwind-protect
       (let ((name "-*-fixed-medium-r-normal--16-*-*-*-*-*-fontset-qa"))
         (list (new-fontset name nil)
               fontset-alias-alist
               qa137-fontset-watcher-events))
     (remove-variable-watcher 'fontset-alias-alist #'qa137-fontset-watcher)))
 '("-*-fixed-medium-r-normal--16-*-*-*-*-*-fontset-qa"
   (("-*-fixed-medium-r-normal--16-*-*-*-*-*-fontset-qa"
     . "-*-fixed-medium-r-normal--16-*-*-*-*-*-fontset-qa"))
   nil))

(qa
 (with-temp-buffer
   (insert "abc")
   (let ((killed-rectangle nil))
     (add-variable-watcher 'killed-rectangle #'qa137-fail-blink-mode-set)
     (unwind-protect
         (list (condition-case error
                   (copy-rectangle-as-kill 1 3)
                 (error (car error)))
               killed-rectangle)
       (remove-variable-watcher 'killed-rectangle #'qa137-fail-blink-mode-set))))
 '(file-error nil))

(qa
 (with-temp-buffer
   (insert "abc")
   (let ((killed-rectangle nil))
     (add-variable-watcher 'killed-rectangle #'qa137-fail-blink-mode-set)
     (unwind-protect
         (list (condition-case error
                   (kill-rectangle 1 3)
                 (error (car error)))
               (buffer-string)
               killed-rectangle)
       (remove-variable-watcher 'killed-rectangle #'qa137-fail-blink-mode-set))))
 '(file-error "c" nil))

(qa
 (let ((kill-ring '("first" "second"))
       (kill-ring-yank-pointer nil))
   (add-variable-watcher 'kill-ring-yank-pointer #'qa137-fail-blink-mode-set)
   (unwind-protect
       (list (condition-case error
                 (rotate-yank-pointer 1)
               (error (car error)))
             kill-ring-yank-pointer)
     (remove-variable-watcher 'kill-ring-yank-pointer #'qa137-fail-blink-mode-set)))
 '(file-error nil))

(qa
 (let ((kill-ring '("first"))
       (kill-ring-yank-pointer nil)
       (interprogram-paste-function
        (lambda () (signal 'file-error '("paste failed")))))
   (condition-case error
       (list :success (rotate-yank-pointer 0))
     (error (car error))))
 'file-error)

(qa
 (with-temp-buffer
   (let ((create-lockfiles nil)
         (file (make-temp-name
                (expand-file-name "qa-visited-file-number-"
                                  temporary-file-directory))))
     (cl-letf (((symbol-function 'file-attribute-file-identifier)
                (lambda (&rest _)
                  (signal 'file-error '("identifier failed")))))
       (list (condition-case error
                 (set-visited-file-name file nil t)
               (error (car error)))
             (equal (buffer-file-name) file)))))
 '(file-error t))

(defun qa137-fail-buffer-file-number-set (_symbol _value operation _where)
  (when (eq operation 'set)
    (signal 'file-error '("buffer file number watcher"))))
(qa
 (with-temp-buffer
   (let ((create-lockfiles nil)
         (file (make-temp-name
                (expand-file-name "qa-buffer-file-number-"
                                  temporary-file-directory))))
     (add-variable-watcher 'buffer-file-number
                           #'qa137-fail-buffer-file-number-set)
     (unwind-protect
         (list (condition-case error
                   (set-visited-file-name file nil t)
                 (error (car error)))
               (local-variable-p 'buffer-file-number)
               buffer-file-number)
       (remove-variable-watcher 'buffer-file-number
                                #'qa137-fail-buffer-file-number-set))))
 '(file-error nil nil))

(qa
 (with-temp-buffer
   (let ((create-lockfiles nil)
         (file (make-temp-name
                (expand-file-name "qa-buffer-file-name-"
                                  temporary-file-directory))))
     (add-variable-watcher 'buffer-file-name #'qa137-fail-blink-mode-set)
     (unwind-protect
         (list (condition-case error
                   (set-visited-file-name file nil t)
                 (error (car error)))
               (equal buffer-file-name file)
               buffer-file-name)
       (remove-variable-watcher 'buffer-file-name #'qa137-fail-blink-mode-set))))
 '(file-error nil nil))

(defun qa137-fail-default-directory-set (_symbol _value operation _where)
  (when (eq operation 'set)
    (signal 'file-error '("default directory watcher"))))
(qa
 (with-temp-buffer
   (let ((create-lockfiles nil)
         (old-directory default-directory)
         (file (make-temp-name
                (expand-file-name "qa-default-directory-"
                                  temporary-file-directory))))
     (add-variable-watcher 'default-directory
                           #'qa137-fail-default-directory-set)
     (unwind-protect
         (list (condition-case error
                   (set-visited-file-name file nil t)
                 (error (car error)))
               (equal buffer-file-name file)
               (equal default-directory old-directory))
       (remove-variable-watcher 'default-directory
                                #'qa137-fail-default-directory-set))))
 '(file-error t t))

(defvar qa137-prefix-set-count 0)
(defun qa137-prefix-watcher (_symbol _value operation _where)
  (when (eq operation 'set)
    (setq qa137-prefix-set-count (1+ qa137-prefix-set-count))
    (when (= qa137-prefix-set-count 2)
      (signal 'file-error '("prefix watcher")))))
(qa
 (let ((prefix-arg nil)
       (current-prefix-arg 2)
       (last-prefix-arg nil)
       (qa137-prefix-set-count 0))
   (add-variable-watcher 'prefix-arg #'qa137-prefix-watcher)
   (unwind-protect
       (list (condition-case error
                 (universal-argument)
               (error (car error)))
             qa137-prefix-set-count
             prefix-arg)
     (remove-variable-watcher 'prefix-arg #'qa137-prefix-watcher)))
 '(file-error 2 2))

(qa
 (let ((prefix-arg nil)
       (current-prefix-arg 2)
       (last-prefix-arg nil)
       (last-command-event ?3)
       (qa137-prefix-set-count 0))
   (add-variable-watcher 'prefix-arg #'qa137-prefix-watcher)
   (unwind-protect
       (list (condition-case error
                 (digit-argument 2)
               (error (car error)))
             qa137-prefix-set-count
             prefix-arg)
     (remove-variable-watcher 'prefix-arg #'qa137-prefix-watcher)))
 '(file-error 2 2))

(qa
 (let ((prefix-arg nil)
       (current-prefix-arg 2)
       (last-prefix-arg nil)
       (qa137-prefix-set-count 0))
   (add-variable-watcher 'prefix-arg #'qa137-prefix-watcher)
   (unwind-protect
       (list (condition-case error
                 (negative-argument 3)
               (error (car error)))
             qa137-prefix-set-count
             prefix-arg)
     (remove-variable-watcher 'prefix-arg #'qa137-prefix-watcher)))
 '(file-error 2 2))

(qa
 (let ((scroll-lock-mode nil)
       (pixel-scroll-mode nil)
       (pixel-scroll-precision-mode nil))
   (dolist (symbol '(scroll-lock-mode pixel-scroll-mode
                     pixel-scroll-precision-mode))
     (add-variable-watcher symbol #'qa137-fail-blink-mode-set))
   (unwind-protect
       (list (condition-case error (scroll-lock-mode 1) (error (car error)))
             scroll-lock-mode
             (condition-case error (pixel-scroll-mode 1) (error (car error)))
             pixel-scroll-mode
             (condition-case error
                 (pixel-scroll-precision-mode 1)
               (error (car error)))
             pixel-scroll-precision-mode)
     (dolist (symbol '(scroll-lock-mode pixel-scroll-mode
                       pixel-scroll-precision-mode))
       (remove-variable-watcher symbol #'qa137-fail-blink-mode-set))))
 '(file-error nil file-error nil file-error nil))

(qa
 (progn
   (require 'isearch)
   (let ((isearch-mode nil))
     (add-variable-watcher 'isearch-mode #'qa137-fail-blink-mode-set)
     (unwind-protect
         (condition-case error
             (progn (execute-kbd-macro (vector ?\C-s ?\C-g)) 'returned)
           (error (car error)))
       (remove-variable-watcher 'isearch-mode #'qa137-fail-blink-mode-set))))
 'file-error)

;; Operation names are fixed GNU symbols even after their public names are
;; removed from the obarray.
(defvar qa137-uninterned-operations
  '(set let unlet defvaralias makunbound))
(defvar qa137-uninterned-events nil)
(defvar qa137-uninterned-variable nil)
(defun qa137-uninterned-watcher (_symbol _value operation _where)
  (push operation qa137-uninterned-events))
(qa
 (let* ((operations qa137-uninterned-operations)
        (variable 'qa137-uninterned-variable)
        (alias (make-symbol "qa137-uninterned-alias"))
        (base (make-symbol "qa137-uninterned-base"))
        (watcher #'qa137-uninterned-watcher))
   (setq qa137-uninterned-events nil qa137-uninterned-variable nil)
   (set base 'base-value)
   (unintern (nth 0 operations) obarray)
   (unintern (nth 1 operations) obarray)
   (unintern (nth 2 operations) obarray)
   (unintern (nth 3 operations) obarray)
   (unintern (nth 4 operations) obarray)
   (add-variable-watcher variable watcher)
   (add-variable-watcher alias watcher)
   (setq qa137-uninterned-variable 1)
   (let ((qa137-uninterned-variable 2)) nil)
   (defvaralias alias base)
   (makunbound variable)
   (remove-variable-watcher variable watcher)
   (remove-variable-watcher alias watcher)
   (list (equal (nreverse qa137-uninterned-events) operations)
         (mapcar (lambda (operation)
                   (and (null (intern-soft (symbol-name operation))) t))
                 operations)))
 '(t (t t t t t)))

(princ (qa--done))
