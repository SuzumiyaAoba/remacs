;;; -*- lexical-binding: nil; -*-
;; The bundled byte conversion fallbacks must be readable and available.
(load-file (expand-file-name "../lisp/hexl.el"
                             (file-name-directory load-file-name)))
(qa (featurep 'hexl) t)
(qa (fboundp 'hexl--remacs-hexlify-bytes) t)
(qa (fboundp 'hexl--remacs-dehexlify-bytes) t)
(princ (qa--done))
