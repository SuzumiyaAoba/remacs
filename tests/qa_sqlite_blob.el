;;; qa_sqlite_blob.el --- SQLite BLOB byte preservation -*- lexical-binding: t; -*-
;; Run with qa_framework.el under remacs and SQLite-enabled GNU Emacs 31.1.

(defun qa-sqlite-blob-shape (value)
  (list (multibyte-string-p value) (length value)
        (string-bytes value) (string-to-list value)))

;; The issue's invalid UTF-8 bytes must survive unchanged, including NUL.
(qa
 (let ((db (sqlite-open)))
   (unwind-protect
       (qa-sqlite-blob-shape (caar (sqlite-select db "select x'0080ff'")))
     (sqlite-close db)))
 '(nil 3 3 (0 128 255)))

;; Empty and ASCII-only blobs are also unibyte; NULL is a separate value.
(qa
 (let ((db (sqlite-open)))
   (unwind-protect
       (let ((row (car (sqlite-select db "select x'', x'00', x'410042', null"))))
         (list (qa-sqlite-blob-shape (nth 0 row))
               (qa-sqlite-blob-shape (nth 1 row))
               (qa-sqlite-blob-shape (nth 2 row)) (nth 3 row)))
     (sqlite-close db)))
 '((nil 0 0 nil) (nil 1 1 (0)) (nil 3 3 (65 0 66)) nil))

;; A BLOB containing valid UTF-8 is still raw bytes, not decoded text.
(qa
 (let ((db (sqlite-open)))
   (unwind-protect
       (let ((row (car (sqlite-select db "select x'c3a9e697a5', 'é日'"))))
         (list (qa-sqlite-blob-shape (car row))
               (qa-sqlite-blob-shape (cadr row))))
     (sqlite-close db)))
 '((nil 5 5 (195 169 230 151 165)) (t 2 5 (233 26085))))

;; Exercise every possible byte, without relying on the string printer.
(qa
 (let* ((db (sqlite-open)) (bytes (number-sequence 0 255))
        (hex (mapconcat (lambda (b) (format "%02x" b)) bytes "")))
   (unwind-protect
       (let ((value (caar (sqlite-select db (concat "select x'" hex "'")))))
         (list (multibyte-string-p value) (length value) (string-bytes value)
               (equal (string-to-list value) bytes)
               (aref value 0) (aref value 128) (aref value 255)))
     (sqlite-close db)))
 '(nil 256 256 t 0 128 255))

;; All result paths must use the same lossless conversion.
(qa
 (let ((db (sqlite-open)))
   (unwind-protect
       (let ((rows (sqlite-select db "select x'0080ff' as payload" nil 'full)))
         (list (car rows) (qa-sqlite-blob-shape (caadr rows))))
     (sqlite-close db)))
 '(("payload") (nil 3 3 (0 128 255))))

(qa
 (let* ((db (sqlite-open))
        (set (sqlite-select db "select x'0080ff' union all select x''" nil 'set)))
   (unwind-protect
       (list (qa-sqlite-blob-shape (car (sqlite-next set)))
             (qa-sqlite-blob-shape (car (sqlite-next set)))
             (sqlite-next set) (sqlite-more-p set))
     (sqlite-finalize set)
     (sqlite-close db)))
 '((nil 3 3 (0 128 255)) (nil 0 0 nil) nil nil))

(qa
 (let ((db (sqlite-open)))
   (unwind-protect
       (qa-sqlite-blob-shape (caar (sqlite-execute db "select x'0080ff'")))
     (sqlite-close db)))
 '(nil 3 3 (0 128 255)))

(qa
 (let ((db (sqlite-open)))
   (unwind-protect
       (progn
         (sqlite-execute db "create table blobs (value blob)")
         (qa-sqlite-blob-shape
          (caar (sqlite-execute db "insert into blobs values (x'0080ff') returning value"))))
     (sqlite-close db)))
 '(nil 3 3 (0 128 255)))

;; Returned strings remain usable after result sets and connections close.
(qa
 (let* ((db (sqlite-open))
        (set (sqlite-select db "select x'0080ff'" nil 'set))
        (value (car (sqlite-next set))))
   (sqlite-finalize set)
   (sqlite-close db)
   (aset value 1 129)
   (qa-sqlite-blob-shape value))
 '(nil 3 3 (0 129 255)))

;; Reusing a returned BLOB as a parameter must not expose the internal
;; unibyte representation.  GNU binds strings as TEXT by default, so use
;; CAST to ask SQLite to return those exact bytes as a BLOB.
(qa
 (let* ((db (sqlite-open))
        (value (caar (sqlite-select db "select x'0080ff'"))))
   (unwind-protect
       (let ((row (car (sqlite-select db
                         "select typeof(?1), hex(?1), cast(?1 as blob)"
                         (list value)))))
         (list (car row) (cadr row) (qa-sqlite-blob-shape (nth 2 row))))
     (sqlite-close db)))
 '("text" "0080FF" (nil 3 3 (0 128 255))))

;; Unibyte Latin-1 bytes and multibyte Unicode have distinct encodings.
;; Valid UTF-8 stored in a BLOB is still passed back byte-for-byte.
(qa
 (let ((db (sqlite-open)))
   (unwind-protect
       (sqlite-select db "select typeof(?1), hex(?1), typeof(?2), hex(?2), hex(?3)"
         (vector (unibyte-string 233) "é"
                 (caar (sqlite-select db "select x'c3a9'"))))
     (sqlite-close db)))
 '(("text" "E9" "text" "C3A9" "C3A9")))

(qa
 (let* ((db (sqlite-open)) (bytes (number-sequence 0 255))
        (value (apply #'unibyte-string bytes)))
   (unwind-protect
       (progn
         (sqlite-execute db "create table roundtrip (value blob)")
         (sqlite-execute db "insert into roundtrip values (cast(? as blob))" (list value))
         (let* ((returned (caar (sqlite-execute db
                                 "insert into roundtrip values (cast(? as blob)) returning value"
                                 (vector value))))
                (stored (sqlite-select db "select value from roundtrip")))
           (list (equal (string-to-list returned) bytes)
                 (multibyte-string-p returned) (string-bytes returned)
                 (mapcar (lambda (row) (equal (string-to-list (car row)) bytes)) stored))))
     (sqlite-close db)))
 '(t nil 256 (t t)))

(qa
 (let* ((db (sqlite-open))
        (set (sqlite-select db "select cast(? as blob)" (vector (unibyte-string 0 128 255)) 'set)))
   (unwind-protect
       (qa-sqlite-blob-shape (car (sqlite-next set)))
     (sqlite-finalize set)
     (sqlite-close db)))
 '(nil 3 3 (0 128 255)))

(qa
 (let* ((db (sqlite-open))
        (value (caar (sqlite-select db "select x''"))))
   (unwind-protect
       (let ((row (car (sqlite-select db
                         "select typeof(?1), hex(?1), cast(?1 as blob)"
                         (list value)))))
         (list (car row) (cadr row) (qa-sqlite-blob-shape (nth 2 row))))
     (sqlite-close db)))
 '("text" "" (nil 0 0 nil)))

;; Reusing the owned SQL-value representation must not change other types.
(qa
 (let ((db (sqlite-open)))
   (unwind-protect
       (sqlite-select db "select ?1, typeof(?1), ?2, typeof(?2), ?3, typeof(?3), ?4, typeof(?4), ?5"
                      (vector nil 42 1.5 t "é日"))
     (sqlite-close db)))
 '((nil "null" 42 "integer" 1.5 "real" 1 "integer" "é日")))

(princ (qa--done))
(terpri)
