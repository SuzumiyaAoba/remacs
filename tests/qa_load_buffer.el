;;; -*- lexical-binding: nil -*-
;; Issue #38: ordinary source loads expose a temporary input buffer.
(defun qa38-exec (source &optional suffix)
  (let ((file (make-temp-file "remacs-qa38-" nil (or suffix ".el"))))
    (unwind-protect
        (progn (with-temp-file file (insert source)) (load file nil t))
      (delete-file file))))
(defun qa38-snapshot ()
  (let ((stream standard-input) (caller (current-buffer)) (dir default-directory))
    (and (bufferp stream)
         (list (eq stream (car eval-buffer-list))
               (not (eq stream caller))
               (and (string-match-p "\\` \\*load\\*" (buffer-name stream)) t)
               (with-current-buffer stream
                 (list (buffer-modified-p) buffer-file-name buffer-undo-list
                       major-mode enable-multibyte-characters
                       (equal default-directory dir)
                       (> (point) (point-min)) (< (point) (point-max))))))))
(defvar qa38-owned nil)
(defvar qa38-state nil)
(defvar qa38-other nil)
(defvar qa38-seen nil)
(defvar qa38-executed nil)
(qa (let (qa38-owned qa38-state)
      (qa38-exec ";;; -*- lexical-binding: nil -*-\n(setq qa38-owned standard-input qa38-state (qa38-snapshot))\n;; tail\n")
      (list qa38-state (buffer-live-p qa38-owned)))
    '((t t t (nil nil t fundamental-mode t t t t)) nil))
(qa (let ((outer-input standard-input) (outer-list eval-buffer-list))
      (qa38-exec ";;; -*- lexical-binding: nil -*-\n42\n")
      (list (eq standard-input outer-input) (eq eval-buffer-list outer-list))) '(t t))
;; Collision: the existing buffer is untouched; the owned buffer is unique.
(qa (let ((existing (generate-new-buffer " *load*")) qa38-owned)
      (unwind-protect
          (progn
            (with-current-buffer existing (insert "keep"))
            (qa38-exec ";;; -*- lexical-binding: nil -*-\n(setq qa38-owned standard-input)\n")
            (list (buffer-live-p existing) (buffer-live-p qa38-owned)
                  (with-current-buffer existing (buffer-string))))
        (kill-buffer existing))) '(t nil "keep"))
;; Nested loads preserve the outer input/list and destroy only the child.
(qa (let (qa38-owned qa38-other qa38-state)
      (qa38-exec ";;; -*- lexical-binding: nil -*-\n(setq qa38-owned standard-input)\n(qa38-exec \";;; -*- lexical-binding: nil -*-\\n(setq qa38-other standard-input qa38-state (list (not (eq standard-input qa38-owned)) (eq (cadr eval-buffer-list) qa38-owned)))\\n\")\n(setq qa38-state (append qa38-state (list (buffer-live-p qa38-other) (eq standard-input qa38-owned))))\n")
      (list qa38-state (buffer-live-p qa38-owned))) '((t t nil t) nil))
;; Cleanup precedes after-load callbacks.
(qa (let (qa38-owned qa38-state
          (after-load-functions (list (lambda (_file) (setq qa38-state (list (bufferp qa38-owned) (buffer-live-p qa38-owned)))))))
      (qa38-exec ";;; -*- lexical-binding: nil -*-\n(setq qa38-owned standard-input)\n")
      qa38-state) '(t nil))
(qa (let (qa38-owned)
      (condition-case e (qa38-exec ";;; -*- lexical-binding: nil -*-\n(setq qa38-owned standard-input)\n(error \"qa38 boom\")\n") (error nil))
      (list (bufferp qa38-owned) (buffer-live-p qa38-owned))) '(t nil))
(qa (let (qa38-owned)
      ;; Cleanup coverage, not throw-propagation parity: remacs already
      ;; converts a cross-load throw to this precise no-catch condition.
      (let ((result (condition-case e
                        (catch 'qa38-throw (qa38-exec ";;; -*- lexical-binding: nil -*-\n(setq qa38-owned standard-input)\n(throw 'qa38-throw 7)\n"))
                      (no-catch e))))
        (list (or (equal result 7) (equal result '(no-catch qa38-throw 7)))
              (bufferp qa38-owned) (buffer-live-p qa38-owned)))) '(t t nil))
(qa (let (qa38-owned)
      (condition-case e (qa38-exec ";;; -*- lexical-binding: nil -*-\n(setq qa38-owned standard-input)\n)\n") (error nil))
      (list (bufferp qa38-owned) (buffer-live-p qa38-owned))) '(t nil))
(qa (let ((kills 0) (queries 0) qa38-owned)
      (let ((kill-buffer-hook (list (lambda () (setq kills (1+ kills)))))
            (kill-buffer-query-functions (list (lambda () (setq queries (1+ queries)) nil))))
        ;; Write before binding hooks: with-temp-file also owns a buffer.
        (let ((file (make-temp-file "remacs-qa38-hooks-" nil ".el")))
          (unwind-protect
              (progn
                (let (kill-buffer-hook kill-buffer-query-functions)
                  (with-temp-file file (insert ";;; -*- lexical-binding: nil -*-\n(setq qa38-owned standard-input)\n")))
                (load file nil t)
                (list kills queries (buffer-live-p qa38-owned)))
            (delete-file file))))) '(0 0 nil))
(qa (let (qa38-owned)
      (qa38-exec ";;; -*- lexical-binding: nil -*-\n(setq qa38-owned standard-input)\n(with-current-buffer standard-input (rename-buffer \" *qa38-renamed*\" t))\n")
      (list (buffer-live-p qa38-owned) (get-buffer " *qa38-renamed*"))) '(nil nil))
;; eval-buffer uses marker-based excursion restoration, not a numeric point.
(qa (with-temp-buffer
      (insert "abcd") (goto-char 3)
      (let ((caller (current-buffer)))
        (qa38-exec ";;; -*- lexical-binding: nil -*-\n(goto-char 1)\n(insert \"X\")\n(set-buffer (get-buffer-create \"*scratch*\"))\n")
        (list (eq (current-buffer) caller) (point) (buffer-string)))) '(t 4 "Xabcd"))
(qa (with-temp-buffer
      (insert "abcd") (goto-char 3)
      (let ((caller (current-buffer)))
        (condition-case e (qa38-exec ";;; -*- lexical-binding: nil -*-\n(goto-char 1)\n(insert \"X\")\n(error \"stop\")\n") (error nil))
        (list (eq (current-buffer) caller) (point) (buffer-string)))) '(t 4 "Xabcd"))
;; A custom reader receives the actual input buffer.
(qa (let (qa38-seen qa38-state
          (load-read-function (lambda (stream) (push (bufferp stream) qa38-seen) (read stream))))
      (qa38-exec ";;; -*- lexical-binding: nil -*-\n(setq qa38-state 1)\n(setq qa38-state (1+ qa38-state))\n")
      (list qa38-seen qa38-state)) '((t t) 2))
;; Explicit reads advance the same stream used by the top-level loop.
(qa (let (qa38-state qa38-executed)
      (qa38-exec ";;; -*- lexical-binding: nil -*-\n(setq qa38-state (read standard-input))\n(setq qa38-executed t)\n")
      (list qa38-state qa38-executed)) '((setq qa38-executed t) nil))
(qa (let (qa38-state)
      (qa38-exec ";;; -*- lexical-binding: nil -*-\n(with-current-buffer standard-input (save-excursion (goto-char (point-max)) (insert \"(setq qa38-state 9)\\n\")))\n")
      qa38-state) 9)
(qa (let (qa38-state)
      (qa38-exec ";;; -*- lexical-binding: nil -*-\n(with-current-buffer standard-input (narrow-to-region (point-min) (point)))\n(setq qa38-state t)\n")
      qa38-state) nil)
(qa (let (qa38-owned)
      (list (condition-case e (qa38-exec ";;; -*- lexical-binding: nil -*-\n(setq qa38-owned standard-input)\n(kill-buffer standard-input)\n42\n") (error e))
            (buffer-live-p qa38-owned))) '((error "Reading from killed buffer") nil))
;; A compiled stream does not acquire a source buffer.
(qa (let ((old-input standard-input) (old-list eval-buffer-list) qa38-state)
      (qa38-exec (concat ";ELC" (string 31 0 0 0) "\n;;; Compiled\n;;; in Emacs version 31.1\n(setq qa38-state (eq eval-buffer-list old-list))\n") ".elc")
      (list qa38-state (eq standard-input old-input))) '(t t))
(qa (let ((old-list eval-buffer-list) qa38-state)
      (qa38-exec (concat ";ELC" (string 31 0 0 0)
        "\n;;; Compiled\n;;; in Emacs version 31.1\n(qa38-exec \";;; -*- lexical-binding: nil -*-\\n(setq qa38-state (and (bufferp standard-input) (not (eq (car eval-buffer-list) (car old-list)))))\\n\")\n") ".elc")
      qa38-state) t)
(qa (let ((source ";;; -*- lexical-binding: nil -*-\n;; 日本語\n(setq qa38-state (with-current-buffer standard-input (buffer-string)))\n") qa38-state)
      (qa38-exec source) (equal qa38-state source)) t)
(princ (qa--done)) (terpri)
