;;; -*- lexical-binding: t; -*-
(defvar qa208-number)
(defvar qa208-mode)

(defun qa208-position (value foreign)
  (cond ((eq value :m1) (copy-marker 1))
        ((eq value :m2) (copy-marker 2))
        ((eq value :m3) (copy-marker 3))
        ((eq value :m4) (copy-marker 4))
        ((eq value :m5) (copy-marker 5))
        ((eq value :foreign3) (with-current-buffer foreign (copy-marker 3)))
        ((eq value :foreign8) (with-current-buffer foreign (copy-marker 8)))
        ((eq value :detached) (make-marker))
        (t value)))
(defun qa208-display (value)
  (if (markerp value) (list 'marker (marker-position value)) value))

(defun qa208-check (start end narrow &optional dead)
  (let ((foreign (generate-new-buffer " *qa208-foreign*")))
    (unwind-protect
        (with-temp-buffer
          (insert "abcdef")
          (with-current-buffer foreign (insert "0123456789"))
          (setq start (qa208-position start foreign) end (qa208-position end foreign))
          (when narrow (narrow-to-region 2 5))
          (let ((p (make-pipe-process :name "qa208-region" :noquery t :sentinel #'ignore)))
            (unwind-protect
                (progn
                  (when dead (delete-process p))
                  (condition-case err
                      (list 'ok (process-send-region p start end))
                    (args-out-of-range
                     (list 'args-out-of-range
                           (eq (nth 1 err) (current-buffer))
                           (if (markerp start) (eq (nth 2 err) start) (equal (nth 2 err) start))
                           (if (markerp end) (eq (nth 3 err) end) (equal (nth 3 err) end))
                           (qa208-display (nth 2 err)) (qa208-display (nth 3 err))))
                    (error err)))
              (when (process-live-p p) (delete-process p)))))
      (kill-buffer foreign))))

(qa (qa208-check 1 4 nil) '(ok nil))
(qa (qa208-check 4 1 nil) '(ok nil))
(qa (qa208-check :m1 :m3 nil) '(ok nil))
(qa (qa208-check :m3 :m1 nil) '(ok nil))
(qa (qa208-check :foreign3 :m5 nil) '(ok nil))
(qa (qa208-check 7 7 nil) '(ok nil))
(qa (qa208-check 2 2 t) '(ok nil))
(qa (qa208-check :m5 :m2 t) '(ok nil))
(qa (qa208-check :m3 2 t) '(ok nil))

;; Neither endpoint is clamped. The error retains order, marker identity
;; and the source buffer, even when that buffer is narrowed.
(qa (qa208-check 0 2 nil) '(args-out-of-range t t t 0 2))
(qa (qa208-check 2 0 nil) '(args-out-of-range t t t 2 0))
(qa (qa208-check 9 2 nil) '(args-out-of-range t t t 9 2))
(qa (qa208-check 2 9 nil) '(args-out-of-range t t t 2 9))
(qa (qa208-check :m1 6 t) '(args-out-of-range t t t (marker 1) 6))
(qa (qa208-check 6 :m1 t) '(args-out-of-range t t t 6 (marker 1)))
(qa (qa208-check :foreign8 :m1 nil) '(args-out-of-range t t t (marker 8) (marker 1)))
(qa (qa208-check :m1 :m4 t) '(args-out-of-range t t t (marker 1) (marker 4)))
(qa (qa208-check 1 6 t) '(args-out-of-range t t t 1 6))

;; Very large integers remain valid integer arguments. Check both types
;; before range validation, including values wider than Rust's i128.
(dolist (qa208-number (list most-positive-fixnum (1+ most-positive-fixnum)
                           most-negative-fixnum (expt 2 70) (- (expt 2 127))
                           (expt 2 200) (- (expt 2 200))))
  (qa (qa208-check qa208-number 1 nil)
      (list 'args-out-of-range t t t qa208-number 1))
  (qa (qa208-check 1 qa208-number nil)
      (list 'args-out-of-range t t t 1 qa208-number)))
(qa (qa208-check (expt 2 200) "bad" nil)
    '(wrong-type-argument integer-or-marker-p "bad"))
(qa (qa208-check 0 "bad" nil) '(wrong-type-argument integer-or-marker-p "bad"))
(qa (qa208-check nil 1 nil) '(wrong-type-argument integer-or-marker-p nil))
(qa (qa208-check 1 nil nil) '(wrong-type-argument integer-or-marker-p nil))
(qa (qa208-check 1.0 2 nil) '(wrong-type-argument integer-or-marker-p 1.0))
(qa (qa208-check :detached 1 nil) '(error "Marker does not point anywhere"))
(qa (qa208-check 1 :detached nil) '(error "Marker does not point anywhere"))
(qa (qa208-check :detached "bad" nil) '(error "Marker does not point anywhere"))
(qa (qa208-check 0 2 nil t) '(args-out-of-range t t t 0 2))
(qa (car (qa208-check 1 2 nil t)) 'error)
(qa-err-data (process-send-region 42 (make-marker) nil) 'wrong-type-argument '(processp 42))

(defun qa208-wire (mode text start end &optional narrow unibyte)
  (let ((output (generate-new-buffer " *qa208-output*"))
        (foreign (generate-new-buffer " *qa208-wire-foreign*")) p)
    (unwind-protect
        (progn
          (when unibyte (with-current-buffer output (set-buffer-multibyte nil)))
          (setq p (make-process :name "qa208-wire" :buffer output :noquery t
                                :sentinel #'ignore :connection-type mode
                                :coding (if unibyte '(binary . utf-8-unix) 'utf-8-unix)
                                :command '("/bin/cat")))
          (with-temp-buffer
            (when unibyte (set-buffer-multibyte nil))
            (insert text)
            (with-current-buffer foreign (insert "0123456789"))
            (setq start (qa208-position start foreign) end (qa208-position end foreign))
            (when narrow (narrow-to-region 2 5))
            (setq buffer-read-only t)
            (process-send-region p start end))
          (process-send-eof p)
          ;; In canonical PTY input, the first EOF flushes an unfinished
          ;; line; a second EOF ends cat's next read. It may already exit
          ;; for an empty region, so an extra EOF is only test cleanup.
          (when (eq mode 'pty) (ignore-errors (process-send-eof p)))
          (let ((deadline (+ (float-time) 3)))
            (while (and (process-live-p p) (< (float-time) deadline))
              (accept-process-output p 0.01)))
          (accept-process-output p 0.01)
          (with-current-buffer output
            (if unibyte (string-to-list (buffer-string)) (buffer-string))))
      (when (and p (process-live-p p)) (delete-process p))
      (kill-buffer output) (kill-buffer foreign))))
(dolist (qa208-mode '(pipe pty))
  (qa (qa208-wire qa208-mode "abcDEF" :m1 :m4) "abc")
  (qa (qa208-wire qa208-mode "abcDEF" :m4 :m1) "abc")
  (qa (qa208-wire qa208-mode "abcDEF" :foreign3 :m5) "cD")
  (qa (qa208-wire qa208-mode "abcDEF" :m5 :m2 t) "bcD")
  (qa (qa208-wire qa208-mode "aあβz" :m2 :m4) "あβ")
  (qa (qa208-wire qa208-mode "abc" :m3 :m3) ""))
(qa (qa208-wire 'pipe (unibyte-string 65 255 66) :m1 :m4 nil t) '(65 255 66))

;; Rejecting a region must not partially send it before the error.
(qa-lex
 (let ((output (generate-new-buffer " *qa208-atomic*")) p)
   (unwind-protect
       (progn
         (setq p (make-process :name "qa208-atomic" :buffer output :noquery t
                              :sentinel #'ignore :connection-type 'pipe
                              :command '("/bin/cat")))
         (let ((condition
                (with-temp-buffer
                  (insert "abc")
                  (prog1 (condition-case err
                             (progn (process-send-region p 0 2) 'sent)
                           (error (car err)))
                    (process-send-region p (copy-marker 1) (copy-marker 4))))))
           (process-send-eof p)
           (let ((deadline (+ (float-time) 3)))
             (while (and (process-live-p p) (< (float-time) deadline))
               (accept-process-output p 0.01)))
           (accept-process-output p 0.01)
           (list condition (with-current-buffer output (buffer-string)))))
     (when (and p (process-live-p p)) (delete-process p))
     (kill-buffer output)))
 '(args-out-of-range "abc"))
(princ (qa--done)) (terpri)
