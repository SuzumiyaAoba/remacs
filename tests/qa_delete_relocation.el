;;; qa_delete_relocation.el --- Relocate deletion across before hooks -*- lexical-binding: t; -*-

(defun qa323-relocate (operation edit &optional count)
  (with-temp-buffer
    (insert "abcd")
    (let* ((count (or count 1))
           (events nil))
      (goto-char (if (eq operation 'backward) (+ 2 count) 2))
      (let ((before-change-functions
             (list (lambda (begin end)
                     (push (list 'before begin end) events)
                     (save-excursion
                       (cond ((eq edit 'prefix-insert) (goto-char 1) (insert "P"))
                             ((eq edit 'prefix-delete) (delete-region 1 2))
                             ((eq edit 'at-start) (goto-char 2) (insert "P"))
                             ((eq edit 'inside) (goto-char 3) (insert "P"))
                             ((eq edit 'erase) (erase-buffer)))))))
            (after-change-functions
             (list (lambda (begin end old) (push (list 'after begin end old) events)))))
        (let ((result (cond ((eq operation 'region) (delete-region 2 (+ 2 count)))
                            ((eq operation 'extract) (delete-and-extract-region 2 (+ 2 count)))
                            ((eq operation 'backward) (delete-backward-char count))
                            (t (delete-char count)))))
          (list result (buffer-substring-no-properties (point-min) (point-max))
                (point) (nreverse events)))))))

;; region follows prefix-insert
(qa (qa323-relocate 'region 'prefix-insert 2)
    '(nil "Pad" 3 ((before 2 4) (after 3 3 2))))

;; region follows prefix-delete
(qa (qa323-relocate 'region 'prefix-delete 2)
    '(nil "d" 1 ((before 2 4) (after 1 1 2))))

;; region follows at-start
(qa (qa323-relocate 'region 'at-start 2)
    '(nil "acd" 2 ((before 2 4) (after 2 2 2))))

;; region follows inside
(qa (qa323-relocate 'region 'inside 2)
    '(nil "acd" 2 ((before 2 4) (after 2 2 2))))

;; region follows erase
(qa (qa323-relocate 'region 'erase 2)
    '(nil "" 1 ((before 2 4) (after 1 1 0))))

;; extract follows prefix-insert
(qa (qa323-relocate 'extract 'prefix-insert 2)
    '("bc" "Pad" 3 ((before 2 4) (after 3 3 2))))

;; extract follows prefix-delete
(qa (qa323-relocate 'extract 'prefix-delete 2)
    '("bc" "d" 1 ((before 2 4) (after 1 1 2))))

;; extract follows at-start
(qa (qa323-relocate 'extract 'at-start 2)
    '("Pb" "acd" 2 ((before 2 4) (after 2 2 2))))

;; extract follows inside
(qa (qa323-relocate 'extract 'inside 2)
    '("bP" "acd" 2 ((before 2 4) (after 2 2 2))))

;; extract follows erase
(qa (qa323-relocate 'extract 'erase 2)
    '("" "" 1 ((before 2 4) (after 1 1 0))))

;; forward follows prefix-insert
(qa (qa323-relocate 'forward 'prefix-insert 2)
    '(nil "Pad" 3 ((before 2 4) (after 3 3 2))))

;; forward follows prefix-delete
(qa (qa323-relocate 'forward 'prefix-delete 2)
    '(nil "d" 1 ((before 2 4) (after 1 1 2))))

;; forward follows at-start
(qa (qa323-relocate 'forward 'at-start 2)
    '(nil "acd" 2 ((before 2 4) (after 2 2 2))))

;; forward follows inside
(qa (qa323-relocate 'forward 'inside 2)
    '(nil "acd" 2 ((before 2 4) (after 2 2 2))))

;; forward follows erase
(qa (qa323-relocate 'forward 'erase 2)
    '(nil "" 1 ((before 2 4) (after 1 1 0))))

;; backward follows prefix-insert
(qa (qa323-relocate 'backward 'prefix-insert 2)
    '(nil "Pad" 3 ((before 2 4) (after 3 3 2))))

;; backward follows prefix-delete
(qa (qa323-relocate 'backward 'prefix-delete 2)
    '(nil "d" 1 ((before 2 4) (after 1 1 2))))

;; backward follows at-start
(qa (qa323-relocate 'backward 'at-start 2)
    '(nil "acd" 3 ((before 2 4) (after 2 2 2))))

;; backward follows inside
(qa (qa323-relocate 'backward 'inside 2)
    '(nil "acd" 3 ((before 2 4) (after 2 2 2))))

;; backward follows erase
(qa (qa323-relocate 'backward 'erase 2)
    '(nil "" 1 ((before 2 4) (after 1 1 0))))

;; first-change-hook also relocates deletion
(qa (with-temp-buffer (insert "abcd") (goto-char 2) (set-buffer-modified-p nil) (let ((first-change-hook (list (lambda () (save-excursion (goto-char 1) (insert "P")))))) (list (delete-and-extract-region 2 3) (buffer-string) (point))))
    '("b" "Pacd" 3))

;; Original single-character reproductions.
(qa (qa323-relocate 'region 'prefix-insert 1)
    '(nil "Pacd" 3 ((before 2 3) (after 3 3 1))))
(qa (qa323-relocate 'extract 'prefix-insert 1)
    '("b" "Pacd" 3 ((before 2 3) (after 3 3 1))))
(qa (qa323-relocate 'forward 'prefix-insert 1)
    '(nil "Pacd" 3 ((before 2 3) (after 3 3 1))))

(princ (qa--done))
