;;; -*- lexical-binding: nil; -*-
;; #624: SQLite's native byte boundary must not create Unicode proxies.
(defun qa-sql624-shape (db code)
  (let* ((text (string code))
         (column (caar (sqlite-select db (concat "select 1 as \"" text "\"") nil 'full)))
         (value (caar (sqlite-select db "select ?" (list text)))))
    (list (string-to-list column) (multibyte-string-p column)
          (string-to-list value) (multibyte-string-p value))))
(let ((db (sqlite-open)))
  (unwind-protect
      (progn
        (qa (qa-sql624-shape db #xf0080) '((983168) t (983168) t))
        (qa (qa-sql624-shape db #xd800) '((55296) t (4194285 4194208 4194176) t))
        (qa (qa-sql624-shape db #x110000) '((1114112) t (1114112) t))
        (qa (qa-sql624-shape db #x200000) '((2097152) t (2097152) t))
        (qa (qa-sql624-shape db #x3fff80) '((128) nil (4194176) t))
        (qa (sqlite-select db "select hex(?), hex(?)"
                           (list (string #x3fff80) (unibyte-string 128)))
            '(("80" "80")))
        (qa (let* ((name (string #xf0080 #x110000))
                   (create (concat "create table \"" name "\" (value)")))
              (sqlite-execute db create)
              (sqlite-execute db (concat "insert into \"" name "\" values (?)")
                              (list (string #xf0100)))
              (mapcar #'string-to-list (car (sqlite-select db (concat "select value from \"" name "\"")))))
            '((983296))))
    (sqlite-close db)))
(princ (qa--done))
