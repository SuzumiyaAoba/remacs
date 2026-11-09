;;; qa_hippie_duplicate.el --- Regression tests for duplicate hippie results -*- lexical-binding:t -*-

;; Load only the driver and string helpers. Loading the whole library also
;; loads comint and its dependency tree, which this regression does not need.
(defvar hippie-expand-try-functions-list nil)
(defvar hippie-expand-verbose nil)
(defvar he-num -1)
(defvar he-string-beg nil)
(defvar he-string-end nil)
(defvar he-search-string nil)
(defvar he-expand-list nil)
(defvar he-tried-table nil)
(defvar he-tried-buffer-hashes nil)
(defvar qa-hippie-call-log nil)

(defun qa-hippie-load-source-functions ()
  "Evaluate the hippie-expand driver and string helpers from source."
  (let ((file (locate-library "hippie-exp"))
        (needed '(hippie-expand-try-functions-list hippie-expand
                  he-init-string he-reset-string
                  he-substitute-string he-string-member)))
    (unless file
      (error "Can't locate hippie-exp source"))
    (with-temp-buffer
      (insert-file-contents file)
      (goto-char (point-min))
      (while (and needed (< (point) (point-max)))
        (let* ((form (read (current-buffer)))
               (name (and (memq (car-safe form)
                               '(defun defsubst defcustom))
                          (cadr form))))
          (when (memq name needed)
            (eval form t)
            (setq needed (delq name needed))))))
    (when needed
      (error "Missing hippie-exp definitions: %S" needed))
    'loaded))

(defun qa-hippie-expand-whole-buffer (old)
  (push (list 'whole old) qa-hippie-call-log)
  (if old
      (progn (he-reset-string) nil)
    (progn
      (he-init-string (point-min) (point))
      (unless (he-string-member "foobar" he-tried-table)
        (he-substitute-string "foobar")
        t))))

(defun qa-hippie-expand-suffix (old)
  (push (list 'suffix old) qa-hippie-call-log)
  (if old
      (progn (he-reset-string) nil)
    (progn
      (he-init-string (1+ (point-min)) (point))
      (unless (he-string-member "oobar" he-tried-table)
        (he-substitute-string "oobar")
        t))))

(qa (qa-hippie-load-source-functions) 'loaded)

(qa
 (with-temp-buffer
   (setq-local hippie-expand-try-functions-list
               '(qa-hippie-expand-whole-buffer qa-hippie-expand-suffix))
   (let ((hippie-expand-verbose nil)
         (he-num -1)
         (he-string-beg (make-marker))
         (he-string-end (make-marker))
         (he-search-string nil)
         (he-expand-list nil)
         (he-tried-table nil)
         (he-tried-buffer-hashes nil)
         (qa-hippie-call-log nil)
         (this-command 'hippie-expand)
         (last-command 'hippie-expand))
     (insert "foo")
     (goto-char (point-max))
     (hippie-expand nil)
     (let ((first-result (buffer-string)))
       (hippie-expand nil)
       (list first-result
             (buffer-string)
             he-num
             (reverse qa-hippie-call-log)))))
 '("foobar" "foo" -1
   ((whole nil) (whole t) (suffix nil) (suffix t))))

(princ (qa--done))
