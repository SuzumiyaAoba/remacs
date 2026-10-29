;;; -*- lexical-binding: t; -*-
;; Issues #2750 and #2098: the recursion guard belongs to the target buffer.

(defvar qa-local-eval-count 0)
(defvar qa-local-eval-nested nil)
(defvar qa-local-eval-form '(qa-local-eval-run))
(defun qa-local-eval-run ()
  (setq qa-local-eval-count (1+ qa-local-eval-count))
  (unless qa-local-eval-nested
    (let ((qa-local-eval-nested t))
      (with-temp-buffer
        (hack-one-local-variable 'eval qa-local-eval-form))))
  nil)

(qa (let ((hack-local-variables--inhibit-eval nil)
          (qa-local-eval-count 0)
          (qa-local-eval-nested nil))
      (with-temp-buffer
        (hack-one-local-variable 'eval qa-local-eval-form))
      qa-local-eval-count)
    2)

(defvar qa-local-eval-same-count 0)
(defvar qa-local-eval-same-nested nil)
(defvar qa-local-eval-same-form '(qa-local-eval-same-run))
(defun qa-local-eval-same-run ()
  (setq qa-local-eval-same-count (1+ qa-local-eval-same-count))
  (unless qa-local-eval-same-nested
    (let ((qa-local-eval-same-nested t))
      (hack-one-local-variable 'eval qa-local-eval-same-form)))
  nil)

;; Same-buffer recursion is still suppressed.
(qa (let ((hack-local-variables--inhibit-eval nil)
          (qa-local-eval-same-count 0)
          (qa-local-eval-same-nested nil))
      (with-temp-buffer
        (hack-one-local-variable 'eval qa-local-eval-same-form))
      qa-local-eval-same-count)
    1)

(princ (qa--done))
