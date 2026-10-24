;;; -*- lexical-binding: t; -*-

;; 'display-buffer:small-selected-window'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(save-window-excursion (delete-other-windows) (let ((buffer (generate-new-buffer " *audit-display*"))) (unwind-protect (progn (split-window (selected-window) 4 'below) (let ((window (display-buffer buffer))) (list (window-live-p window) (eq (window-buffer window) buffer)))) (kill-buffer buffer)))) t))
           (error (list 'error err))))
  (terpri))

;; 'display-buffer:repeated-buffer-menu'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(save-window-excursion (delete-other-windows) (let ((completed 0)) (unwind-protect (condition-case err (progn (dotimes (_ 4) (list-buffers) (setq completed (1+ completed)) (kill-buffer "*Buffer List*")) (list completed nil)) (error (list completed (car err)))) (when (get-buffer "*Buffer List*") (kill-buffer "*Buffer List*"))))) t))
           (error (list 'error err))))
  (terpri))

;; 'display-buffer:explicit-reuse-control'
(let ((print-circle t) (print-escape-newlines t)
      (print-escape-control-characters t))
  (prin1 (condition-case err
             (list 'ok (eval '(save-window-excursion (delete-other-windows) (let ((buffer (generate-new-buffer " *audit-display-control*"))) (unwind-protect (let ((window (display-buffer buffer '((display-buffer-use-some-window))))) (list (window-live-p window) (eq (window-buffer window) buffer))) (kill-buffer buffer)))) t))
           (error (list 'error err))))
  (terpri))
