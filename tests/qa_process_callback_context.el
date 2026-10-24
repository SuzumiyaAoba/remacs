;;; Asynchronous process callbacks preserve the interrupted context. -*- lexical-binding: t; -*-

(defun qa-pcc-wait (process ready)
  (let ((deadline (+ (float-time) 3)))
    (while (and (not (funcall ready)) (< (float-time) deadline))
      (accept-process-output process 0.01)))
  (funcall ready))

(defun qa-pcc-context (kind source tail)
  (let ((owner (generate-new-buffer " *qa-pcc-owner*"))
        (other (generate-new-buffer " *qa-pcc-other*"))
        (inhibit-quit 'outside) (last-nonmenu-event 'outside)
        (deactivate-mark nil) before seen p)
    (unwind-protect
        (with-current-buffer owner
          (insert "abc") (goto-char 1)
          (let ((callback
                 (lambda (_process _text)
                   (setq seen (list (eq (current-buffer) owner)
                                    (eq inhibit-quit t)
                                    (eq last-nonmenu-event t)
                                    (equal (match-data t) before)))
                   (goto-char 2)
                   (string-match "z" "z")
                   (setq deactivate-mark t inhibit-quit 'changed
                         last-nonmenu-event 'changed)
                   (set-buffer other))))
            (setq p (make-process
                     :name "qa-pcc" :buffer other :noquery t
                     :connection-type 'pipe :coding 'utf-8-unix
                     :filter (if (eq kind 'filter) callback #'ignore)
                     :sentinel (if (eq kind 'sentinel) callback #'ignore)
                     :command (list "/bin/sh" "-c"
                                    (if tail "printf '\\343'" "printf payload")))))
          (cond ((eq source 'string) (string-match "\\(b\\)" "abc"))
                ((eq source 'buffer) (re-search-forward "\\(b\\)"))
                (t (set-match-data nil)))
          (goto-char 1)
          (setq deactivate-mark nil)
          (setq before (match-data t))
          (qa-pcc-wait p (lambda () seen))
          (list seen (eq (current-buffer) owner) (point)
                (equal (match-data t) before) deactivate-mark
                inhibit-quit last-nonmenu-event))
      (when p (set-process-sentinel p #'ignore) (delete-process p))
      (kill-buffer owner) (kill-buffer other))))

(defvar qa-pcc-kind)
(defvar qa-pcc-source)
(defvar qa-pcc-tail)
(dolist (qa-pcc-kind '(filter sentinel))
  (dolist (qa-pcc-source '(nil string buffer))
    (dolist (qa-pcc-tail '(nil t))
      (qa (qa-pcc-context qa-pcc-kind qa-pcc-source qa-pcc-tail)
          '((t t t t) t 2 t t outside outside)))))

;; A nested wait must restore the outer callback's state, before the outer
;; callback in turn restores the interrupted Lisp caller's state.
(defun qa-pcc-nested (outer-kind inner-kind)
  (let ((owner (generate-new-buffer " *qa-pcc-outer*"))
        (middle (generate-new-buffer " *qa-pcc-middle*"))
        (other (generate-new-buffer " *qa-pcc-inner*"))
        (inhibit-quit nil) (last-nonmenu-event 'outside)
        (deactivate-mark nil) outer-seen inner-seen p q)
    (unwind-protect
        (with-current-buffer owner
          (let ((inner
                 (lambda (_process _text)
                   (setq inner-seen (list (eq (current-buffer) middle)
                                          (eq inhibit-quit t)
                                          (eq last-nonmenu-event t)
                                          (match-data t)))
                   (string-match "\\(inner\\)" "inner")
                   (set-buffer other)
                   (setq deactivate-mark 'inner))))
            (setq q (make-process
                     :name "qa-pcc-inner" :noquery t :connection-type 'pipe
                     :command '("/bin/sh" "-c" "read go; printf nested; read done")
                     :filter (if (eq inner-kind 'filter) inner #'ignore)
                     :sentinel (if (eq inner-kind 'sentinel) inner #'ignore))))
          (let ((outer
                 (lambda (_process _text)
                   (set-buffer middle)
                   (string-match "\\(outer\\)" "outer")
                   (setq deactivate-mark 'outer last-nonmenu-event 'outer)
                   (if (eq inner-kind 'sentinel)
                       (delete-process q)
                     (process-send-string q "go\n")
                     (qa-pcc-wait q (lambda () inner-seen)))
                   (setq outer-seen
                         (list (eq (current-buffer) middle)
                               (match-data t)
                               last-nonmenu-event)))))
            (setq p (make-process
                     :name "qa-pcc-outer" :noquery t :connection-type 'pipe
                     :command '("/bin/sh" "-c" "printf outer")
                     :filter (if (eq outer-kind 'filter) outer #'ignore)
                     :sentinel (if (eq outer-kind 'sentinel) outer #'ignore))))
          (string-match "\\(caller\\)" "caller")
          (qa-pcc-wait p (lambda () outer-seen))
          (list inner-seen outer-seen (eq (current-buffer) owner)
                (match-string 1 "caller")
                inhibit-quit last-nonmenu-event))
      (dolist (process (list p q))
        (when process
          (set-process-sentinel process #'ignore) (delete-process process)))
      (kill-buffer owner) (kill-buffer middle) (kill-buffer other))))

(dolist (qa-pcc-kind '(filter sentinel))
  (dolist (qa-pcc-source '(filter sentinel))
    (qa (qa-pcc-nested qa-pcc-kind qa-pcc-source)
        '((t t t (0 5 0 5)) (t (0 6 0 6) outer)
          t "caller" nil outside))))

;; Restoring the current buffer must not restore point or resurrect a buffer
;; killed by the callback. Dynamic bindings still unwind across a throw.
(defun qa-pcc-leave (kind leave)
  (let ((owner (generate-new-buffer " *qa-pcc-leave*"))
        (other (generate-new-buffer " *qa-pcc-survivor*"))
        (inhibit-quit nil) (last-nonmenu-event 'outside) seen p result)
    (unwind-protect
        (with-current-buffer owner
          (let ((callback
                 (lambda (_process _text)
                   (setq seen t)
                   (when (eq leave 'kill) (kill-buffer owner))
                   (set-buffer other)
                   (when (eq leave 'throw) (throw 'qa-pcc 'escaped)))))
            (setq p (make-process
                     :name "qa-pcc-leave" :noquery t :connection-type 'pipe
                     :command '("/bin/sh" "-c" "printf payload")
                     :filter (if (eq kind 'filter) callback #'ignore)
                     :sentinel (if (eq kind 'sentinel) callback #'ignore))))
          (setq result (catch 'qa-pcc (qa-pcc-wait p (lambda () seen)) 'returned))
          (list seen result (buffer-live-p owner)
                (eq (current-buffer) (if (eq leave 'kill) other owner))
                inhibit-quit last-nonmenu-event))
      (when p (set-process-sentinel p #'ignore) (delete-process p))
      (when (buffer-live-p owner) (kill-buffer owner))
      (kill-buffer other))))

;; The scope includes default insertion/change hooks and separately decoded
;; stderr, rather than protecting only the explicit filter function call.
(defun qa-pcc-insertion (stderr)
  (let ((owner (current-buffer))
        (destination (generate-new-buffer " *qa-pcc-output*"))
        (inhibit-quit nil) (last-nonmenu-event 'outside) seen p errors)
    (unwind-protect
        (progn
          (with-current-buffer destination
            (setq-local after-change-functions
                        (list (lambda (&rest _)
                                (setq seen (list (eq (current-buffer) destination)
                                                 (eq inhibit-quit t)
                                                 (eq last-nonmenu-event t)))
                                (string-match "hook" "hook")))))
          (when stderr
            (setq errors (make-pipe-process :name "qa-pcc-stderr" :noquery t
                                            :buffer destination :sentinel #'ignore)))
          (setq p (make-process
                   :name "qa-pcc-insert" :noquery t :sentinel #'ignore
                   :connection-type 'pipe
                   :buffer (unless stderr destination)
                   :stderr errors
                   :command (list "/bin/sh" "-c"
                                  (if stderr "printf payload >&2" "printf payload"))))
          (string-match "\\(caller\\)" "caller")
          (qa-pcc-wait nil (lambda () seen))
          (list seen (eq (current-buffer) owner) (match-string 1 "caller")
                inhibit-quit last-nonmenu-event
                (with-current-buffer destination (buffer-string))))
      (when p (set-process-sentinel p #'ignore) (delete-process p))
      (when errors (delete-process errors))
      (kill-buffer destination))))

(qa (qa-pcc-insertion nil) '((t t t) t "caller" nil outside "payload"))
(qa (qa-pcc-insertion t) '((t t t) t "caller" nil outside "payload"))

(defvar qa-pcc-decoded)
(defun qa-pcc-post-read (length)
  (setq qa-pcc-decoded (list (eq inhibit-quit t) (eq last-nonmenu-event t)))
  (string-match "\\(decoded\\)" "decoded")
  length)

(defun qa-pcc-conversion ()
  (let ((old (coding-system-get 'utf-8 :post-read-conversion))
        (owner (current-buffer))
        (inhibit-quit nil) (last-nonmenu-event 'outside) qa-pcc-decoded filtered p)
    (unwind-protect
        (progn
          (coding-system-put 'utf-8 :post-read-conversion #'qa-pcc-post-read)
          (setq p (make-process
                   :name "qa-pcc-convert" :noquery t :sentinel #'ignore
                   :connection-type 'pipe :coding 'utf-8
                   :command '("/bin/sh" "-c" "printf payload")
                   :filter (lambda (_process _text)
                             (setq filtered (list (eq (current-buffer) owner)
                                                  (match-string 1 "decoded"))))))
          (string-match "\\(caller\\)" "caller")
          (qa-pcc-wait p (lambda () filtered))
          (list qa-pcc-decoded filtered (match-string 1 "caller")
                inhibit-quit last-nonmenu-event))
      (coding-system-put 'utf-8 :post-read-conversion old)
      (when p (set-process-sentinel p #'ignore) (delete-process p)))))

(qa (qa-pcc-conversion) '((t t) (t "decoded") "caller" nil outside))

;; Search entry points share GNU's lazy save, including predicates and
;; failed searches. The first such call clears the callback's own registers
;; while retaining the interrupted caller's data for restoration.
(defun qa-pcc-search (form)
  (with-temp-buffer
    (insert "abc") (goto-char 1)
    (let (done inside p)
      (unwind-protect
          (progn
            (setq p (make-process
                     :name "qa-pcc-search" :noquery t :sentinel #'ignore
                     :connection-type 'pipe
                     :command '("/bin/sh" "-c" "printf payload")
                     :filter (lambda (_process _text)
                               (eval form t)
                               (setq inside (match-data t) done t))))
            (string-match "\\(caller\\)" "caller")
            (qa-pcc-wait p (lambda () done))
            ;; Normalize the buffer identity in integer match data.
            (when (bufferp (car (last inside)))
              (setq inside (append (butlast inside) '(buffer))))
            (list done inside (match-string 1 "caller")))
        (when p (set-process-sentinel p #'ignore) (delete-process p))))))

(qa (qa-pcc-search '(string-match "\\(b\\)" "abc")) '(t (1 2 1 2) "caller"))
(qa (qa-pcc-search '(posix-string-match "\\(b\\)" "abc")) '(t (1 2 1 2) "caller"))
(qa (qa-pcc-search '(looking-at "\\(a\\)")) '(t (1 2 1 2 buffer) "caller"))
(qa (qa-pcc-search '(posix-looking-at "\\(a\\)")) '(t (1 2 1 2 buffer) "caller"))
(qa (qa-pcc-search '(re-search-forward "\\(b\\)")) '(t (2 3 2 3 buffer) "caller"))
(qa (qa-pcc-search '(progn (goto-char 4) (re-search-backward "\\(b\\)")))
    '(t (2 3 2 3 buffer) "caller"))
(qa (qa-pcc-search '(set-match-data '(1 2 3 4))) '(t (1 2 3 4) "caller"))
(qa (qa-pcc-search '(string-match-p "b" "abc")) '(t nil "caller"))
(qa (qa-pcc-search '(looking-at-p "a")) '(t nil "caller"))
(qa (qa-pcc-search '(string-match "missing" "abc")) '(t nil "caller"))
(qa (qa-pcc-search '(re-search-forward "missing" nil t)) '(t nil "caller"))
(qa (qa-pcc-search '(string-match "b" "abc" nil t)) '(t nil "caller"))
(qa (qa-pcc-search '(condition-case nil (search-forward "a" 0) (error nil)))
    '(t (0 6 0 6) "caller"))
(qa (qa-pcc-search '(search-forward "missing" nil t 0)) '(t (1 1 buffer) "caller"))
(qa (with-temp-buffer
      (insert "abc") (goto-char 2)
      (string-match "\\(caller\\)" "caller")
      (list (search-forward "missing" nil t 0)
            (match-beginning 0) (match-end 0) (match-beginning 1)
            (condition-case err (search-backward "missing" 3 t 0) (error (car err)))))
    '(2 2 2 nil error))
(qa (qa-pcc-search '(progn (string-match "\\(b\\)" "abc")
                           (string-match "\\(c\\)" "abc")))
    '(t (2 3 2 3) "caller"))
(qa (qa-pcc-search
     '(progn (thread-join (make-thread (lambda () (string-match "worker" "worker"))))
             (string-match "\\(b\\)" "abc")))
    '(t (1 2 1 2) "caller"))

;; GNU leaves its asynchronous-search flag set across a throw. Keep these
;; nonlocal-exit cases last so the ordinary search checks start untainted.
(dolist (qa-pcc-kind '(filter sentinel))
  (qa (qa-pcc-leave qa-pcc-kind 'kill) '(t returned nil t nil outside))
  (qa (qa-pcc-leave qa-pcc-kind 'throw) '(t escaped t t nil outside)))
(qa (qa-pcc-search '(string-match-p "b" "abc")) '(t (0 6 0 6) "caller"))

(princ (qa--done))
