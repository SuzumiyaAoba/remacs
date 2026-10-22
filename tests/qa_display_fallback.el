;;; -*- lexical-binding: t; -*-
;; GNU 31.1 display-buffer fallback behavior (#294).

(defun qa-display-fallback-run (function)
  (save-window-excursion
    (delete-other-windows)
    (let ((buffer (generate-new-buffer " *qa-display-fallback*")))
      (unwind-protect (funcall function buffer) (kill-buffer buffer)))))

(qa (qa-display-fallback-run
     (lambda (buffer)
       (split-window (selected-window) 4 (quote below))
       (let* ((selected (selected-window)) (window (display-buffer buffer)))
         (list (window-live-p window) (eq (window-buffer window) buffer)
               (eq selected (selected-window)) (eq selected window)
               (length (window-list))))))
    (quote (t t t nil 2)))

(qa (save-window-excursion
      (delete-other-windows)
      (unwind-protect
          (let ((completed 0) (maximum 0))
            (dotimes (_ 4)
              (list-buffers)
              (setq completed (1+ completed))
              (setq maximum (max maximum (length (window-list))))
              (kill-buffer "*Buffer List*"))
            (list completed maximum))
        (when (get-buffer "*Buffer List*") (kill-buffer "*Buffer List*"))))
    (quote (4 2)))

(qa (qa-display-fallback-run
     (lambda (buffer)
       (let ((window (display-buffer buffer (quote ((display-buffer-use-some-window))))))
         (list (window-live-p window) (eq (window-buffer window) buffer)))))
    (quote (t t)))

(qa (qa-display-fallback-run
     (lambda (buffer)
       (let* ((calls 0)
              (split-window-preferred-function (lambda (_) (setq calls (1+ calls)) (error "cannot split")))
              (window (display-buffer buffer)))
         (list calls (eq window (selected-window))
               (eq (window-buffer window) buffer) (length (window-list))))))
    (quote (2 t t 1)))

(qa (qa-display-fallback-run
     (lambda (buffer)
       (let* ((calls 0)
              (split-window-preferred-function (lambda (_) (setq calls (1+ calls)) nil))
              (window (display-buffer buffer)))
         (list calls (eq window (selected-window)) (length (window-list))))))
    (quote (2 t 1)))

(qa (qa-display-fallback-run
     (lambda (buffer)
       (let* ((calls 0)
              (pop-up-windows nil)
              (split-window-preferred-function (lambda (_) (setq calls (1+ calls)) (error "unexpected split")))
              (window (display-buffer buffer)))
         (list calls (eq window (selected-window)) (length (window-list))))))
    (quote (0 t 1)))

(qa (qa-display-fallback-run
     (lambda (buffer)
       (let* ((large (split-window (selected-window) 4 (quote below)))
              (attempted nil)
              (split-window-preferred-function
               (lambda (window) (setq attempted window) (split-window window)))
              (window (display-buffer buffer)))
         (list (eq attempted large) (eq (window-buffer window) buffer)
               (length (window-list))))))
    (quote (t t 3)))

(qa (qa-display-fallback-run
     (lambda (buffer)
       (let ((split-window-preferred-function (lambda (_) (signal (quote quit) nil))))
         (condition-case nil (display-buffer buffer) (quit (quote quit))))))
    (quote quit))

(princ (qa--done))
