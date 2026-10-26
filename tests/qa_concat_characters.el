;;; -*- lexical-binding: nil; -*-
;; concat input types and the complete Emacs character range (#549-#551).
(dolist (bits (list (bool-vector) (bool-vector nil) (bool-vector t)
                    (bool-vector t nil)))
  (qa-err-data (concat bits) 'wrong-type-argument (list 'sequencep bits))
  (qa-err-data (concat "prefix" bits) 'wrong-type-argument (list 'sequencep bits))
  (qa-err-data (concat bits 42) 'wrong-type-argument (list 'sequencep bits))
  ;; An earlier invalid character still takes precedence.
  (qa-err-data (concat '(invalid) bits)
               'wrong-type-argument '(characterp invalid))
  (qa (vconcat bits) (vconcat (append bits nil))))
(dolist (code (list -1 #x400000 #x100000041 (- 65 #x100000000)
                    most-positive-fixnum (expt 2 100) 1.0 nil t 'invalid))
  (qa-err-data (concat (list code)) 'wrong-type-argument (list 'characterp code))
  (qa-err-data (concat (vector code)) 'wrong-type-argument (list 'characterp code))
  (qa-err-data (concat (list 65 code)) 'wrong-type-argument (list 'characterp code)))
(dolist (code '(0 65 127 128 255 #xd800 #xdfff #x10ffff #x110000
                #x200000 #x3fff7f #x3fff80 #x3fffff))
  (dolist (source (list (list code) (vector code)))
    (let ((result (concat source))
          (raw-byte (>= code #x3fff80)))
      (qa (length result) 1)
      (qa (string-to-list result)
          (list (if raw-byte (- code #x3fff00) code)))
      (qa (multibyte-string-p result) (and (>= code 128) (not raw-byte))))))
;; Error ordering across sequence and character failures.
(qa-err-data (concat '(invalid) 42) 'wrong-type-argument '(characterp invalid))
(qa-err-data (concat 42 '(invalid)) 'wrong-type-argument '(sequencep 42))
(princ (qa--done))
