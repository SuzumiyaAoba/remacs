;;; -*- lexical-binding: nil -*-
;; Issue #48: string representation comes from the buffer, not its content.
(dolist (multibyte '(nil t))
  (dolist (case (append '((nil 0) ((65 66 67) 3) ((0 9 10 127) 4))
                       (if multibyte
                           '(((233) 2) ((945 128512) 6) ((4194176 4194303) 4)
                             ((65 233 4194176) 5))
                         '(((128 233 255) 3)))))
    (let ((chars (car case)) (bytes (cadr case)))
      (with-temp-buffer
        (set-buffer-multibyte multibyte)
        (dolist (ch chars) (insert-char ch))
        (when chars (put-text-property 1 (point-max) 'qa48-tag 'value))
        (dolist (fn '(buffer-string buffer-substring buffer-substring-no-properties))
          (let ((s (if (eq fn 'buffer-string) (buffer-string)
                     (funcall fn (point-min) (point-max)))))
            (qa (multibyte-string-p s) multibyte)
            (qa (string-to-list s) chars)
            (qa (length s) (length chars))
            (qa (string-bytes s) bytes)
            (when chars
              (qa (get-text-property 0 'qa48-tag s)
                  (if (eq fn 'buffer-substring-no-properties) nil 'value)))))
        ;; Every empty/narrowed slice keeps the original buffer representation.
        (dotimes (lo (1+ (length chars)))
          (let ((hi lo))
            (while (<= hi (length chars))
              (save-restriction
                (narrow-to-region (1+ lo) (1+ hi))
                (let ((s (buffer-string)) (expected nil) (j lo))
                  (while (< j hi) (push (nth j chars) expected) (setq j (1+ j)))
                  (qa (multibyte-string-p s) multibyte)
                  (qa (string-to-list s) (reverse expected))
                  (qa (equal-including-properties s (buffer-substring (point-min) (point-max))) t)))
              (setq hi (1+ hi)))))))))
;; Returned strings retain their representation after buffer changes/death.
(let ((saved (with-temp-buffer
               (insert "abc")
               (let ((multi (buffer-string)))
                 (set-buffer-multibyte nil)
                 (let ((uni (buffer-string)))
                   (set-buffer-multibyte t)
                   (list multi uni (buffer-string)))))))
  (garbage-collect)
  (qa (mapcar #'multibyte-string-p saved) '(t nil t))
  (qa (mapcar #'string-to-list saved) '((97 98 99) (97 98 99) (97 98 99))))
(dolist (case '((nil nil) ((65) (65)) ((233) (195 169))
                ((945) (206 177)) ((4194176) (128))))
  (with-temp-buffer
    (dolist (ch (car case)) (insert-char ch))
    (let ((s (buffer-string)))
      (qa (multibyte-string-p s) t)
      (qa (string-to-list (encode-coding-string s 'utf-8)) (cadr case)))))
;; Preexisting coding alias gap: utf-8-unix is not canonicalized by the
;; encoder for Unicode, even on master before the buffer extraction fix.
(dolist (case '((233 (195 169)) (945 (206 177))))
  (with-temp-buffer
    (insert-char (car case))
    (qa-xf (string-to-list (encode-coding-string (buffer-string) 'utf-8-unix))
           (cadr case) "preexisting utf-8-unix encoder alias gap")))
(princ (qa--done)) (terpri)
