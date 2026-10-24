;;; -*- lexical-binding: t; -*-
(load (expand-file-name "audit_helpers.el" (file-name-directory load-file-name)) nil t)
(audit-check "overlay:remove-middle"
 '(with-temp-buffer (insert "abcdef")
    (let ((o (make-overlay 1 7)))
      (overlay-put o 'face 'bold) (remove-overlays 3 5)
      (mapcar (lambda (x) (list (overlay-start x) (overlay-end x) (overlay-get x 'face)))
              (sort (overlays-in 1 7) (lambda (a b) (< (overlay-start a) (overlay-start b))))))))
(audit-check "overlay:remove-category"
 '(with-temp-buffer (insert "abc")
    (let ((cat (make-symbol "audit-category")) (o (make-overlay 1 4)))
      (put cat 'audit-tag t) (overlay-put o 'category cat)
      (remove-overlays 1 4 'audit-tag t) (and (overlay-buffer o) t))))
(audit-check "overlay:remove-narrowed-default"
 '(with-temp-buffer (insert "abcdef")
    (let ((a (make-overlay 1 2)) (b (make-overlay 3 4)) (c (make-overlay 6 7)))
      (narrow-to-region 3 5) (remove-overlays)
      (mapcar (lambda (o) (and (overlay-buffer o) t)) (list a b c)))))
(audit-check "overlay:markers"
 '(with-temp-buffer (insert "abc")
    (let ((o (make-overlay (copy-marker 1) (copy-marker 3))))
      (move-overlay o (copy-marker 2) (copy-marker 4))
      (list (overlay-start o) (overlay-end o) (length (overlays-at (copy-marker 2)))))))
(audit-check "overlay:fake"
 '(list (overlayp [overlay 0 0])
        (condition-case e (overlay-start [overlay 0 0]) (error (car e)))
        (condition-case e (overlay-put [overlay 0 0] 'x 1) (error (car e)))))
(audit-check "overlay:dead-buffer-properties"
 '(let* ((b (generate-new-buffer " *audit-dead*")) (o (make-overlay 1 1 b)))
    (overlay-put o 'x 1) (kill-buffer b)
    (list (overlay-buffer o) (overlay-start o) (overlay-get o 'x)
          (progn (overlay-put o 'y 2) (overlay-get o 'y)))))
(audit-check "overlay:revive-after-kill"
 '(with-temp-buffer (insert "abc")
    (let* ((b (generate-new-buffer " *audit-dead*")) (o (make-overlay 1 1 b)))
      (overlay-put o 'x 1) (kill-buffer b)
      (move-overlay o 1 3 (current-buffer))
      (list (eq (overlay-buffer o) (current-buffer)) (overlay-start o) (overlay-get o 'x)))))
(audit-check "overlay:copy-deleted"
 '(with-temp-buffer (insert "abc")
    (let ((o (make-overlay 1 3)))
      (overlay-put o 'x 1) (delete-overlay o)
      (let ((c (copy-overlay o))) (list (overlay-buffer c) (overlay-start c) (overlay-get c 'x))))))
(audit-check "overlay:category-evaporate"
 '(with-temp-buffer (insert "abc")
    (let ((cat (make-symbol "audit-evap")) (o (make-overlay 1 3)))
      (put cat 'evaporate t) (overlay-put o 'category cat)
      (delete-region 1 3) (list (overlay-start o) (and (overlay-buffer o) t)))))
(audit-check "overlay:sorted-priority-cons"
 '(with-temp-buffer (insert "abc")
    (let ((a (make-overlay 1 4)) (b (make-overlay 1 4)))
      (overlay-put a 'priority '(nil . 10)) (overlay-put b 'priority '(nil . 1))
      (overlay-put a 'face 'bold) (overlay-put b 'face 'italic)
      (list (eq a (car (overlays-at 2 t))) (get-char-property 2 'face)))))
(audit-check "overlay:sorted-nesting"
 '(with-temp-buffer (insert "abcdef")
    (let ((a (make-overlay 3 5)) (b (make-overlay 1 5)))
      (overlay-put a 'face 'bold) (overlay-put b 'face 'italic)
      (list (eq a (car (overlays-at 3 t))) (get-char-property 3 'face)))))
(audit-check "overlay:modification-hooks"
 '(with-temp-buffer (insert "abcd")
    (let ((events nil) (o (make-overlay 1 5)))
      (overlay-put o 'modification-hooks
                   (list (lambda (_ after beg end &optional old) (push (list after beg end old) events))))
      (delete-region 2 3) (nreverse events))))
(audit-check "overlay:insert-boundary-hooks"
 '(with-temp-buffer (insert "abcd")
    (let ((events nil) (o (make-overlay 2 4)))
      (overlay-put o 'insert-in-front-hooks
                   (list (lambda (_ after beg end &optional old) (push (list after beg end old) events))))
      (goto-char 2) (insert "X") (nreverse events))))
(audit-check "overlay:read-only"
 '(with-temp-buffer (insert "abcd")
    (let ((o (make-overlay 1 5))) (overlay-put o 'read-only t)
      (list (condition-case e (progn (delete-region 2 3) 'deleted) (error (car e)))
            (buffer-string)))))

(audit-check "overlay:category-visible" '(with-temp-buffer (insert "abc") (let ((o (make-overlay 1 4)) (cat (make-symbol "audit-cat"))) (put cat 'face 'bold) (overlay-put o 'category cat) (list (overlay-get o 'face) (get-char-property 2 'face)))))
(audit-check "overlay:copy-independent-plist" '(with-temp-buffer (insert "abc") (let ((o (make-overlay 1 3))) (overlay-put o 'face 'bold) (let ((c (copy-overlay o))) (overlay-put c 'face 'italic) (list (overlay-get o 'face) (overlay-get c 'face))))))
(audit-check "overlay:window-restricted" '(save-window-excursion (with-temp-buffer (insert "abc") (switch-to-buffer (current-buffer)) (let ((o (make-overlay 1 4)) (other (split-window))) (overlay-put o 'face 'bold) (overlay-put o 'window other) (list (get-char-property 2 'face (selected-window)) (get-char-property 2 'face other) (get-char-property 2 'face (current-buffer)))))))
(audit-check "overlay:make-dead-buffer" '(let ((b (generate-new-buffer " *audit-dead-arg*"))) (kill-buffer b) (condition-case e (make-overlay 1 1 b) (error (car e)))))
(princ "AUDIT-DONE\n")
