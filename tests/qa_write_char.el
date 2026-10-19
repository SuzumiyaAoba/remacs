;;; -*- lexical-binding: t; -*-
;; Issue #95: write-char checks fixnump, not characterp. GNU printchar
;; passes unsigned 32-bit codes to functions; text sinks resolve event
;; modifiers, preserve high Mule characters, and reject remaining bits.
(defvar qa95-case)
(defvar qa95-value)
(dolist (qa95-case '((0 0) (65 65) (255 255) (#xD800 #xD800)
                     (#x110000 #x110000) (#x200000 #x200000)
                     (#x3FFF7F #x3FFF7F) (#x3FFF80 #x3FFF80)
                     (#x3FFFFF #x3FFFFF) (#x400000 0) (#x7FFFFF #x3FFFFF)
                     (-4294967296 0) (-2305843009213693952 0)
                     (4294967361 65) (#x2000061 65) (#x4000041 1)
                     (#x8000041 65) (#x1000041 65) (#x6000061 1)
                     (#x4000020 0) (#x400003F 127) (#x4000030 48)
                     (#x20003B1 #x3B1)))
  (qa (let ((result (with-output-to-string (write-char (car qa95-case)))))
        (string-to-list result)) (list (cadr qa95-case)))
  (qa (with-temp-buffer
        (let ((result (write-char (car qa95-case) (current-buffer))))
          (list result (string-to-list (buffer-string)) (point))))
      (list (car qa95-case) (list (cadr qa95-case)) 2))
  (qa (with-temp-buffer
        (insert "ab") (goto-char 1)
        (let* ((marker (copy-marker 2))
               (result (write-char (car qa95-case) marker)))
          (list result (string-to-list (buffer-string))
                (point) (marker-position marker))))
      (list (car qa95-case) (list 97 (cadr qa95-case) 98) 1 3))
  (qa (let (seen)
        (list (write-char (car qa95-case) (lambda (code) (push code seen) 'ignored))
              seen))
      (list (car qa95-case) (list (logand (car qa95-case) #xFFFFFFFF)))))

;; Function destinations receive even codes that text sinks reject.
(dolist (qa95-case '((-1 "f03fffff") (-5 "f03ffffb")
                     (2305843009213693951 "f03fffff")
                     (#x10000041 "10000041")))
  (qa-err-data (with-output-to-string (write-char (car qa95-case)))
               'error (list (concat "Invalid character: " (cadr qa95-case))))
  (qa (with-temp-buffer
        (insert "ab") (goto-char 2)
        (let ((marker (point-marker)))
          (list (condition-case error (write-char (car qa95-case) marker) (error error))
                (buffer-string) (point) (marker-position marker))))
      (list (list 'error (concat "Invalid character: " (cadr qa95-case))) "ab" 2 2))
  (qa (let (seen)
        (list (write-char (car qa95-case) (lambda (code) (setq seen code))) seen))
      (list (car qa95-case) (logand (car qa95-case) #xFFFFFFFF))))

(dolist (qa95-value '(nil t 65.0 "x" 2305843009213693952 -2305843009213693953))
  (qa-err-data (write-char qa95-value #'ignore) 'wrong-type-argument (list 'fixnump qa95-value)))
(qa (let* (seen (standard-output (lambda (code) (push code seen))))
      (list (write-char #x400041 nil) seen)) '(4194369 (4194369)))
(qa-err-data (write-char -1 (lambda (code) (signal 'error (list code))))
             'error '(4294967295))
(qa-err-data (write-char -1 'qa95-undefined-output) 'void-function '(qa95-undefined-output))
(qa-err-data (write-char #x400000 17) 'invalid-function '(17))
;; Type validation precedes stream selection; stream selection precedes
;; the text encoder's invalid-character check.
(qa-err-data (let ((buffer (generate-new-buffer " *qa95-dead*")))
               (kill-buffer buffer) (write-char 2305843009213693952 buffer))
             'wrong-type-argument '(fixnump 2305843009213693952))
(qa-err-data (let ((buffer (generate-new-buffer " *qa95-dead*")))
               (kill-buffer buffer) (write-char -1 buffer))
             'error '("Selecting deleted buffer"))

(princ (qa--done)) (terpri)
