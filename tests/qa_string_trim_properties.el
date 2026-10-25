;;; -*- lexical-binding: nil; -*-
;; string-trim variants preserve identity and retained text properties (#372).
(let ((s (propertize "abc" 'face 'bold)))
  (qa (eq (string-trim s) s) t)
  (qa (eq (string-trim-left s) s) t)
  (qa (get-text-property 1 'face
                         (string-trim-left (propertize " abc" 'face 'bold)))
      'bold)
  (qa (get-text-property 0 'face
                         (string-trim-right (propertize "abc " 'face 'bold)))
      'bold))
(princ (qa--done))
