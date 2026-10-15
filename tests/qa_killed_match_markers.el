;;; -*- lexical-binding: nil -*-
;; Issue #40: search registers survive kill, newly materialized markers detach.
(defun qa40-match (buffer)
 (with-current-buffer buffer
  (insert "abc") (goto-char 1) (re-search-forward "a\\(b\\)")))
(defun qa40-marker-state (data)
 (mapcar (lambda (m) (and (markerp m) (list (marker-buffer m) (marker-position m)))) data))
(dolist (materialize '(nil t))
 (qa (let ((b (generate-new-buffer "qa40-kill")) saved)
  (qa40-match b)
  (when materialize (setq saved (match-data)))
  (kill-buffer b)
  (let ((data (match-data)) (ints (match-data t)))
   (list (qa40-marker-state data)
         (if materialize (qa40-marker-state saved) nil)
         (butlast ints) (eq (car (last ints)) b) (buffer-live-p (car (last ints)))
         (match-beginning 0) (match-end 0) (match-beginning 1) (match-end 1))))
  (list '((nil nil) (nil nil) (nil nil) (nil nil))
        (if materialize '((nil nil) (nil nil) (nil nil) (nil nil)) nil)
        '(1 3 2 3) t nil 1 3 2 3)))
(qa (let ((b (generate-new-buffer "qa40-printer")))
 (qa40-match b) (kill-buffer b) (prin1-to-string (match-data)))
 "(#<marker in no buffer> #<marker in no buffer> #<marker in no buffer> #<marker in no buffer>)")
;; save-match-data saves markers, whose detached restoration becomes zeros.
(qa (let ((b (generate-new-buffer "qa40-save")))
 (qa40-match b)
 (save-match-data (string-match "z" "zz") (kill-buffer b))
 (list (match-data) (match-data t))) '((0 0 0 0) (0 0 0 0)))
(qa (let ((b (generate-new-buffer "qa40-unwind")))
 (qa40-match b)
 (condition-case nil (save-match-data (kill-buffer b) (error "qa40 unwind")) (error nil))
 (match-data)) '(0 0 0 0))
;; Test restored buffer identity/detachment only here: the pre-existing
;; set-match-data integer-tail round trip adds one to numeric positions.
;; Direct search/kill tests above still check exact unchanged raw positions.
(qa (let ((b (generate-new-buffer "qa40-set")) saved)
 (qa40-match b) (setq saved (match-data t)) (kill-buffer b)
 (string-match "z" "zz") (set-match-data saved)
 (list (qa40-marker-state (match-data)) (eq (car (last (match-data t))) b)))
 '(((nil nil) (nil nil) (nil nil) (nil nil)) t))
;; Materialized markers are still tracked while live, then detached on kill.
(qa (let ((b (generate-new-buffer "qa40-edit")) saved)
 (unwind-protect
  (progn (qa40-match b) (setq saved (match-data))
   (with-current-buffer b (goto-char 1) (insert "X"))
   (let ((before (mapcar #'marker-position saved)))
    (kill-buffer b) (list before (qa40-marker-state saved))))
  (when (buffer-live-p b) (kill-buffer b))))
 '((1 4 3 4) ((nil nil) (nil nil) (nil nil) (nil nil))))
;; New markers clamp to full live buffer bounds; raw registers do not move.
(qa (with-temp-buffer
 (insert "abcdef") (goto-char 1) (re-search-forward "ef") (delete-region 1 7)
 (list (mapcar #'marker-position (match-data)) (butlast (match-data t)) (match-beginning 0) (match-end 0)))
 '((1 1) (5 7) 5 7))
(qa (with-temp-buffer
 (insert "abcdef") (goto-char 1) (re-search-forward "ef") (narrow-to-region 2 3)
 (mapcar #'marker-position (match-data))) '(5 7))
;; An unrelated kill does not detach the searched buffer's markers.
(qa (let ((b (generate-new-buffer "qa40-live")) (other (generate-new-buffer "qa40-other")))
 (unwind-protect
  (progn (qa40-match b) (kill-buffer other)
   (list (mapcar #'marker-position (match-data))
         (mapcar (lambda (m) (eq (marker-buffer m) b)) (match-data))))
  (kill-buffer b))) '((1 3 2 3) (t t t t)))
;; String matches remain integers after any buffer kill.
(qa (let ((b (generate-new-buffer "qa40-string")))
 (string-match "a\\(b\\)" "abc") (kill-buffer b) (list (match-data) (match-data t))) '((0 2 1 2) (0 2 1 2)))
;; Indirect kills affect only that buffer; killing a base also kills children.
(dolist (kill-base '(nil t))
 (qa (let* ((base (generate-new-buffer "qa40-base"))
            (child (make-indirect-buffer base "qa40-child")))
  (unwind-protect
   (progn (qa40-match child) (kill-buffer (if kill-base base child))
    (list (qa40-marker-state (match-data)) (eq (car (last (match-data t))) child)
          (buffer-live-p base) (buffer-live-p child)))
   (when (buffer-live-p base) (kill-buffer base))))
  (list '((nil nil) (nil nil) (nil nil) (nil nil)) t (not kill-base) nil)))
;; A later search replaces the dead searched object normally.
(qa (let ((b (generate-new-buffer "qa40-replace")))
 (qa40-match b) (kill-buffer b) (string-match "c" "abc") (match-data t)) '(2 3))
;; Interior unmatched groups stay nil; only the real markers detach.
(qa (let ((b (generate-new-buffer "qa40-holes")))
 (with-current-buffer b (insert "abc") (goto-char 1) (re-search-forward "\\(z\\)?a\\(b\\)"))
 (kill-buffer b)
 (list (qa40-marker-state (match-data)) (butlast (match-data t))))
 '(((nil nil) (nil nil) nil nil (nil nil) (nil nil)) (1 3 nil nil 2 3)))
;; Restoring already detached marker data cannot reattach it to another buffer.
(qa (let ((b (generate-new-buffer "qa40-restore")) data)
 (qa40-match b) (setq data (match-data)) (kill-buffer b)
 (set-match-data data) (match-data)) '(0 0 0 0))
(princ (qa--done)) (terpri)
