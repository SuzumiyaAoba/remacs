;;; #789: direct-load syntax uses the decoded code before storage truncation. -*- lexical-binding: nil; -*-
(defun qa789-bytes (code)
  (unibyte-string (logior 248 (logand 7 (lsh code -24)))
                  (logior 128 (logand 63 (lsh code -18)))
                  (logior 128 (logand 63 (lsh code -12)))
                  (logior 128 (logand 63 (lsh code -6)))
                  (logior 128 (logand 63 code))))
(defun qa789-write (file body)
  (let ((coding-system-for-write 'no-conversion))
    (write-region (concat ";ELC\037\0\0\0\n;;; in Emacs version 31.1\n" body)
                  nil file nil 'silent)))
(let ((file (make-temp-file "qa-oversized-code-" nil ".elc"))
      (inner (make-temp-file "qa-oversized-inner-" nil ".elc"))
      qa789-string qa789-object qa789-character qa789-nested qa789-positioned)
  (unwind-protect
      (progn
        (dolist (custom '(nil t))
          (let ((load-read-function (and custom (lambda (stream) (read stream)))))
            (dolist (code '(4194304 4194336 4194339 4194363 4194350
				    4194345 4194338 4194396 8388705 12583009
				    5898240 8388480 16777215 4194432 4249600
                           16777216 33554432 50331648 67108864
                           83886080 100663296 117440512 18481152 134217727))
              (let ((raw (qa789-bytes code)) (base (logand code 4194303)))
                (qa789-write file (concat "(setq qa789-string \"" raw
                                          "\" qa789-object '" raw
                                          " qa789-character ?" raw ")\n"))
                (qa (load file nil t t) t)
                (qa (string-to-list qa789-string)
                    (cond ((memq code '(5898240 18481152)) '(246 160 128 128))
                          ((= code 4194432) '(194 128))
                          ((= code 4249600) '(237 160 128))
                          ((= code 8388480) '(128)) ((memq code '(16777215 134217727)) '(255)) (t (list base))))
                (qa (multibyte-string-p qa789-string) nil)
                (qa (symbolp qa789-object) t)
                (qa (string-to-list (symbol-name qa789-object)) (list base))
                (qa qa789-character
                    (cond ((= code 8388480) 4194432) ((= code 16777215) 12583167) ((= code 134217727) 130023679) (t code)))))
            ;; Numeric classification still sees the stored spelling.
            (qa789-write file (concat "(setq qa789-object '" (qa789-bytes 4194353) ")\n"))
            (qa (load file nil t t) t)
            (qa qa789-object 1)
            ;; An oversized dot is a constituent even before whitespace.
            (qa789-write file (concat "(setq qa789-object '(" (qa789-bytes 4194350) " a))\n"))
            (qa (load file nil t t) t)
            (qa (mapcar #'symbol-name qa789-object) '("." "a"))
            ;; Genuine overlong NULs remain layout, rather than constituents.
            (qa789-write file (concat "'" (unibyte-string 224 128 128) ")\n"))
            (qa-err (load file nil t t) 'invalid-read-syntax)))
        ;; Escaped codes keep their modifier bits until string-specific validation.
        (dolist (code '(4194401 8388705 16777313 33554529 67108961 67108896))
          (qa789-write file (concat "(setq qa789-character ?\\" (qa789-bytes code) ")\n"))
          (qa (load file nil t t) t)
          (qa qa789-character code)
          (qa789-write file (concat "(setq qa789-string \"\\" (qa789-bytes code) "\")\n"))
          (cond ((= code 33554529)
                 (qa (load file nil t t) t)
                 (qa (string-to-list qa789-string) '(65)))
                ((= code 67108896)
                 (qa (load file nil t t) t)
                 (qa (string-to-list qa789-string) '(0)))
                (t (qa-err (load file nil t t) 'invalid-read-syntax))))
        ;; A genuine multibyte character still forces the whole literal multibyte.
        (qa789-write file (concat "(setq qa789-string \"λ" (qa789-bytes 5898240) "\")\n"))
        (qa (load file nil t t) t)
        (qa (string-to-list qa789-string) '(955 1703936))
        (qa (multibyte-string-p qa789-string) t)
        ;; Pre-read layout/comment scanning must use the original code too.
        (dolist (code '(4194304 4194314 4194336 4194363 4194464))
          (let ((qa789-calls 0)
                (load-read-function
                 (lambda (stream) (setq qa789-calls (1+ qa789-calls)) (read stream))))
            (qa789-write file (qa789-bytes code))
            (eval `(let ((,(intern (string (logand code 4194303))) 99))
                     (qa (load file nil t t) t)
                     (qa qa789-calls 1)))))
        ;; Explicit reads and positioned reads share the same direct cursor.
        (qa789-write file (concat "(setq qa789-positioned (read-positioning-symbols))\n("
                                  (qa789-bytes 4194304) " next)\n"))
        (qa (load file nil t t) t)
        (qa (string-to-list (symbol-name (bare-symbol (car qa789-positioned)))) '(0))
        (qa (symbol-with-pos-pos (car qa789-positioned)) 2)
        (qa (symbol-name (bare-symbol (cadr qa789-positioned))) "next")
        (qa (symbol-with-pos-pos (cadr qa789-positioned)) 4)
        ;; Nested loads restore the outer stream's original-code metadata.
        (qa789-write inner "(setq qa789-nested 'inner)\n")
        (qa789-write file (concat (format "(load %S nil t t)\n" inner)
                                  "(setq qa789-object '" (qa789-bytes 4194336) ")\n"))
        (qa (load file nil t t) t)
        (qa qa789-nested 'inner)
        (qa (string-to-list (symbol-name qa789-object)) '(32))
        ;; Unwinding an inner reader error restores the same outer metadata.
        (qa789-write inner (concat "'" (unibyte-string 224 128 128) ")\n"))
        (qa789-write file (concat (format "(condition-case nil (load %S nil t t) (error nil))\n" inner)
                                  "(setq qa789-object '" (qa789-bytes 4194339) ")\n"))
        (qa (load file nil t t) t)
        (qa (string-to-list (symbol-name qa789-object)) '(35)))
    (delete-file file)
    (delete-file inner)))
(princ (qa--done))
