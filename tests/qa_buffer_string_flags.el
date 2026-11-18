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
;; Explicit UTF-8 EOL aliases retain Unicode encoding behavior.
(dolist (case '((233 (195 169)) (945 (206 177))))
  (with-temp-buffer
    (insert-char (car case))
    (qa (string-to-list (encode-coding-string (buffer-string) 'utf-8-unix))
        (cadr case))))

;; set-buffer-multibyte preserves the internal byte stream while remapping
;; character positions, markers, and property intervals.
(qa
 (with-temp-buffer
   (insert "aéz")
   (put-text-property 2 3 'qa48-tag 'value)
   (goto-char 3)
   (let ((marker (copy-marker (point))))
     (set-buffer-modified-p nil)
     (let ((tick (buffer-chars-modified-tick)))
       (list (set-buffer-multibyte nil)
             (buffer-size) (point) (marker-position marker)
             (multibyte-string-p (buffer-string))
             (string-to-list (buffer-string))
             (get-text-property 2 'qa48-tag)
             (get-text-property 3 'qa48-tag)
             (get-text-property 4 'qa48-tag)
             (buffer-modified-p)
             (= tick (buffer-chars-modified-tick))))))
 '(nil 4 4 4 nil (97 195 169 122) value value nil nil t))
(qa
 (with-temp-buffer
   (set-buffer-multibyte nil)
   (insert (unibyte-string 97 195 169 122))
   (goto-char 4)
   (let ((marker (copy-marker (point))))
     (set-buffer-multibyte t)
     (list (buffer-size) (point) (marker-position marker)
           (multibyte-string-p (buffer-string))
           (string-to-list (buffer-string)))))
 '(3 3 3 t (97 233 122)))
(dolist (case '((233 (195 169))
                (945 (206 177))
                (128512 (240 159 152 128))
                (4194176 (128))
                (4194303 (255))))
  (with-temp-buffer
    (insert-char (car case))
    (set-buffer-multibyte nil)
    (qa (buffer-size) (length (cadr case)))
    (qa (string-to-list (buffer-string)) (cadr case))))
(dolist (case '((233 (195 169))
                (945 (206 177))
                (128512 (240 159 152 128))))
  (with-temp-buffer
    (set-buffer-multibyte nil)
    (insert (apply #'unibyte-string (cadr case)))
    (set-buffer-multibyte t)
    (qa (list (buffer-size) (string-to-list (buffer-string)))
        (list 1 (list (car case))))))
(qa
 (with-temp-buffer
   (set-buffer-multibyte nil)
   (insert (unibyte-string 128 255))
   (set-buffer-multibyte t)
   (list (buffer-size) (string-to-list (buffer-string))
         (string-bytes (buffer-string)) (multibyte-string-p (buffer-string))))
 '(2 (4194176 4194303) 4 t))
(qa
 (with-temp-buffer
   (set-buffer-multibyte nil)
   (insert (unibyte-string 195 169))
   (list (set-buffer-multibyte 'to)
         enable-multibyte-characters
         (buffer-size)
         (string-to-list (buffer-string))))
 '(to t 2 (4194243 4194217)))
(qa
 (with-temp-buffer
   (set-buffer-multibyte nil)
   (insert (unibyte-string 128 255))
   (set-buffer-modified-p nil)
   (set-buffer-multibyte t)
   (list (buffer-size) (string-to-list (buffer-string))
         (string-bytes (buffer-string)) (buffer-modified-p)))
 '(2 (4194176 4194303) 4 nil))
(qa
 (with-temp-buffer
   (set-buffer-multibyte nil)
   (insert (unibyte-string 195 169))
   (set-buffer-modified-p nil)
   (set-buffer-multibyte 'to)
   (set-buffer-multibyte nil)
   (list (buffer-size) (string-to-list (buffer-string))
         (buffer-modified-p)))
 '(2 (195 169) nil))
(qa
 (with-temp-buffer
   (list (set-buffer-multibyte 'probe) enable-multibyte-characters))
 '(probe t))
(qa
 (with-temp-buffer
   (insert "abc")
   (narrow-to-region 2 3)
   (condition-case err
       (set-buffer-multibyte nil)
     (error (list (car err) (cadr err)))))
 '(error "Changing multibyteness in a narrowed buffer"))
(qa
 (with-temp-buffer
   (insert "abc")
   (setq buffer-undo-list nil)
   (narrow-to-region 2 3)
   (condition-case nil (set-buffer-multibyte nil) (error nil))
   (list enable-multibyte-characters buffer-undo-list (buffer-size)))
 '(t t 3))
(qa
 (with-temp-buffer
   (insert "é")
   (setq buffer-undo-list nil)
   (set-buffer-modified-p nil)
   (set-buffer-multibyte nil)
   (car buffer-undo-list))
 '(apply set-buffer-multibyte t))
(qa
 (let* ((base (generate-new-buffer "qa48-multi-base"))
        (indirect (make-indirect-buffer base "qa48-multi-indirect")))
   (unwind-protect
       (progn
         (with-current-buffer base
           (insert "aéz")
           (goto-char 3)
           (set-buffer-multibyte nil))
         (list (with-current-buffer base
                 (list enable-multibyte-characters (buffer-size) (point)))
               (with-current-buffer indirect
                 (list enable-multibyte-characters (buffer-size) (point)))))
     (mapc #'kill-buffer (list indirect base))))
 '((nil 4 4) (nil 4 1)))
(princ (qa--done)) (terpri)
