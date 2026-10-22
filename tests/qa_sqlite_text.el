;;; qa_sqlite_text.el --- Preserve GNU SQLite TEXT decoding -*- lexical-binding: t; -*-
(defun qa352-value (mode sql)
  (let ((db (sqlite-open)))
    (unwind-protect
        (pcase mode
          ('execute (caar (sqlite-execute db sql)))
          ('full (caadr (sqlite-select db sql nil 'full)))
          ('set (let ((set (sqlite-select db sql nil 'set)))
                  (unwind-protect (car (sqlite-next set))
                    (sqlite-finalize set))))
          (_ (caar (sqlite-select db sql))))
      (sqlite-close db))))
(defun qa352-info (value)
  (list (multibyte-string-p value) (string-bytes value)
        (string-to-list value) (text-properties-at 0 value)))

;; select: empty
(qa (qa352-info (qa352-value 'select "select cast(x'' as text) as value"))
    '(t 0 nil nil))

;; select: 61
(qa (qa352-info (qa352-value 'select "select cast(x'61' as text) as value"))
    '(t 1 (97) nil))

;; select: 80ff
(qa (qa352-info (qa352-value 'select "select cast(x'80ff' as text) as value"))
    '(t 4 (4194176 4194303) nil))

;; select: e282
(qa (qa352-info (qa352-value 'select "select cast(x'e282' as text) as value"))
    '(t 4 (4194274 4194178) nil))

;; select: c3a9e38182
(qa (qa352-info (qa352-value 'select "select cast(x'c3a9e38182' as text) as value"))
    '(t 5 (233 12354) nil))

;; select: 610062
(qa (qa352-info (qa352-value 'select "select cast(x'610062' as text) as value"))
    '(t 3 (97 0 98) nil))

;; select: c0af
(qa (qa352-info (qa352-value 'select "select cast(x'c0af' as text) as value"))
    '(t 4 (4194240 4194223) nil))

;; select: e080af
(qa (qa352-info (qa352-value 'select "select cast(x'e080af' as text) as value"))
    '(t 6 (4194272 4194176 4194223) nil))

;; select: eda080
(qa (qa352-info (qa352-value 'select "select cast(x'eda080' as text) as value"))
    '(t 6 (4194285 4194208 4194176) nil))

;; select: f4908080
(qa (qa352-info (qa352-value 'select "select cast(x'f4908080' as text) as value"))
    '(t 4 (1114112) nil))

;; select: f888808080
(qa (qa352-info (qa352-value 'select "select cast(x'f888808080' as text) as value"))
    '(t 5 (2097152) nil))

;; select: feff
(qa (qa352-info (qa352-value 'select "select cast(x'feff' as text) as value"))
    '(t 4 (4194302 4194303) nil))

;; select: efbbbf61
(qa (qa352-info (qa352-value 'select "select cast(x'efbbbf61' as text) as value"))
    '(t 4 (65279 97) nil))

;; select: 610d0a62
(qa (qa352-info (qa352-value 'select "select cast(x'610d0a62' as text) as value"))
    '(t 3 (97 10 98) nil))

;; select: 610d62
(qa (qa352-info (qa352-value 'select "select cast(x'610d62' as text) as value"))
    '(t 3 (97 10 98) nil))

;; select: 610d0a620d63
(qa (qa352-info (qa352-value 'select "select cast(x'610d0a620d63' as text) as value"))
    '(t 5 (97 10 98 13 99) nil))

;; select: f09f9880
(qa (qa352-info (qa352-value 'select "select cast(x'f09f9880' as text) as value"))
    '(t 4 (128512) nil))

;; select: e28261
(qa (qa352-info (qa352-value 'select "select cast(x'e28261' as text) as value"))
    '(t 5 (4194274 4194178 97) nil))

;; select: f88fbfbfbf
(qa (qa352-info (qa352-value 'select "select cast(x'f88fbfbfbf' as text) as value"))
    '(t 2 (4194303) nil))

;; select: f890808080
(qa (qa352-info (qa352-value 'select "select cast(x'f890808080' as text) as value"))
    '(t 10 (4194296 4194192 4194176 4194176 4194176) nil))

;; select: f7bfbfbf
(qa (qa352-info (qa352-value 'select "select cast(x'f7bfbfbf' as text) as value"))
    '(t 4 (2097151) nil))

;; select: f880808080
(qa (qa352-info (qa352-value 'select "select cast(x'f880808080' as text) as value"))
    '(t 10 (4194296 4194176 4194176 4194176 4194176) nil))

;; select: 610d620d0a63
(qa (qa352-info (qa352-value 'select "select cast(x'610d620d0a63' as text) as value"))
    '(t 5 (97 13 98 10 99) nil))

;; select: 610a620d0a63
(qa (qa352-info (qa352-value 'select "select cast(x'610a620d0a63' as text) as value"))
    '(t 6 (97 10 98 13 10 99) nil))

(qa (qa352-info (qa352-value 'select "select x'80ff' as value")) '(nil 2 (128 255) nil))

(qa (let ((last-coding-system-used 'qa352-saved))
      (qa352-value 'select "select cast(x'80ff' as text) as value")
      (eq last-coding-system-used 'qa352-saved)) t)

;; execute: empty
(qa (qa352-info (qa352-value 'execute "select cast(x'' as text) as value"))
    '(t 0 nil nil))

;; execute: 61
(qa (qa352-info (qa352-value 'execute "select cast(x'61' as text) as value"))
    '(t 1 (97) nil))

;; execute: 80ff
(qa (qa352-info (qa352-value 'execute "select cast(x'80ff' as text) as value"))
    '(t 4 (4194176 4194303) nil))

;; execute: e282
(qa (qa352-info (qa352-value 'execute "select cast(x'e282' as text) as value"))
    '(t 4 (4194274 4194178) nil))

;; execute: c3a9e38182
(qa (qa352-info (qa352-value 'execute "select cast(x'c3a9e38182' as text) as value"))
    '(t 5 (233 12354) nil))

;; execute: 610062
(qa (qa352-info (qa352-value 'execute "select cast(x'610062' as text) as value"))
    '(t 3 (97 0 98) nil))

;; execute: c0af
(qa (qa352-info (qa352-value 'execute "select cast(x'c0af' as text) as value"))
    '(t 4 (4194240 4194223) nil))

;; execute: e080af
(qa (qa352-info (qa352-value 'execute "select cast(x'e080af' as text) as value"))
    '(t 6 (4194272 4194176 4194223) nil))

;; execute: eda080
(qa (qa352-info (qa352-value 'execute "select cast(x'eda080' as text) as value"))
    '(t 6 (4194285 4194208 4194176) nil))

;; execute: f4908080
(qa (qa352-info (qa352-value 'execute "select cast(x'f4908080' as text) as value"))
    '(t 4 (1114112) nil))

;; execute: f888808080
(qa (qa352-info (qa352-value 'execute "select cast(x'f888808080' as text) as value"))
    '(t 5 (2097152) nil))

;; execute: feff
(qa (qa352-info (qa352-value 'execute "select cast(x'feff' as text) as value"))
    '(t 4 (4194302 4194303) nil))

;; execute: efbbbf61
(qa (qa352-info (qa352-value 'execute "select cast(x'efbbbf61' as text) as value"))
    '(t 4 (65279 97) nil))

;; execute: 610d0a62
(qa (qa352-info (qa352-value 'execute "select cast(x'610d0a62' as text) as value"))
    '(t 3 (97 10 98) nil))

;; execute: 610d62
(qa (qa352-info (qa352-value 'execute "select cast(x'610d62' as text) as value"))
    '(t 3 (97 10 98) nil))

;; execute: 610d0a620d63
(qa (qa352-info (qa352-value 'execute "select cast(x'610d0a620d63' as text) as value"))
    '(t 5 (97 10 98 13 99) nil))

;; execute: f09f9880
(qa (qa352-info (qa352-value 'execute "select cast(x'f09f9880' as text) as value"))
    '(t 4 (128512) nil))

;; execute: e28261
(qa (qa352-info (qa352-value 'execute "select cast(x'e28261' as text) as value"))
    '(t 5 (4194274 4194178 97) nil))

;; execute: f88fbfbfbf
(qa (qa352-info (qa352-value 'execute "select cast(x'f88fbfbfbf' as text) as value"))
    '(t 2 (4194303) nil))

;; execute: f890808080
(qa (qa352-info (qa352-value 'execute "select cast(x'f890808080' as text) as value"))
    '(t 10 (4194296 4194192 4194176 4194176 4194176) nil))

;; execute: f7bfbfbf
(qa (qa352-info (qa352-value 'execute "select cast(x'f7bfbfbf' as text) as value"))
    '(t 4 (2097151) nil))

;; execute: f880808080
(qa (qa352-info (qa352-value 'execute "select cast(x'f880808080' as text) as value"))
    '(t 10 (4194296 4194176 4194176 4194176 4194176) nil))

;; execute: 610d620d0a63
(qa (qa352-info (qa352-value 'execute "select cast(x'610d620d0a63' as text) as value"))
    '(t 5 (97 13 98 10 99) nil))

;; execute: 610a620d0a63
(qa (qa352-info (qa352-value 'execute "select cast(x'610a620d0a63' as text) as value"))
    '(t 6 (97 10 98 13 10 99) nil))

(qa (qa352-info (qa352-value 'execute "select x'80ff' as value")) '(nil 2 (128 255) nil))

(qa (let ((last-coding-system-used 'qa352-saved))
      (qa352-value 'execute "select cast(x'80ff' as text) as value")
      (eq last-coding-system-used 'qa352-saved)) t)

;; full: empty
(qa (qa352-info (qa352-value 'full "select cast(x'' as text) as value"))
    '(t 0 nil nil))

;; full: 61
(qa (qa352-info (qa352-value 'full "select cast(x'61' as text) as value"))
    '(t 1 (97) nil))

;; full: 80ff
(qa (qa352-info (qa352-value 'full "select cast(x'80ff' as text) as value"))
    '(t 4 (4194176 4194303) nil))

;; full: e282
(qa (qa352-info (qa352-value 'full "select cast(x'e282' as text) as value"))
    '(t 4 (4194274 4194178) nil))

;; full: c3a9e38182
(qa (qa352-info (qa352-value 'full "select cast(x'c3a9e38182' as text) as value"))
    '(t 5 (233 12354) nil))

;; full: 610062
(qa (qa352-info (qa352-value 'full "select cast(x'610062' as text) as value"))
    '(t 3 (97 0 98) nil))

;; full: c0af
(qa (qa352-info (qa352-value 'full "select cast(x'c0af' as text) as value"))
    '(t 4 (4194240 4194223) nil))

;; full: e080af
(qa (qa352-info (qa352-value 'full "select cast(x'e080af' as text) as value"))
    '(t 6 (4194272 4194176 4194223) nil))

;; full: eda080
(qa (qa352-info (qa352-value 'full "select cast(x'eda080' as text) as value"))
    '(t 6 (4194285 4194208 4194176) nil))

;; full: f4908080
(qa (qa352-info (qa352-value 'full "select cast(x'f4908080' as text) as value"))
    '(t 4 (1114112) nil))

;; full: f888808080
(qa (qa352-info (qa352-value 'full "select cast(x'f888808080' as text) as value"))
    '(t 5 (2097152) nil))

;; full: feff
(qa (qa352-info (qa352-value 'full "select cast(x'feff' as text) as value"))
    '(t 4 (4194302 4194303) nil))

;; full: efbbbf61
(qa (qa352-info (qa352-value 'full "select cast(x'efbbbf61' as text) as value"))
    '(t 4 (65279 97) nil))

;; full: 610d0a62
(qa (qa352-info (qa352-value 'full "select cast(x'610d0a62' as text) as value"))
    '(t 3 (97 10 98) nil))

;; full: 610d62
(qa (qa352-info (qa352-value 'full "select cast(x'610d62' as text) as value"))
    '(t 3 (97 10 98) nil))

;; full: 610d0a620d63
(qa (qa352-info (qa352-value 'full "select cast(x'610d0a620d63' as text) as value"))
    '(t 5 (97 10 98 13 99) nil))

;; full: f09f9880
(qa (qa352-info (qa352-value 'full "select cast(x'f09f9880' as text) as value"))
    '(t 4 (128512) nil))

;; full: e28261
(qa (qa352-info (qa352-value 'full "select cast(x'e28261' as text) as value"))
    '(t 5 (4194274 4194178 97) nil))

;; full: f88fbfbfbf
(qa (qa352-info (qa352-value 'full "select cast(x'f88fbfbfbf' as text) as value"))
    '(t 2 (4194303) nil))

;; full: f890808080
(qa (qa352-info (qa352-value 'full "select cast(x'f890808080' as text) as value"))
    '(t 10 (4194296 4194192 4194176 4194176 4194176) nil))

;; full: f7bfbfbf
(qa (qa352-info (qa352-value 'full "select cast(x'f7bfbfbf' as text) as value"))
    '(t 4 (2097151) nil))

;; full: f880808080
(qa (qa352-info (qa352-value 'full "select cast(x'f880808080' as text) as value"))
    '(t 10 (4194296 4194176 4194176 4194176 4194176) nil))

;; full: 610d620d0a63
(qa (qa352-info (qa352-value 'full "select cast(x'610d620d0a63' as text) as value"))
    '(t 5 (97 13 98 10 99) nil))

;; full: 610a620d0a63
(qa (qa352-info (qa352-value 'full "select cast(x'610a620d0a63' as text) as value"))
    '(t 6 (97 10 98 13 10 99) nil))

(qa (qa352-info (qa352-value 'full "select x'80ff' as value")) '(nil 2 (128 255) nil))

(qa (let ((last-coding-system-used 'qa352-saved))
      (qa352-value 'full "select cast(x'80ff' as text) as value")
      (eq last-coding-system-used 'qa352-saved)) t)

;; set: empty
(qa (qa352-info (qa352-value 'set "select cast(x'' as text) as value"))
    '(t 0 nil nil))

;; set: 61
(qa (qa352-info (qa352-value 'set "select cast(x'61' as text) as value"))
    '(t 1 (97) nil))

;; set: 80ff
(qa (qa352-info (qa352-value 'set "select cast(x'80ff' as text) as value"))
    '(t 4 (4194176 4194303) nil))

;; set: e282
(qa (qa352-info (qa352-value 'set "select cast(x'e282' as text) as value"))
    '(t 4 (4194274 4194178) nil))

;; set: c3a9e38182
(qa (qa352-info (qa352-value 'set "select cast(x'c3a9e38182' as text) as value"))
    '(t 5 (233 12354) nil))

;; set: 610062
(qa (qa352-info (qa352-value 'set "select cast(x'610062' as text) as value"))
    '(t 3 (97 0 98) nil))

;; set: c0af
(qa (qa352-info (qa352-value 'set "select cast(x'c0af' as text) as value"))
    '(t 4 (4194240 4194223) nil))

;; set: e080af
(qa (qa352-info (qa352-value 'set "select cast(x'e080af' as text) as value"))
    '(t 6 (4194272 4194176 4194223) nil))

;; set: eda080
(qa (qa352-info (qa352-value 'set "select cast(x'eda080' as text) as value"))
    '(t 6 (4194285 4194208 4194176) nil))

;; set: f4908080
(qa (qa352-info (qa352-value 'set "select cast(x'f4908080' as text) as value"))
    '(t 4 (1114112) nil))

;; set: f888808080
(qa (qa352-info (qa352-value 'set "select cast(x'f888808080' as text) as value"))
    '(t 5 (2097152) nil))

;; set: feff
(qa (qa352-info (qa352-value 'set "select cast(x'feff' as text) as value"))
    '(t 4 (4194302 4194303) nil))

;; set: efbbbf61
(qa (qa352-info (qa352-value 'set "select cast(x'efbbbf61' as text) as value"))
    '(t 4 (65279 97) nil))

;; set: 610d0a62
(qa (qa352-info (qa352-value 'set "select cast(x'610d0a62' as text) as value"))
    '(t 3 (97 10 98) nil))

;; set: 610d62
(qa (qa352-info (qa352-value 'set "select cast(x'610d62' as text) as value"))
    '(t 3 (97 10 98) nil))

;; set: 610d0a620d63
(qa (qa352-info (qa352-value 'set "select cast(x'610d0a620d63' as text) as value"))
    '(t 5 (97 10 98 13 99) nil))

;; set: f09f9880
(qa (qa352-info (qa352-value 'set "select cast(x'f09f9880' as text) as value"))
    '(t 4 (128512) nil))

;; set: e28261
(qa (qa352-info (qa352-value 'set "select cast(x'e28261' as text) as value"))
    '(t 5 (4194274 4194178 97) nil))

;; set: f88fbfbfbf
(qa (qa352-info (qa352-value 'set "select cast(x'f88fbfbfbf' as text) as value"))
    '(t 2 (4194303) nil))

;; set: f890808080
(qa (qa352-info (qa352-value 'set "select cast(x'f890808080' as text) as value"))
    '(t 10 (4194296 4194192 4194176 4194176 4194176) nil))

;; set: f7bfbfbf
(qa (qa352-info (qa352-value 'set "select cast(x'f7bfbfbf' as text) as value"))
    '(t 4 (2097151) nil))

;; set: f880808080
(qa (qa352-info (qa352-value 'set "select cast(x'f880808080' as text) as value"))
    '(t 10 (4194296 4194176 4194176 4194176 4194176) nil))

;; set: 610d620d0a63
(qa (qa352-info (qa352-value 'set "select cast(x'610d620d0a63' as text) as value"))
    '(t 5 (97 13 98 10 99) nil))

;; set: 610a620d0a63
(qa (qa352-info (qa352-value 'set "select cast(x'610a620d0a63' as text) as value"))
    '(t 6 (97 10 98 13 10 99) nil))

(qa (qa352-info (qa352-value 'set "select x'80ff' as value")) '(nil 2 (128 255) nil))

(qa (let ((last-coding-system-used 'qa352-saved))
      (qa352-value 'set "select cast(x'80ff' as text) as value")
      (eq last-coding-system-used 'qa352-saved)) t)

(qa (let ((inhibit-eol-conversion t))
      (qa352-info (qa352-value 'select "select cast(x'610d0a62' as text) as value")))
    '(t 4 (97 13 10 98) nil))

(qa (let ((inhibit-eol-conversion t))
      (qa352-info (qa352-value 'select "select cast(x'610d62' as text) as value")))
    '(t 3 (97 13 98) nil))

(qa (with-temp-buffer (set-buffer-multibyte nil)
      (qa352-info (qa352-value 'select "select cast(x'61' as text) as value")))
    '(t 1 (97) nil))

(qa (with-temp-buffer (set-buffer-multibyte nil)
      (qa352-info (qa352-value 'select "select cast(x'80ff' as text) as value")))
    '(t 4 (4194176 4194303) nil))

(princ (qa--done))
