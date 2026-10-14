;;; -*- lexical-binding: nil; -*-
;; Issue #37, including rename-buffer and set-window-buffer reports.
(let ((original (current-buffer))
      (shown (window-buffer)))
  (dolist (value (list t 5 -2 1.5 '(x) [x] (make-marker) :name))
    (qa-err-data (kill-buffer value) 'wrong-type-argument (list 'stringp value))
    (qa-err-data (set-window-buffer nil value) 'wrong-type-argument (list 'stringp value))
    (qa-err-data (rename-buffer value) 'wrong-type-argument (list 'stringp value))
    (qa (eq original (current-buffer)) t)
    (qa (eq shown (window-buffer)) t))
  (qa-err-data (rename-buffer original) 'wrong-type-argument (list 'stringp original))
  (qa-err-data (rename-buffer nil) 'wrong-type-argument '(stringp nil))
  (qa-err-data (set-window-buffer nil nil) 'wrong-type-argument '(stringp nil))
  (qa-err-data (set-window-buffer nil " *qa37-missing*") 'wrong-type-argument '(bufferp nil))
  (qa-err-data (kill-buffer " *qa37-missing*") 'error '("No buffer named  *qa37-missing*"))
  (qa-err-data (rename-buffer "") 'error '("Empty string is invalid as a buffer name"))
  (let ((buffer (generate-new-buffer " *qa37-live*")))
    (unwind-protect
        (progn
          (qa (set-window-buffer nil buffer) nil)
          (qa (eq (window-buffer) buffer) t)
          (qa (set-window-buffer nil (buffer-name buffer)) nil)
          (with-current-buffer buffer
            (qa (rename-buffer " *qa37-renamed*") " *qa37-renamed*")
            (qa (rename-buffer " *qa37-renamed*") " *qa37-renamed*"))
          (qa (kill-buffer (buffer-name buffer)) t)
          (qa (kill-buffer buffer) nil)
          (qa-err-data (set-window-buffer nil buffer) 'error '("Attempt to display deleted buffer")))
      (set-window-buffer nil shown)
      (kill-buffer buffer)))
  (dolist (explicit '(nil t))
    (let ((buffer (generate-new-buffer " *qa37-current*")))
      (with-current-buffer buffer
        (qa (if explicit (kill-buffer nil) (kill-buffer)) t))
      (qa (buffer-live-p buffer) nil))))
;; WINDOW validation must precede BUFFER-OR-NAME validation.
(qa-err-data (set-window-buffer 5 5) 'wrong-type-argument '(window-live-p 5))
(princ (qa--done))
