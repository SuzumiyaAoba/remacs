;;; audit_buffer_more.el --- replacement, transposition and groups -*- lexical-binding: t; -*-
(load (expand-file-name "audit_helpers.el" (file-name-directory load-file-name)) nil t)

(audit-check "replace-region:unchanged"
 '(with-temp-buffer
    (insert "abcdef") (goto-char 4)
    (buffer-enable-undo) (set-buffer-modified-p nil)
    (let* ((m (copy-marker 3)) (changes nil)
           (after-change-functions (list (lambda (&rest args) (push args changes)))))
      (list (replace-region-contents 1 7 "abcdef")
            (point) (marker-position m) (buffer-modified-p)
            buffer-undo-list changes))))

(audit-check "replace-region:preserve-marker"
 '(with-temp-buffer
    (insert "abcdef") (goto-char 4)
    (let ((m (copy-marker 3)))
      (replace-region-contents 1 7 "abcdeX")
      (list (buffer-string) (point) (marker-position m)))))

(audit-check "replace-region:properties"
 '(with-temp-buffer
    (insert "abc")
    (replace-region-contents 1 4 (propertize "aXc" 'face 'bold))
    (list (buffer-substring-no-properties 1 4)
          (mapcar (lambda (p) (get-text-property p 'face)) '(1 2 3)))))

(audit-check "replace-region:narrowed-source"
 '(let ((source (generate-new-buffer " *audit-replace-source*")))
    (unwind-protect
        (progn
          (with-current-buffer source (insert "xxNEWyy") (narrow-to-region 3 6))
          (with-temp-buffer
            (insert "old")
            (replace-region-contents 1 4 source)
            (buffer-string)))
      (kill-buffer source))))

(audit-check "replace-region:vector-source"
 '(let ((source (generate-new-buffer " *audit-replace-source*")))
    (unwind-protect
        (progn
          (with-current-buffer source (insert "xxNEWyy"))
          (with-temp-buffer
            (insert "old")
            (replace-region-contents 1 4 (vector source 3 6))
            (buffer-string)))
      (kill-buffer source))))

(audit-check "replace-region:zero-time-limit"
 '(with-temp-buffer
    (insert "abc")
    (let ((result (replace-region-contents 1 4 "xyz" 0)))
      (list result (buffer-string)))))

(audit-check "replace-region:callback-narrowing"
 '(with-temp-buffer
    (insert "[abc]")
    (let (seen)
      (replace-region-contents 2 5
        (lambda () (setq seen (list (point-min) (point-max) (buffer-string))) "X"))
      (list seen (buffer-string) (point-min) (point-max)))))

(audit-check "transpose:properties"
 '(with-temp-buffer
    (insert (propertize "ab" 'face 'bold) "--" (propertize "XYZ" 'face 'italic))
    (transpose-regions 1 3 5 8)
    (list (buffer-substring-no-properties 1 8)
          (mapcar (lambda (p) (get-text-property p 'face)) '(1 2 3 4 5 6 7)))))

(dolist (leave '(nil t))
  (audit-check (format "transpose:markers-%s" leave)
   `(with-temp-buffer
      (insert "ab--XYZ") (goto-char 2)
      (let ((first (copy-marker 2)) (second (copy-marker 6)) (middle (copy-marker 4)))
        (transpose-regions 1 3 5 8 ,leave)
        (list (buffer-string) (point)
              (mapcar #'marker-position (list first second middle)))))))

(audit-check "change-group:restore-disabled-undo"
 '(with-temp-buffer
    (let ((handle (prepare-change-group)))
      (activate-change-group handle)
      (insert "x")
      (accept-change-group handle)
      (list (buffer-string) (eq buffer-undo-list t)))))

(audit-check "change-group:atomic-failure-control"
 '(with-temp-buffer
    (insert "abc")
    (condition-case nil
        (atomic-change-group (insert "XYZ") (error "stop"))
      (error nil))
    (list (buffer-string) (eq buffer-undo-list t))))

(princ "AUDIT-DONE\n")
