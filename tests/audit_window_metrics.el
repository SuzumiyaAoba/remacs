;;; audit_window_metrics.el --- size ratios and window use order -*- lexical-binding: t; -*-
(load (expand-file-name "audit_helpers.el" (file-name-directory load-file-name)) nil t)

(defun audit-window-metrics-fixture (fn)
  (save-window-excursion
    (delete-other-windows)
    (with-temp-buffer
      (switch-to-buffer (current-buffer))
      (insert "012345678901234567890123456789\n")
      (goto-char 1)
      (funcall fn (selected-window)))))

(audit-check "window-metrics:horizontal-normal-size"
 '(audit-window-metrics-fixture
   (lambda (left)
     (let ((right (split-window left 20 'right)))
       (list (< 0.0 (window-normal-size left t) 1.0)
             (< 0.0 (window-normal-size right t) 1.0)
             (= 1.0 (+ (window-normal-size left t) (window-normal-size right t)))
             (window-normal-size left)
             (window-normal-size right))))))

(audit-check "window-metrics:vertical-normal-size"
 '(audit-window-metrics-fixture
   (lambda (top)
     (let ((bottom (split-window top 10 'below)))
       (list (< 0.0 (window-normal-size top) 1.0)
             (< 0.0 (window-normal-size bottom) 1.0)
             (= 1.0 (+ (window-normal-size top) (window-normal-size bottom)))
             (window-normal-size top t)
             (window-normal-size bottom t))))))

(audit-check "window-metrics:bump-use-order"
 '(audit-window-metrics-fixture
   (lambda (first)
     (let* ((second (split-window first 20 'right))
            (third (split-window second 20 'right)))
       (select-window first)
       (select-window second)
       (select-window third)
       (let ((before (window-use-time third))
             (result (window-bump-use-time first)))
         (list (integerp result)
               (= (window-use-time first) before)
               (> (window-use-time third) before)
               (> (window-use-time first) (window-use-time second))
               (eq (selected-window) third)))))))

(audit-check "window-metrics:bump-selected-control"
 '(audit-window-metrics-fixture
   (lambda (window)
     (let ((before (window-use-time window)))
       (list (window-bump-use-time window)
             (= before (window-use-time window)))))))

(audit-check "window-metrics:old-point-after-set"
 '(audit-window-metrics-fixture
   (lambda (first)
     (let* ((other (split-window first 20 'right))
            (before (window-old-point other)))
       (set-window-point other 10)
       (list (window-point other) before (window-old-point other))))))

(audit-check "window-metrics:old-buffer-after-set"
 '(audit-window-metrics-fixture
   (lambda (window)
     (let ((other (generate-new-buffer " *audit-window-old-buffer*"))
           (old (window-old-buffer window))
           (previous (window-buffer window)))
       (unwind-protect
           (progn
             (set-window-buffer window other)
             (list (eq (window-old-buffer window) old)
                   (eq (window-old-buffer window) previous)))
         (set-window-buffer window previous)
         (kill-buffer other))))))

(princ "AUDIT-DONE\n")
